//! Settings' clients for the system services it sets up through: each is a
//! thin, typed wrapper over the service's D-Bus API (NetworkManager, BlueZ,
//! timedated, ...), never a reimplementation of the service. Privileged
//! changes are the service's own polkit actions.
//!
//! Every call blocks, with a timeout ([`bus::METHOD_TIMEOUT`]), so run them
//! on a worker thread, never the GUI thread. Errors are [`Error`]s with a
//! plain-words description.
//!
//! The tests run each client against python-dbusmock's model of the service
//! on a private bus (`tests/common`), never the machine's own services.

pub mod accounts;
pub mod bus;
pub mod error;
pub mod firewall;
pub mod fprint;
pub mod hostname;
pub mod locale;
pub mod power;
pub mod sysinfo;
pub mod timedate;
pub mod updater;

pub use bus::Bus;
pub use error::{Error, ErrorKind};
