//! Telling Updater's tray about a changed setting.

mod common;
mod ext;

use settings_sys::ErrorKind;
use settings_sys::updater::{reload_tray, set_working};

const NAME: &str = "net.eterneon.telamon.updater.Tray";
const PATH: &str = "/net/eterneon/telamon/updater/Tray";
const OLD_NAME: &str = "net.eterneon.atlas.updater.Tray";
const OLD_PATH: &str = "/net/eterneon/atlas/updater/Tray";

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
fn tells_the_tray_when_the_system_is_being_changed() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let _tray = ext::generic(&bus, NAME, PATH, NAME);
    ext::add_method(&bus, NAME, PATH, NAME, "SetWorking", ("b", ""), "pass");
    set_working(&bus.bus(), true).expect("on");
    set_working(&bus.bus(), false).expect("off");
    assert_eq!(ext::calls(&bus, NAME, PATH), ["SetWorking", "SetWorking"]);
}

#[test]
fn no_tray_is_not_a_hang_for_the_glow_either() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let e = set_working(&bus.bus(), true).unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotRunning, "{e:?}");
}

/// An Updater from before the rename owns only the old name.
#[test]
fn falls_back_to_the_old_name() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let _tray = ext::generic(&bus, OLD_NAME, OLD_PATH, OLD_NAME);
    ext::add_method(
        &bus,
        OLD_NAME,
        OLD_PATH,
        OLD_NAME,
        "Reload",
        ("", ""),
        "pass",
    );
    reload_tray(&bus.bus()).expect("reload through the old name");
    assert_eq!(ext::calls(&bus, OLD_NAME, OLD_PATH), ["Reload"]);
}

#[test]
fn a_tray_that_cannot_start_is_an_error_not_a_hang() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let e = reload_tray(&bus.bus()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotRunning, "{e:?}");
}
