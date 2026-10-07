//! The BlueZ client against python-dbusmock's `bluez5` template.

mod common;

use settings_sys::bluetooth::{Bluetooth, Kind};
use settings_sys::{Bus, ErrorKind};
use std::collections::HashMap;
use zbus::zvariant::{DynamicType, Value};

const BLUEZ: &str = "org.bluez";
const MOCK: &str = "org.bluez.Mock";

fn call<B, R>(bus: &common::MockBus, path: &str, iface: &str, method: &str, body: &B) -> R
where
    B: zbus::export::serde::Serialize + DynamicType,
    R: zbus::export::serde::de::DeserializeOwned + zbus::zvariant::Type,
{
    let conn = zbus::blocking::connection::Builder::address(bus.address.as_str())
        .and_then(|b| b.build())
        .expect("connect to the test bus");
    conn.call_method(Some(BLUEZ), path, Some(iface), method, body)
        .unwrap_or_else(|e| panic!("{method}: {e}"))
        .body()
        .deserialize()
        .unwrap_or_else(|e| panic!("{method} reply: {e}"))
}

fn start() -> Option<common::MockBus> {
    let mut bus = common::MockBus::start()?;
    bus.template("bluez5", None, BLUEZ);
    Some(bus)
}

fn adapter(bus: &common::MockBus) {
    let _: String = call(bus, "/", MOCK, "AddAdapter", &("hci0", "Zach's PC"));
}

fn device(bus: &common::MockBus, address: &str, name: &str) -> String {
    call(bus, "/", MOCK, "AddDevice", &("hci0", address, name))
}

#[test]
fn lists_paired_and_nearby_devices() {
    let Some(bus) = start() else { return };
    adapter(&bus);
    device(&bus, "AA:BB:CC:00:00:01", "Headphones");
    let nearby = device(&bus, "AA:BB:CC:00:00:02", "Speaker");
    device(&bus, "AA:BB:CC:00:00:03", "");
    device(&bus, "AA:BB:CC:00:00:04", "Old Phone");
    let _: () = call(&bus, "/", MOCK, "PairDevice", &("hci0", "AA:BB:CC:00:00:04"));
    let _: () = call(&bus, "/", MOCK, "ConnectDevice", &("hci0", "AA:BB:CC:00:00:04"));
    // A battery level.
    let battery: HashMap<&str, Value<'_>> = HashMap::from([("Percentage", Value::from(87u8))]);
    let _: () = call(
        &bus,
        &nearby,
        "org.freedesktop.DBus.Mock",
        "AddProperties",
        &("org.bluez.Battery1", battery),
    );

    let bt = Bluetooth::new(&bus.bus()).expect("connect");
    let s = bt.snapshot().expect("snapshot");
    let adapter = s.adapter.expect("an adapter");
    assert_eq!(adapter.name, "Zach's PC");
    assert!(adapter.powered && !adapter.discovering);

    let names: Vec<(&str, bool, bool)> = s
        .devices
        .iter()
        .map(|d| (d.name.as_str(), d.paired, d.connected))
        .collect();
    // Paired first; the nameless unpaired one is left out.
    assert_eq!(
        names,
        [
            ("Old Phone", true, true),
            ("Headphones", false, false),
            ("Speaker", false, false)
        ]
    );
    let speaker = s.devices.iter().find(|d| d.name == "Speaker").expect("Speaker");
    assert_eq!(speaker.battery, Some(87));
    assert_eq!(speaker.address, "AA:BB:CC:00:00:02");
    // The template's devices are phones.
    assert_eq!(speaker.kind, Kind::Phone);
}

#[test]
fn pairs_connects_and_forgets() {
    let Some(bus) = start() else { return };
    adapter(&bus);
    device(&bus, "AA:BB:CC:00:00:01", "Headphones");
    let bt = Bluetooth::new(&bus.bus()).expect("connect");

    bt.pair("AA:BB:CC:00:00:01").expect("pair");
    let d = &bt.snapshot().expect("snapshot").devices[0];
    assert!(d.paired, "{d:?}");

    // The template's Connect and Disconnect answer without changing the
    // property (its ConnectDevice helper does), so what is checked is that
    // they are accepted, and that a second Connect says what BlueZ says.
    // (Pairing connected it.)
    bt.disconnect("AA:BB:CC:00:00:01").expect("disconnect");
    assert_eq!(
        bt.disconnect("AA:BB:CC:00:00:01").unwrap_err().kind,
        ErrorKind::Refused
    );
    bt.connect("aa:bb:cc:00:00:01").expect("connect, any case");
    assert_eq!(
        bt.connect("AA:BB:CC:00:00:01").unwrap_err().kind,
        ErrorKind::Refused
    );
    let _: () = call(&bus, "/", MOCK, "ConnectDevice", &("hci0", "AA:BB:CC:00:00:01"));
    assert!(bt.snapshot().expect("snapshot").devices[0].connected);

    bt.forget("AA:BB:CC:00:00:01").expect("forget");
    assert!(bt.snapshot().expect("snapshot").devices.is_empty());
}

#[test]
fn power_visibility_and_discovery() {
    let Some(bus) = start() else { return };
    adapter(&bus);
    let bt = Bluetooth::new(&bus.bus()).expect("connect");

    bt.set_powered(false).expect("off");
    assert!(!bt.snapshot().expect("snapshot").adapter.expect("adapter").powered);
    bt.set_powered(true).expect("on");
    assert!(bt.snapshot().expect("snapshot").adapter.expect("adapter").powered);

    bt.set_discoverable(true).expect("visible");
    assert!(bt.snapshot().expect("snapshot").adapter.expect("adapter").discoverable);
    bt.set_discoverable(false).expect("hidden");
    assert!(!bt.snapshot().expect("snapshot").adapter.expect("adapter").discoverable);

    // The template's discovery needs a filter set first.
    let filter: HashMap<&str, Value<'_>> = HashMap::new();
    let _: () = call(
        &bus,
        "/org/bluez/hci0",
        "org.bluez.Adapter1",
        "SetDiscoveryFilter",
        &(filter,),
    );
    bt.start_discovery().expect("start");
    assert!(bt.snapshot().expect("snapshot").adapter.expect("adapter").discovering);
    bt.stop_discovery().expect("stop");
    assert!(!bt.snapshot().expect("snapshot").adapter.expect("adapter").discovering);
}

#[test]
fn junk_never_reaches_the_bus() {
    let Some(bus) = start() else { return };
    adapter(&bus);
    let bt = Bluetooth::new(&bus.bus()).expect("connect");
    for bad in ["", "../org/bluez/hci0", "AA:BB:CC:00:00", "AA:BB:CC:00:00:ZZ"] {
        assert_eq!(bt.connect(bad).unwrap_err().kind, ErrorKind::Refused, "{bad:?}");
        assert_eq!(bt.forget(bad).unwrap_err().kind, ErrorKind::Refused, "{bad:?}");
    }
    // A device that isn't there.
    assert_eq!(
        bt.pair("AA:BB:CC:00:00:09").unwrap_err().kind,
        ErrorKind::Refused
    );
}

#[test]
fn no_adapter_is_not_an_error_to_read() {
    let Some(bus) = start() else { return };
    let bt = Bluetooth::new(&bus.bus()).expect("connect");
    let s = bt.snapshot().expect("snapshot");
    assert!(s.adapter.is_none() && s.devices.is_empty());
    assert_eq!(bt.set_powered(true).unwrap_err().kind, ErrorKind::Refused);
}

#[test]
fn device_names_are_safe_text() {
    let Some(bus) = start() else { return };
    adapter(&bus);
    device(&bus, "AA:BB:CC:00:00:01", "ev\u{1b}[31mil\u{202E}name");
    device(&bus, "AA:BB:CC:00:00:02", &"x".repeat(500));
    let bt = Bluetooth::new(&bus.bus()).expect("connect");
    let s = bt.snapshot().expect("snapshot");
    let evil = s.devices.iter().find(|d| d.address.ends_with("01")).expect("evil");
    assert_eq!(evil.name, "ev\u{FFFD}[31mil\u{FFFD}name");
    let long = s.devices.iter().find(|d| d.address.ends_with("02")).expect("long");
    assert_eq!(long.name.chars().count(), 65);
}

#[test]
fn a_missing_service_says_so() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let bt = Bluetooth::new(&bus.bus()).expect("connect");
    assert_eq!(bt.snapshot().unwrap_err().kind, ErrorKind::NotRunning);
}

#[test]
fn no_bus_is_an_error_not_a_panic() {
    let e = Bluetooth::new(&Bus::Address("unix:path=/nonexistent/bus".into()))
        .err()
        .expect("must fail");
    assert_eq!(e.kind, ErrorKind::Bus, "{e:?}");
}
