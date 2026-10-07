//! The hostname1 client against this crate's hostnamed mock.

mod common;

use settings_sys::ErrorKind;
use settings_sys::hostname::Hostname;

#[test]
fn reads_and_renames_the_device() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template(
        "hostnamed",
        Some(r#"{"Hostname": "fedora", "StaticHostname": "fedora", "PrettyHostname": ""}"#),
        "org.freedesktop.hostname1",
    );
    let h = Hostname::new(&bus.bus()).expect("connect");
    let s = h.status().expect("status");
    // No pretty name: the static one stands in.
    assert_eq!(s.name, "fedora");
    assert_eq!(s.chassis, "laptop");

    h.set_name("  Zach's Laptop ").expect("rename");
    let s = h.status().expect("status");
    assert_eq!(s.name, "Zach's Laptop");
    assert_eq!(s.hostname, "zach-s-laptop");

    // A name with nothing left for a hostname keeps the old hostname.
    h.set_name("日本").expect("rename");
    let s = h.status().expect("status");
    assert_eq!(
        (s.name.as_str(), s.hostname.as_str()),
        ("日本", "zach-s-laptop")
    );

    for bad in ["", "a\nb", &"x".repeat(65)] {
        assert_eq!(h.set_name(bad).unwrap_err().kind, ErrorKind::Refused);
    }
}

#[test]
fn untrusted_names_are_made_safe() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    // 100 characters, an escape sequence and a bidi override.
    bus.local_template(
        "hostnamed",
        Some(&format!(
            "{{\"PrettyHostname\": \"{}\\u001b[2J\\u202e\"}}",
            "A".repeat(100)
        )),
        "org.freedesktop.hostname1",
    );
    let s = Hostname::new(&bus.bus())
        .expect("connect")
        .status()
        .expect("status");
    assert!(
        s.name.chars().count() <= settings_sys::hostname::MAX_NAME + 1,
        "{}",
        s.name
    );
    assert!(!s.name.contains('\u{1b}') && !s.name.contains('\u{202e}'));
}
