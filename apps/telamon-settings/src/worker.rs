//! Runs a blocking system call on a thread of its own and hands the result
//! back to the page's QObject on the GUI thread (docs/DESIGN.md, "Threads").
//! When the page is gone by then, the result is dropped.

use core::pin::Pin;
use cxx_qt::{CxxQtThread, Threading};
use std::panic::AssertUnwindSafe;

/// Runs `job` on a new thread, then `done` with its result on the GUI
/// thread. A job that panics answers `None`. False when no thread could be
/// started (`done` is not called).
pub fn run<T, R, J, D>(qt: CxxQtThread<T>, name: &str, job: J, done: D) -> bool
where
    T: Threading + 'static,
    J: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
    D: FnOnce(Pin<&mut T>, Option<R>) + Send + 'static,
{
    let spawned = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let result = std::panic::catch_unwind(AssertUnwindSafe(job)).ok();
            // Err: the page was closed meanwhile; nothing to tell.
            let _ = qt.queue(move |o| done(o, result));
        });
    if let Err(e) = &spawned {
        log::error!("starting {name}: {e}");
    }
    spawned.is_ok()
}

/// What a page says when a call failed: the plain-words sentence, with the
/// service's own message logged.
pub fn describe(what: &str, e: &settings_sys::Error) -> String {
    log::warn!("{what}: {}", e.detail);
    e.describe().to_string()
}
