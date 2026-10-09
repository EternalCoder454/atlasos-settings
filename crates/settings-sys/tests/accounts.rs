//! The AccountsService client against a mock of it
//! (`tests/templates/accounts_service.py`).

mod common;
mod ext;

use settings_sys::accounts::Accounts;
use settings_sys::{Bus, ErrorKind};

const NAME: &str = "org.freedesktop.Accounts";
const USERS: &str = r#"{"Users": [
  {"Uid": 1000, "UserName": "ada", "RealName": "Ada Lovelace", "AccountType": 1,
   "IconFile": "/var/lib/AccountsService/icons/ada.png"},
  {"Uid": 1001, "UserName": "bob", "RealName": "", "IconFile": "/etc/shadow"}
]}"#;

fn start() -> Option<common::MockBus> {
    let mut bus = common::MockBus::start()?;
    bus.local_template("accounts_service", Some(USERS), NAME);
    Some(bus)
}

#[test]
fn lists_users_with_safe_values() {
    let Some(bus) = start() else { return };
    let a = Accounts::new(&bus.bus()).expect("connect");
    let users = a.users().expect("users");
    assert_eq!(users.len(), 2);
    let ada = &users[0];
    assert_eq!((ada.uid, ada.name.as_str()), (1000, "ada"));
    assert_eq!(ada.real_name, "Ada Lovelace");
    assert!(ada.admin);
    assert_eq!(ada.icon, "/var/lib/AccountsService/icons/ada.png");
    let bob = &users[1];
    // No full name: the sign-in name; a picture that isn't one: none.
    assert_eq!(bob.real_name, "bob");
    assert_eq!(bob.icon, "");
    assert!(!bob.admin);

    assert_eq!(a.user(1000).expect("user").name, "ada");
    assert!(a.user(4242).is_err());
}

#[test]
fn changes_a_users_name_picture_and_type() {
    let Some(bus) = start() else { return };
    let a = Accounts::new(&bus.bus()).expect("connect");
    a.set_real_name(1000, "  Ada King  ").expect("name");
    // A picture that is one (a JPEG by its contents) is handed on by its real
    // path; one that isn't, or isn't there, never reaches the bus.
    let dir = std::env::temp_dir().join(format!("settings-sys-accounts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch folder");
    let jpeg = dir.join("me.jpg");
    std::fs::write(
        &jpeg,
        [
            0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F', 0, 1,
        ],
    )
    .unwrap();
    let not_a_picture = dir.join("notes.jpg");
    std::fs::write(&not_a_picture, "#!/bin/sh\necho hello\n").unwrap();
    for bad in [not_a_picture.to_str().unwrap(), "/home/ada/missing.jpg"] {
        assert_eq!(a.set_icon(1000, bad).unwrap_err().kind, ErrorKind::Refused);
    }
    assert_eq!(
        a.user(1000).unwrap().icon,
        "/var/lib/AccountsService/icons/ada.png"
    );
    a.set_icon(1000, jpeg.to_str().unwrap()).expect("icon");
    let want = std::fs::canonicalize(&jpeg).unwrap();
    a.set_admin(1001, true).expect("admin");
    a.set_auto_login(1000, true).expect("auto login");
    let ada = a.user(1000).expect("user");
    assert_eq!(ada.real_name, "Ada King");
    assert_eq!(ada.icon, want.to_str().unwrap());
    assert!(ada.auto_login);
    assert!(a.user(1001).unwrap().admin);

    // Junk never reaches the bus.
    for bad in ["", "a\nb", "a:b"] {
        assert_eq!(
            a.set_real_name(1000, bad).unwrap_err().kind,
            ErrorKind::Refused
        );
    }
    assert_eq!(
        a.set_icon(1000, "../../etc/passwd").unwrap_err().kind,
        ErrorKind::Refused
    );
    assert_eq!(a.user(1000).unwrap().real_name, "Ada King");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_password_goes_as_a_hash_never_as_text() {
    let Some(bus) = start() else { return };
    let a = Accounts::new(&bus.bus()).expect("connect");
    a.set_password(1000, "correct horse battery")
        .expect("password");
    let hash: String = ext::get_property(
        &bus,
        NAME,
        "/org/freedesktop/Accounts/User1000",
        "org.freedesktop.Accounts.User",
        "TestPasswordHash",
    )
    .try_into()
    .expect("a string");
    // A yescrypt hash (Fedora's libcrypt has it), else SHA-512; never text.
    assert!(hash.starts_with("$y$") || hash.starts_with("$6$"), "{hash}");
    assert!(!hash.contains("correct"));
    assert_eq!(
        a.set_password(1000, "").unwrap_err().kind,
        ErrorKind::Refused
    );
    // Short passwords are refused here: the service would store any hash.
    assert_eq!(
        a.set_password(1000, "short").unwrap_err().kind,
        ErrorKind::Refused
    );
    // What the mock saw of the calls: the arguments are logged by the
    // mock, so check the clear text isn't among them.
    let log = ext::call_log(&bus, NAME, "/org/freedesktop/Accounts/User1000");
    assert!(log.contains("SetPassword"), "{log}");
    assert!(!log.contains("correct horse"), "{log}");
}

#[test]
fn adds_and_removes_a_user() {
    let Some(bus) = start() else { return };
    let a = Accounts::new(&bus.bus()).expect("connect");
    let uid = a
        .create_user("eve", "Eve Online", false, "s3cret-pass")
        .expect("create");
    assert!(uid >= 1002);
    let eve = a.user(uid).expect("eve");
    assert_eq!(
        (eve.name.as_str(), eve.real_name.as_str()),
        ("eve", "Eve Online")
    );
    assert!(!eve.admin);
    let hash: String = ext::get_property(
        &bus,
        NAME,
        &format!("/org/freedesktop/Accounts/User{uid}"),
        "org.freedesktop.Accounts.User",
        "TestPasswordHash",
    )
    .try_into()
    .expect("a string");
    assert!(hash.starts_with("$y$") || hash.starts_with("$6$"), "{hash}");
    assert_eq!(a.users().unwrap().len(), 3);

    // The same name twice is the service's refusal.
    assert!(
        a.create_user("eve", "Eve Again", false, "pw-pw-pw")
            .is_err()
    );
    // A bad name or password is refused before anything is created.
    for (n, r, p) in [
        ("Eve", "E", "x"),
        ("e v", "E", "x"),
        ("zed", "", "x"),
        ("zed", "Z", ""),
    ] {
        assert_eq!(
            a.create_user(n, r, false, p).unwrap_err().kind,
            ErrorKind::Refused,
            "{n} {r} {p}"
        );
    }
    assert_eq!(a.users().unwrap().len(), 3);

    a.delete_user(uid, true).expect("delete");
    assert_eq!(a.users().unwrap().len(), 2);
    assert!(a.user(uid).is_err());
}

#[test]
fn a_missing_service_says_so() {
    let Some(bus) = common::MockBus::start() else {
        return;
    };
    let a = Accounts::new(&bus.bus()).expect("connect");
    assert_eq!(a.users().unwrap_err().kind, ErrorKind::NotRunning);
}

#[test]
fn no_bus_is_an_error_not_a_panic() {
    let e = Accounts::new(&Bus::Address("unix:path=/nonexistent/bus".into()))
        .err()
        .expect("must fail");
    assert_eq!(e.kind, ErrorKind::Bus, "{e:?}");
}
