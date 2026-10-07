//! The NetworkManager client against python-dbusmock's `networkmanager`
//! template.

mod common;

use settings_sys::network::{Link, Network, Secret, Security};
use settings_sys::{Bus, ErrorKind};
use std::collections::HashMap;
use zbus::zvariant::{DynamicType, Value};

const NM: &str = "org.freedesktop.NetworkManager";
const MOCK: &str = "org.freedesktop.DBus.Mock";
// The template's own helpers live on its main object.
const MAIN: &str = "/org/freedesktop";

fn call<B, R>(bus: &common::MockBus, path: &str, iface: &str, method: &str, body: &B) -> R
where
    B: zbus::export::serde::Serialize + DynamicType,
    R: zbus::export::serde::de::DeserializeOwned + zbus::zvariant::Type,
{
    let conn = zbus::blocking::connection::Builder::address(bus.address.as_str())
        .and_then(|b| b.build())
        .expect("connect to the test bus");
    conn.call_method(Some(NM), path, Some(iface), method, body)
        .unwrap_or_else(|e| panic!("{method}: {e}"))
        .body()
        .deserialize()
        .unwrap_or_else(|e| panic!("{method} reply: {e}"))
}

fn start() -> Option<common::MockBus> {
    let mut bus = common::MockBus::start()?;
    bus.template("networkmanager", None, NM);
    Some(bus)
}

fn wifi_device(bus: &common::MockBus) -> String {
    call(bus, MAIN, MOCK, "AddWiFiDevice", &("wlan0", "wlan0", 30u32))
}

fn access_point(bus: &common::MockBus, dev: &str, name: &str, ssid: &str, strength: u8, sec: u32) {
    let _: String = call(
        bus,
        MAIN,
        MOCK,
        "AddAccessPoint",
        &(
            dev,
            name,
            ssid,
            "00:11:22:33:44:55",
            2u32,
            2412u32,
            54000u32,
            strength,
            sec,
        ),
    );
}

/// Replaces the template's AddAndActivateConnection (which drops the
/// password and needs the access point's name to match) with one that
/// records what it was given, then adds and activates the connection as
/// NetworkManager would. `LastSettings` returns the record.
fn record_new_connections(bus: &common::MockBus) {
    bus.add_method(
        NM,
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
        "AddAndActivateConnection",
        ("a{sa{sv}}oo", "oo"),
        r#"
self.last = repr(args[0])
self.n = getattr(self, "n", 0) + 1
main = objects["/org/freedesktop"]
conn = main.SettingsAddConnection(args[0])
act = main.AddActiveConnection([args[1]], conn, args[2], "new%d" % self.n, 2)
ret = (conn, act)
"#,
    );
    bus.add_method(
        NM,
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
        "LastSettings",
        ("", "s"),
        "ret = getattr(self, 'last', '')",
    );
}

#[test]
fn lists_networks_strongest_per_name_and_marks_the_known_ones() {
    let Some(bus) = start() else { return };
    let dev = wifi_device(&bus);
    access_point(&bus, &dev, "ap1", "Home", 80, 0x100);
    access_point(&bus, &dev, "ap2", "Home", 30, 0x100);
    access_point(&bus, &dev, "ap3", "Cafe", 60, 0);
    access_point(&bus, &dev, "ap4", "Work", 90, 0x200);
    access_point(&bus, &dev, "ap5", "bad\u{1b}[31m\u{202E}name", 20, 0x100);
    access_point(&bus, &dev, "ap6", "", 99, 0);
    let _: String = call(
        &bus,
        MAIN,
        MOCK,
        "AddWiFiConnection",
        &(&dev, "saved1", "Cafe", ""),
    );

    let net = Network::new(&bus.bus()).expect("connect");
    let s = net.status().expect("status");
    assert!(s.wifi_present && s.wifi_enabled);
    assert_eq!(s.wifi_ssid, None);
    assert!(s.wired.is_none() && s.vpns.is_empty() && s.hotspot.is_none());

    let list = net.wifi_networks().expect("networks");
    let names: Vec<&str> = list.iter().map(|n| n.ssid.as_str()).collect();
    // Saved first, then by signal; the hidden one is left out; control and
    // bidi characters are replaced.
    assert_eq!(
        names,
        ["Cafe", "Work", "Home", "bad\u{FFFD}[31m\u{FFFD}name"]
    );
    let home = list.iter().find(|n| n.ssid == "Home").expect("Home");
    assert_eq!((home.signal, home.security), (80, Security::Personal));
    assert!(list[0].known && !list[0].connected);
    assert_eq!(list[1].security, Security::Enterprise);
    assert!(!list[2].known);
    assert_eq!(list[0].security, Security::Open);
}

#[test]
fn joins_a_network_with_a_password_and_forgets_it() {
    let Some(bus) = start() else { return };
    let dev = wifi_device(&bus);
    access_point(&bus, &dev, "ap1", "Home", 80, 0x100);
    access_point(&bus, &dev, "ap2", "Cafe", 60, 0);
    access_point(&bus, &dev, "ap3", "Work", 90, 0x200);
    record_new_connections(&bus);
    let net = Network::new(&bus.bus()).expect("connect");

    // A secured network needs a password, a short one never reaches the bus.
    assert_eq!(
        net.connect_wifi("Home", None).unwrap_err().kind,
        ErrorKind::Refused
    );
    assert_eq!(
        net.connect_wifi("Home", Some(&Secret::new("short")))
            .unwrap_err()
            .kind,
        ErrorKind::Refused
    );
    // Work needs a login set up in Plasma, so it is refused.
    assert_eq!(
        net.connect_wifi("Work", Some(&Secret::new("correct horse")))
            .unwrap_err()
            .kind,
        ErrorKind::Refused
    );
    let last: String = call(
        &bus,
        "/org/freedesktop/NetworkManager",
        NM,
        "LastSettings",
        &(),
    );
    assert_eq!(last, "", "nothing was sent for those");

    net.connect_wifi("Home", Some(&Secret::new("correct horse battery")))
        .expect("join Home");
    let sent: String = call(
        &bus,
        "/org/freedesktop/NetworkManager",
        NM,
        "LastSettings",
        &(),
    );
    // The password went to NetworkManager, with the key management.
    assert!(sent.contains("correct horse battery"), "{sent}");
    assert!(sent.contains("wpa-psk"), "{sent}");
    assert!(sent.contains("802-11-wireless-security"), "{sent}");

    let s = net.status().expect("status");
    assert_eq!(s.wifi_ssid.as_deref(), Some("Home"));
    let list = net.wifi_networks().expect("networks");
    let home = list.iter().find(|n| n.ssid == "Home").expect("Home");
    assert!(home.connected && home.known);
    assert_eq!(list[0].ssid, "Home", "the one in use first");

    // An open network needs none.
    net.connect_wifi("Cafe", None).expect("join Cafe");
    let sent: String = call(
        &bus,
        "/org/freedesktop/NetworkManager",
        NM,
        "LastSettings",
        &(),
    );
    assert!(!sent.contains("802-11-wireless-security"), "{sent}");

    // Leaving keeps it saved; forgetting removes it.
    net.disconnect_wifi().expect("disconnect");
    assert!(
        net.wifi_networks()
            .expect("networks")
            .iter()
            .all(|n| !n.connected)
    );
    net.forget_wifi("Home").expect("forget");
    let list = net.wifi_networks().expect("networks");
    assert!(!list.iter().find(|n| n.ssid == "Home").expect("Home").known);
    assert_eq!(
        net.forget_wifi("Home").unwrap_err().kind,
        ErrorKind::Refused
    );

    // Out of range.
    assert_eq!(
        net.connect_wifi("Nowhere", None).unwrap_err().kind,
        ErrorKind::Refused
    );
}

#[test]
fn the_radios_and_airplane_mode() {
    let Some(bus) = start() else { return };
    let _dev = wifi_device(&bus);
    let net = Network::new(&bus.bus()).expect("connect");
    let s = net.status().expect("status");
    assert!(s.wifi_enabled);

    net.set_wifi_enabled(false).expect("wifi off");
    assert!(!net.status().expect("status").wifi_enabled);
    net.set_radios_enabled(true).expect("radios on");
    let s = net.status().expect("status");
    assert!(s.wifi_enabled && s.wwan_enabled);
    net.set_radios_enabled(false).expect("radios off");
    let s = net.status().expect("status");
    assert!(!s.wifi_enabled && !s.wwan_enabled);
}

#[test]
fn wired_status() {
    let Some(bus) = start() else { return };
    let _: String = call(
        &bus,
        MAIN,
        MOCK,
        "AddEthernetDevice",
        &("eth0", "enp3s0", 100u32),
    );
    let net = Network::new(&bus.bus()).expect("connect");
    let s = net.status().expect("status");
    let wired = s.wired.expect("a wired device");
    assert_eq!(wired.interface, "enp3s0");
    assert_eq!(wired.link, Link::Connected);
    assert!(!s.wifi_present);
    assert!(net.wifi_networks().expect("networks").is_empty());

    // No cable.
    let bus2 = start().expect("second bus");
    let _: String = call(
        &bus2,
        MAIN,
        MOCK,
        "AddEthernetDevice",
        &("eth0", "enp3s0", 20u32),
    );
    let s = Network::new(&bus2.bus())
        .expect("connect")
        .status()
        .expect("status");
    assert_eq!(s.wired.expect("wired").link, Link::Unplugged);
}

#[test]
fn vpn_connections() {
    let Some(bus) = start() else { return };
    let _dev = wifi_device(&bus);
    let mut vpn: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::new();
    vpn.entry("connection").or_default().extend([
        ("type", Value::from("vpn")),
        ("id", Value::from("Work VPN")),
        ("uuid", Value::from("11111111-2222-3333-4444-555555555555")),
    ]);
    vpn.entry("vpn").or_default().insert(
        "service-type",
        Value::from("org.freedesktop.NetworkManager.openvpn"),
    );
    let _: zbus::zvariant::OwnedObjectPath = call(
        &bus,
        "/org/freedesktop/NetworkManager/Settings",
        "org.freedesktop.NetworkManager.Settings",
        "AddConnection",
        &(vpn,),
    );

    let net = Network::new(&bus.bus()).expect("connect");
    let s = net.status().expect("status");
    assert_eq!(s.vpns.len(), 1);
    assert_eq!(s.vpns[0].id, "Work VPN");
    assert!(!s.vpns[0].active);

    let uuid = s.vpns[0].uuid.clone();
    net.set_vpn(&uuid, true).expect("connect");
    assert!(net.status().expect("status").vpns[0].active);
    net.set_vpn(&uuid, false).expect("disconnect");
    assert!(!net.status().expect("status").vpns[0].active);
    assert_eq!(
        net.set_vpn("no-such-vpn", true).unwrap_err().kind,
        ErrorKind::Refused
    );
}

#[test]
fn the_hotspot() {
    let Some(bus) = start() else { return };
    let _dev = wifi_device(&bus);
    record_new_connections(&bus);
    let net = Network::new(&bus.bus()).expect("connect");

    // Nothing saved yet.
    assert_eq!(
        net.start_hotspot("x", None).unwrap_err().kind,
        ErrorKind::Refused
    );
    // A weak password never reaches the bus.
    assert_eq!(
        net.start_hotspot("Zach's PC", Some(&Secret::new("1234")))
            .unwrap_err()
            .kind,
        ErrorKind::Refused
    );

    net.start_hotspot("Zach's PC", Some(&Secret::new("sharing is caring")))
        .expect("start");
    let sent: String = call(
        &bus,
        "/org/freedesktop/NetworkManager",
        NM,
        "LastSettings",
        &(),
    );
    assert!(
        sent.contains("sharing is caring") && sent.contains("shared"),
        "{sent}"
    );
    assert!(sent.contains("'ap'") || sent.contains("\"ap\""), "{sent}");
    let h = net.status().expect("status").hotspot.expect("hotspot");
    assert_eq!(h.ssid, "Zach's PC");
    assert!(h.active);
    // It isn't the Wi-Fi in use.
    assert_eq!(net.status().expect("status").wifi_ssid, None);

    net.stop_hotspot().expect("stop");
    let h = net.status().expect("status").hotspot.expect("still saved");
    assert!(!h.active);

    net.start_hotspot("", None).expect("start the saved one");
    assert!(
        net.status()
            .expect("status")
            .hotspot
            .expect("hotspot")
            .active
    );
}

#[test]
fn a_missing_service_says_so() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let net = Network::new(&bus.bus()).expect("connect");
    assert_eq!(net.status().unwrap_err().kind, ErrorKind::NotRunning);
}

#[test]
fn no_bus_is_an_error_not_a_panic() {
    let e = Network::new(&Bus::Address("unix:path=/nonexistent/bus".into()))
        .err()
        .expect("must fail");
    assert_eq!(e.kind, ErrorKind::Bus, "{e:?}");
}
