//! Atlas Updater's tray (`net.eterneon.atlas.updater.Tray`, in the user's
//! session): the part of Updater that watches for crashes and background
//! updates. After the crash report setting changes, Updater's own window
//! tells it with `Reload`, and so does Settings.

use crate::Error;
use crate::bus::Bus;
use std::time::Duration;
use zbus::blocking::Proxy;

const NAME: &str = "net.eterneon.atlas.updater.Tray";
const PATH: &str = "/net/eterneon/atlas/updater/Tray";

/// Long enough for D-Bus activation to start the tray.
const LIMIT: Duration = Duration::from_secs(25);

/// Asks the tray to read the settings again (it is started when it isn't
/// running). Blocking, so for a worker thread.
pub fn reload_tray(bus: &Bus) -> Result<(), Error> {
    let conn = bus.connect_with(LIMIT)?;
    Proxy::new(&conn, NAME, PATH, NAME)?.call::<_, _, ()>("Reload", &())?;
    Ok(())
}
