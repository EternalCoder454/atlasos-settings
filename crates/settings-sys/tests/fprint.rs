//! The fprintd client against a mock of it (`tests/templates/fprintd.py`).

mod common;
mod ext;

use settings_sys::fprint::{Fprint, Progress};
use settings_sys::{Bus, ErrorKind};
use std::sync::atomic::AtomicBool;

const NAME: &str = "net.reactivated.Fprint";

#[test]
fn no_reader_is_none() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template("fprintd", Some(r#"{"NoDevice": true}"#), NAME);
    let f = Fprint::new(&bus.bus()).expect("connect");
    assert_eq!(f.reader().expect("not an error"), None);
}

#[test]
fn no_fprintd_is_none() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let f = Fprint::new(&bus.bus()).expect("connect");
    assert_eq!(f.reader().expect("not an error"), None);
}

#[test]
fn reads_the_reader_and_the_enrolled_fingers() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template(
        "fprintd",
        Some(r#"{"Enrolled": ["left-thumb", "right-index-finger"], "Stages": 4}"#),
        NAME,
    );
    let f = Fprint::new(&bus.bus()).expect("connect");
    let r = f.reader().expect("reader").expect("a reader");
    assert_eq!(r.name, "Mock Fingerprint Reader");
    assert_eq!(r.stages, 4);
    assert!(!r.swipe);
    // In the order a person counts them, whatever order fprintd gave.
    assert_eq!(
        f.enrolled(&r, "ada").expect("enrolled"),
        ["right-index-finger", "left-thumb"]
    );
}

#[test]
fn nothing_enrolled_is_an_empty_list_not_an_error() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template("fprintd", None, NAME);
    let f = Fprint::new(&bus.bus()).expect("connect");
    let r = f.reader().unwrap().unwrap();
    assert!(f.enrolled(&r, "ada").expect("enrolled").is_empty());
}

#[test]
fn enrols_a_finger_and_reports_each_scan() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template("fprintd", Some(r#"{"Stages": 3}"#), NAME);
    let f = Fprint::new(&bus.bus()).expect("connect");
    let r = f.reader().unwrap().unwrap();
    let mut seen = Vec::new();
    let saved = f
        .enroll(
            &r,
            "ada",
            "left-index-finger",
            &AtomicBool::new(false),
            |p| seen.push(p),
        )
        .expect("enroll");
    assert!(saved);
    assert_eq!(
        seen,
        [
            Progress::Stage { done: 1, of: 3 },
            Progress::Stage { done: 2, of: 3 },
            Progress::Stage { done: 3, of: 3 },
            Progress::Completed,
        ]
    );
    assert_eq!(f.enrolled(&r, "ada").unwrap(), ["left-index-finger"]);
    // Claimed, scanned, stopped and let go, in that order.
    let calls = ext::calls(&bus, NAME, "/net/reactivated/Fprint/Device/0");
    let pos = |m: &str| {
        calls
            .iter()
            .position(|c| c == m)
            .unwrap_or_else(|| panic!("{m}: {calls:?}"))
    };
    assert!(pos("Claim") < pos("EnrollStart"));
    assert!(pos("EnrollStart") < pos("EnrollStop"));
    assert!(pos("EnrollStop") < pos("Release"));
}

#[test]
fn a_failed_scan_is_reported_and_ends_it() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template(
        "fprintd",
        Some(
            r#"{"Script": [["enroll-retry-scan", false], ["enroll-stage-passed", false], ["enroll-data-full", true]]}"#,
        ),
        NAME,
    );
    let f = Fprint::new(&bus.bus()).expect("connect");
    let r = f.reader().unwrap().unwrap();
    let mut seen = Vec::new();
    let saved = f
        .enroll(&r, "ada", "left-thumb", &AtomicBool::new(false), |p| {
            seen.push(p)
        })
        .expect("enroll");
    assert!(!saved);
    assert!(matches!(seen[0], Progress::Retry(_)), "{seen:?}");
    assert!(matches!(seen.last(), Some(Progress::Failed(_))), "{seen:?}");
    assert!(f.enrolled(&r, "ada").unwrap().is_empty());
}

#[test]
fn cancelling_stops_the_scan_and_lets_go_of_the_reader() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    // The reader never answers.
    bus.local_template("fprintd", Some(r#"{"Script": []}"#), NAME);
    let f = Fprint::new(&bus.bus()).expect("connect");
    let r = f.reader().unwrap().unwrap();
    let cancel = AtomicBool::new(true);
    let saved = f
        .enroll(&r, "ada", "left-thumb", &cancel, |_| {})
        .expect("enroll");
    assert!(!saved);
    let calls = ext::calls(&bus, NAME, "/net/reactivated/Fprint/Device/0");
    assert!(calls.iter().any(|c| c == "EnrollStop"), "{calls:?}");
    assert_eq!(
        calls.last().map(String::as_str),
        Some("Release"),
        "{calls:?}"
    );
}

#[test]
fn removes_all_fingerprints() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template("fprintd", Some(r#"{"Enrolled": ["left-thumb"]}"#), NAME);
    let f = Fprint::new(&bus.bus()).expect("connect");
    let r = f.reader().unwrap().unwrap();
    f.delete_all(&r, "ada").expect("delete");
    assert!(f.enrolled(&r, "ada").unwrap().is_empty());
}

#[test]
fn an_unknown_finger_never_reaches_the_bus() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template("fprintd", None, NAME);
    let f = Fprint::new(&bus.bus()).expect("connect");
    let r = f.reader().unwrap().unwrap();
    let e = f
        .enroll(&r, "ada", "left-toe", &AtomicBool::new(false), |_| {})
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Refused);
    assert!(ext::calls(&bus, NAME, "/net/reactivated/Fprint/Device/0").is_empty());
}

#[test]
fn no_bus_is_an_error_not_a_panic() {
    let e = Fprint::new(&Bus::Address("unix:path=/nonexistent/bus".into()))
        .err()
        .expect("must fail");
    assert_eq!(e.kind, ErrorKind::Bus, "{e:?}");
}
