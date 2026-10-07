//! Telamon Updater's tray (`net.eterneon.telamon.updater.Tray`, in the user's
//! session): the part of Updater that watches for crashes and background
//! updates. After the crash report setting changes, Settings tells it with
//! `Reload`.
//!
//! Until the next release the tray also answers to its old name
//! (`net.eterneon.atlas.updater.Tray`), which an Updater from before the
//! rename owns alone: the new name is asked first, the old one when nothing
//! owns the new.

use crate::Error;
use crate::ErrorKind;
use crate::bus::Bus;
use std::time::Duration;
use zbus::blocking::Proxy;

const NAME: &str = "net.eterneon.telamon.updater.Tray";
const PATH: &str = "/net/eterneon/telamon/updater/Tray";
/// The name before the rename.
const OLD_NAME: &str = "net.eterneon.atlas.updater.Tray";
const OLD_PATH: &str = "/net/eterneon/atlas/updater/Tray";

/// Long enough for D-Bus activation to start the tray.
const LIMIT: Duration = Duration::from_secs(25);

/// Asks the tray to read the settings again (it is started when it isn't
/// running). Blocking, so for a worker thread.
pub fn reload_tray(bus: &Bus) -> Result<(), Error> {
    let conn = bus.connect_with(LIMIT)?;
    let call = |name: &str, path: &str| -> Result<(), Error> {
        Proxy::new(&conn, name, path, name)?.call::<_, _, ()>("Reload", &())?;
        Ok(())
    };
    match call(NAME, PATH) {
        Err(e) if e.kind == ErrorKind::NotRunning => call(OLD_NAME, OLD_PATH),
        other => other,
    }
}
