//! Telling Updater's tray about a changed setting.

mod common;
mod ext;

use settings_sys::ErrorKind;
use settings_sys::updater::reload_tray;

const NAME: &str = "net.eterneon.atlas.updater.Tray";
const PATH: &str = "/net/eterneon/atlas/updater/Tray";

#[test]
fn asks_the_tray_to_reload() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let _tray = ext::generic(&bus, NAME, PATH, NAME);
    ext::add_method(&bus, NAME, PATH, NAME, "Reload", ("", ""), "pass");
    reload_tray(&bus.bus()).expect("reload");
    assert_eq!(ext::calls(&bus, NAME, PATH), ["Reload"]);
}

#[test]
fn a_tray_that_cannot_start_is_an_error_not_a_hang() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let e = reload_tray(&bus.bus()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotRunning, "{e:?}");
}
