//! The locale1 client against this crate's localed mock.

mod common;

use settings_sys::ErrorKind;
use settings_sys::locale::Localed;

#[test]
fn reads_and_sets_the_system_language() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template(
        "localed",
        Some(r#"{"Locale": ["LANG=en_US.UTF-8", "LC_TIME=en_GB.UTF-8"]}"#),
        "org.freedesktop.locale1",
    );
    let l = Localed::new(&bus.bus()).expect("connect");
    assert_eq!(l.language().expect("language"), "en_US.UTF-8");
    l.set_language("de_DE.UTF-8").expect("set");
    assert_eq!(l.language().expect("language"), "de_DE.UTF-8");
    assert_eq!(
        l.set_language("de_DE.UTF-8; reboot").unwrap_err().kind,
        ErrorKind::Refused
    );
}

#[test]
fn junk_from_the_service_reads_as_unset() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template(
        "localed",
        Some(r#"{"Locale": ["LANG=../../x"]}"#),
        "org.freedesktop.locale1",
    );
    let l = Localed::new(&bus.bus()).expect("connect");
    assert_eq!(l.language().expect("language"), "");
}
