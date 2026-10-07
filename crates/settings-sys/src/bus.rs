//! Which bus to talk to, and connecting to it with a timeout.

use crate::{Error, ErrorKind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use zbus::blocking::Connection;

/// How long a method call may take before it fails with
/// [`ErrorKind::Timeout`](crate::ErrorKind::Timeout). Calls that wait for a
/// polkit password prompt use [`INTERACTIVE_TIMEOUT`] instead.
pub const METHOD_TIMEOUT: Duration = Duration::from_secs(10);

/// For calls that may show a polkit prompt: the person has time to type.
pub const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(120);

/// Most connect attempts that timed out and are still stuck on their helper
/// threads (a hung bus leaves one per attempt); past it, connecting fails at
/// once instead of leaving another thread behind.
const MAX_STUCK: usize = 4;
static STUCK: AtomicUsize = AtomicUsize::new(0);

/// A connect attempt's state, shared by the caller and its helper thread.
#[derive(Default)]
struct Attempt {
    /// The helper has finished.
    done: bool,
    /// The caller gave up on it and counted it in [`STUCK`].
    abandoned: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bus {
    System,
    Session,
    /// A bus by address: the tests' private bus.
    Address(String),
}

impl Bus {
    /// A new connection to this bus, its method calls limited to `timeout`.
    /// Connecting and authenticating take at most [`METHOD_TIMEOUT`] (a
    /// polkit prompt only ever waits on a call): they run on a helper thread,
    /// which on a timeout is left to finish (and drop its connection) on its
    /// own. While [`MAX_STUCK`] such threads are still stuck, connecting
    /// fails at once.
    pub fn connect_with(&self, timeout: Duration) -> Result<Connection, Error> {
        if STUCK.load(Ordering::Acquire) >= MAX_STUCK {
            return Err(Error::new(
                ErrorKind::Bus,
                "earlier connections to the bus are still hanging",
            ));
        }
        let attempt = Arc::new(Mutex::new(Attempt::default()));
        let bus = self.clone();
        let (tx, rx) = mpsc::channel();
        let shared = Arc::clone(&attempt);
        std::thread::Builder::new()
            .name("settings-sys-connect".into())
            .spawn(move || {
                // The receiver is gone after a timeout: nothing to tell.
                let _ = tx.send(bus.build(timeout));
                let mut a = shared.lock().unwrap_or_else(|e| e.into_inner());
                a.done = true;
                if a.abandoned {
                    STUCK.fetch_sub(1, Ordering::AcqRel);
                }
            })
            .map_err(|e| Error::new(ErrorKind::Bus, format!("can't start a thread: {e}")))?;
        let connect_timeout = timeout.min(METHOD_TIMEOUT);
        match rx.recv_timeout(connect_timeout) {
            Ok(r) => r,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let mut a = attempt.lock().unwrap_or_else(|e| e.into_inner());
                if !a.done {
                    a.abandoned = true;
                    STUCK.fetch_add(1, Ordering::AcqRel);
                }
                Err(Error::new(
                    ErrorKind::Timeout,
                    format!("connecting to {self:?} took longer than {connect_timeout:?}"),
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(Error::new(ErrorKind::Bus, "the connect thread died"))
            }
        }
    }

    fn build(&self, timeout: Duration) -> Result<Connection, Error> {
        // The async builder under async-io's own `block_on`, not the blocking
        // builder: when something else in the program turns on zbus's tokio
        // backend (the framework's notifier does), the blocking builder runs
        // on a throw-away tokio runtime, and the connection it makes keeps
        // tokio as its executor after that runtime is gone (a later
        // `Proxy` drop panics: "there is no reactor running"). Built here,
        // outside any tokio runtime, the connection keeps its own executor.
        let builder = match self {
            Bus::System => zbus::connection::Builder::system()?,
            Bus::Session => zbus::connection::Builder::session()?,
            Bus::Address(a) => zbus::connection::Builder::address(a.as_str())?,
        };
        let conn = async_io::block_on(builder.method_timeout(timeout).build())?;
        Ok(Connection::from(conn))
    }

    /// A new connection with the usual [`METHOD_TIMEOUT`].
    pub fn connect(&self) -> Result<Connection, Error> {
        self.connect_with(METHOD_TIMEOUT)
    }
}
