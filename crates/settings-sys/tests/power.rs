//! The power clients against python-dbusmock's power-profiles-daemon and
//! UPower templates.

mod common;
mod ext;

use settings_sys::power::{self, ChargeState, Power};
use settings_sys::{Bus, ErrorKind};

const UPOWER: &str = "org.freedesktop.UPower";

#[test]
fn reads_and_sets_the_power_mode() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.template(
        "power_profiles_daemon",
        Some(r#"{"ActiveProfile": "performance"}"#),
        "net.hadess.PowerProfiles",
    );
    let p = Power::new(&bus.bus()).expect("connect");
    let s = p.profile().expect("profile").expect("a power mode service");
    assert_eq!(s.active, "performance");
    assert_eq!(s.available, ["power-saver", "balanced", "performance"]);
    assert_eq!(s.degraded, "");

    p.set_profile("power-saver").expect("set");
    assert_eq!(p.profile().unwrap().unwrap().active, "power-saver");

    // Anything else never reaches the bus.
    assert_eq!(p.set_profile("turbo").unwrap_err().kind, ErrorKind::Refused);
    assert_eq!(p.profile().unwrap().unwrap().active, "power-saver");
}

#[test]
fn finds_the_newer_power_profiles_name() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.template(
        "upower_power_profiles_daemon",
        None,
        "org.freedesktop.UPower.PowerProfiles",
    );
    let p = Power::new(&bus.bus()).expect("connect");
    let s = p.profile().expect("profile").expect("a power mode service");
    assert_eq!(s.active, "balanced");
    p.set_profile("performance").expect("set");
    assert_eq!(p.profile().unwrap().unwrap().active, "performance");
}

#[test]
fn no_power_profiles_service_is_none() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let p = Power::new(&bus.bus()).expect("connect");
    assert_eq!(p.profile().expect("not an error"), None);
}

#[test]
fn a_desktop_has_no_battery() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.template("upower", None, UPOWER);
    // Only mains power: not a battery.
    ext::call(
        &bus,
        UPOWER,
        "/org/freedesktop/UPower",
        "org.freedesktop.DBus.Mock",
        "AddAC",
        &("AC", "Mains"),
    );
    let p = Power::new(&bus.bus()).expect("connect");
    assert_eq!(p.battery().expect("battery"), None);
}

#[test]
fn reads_the_battery_and_the_charge_limit() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.template("upower", None, UPOWER);
    let path = "/org/freedesktop/UPower/devices/BAT0";
    ext::call(
        &bus,
        UPOWER,
        "/org/freedesktop/UPower",
        "org.freedesktop.DBus.Mock",
        "AddDischargingBattery",
        &("BAT0", "Test Battery", 42.0f64, 5400i64),
    );
    let dev = "org.freedesktop.UPower.Device";
    ext::add_property(&bus, UPOWER, path, dev, "Capacity", 87.5f64);
    ext::add_property(&bus, UPOWER, path, dev, "ChargeThresholdSupported", true);
    ext::add_property(&bus, UPOWER, path, dev, "ChargeThresholdEnabled", false);
    ext::add_method(
        &bus,
        UPOWER,
        path,
        dev,
        "EnableChargeThreshold",
        ("b", ""),
        "self.Set('org.freedesktop.UPower.Device', 'ChargeThresholdEnabled', args[0])",
    );

    let p = Power::new(&bus.bus()).expect("connect");
    let b = p.battery().expect("battery").expect("a battery");
    assert_eq!(b.percentage, 42.0);
    assert_eq!(b.state, ChargeState::Discharging);
    assert_eq!(b.time_to_empty, 5400);
    assert_eq!(b.health, Some(87.5));
    assert!(b.limit_supported);
    assert!(!b.limit_enabled);

    p.set_charge_limit(true).expect("limit on");
    assert!(p.battery().unwrap().unwrap().limit_enabled);
    p.set_charge_limit(false).expect("limit off");
    assert!(!p.battery().unwrap().unwrap().limit_enabled);
}

#[test]
fn a_battery_without_a_limit_says_so() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.template("upower", None, UPOWER);
    ext::call(
        &bus,
        UPOWER,
        "/org/freedesktop/UPower",
        "org.freedesktop.DBus.Mock",
        "AddChargingBattery",
        &("BAT0", "Test Battery", 90.0f64, 600i64),
    );
    let p = Power::new(&bus.bus()).expect("connect");
    let b = p.battery().expect("battery").expect("a battery");
    assert_eq!(b.state, ChargeState::Charging);
    assert_eq!(b.time_to_full, 600);
    assert!(!b.limit_supported);
    assert_eq!(b.health, None);
}

#[test]
fn asks_powerdevil_to_read_its_settings_again() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    // A session service: python-dbusmock's generic mode, on the test bus.
    let name = "org.kde.Solid.PowerManagement";
    let path = "/org/kde/Solid/PowerManagement";
    let _powerdevil = ext::generic(&bus, name, path, name);
    ext::add_method(
        &bus,
        name,
        path,
        name,
        "reparseConfiguration",
        ("", ""),
        "pass",
    );
    power::reconfigure_powerdevil(&bus.bus()).expect("reconfigure");
    assert_eq!(
        ext::calls(&bus, name, path),
        ["reparseConfiguration"],
        "PowerDevil was asked once"
    );
}

#[test]
fn powerdevil_not_running_says_so() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let e = power::reconfigure_powerdevil(&bus.bus()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotRunning, "{e:?}");
}

#[test]
fn no_bus_is_an_error_not_a_panic() {
    let e = Power::new(&Bus::Address("unix:path=/nonexistent/bus".into()))
        .err()
        .expect("must fail");
    assert_eq!(e.kind, ErrorKind::Bus, "{e:?}");
}
