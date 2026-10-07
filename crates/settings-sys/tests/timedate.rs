//! The timedate1 client against python-dbusmock's `timedated` template.

mod common;

use settings_sys::timedate::Timedate;
use settings_sys::{Bus, ErrorKind};

#[test]
fn reads_and_changes_the_time_zone() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.template(
        "timedated",
        Some(r#"{"Timezone": "Europe/Berlin", "NTP": false}"#),
        "org.freedesktop.timedate1",
    );
    let td = Timedate::new(&bus.bus()).expect("connect");
    let s = td.status().expect("status");
    assert_eq!(s.timezone, "Europe/Berlin");
    assert!(!s.ntp);

    td.set_timezone("America/New_York").expect("set timezone");
    td.set_ntp(true).expect("set ntp");
    let s = td.status().expect("status");
    assert_eq!(s.timezone, "America/New_York");
    assert!(s.ntp);

    // Junk never reaches the bus.
    let e = td.set_timezone("../../etc/passwd").unwrap_err();
    assert_eq!(e.kind, ErrorKind::Refused);
}

#[test]
fn lists_valid_time_zones_sorted() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.template("timedated", None, "org.freedesktop.timedate1");
    // The template has no ListTimezones; this one answers with junk mixed in.
    bus.add_method(
        "org.freedesktop.timedate1",
        "/org/freedesktop/timedate1",
        "org.freedesktop.timedate1",
        "ListTimezones",
        ("", "as"),
        r#"ret = ["UTC", "Europe/Berlin", "../etc/passwd", "America/New_York", "Europe/Berlin", "a\x1bb"]"#,
    );
    let td = Timedate::new(&bus.bus()).expect("connect");
    assert_eq!(
        td.timezones().expect("timezones"),
        ["America/New_York", "Europe/Berlin", "UTC"]
    );
}

#[test]
fn a_missing_service_says_so() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let td = Timedate::new(&bus.bus()).expect("connect");
    let e = td.status().unwrap_err();
    assert_eq!(e.kind, ErrorKind::NotRunning, "{e:?}");
}

#[test]
fn no_bus_is_an_error_not_a_panic() {
    let e = Timedate::new(&Bus::Address("unix:path=/nonexistent/bus".into()))
        .err()
        .expect("must fail");
    assert_eq!(e.kind, ErrorKind::Bus, "{e:?}");
}
