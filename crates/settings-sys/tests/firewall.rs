//! The firewall client against mocks of firewalld and systemd
//! (`tests/templates/firewalld.py`, `systemd1.py`).

mod common;
mod ext;

use settings_sys::firewall::{Firewall, Port};
use settings_sys::{Bus, ErrorKind};

const FW: &str = "org.fedoraproject.FirewallD1";
const FW_PATH: &str = "/org/fedoraproject/FirewallD1";
const SD: &str = "org.freedesktop.systemd1";

fn start(zones: &str) -> Option<common::MockBus> {
    let mut bus = common::MockBus::start()?;
    bus.local_template("systemd1", None, SD);
    bus.local_template("firewalld", Some(zones), FW);
    Some(bus)
}

const ZONES: &str = r#"{"DefaultZone": "AtlasOS",
  "Zones": {"AtlasOS": {"Services": ["mdns", "samba-client", "ssh"], "Ports": [["8080", "tcp"], ["27015-27030", "udp"]]},
            "home": {"Services": ["http"]}}}"#;

#[test]
fn says_whether_the_firewall_is_on() {
    let Some(bus) = start(ZONES) else { return };
    let f = Firewall::new(&bus.bus()).expect("connect");
    let s = f.status().expect("status");
    assert!(s.installed && s.running && s.enabled);
}

#[test]
fn turns_the_firewall_off_and_on() {
    let Some(bus) = start(ZONES) else { return };
    let f = Firewall::new(&bus.bus()).expect("connect");
    f.set_enabled(false).expect("off");
    let s = f.status().unwrap();
    assert!(s.installed && !s.running && !s.enabled, "{s:?}");
    // Off means off: firewalld is not asked anything (a call would start it).
    assert_eq!(f.rules().expect("not an error"), None);
    assert!(ext::calls(&bus, FW, FW_PATH).is_empty());
    f.set_enabled(true).expect("on");
    let s = f.status().unwrap();
    assert!(s.running && s.enabled, "{s:?}");
}

#[test]
fn not_installed_is_said() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template(
        "systemd1",
        Some(r#"{"LoadState": "not-found", "ActiveState": "inactive", "UnitFileState": ""}"#),
        SD,
    );
    let f = Firewall::new(&bus.bus()).expect("connect");
    let s = f.status().unwrap();
    assert!(!s.installed && !s.running && !s.enabled);
}

#[test]
fn reads_what_the_active_zone_allows() {
    let Some(bus) = start(ZONES) else { return };
    let f = Firewall::new(&bus.bus()).expect("connect");
    let r = f.rules().expect("rules").expect("running");
    assert_eq!(r.zone, "AtlasOS");
    assert_eq!(r.services, ["mdns", "samba-client", "ssh"]);
    assert_eq!(
        r.ports,
        [
            Port {
                port: "27015-27030".into(),
                protocol: "udp".into()
            },
            Port {
                port: "8080".into(),
                protocol: "tcp".into()
            },
        ]
    );
    assert!(r.available.contains(&"kdeconnect".to_string()));
}

#[test]
fn a_zone_in_use_that_isnt_the_default_is_used() {
    let Some(bus) = start(
        r#"{"DefaultZone": "public", "ActiveZones": ["home"],
            "Zones": {"public": {}, "home": {"Services": ["http"]}}}"#,
    ) else {
        return;
    };
    let f = Firewall::new(&bus.bus()).expect("connect");
    let r = f.rules().unwrap().unwrap();
    assert_eq!(
        (r.zone.as_str(), r.services.as_slice()),
        ("home", ["http".to_string()].as_slice())
    );
}

#[test]
fn adds_and_removes_services_and_ports_and_keeps_them() {
    let Some(bus) = start(ZONES) else { return };
    let f = Firewall::new(&bus.bus()).expect("connect");
    f.add_service("kdeconnect").expect("add service");
    f.remove_service("ssh").expect("remove service");
    let p = Port {
        port: "9000".into(),
        protocol: "tcp".into(),
    };
    f.add_port(&p).expect("add port");
    f.remove_port(&Port {
        port: "8080".into(),
        protocol: "tcp".into(),
    })
    .expect("remove port");
    let r = f.rules().unwrap().unwrap();
    assert_eq!(r.services, ["kdeconnect", "mdns", "samba-client"]);
    assert!(r.ports.contains(&p));
    assert!(!r.ports.iter().any(|p| p.port == "8080"));
    // Each change was made permanent too.
    let calls = ext::calls(&bus, FW, FW_PATH);
    assert_eq!(
        calls.iter().filter(|c| *c == "runtimeToPermanent").count(),
        4,
        "{calls:?}"
    );
    // Home isn't the zone in use: untouched.
    let log = ext::call_log(&bus, FW, FW_PATH);
    assert!(
        log.contains("AtlasOS") && !log.contains("\"home\""),
        "{log}"
    );
}

#[test]
fn what_firewalld_refuses_comes_back_as_refused() {
    let Some(bus) = start(ZONES) else { return };
    let f = Firewall::new(&bus.bus()).expect("connect");
    let e = f.add_service("no-such-service").unwrap_err();
    assert_eq!(e.kind, ErrorKind::Refused, "{e:?}");
    // Already allowed.
    assert!(f.add_service("ssh").is_err());
}

#[test]
fn junk_never_reaches_the_bus() {
    let Some(bus) = start(ZONES) else { return };
    let f = Firewall::new(&bus.bus()).expect("connect");
    assert_eq!(f.add_service("../x").unwrap_err().kind, ErrorKind::Refused);
    for (port, proto) in [
        ("0", "tcp"),
        ("70000", "udp"),
        ("80", "icmp"),
        ("80;", "tcp"),
        ("", "tcp"),
    ] {
        let e = f
            .add_port(&Port {
                port: port.into(),
                protocol: proto.into(),
            })
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::Refused, "{port} {proto}");
    }
    let calls = ext::calls(&bus, FW, FW_PATH);
    assert!(!calls.iter().any(|c| c.starts_with("add")), "{calls:?}");
}

#[test]
fn firewalld_not_running_has_no_rules() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template("systemd1", Some(r#"{"ActiveState": "inactive"}"#), SD);
    let f = Firewall::new(&bus.bus()).expect("connect");
    assert_eq!(f.rules().expect("not an error"), None);
    assert!(!f.status().unwrap().running);
}

#[test]
fn no_bus_is_an_error_not_a_panic() {
    let e = Firewall::new(&Bus::Address("unix:path=/nonexistent/bus".into()))
        .err()
        .expect("must fail");
    assert_eq!(e.kind, ErrorKind::Bus, "{e:?}");
}
