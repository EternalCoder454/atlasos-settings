//! The portal permission store client against a mock of it
//! (`tests/templates/permission_store.py`).

mod common;
mod ext;

use settings_sys::portal::{Answer, Permission, Portal};
use settings_sys::{Bus, ErrorKind};

const NAME: &str = "org.freedesktop.impl.portal.PermissionStore";
const PATH: &str = "/org/freedesktop/impl/portal/PermissionStore";

#[test]
fn reads_what_an_app_was_told() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template(
        "permission_store",
        Some(
            r#"{"Tables": {"background": {"background": {"org.example.Chat": ["yes"], "org.example.Mail": ["no"]}},
                           "devices": {"camera": {"org.example.Chat": ["ask"]}}}}"#,
        ),
        NAME,
    );
    let p = Portal::new(&bus.bus()).expect("connect");
    assert!(p.available());
    assert_eq!(
        p.get(Permission::Background, "org.example.Chat").unwrap(),
        Answer::Yes
    );
    assert_eq!(
        p.get(Permission::Background, "org.example.Mail").unwrap(),
        Answer::No
    );
    assert_eq!(
        p.get(Permission::Camera, "org.example.Chat").unwrap(),
        Answer::Ask
    );
    // Nothing stored (an app, a table) is "ask".
    assert_eq!(
        p.get(Permission::Background, "org.example.New").unwrap(),
        Answer::Ask
    );
    assert_eq!(
        p.get(Permission::Microphone, "org.example.Chat").unwrap(),
        Answer::Ask
    );
}

#[test]
fn changes_what_an_app_may_do() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template("permission_store", None, NAME);
    let p = Portal::new(&bus.bus()).expect("connect");
    p.set(Permission::Screen, "org.example.Chat", Answer::No)
        .expect("no");
    assert_eq!(
        p.get(Permission::Screen, "org.example.Chat").unwrap(),
        Answer::No
    );
    p.set(Permission::Screen, "org.example.Chat", Answer::Yes)
        .expect("yes");
    assert_eq!(
        p.get(Permission::Screen, "org.example.Chat").unwrap(),
        Answer::Yes
    );
    // The store's own table and ID for it.
    let log = ext::call_log(&bus, NAME, PATH);
    assert!(log.contains("screenshot"), "{log}");
    // "Ask" takes what is stored away; again is not an error.
    p.set(Permission::Screen, "org.example.Chat", Answer::Ask)
        .expect("ask");
    assert_eq!(
        p.get(Permission::Screen, "org.example.Chat").unwrap(),
        Answer::Ask
    );
    p.set(Permission::Screen, "org.example.Chat", Answer::Ask)
        .expect("ask again");
    p.set(Permission::Camera, "org.example.Chat", Answer::Yes)
        .expect("camera");
    p.set(Permission::Background, "org.example.Chat", Answer::No)
        .expect("background");
}

#[test]
fn junk_never_reaches_the_bus() {
    let Some(mut bus) = common::MockBus::start() else {
        return;
    };
    bus.local_template("permission_store", None, NAME);
    let p = Portal::new(&bus.bus()).expect("connect");
    for bad in ["", "chat", "../x.y", "a b.c"] {
        assert_eq!(
            p.get(Permission::Background, bad).unwrap_err().kind,
            ErrorKind::Refused
        );
        assert_eq!(
            p.set(Permission::Background, bad, Answer::Yes)
                .unwrap_err()
                .kind,
            ErrorKind::Refused
        );
    }
    assert!(ext::calls(&bus, NAME, PATH).is_empty());
}

#[test]
fn a_session_without_portals_is_not_available() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let p = Portal::new(&bus.bus()).expect("connect");
    assert!(!p.available());
    assert_eq!(
        p.get(Permission::Background, "org.example.Chat")
            .unwrap_err()
            .kind,
        ErrorKind::NotRunning
    );
}

#[test]
fn no_bus_is_an_error_not_a_panic() {
    let e = Portal::new(&Bus::Address("unix:path=/nonexistent/bus".into()))
        .err()
        .expect("must fail");
    assert_eq!(e.kind, ErrorKind::Bus, "{e:?}");
}
