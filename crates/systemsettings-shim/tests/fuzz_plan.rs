//! Property tests of the `systemsettings` command line: whatever argv another
//! program starts it with (any bytes, option-shaped text, absurd lengths),
//! the plan is one of three things, and an exec is only ever one of the two
//! programs, by absolute path, with a KCM name the registry accepts and
//! nothing that kcmshell6 or Settings could read as another option.
//!
//! `PROPTEST_CASES=20000 cargo test -p systemsettings-shim --test fuzz_plan`
//! is the longer run CI does.

use proptest::prelude::*;
use settings_registry::kcm::{self, Target};
use settings_registry::launch::{MAX_ARG, MAX_ARGS, MAX_KCM_ARGS};
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use systemsettings_shim::{KCMSHELL, Plan, SETTINGS, help, plan, version};

fn piece() -> impl Strategy<Value = Vec<u8>> {
    let known = proptest::sample::select(vec![
        "--args",
        "--args=",
        "--args=-h",
        "--args=--help",
        "--help",
        "-v",
        "--",
        "-",
        "-x",
        "--version",
        "kcm_fonts",
        "kcm_kscreen",
        "kcm_landingpage",
        "systemsettings",
        "kcm_x.desktop",
        "plasma/kcms/systemsettings/kcm_x",
        "../../bin/sh",
        "/usr/bin/sh",
        "kcm_a;rm",
        "kcm_a b",
        "",
        " ",
    ])
    .prop_map(|s| s.as_bytes().to_vec());
    prop_oneof![
        // Bytes that are not text, too.
        2 => prop::collection::vec(any::<u8>(), 0..40),
        3 => any::<String>().prop_map(String::into_bytes),
        3 => known,
        2 => "[ -~]{0,40}".prop_map(String::into_bytes),
        1 => (0usize..3).prop_map(|n| vec![b'k'; [MAX_ARG + 1, MAX_KCM_ARGS + 9, 5000][n]]),
    ]
}

fn argv() -> impl Strategy<Value = Vec<OsString>> {
    prop::collection::vec(
        piece().prop_map(|b| OsStr::from_bytes(&b).to_os_string()),
        0..(MAX_ARGS + 6),
    )
}

fn plain_kcm_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("kcm") else {
        return false;
    };
    let body = rest.strip_prefix('_').unwrap_or(rest);
    (1..=kcm::MAX_NAME).contains(&rest.len())
        && body.starts_with(|c: char| c.is_ascii_alphanumeric())
        && rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

proptest! {
    // 256 cases, or $PROPTEST_CASES (CI runs far more). Failures are printed,
    // not saved next to the source.
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn a_plan_is_one_of_three_safe_things(a in argv()) {
        match plan(&a) {
            Plan::Print(text) => prop_assert!(text == help() || text == version()),
            Plan::Refuse(why) => {
                // One line for the standard error, whatever was sent.
                prop_assert!(!why.is_empty());
                prop_assert!(why.len() < 2000, "{} bytes", why.len());
                prop_assert!(!why.chars().any(char::is_control), "{why:?}");
            }
            Plan::Exec { program, args } if program == SETTINGS => {
                // Settings gets nothing but a KCM name it has a page for, or
                // no arguments.
                match args.as_slice() {
                    [] => {}
                    [flag, name] => {
                        prop_assert_eq!(flag.as_str(), "--kcm");
                        prop_assert!(plain_kcm_name(name), "{name:?}");
                        prop_assert!(matches!(kcm::resolve(name), Target::Page { .. }), "{name:?}");
                    }
                    other => prop_assert!(false, "unexpected arguments {:?}", other),
                }
            }
            Plan::Exec { program, args } if program == KCMSHELL => {
                prop_assert!(args.len() == 1 || args.len() == 2, "{args:?}");
                // The module is a plain name that is not a page of Settings,
                // and cannot start with `-`.
                prop_assert!(plain_kcm_name(&args[0]), "{:?}", args[0]);
                prop_assert_eq!(kcm::resolve(&args[0]), Target::Kcm);
                // The module's arguments are one `--args=` word: text that
                // starts with `-` stays text.
                if let Some(second) = args.get(1) {
                    let text = second.strip_prefix("--args=");
                    prop_assert!(text.is_some(), "{second:?}");
                    let text = text.unwrap();
                    prop_assert!(text.len() <= MAX_KCM_ARGS);
                    prop_assert!(!text.chars().any(char::is_control));
                }
            }
            Plan::Exec { program, .. } => prop_assert!(false, "another program: {program}"),
        }
    }

    #[test]
    fn plan_never_depends_on_anything_but_its_arguments(a in argv()) {
        prop_assert_eq!(plan(&a), plan(&a));
    }
}

#[test]
fn programs_are_absolute_and_never_looked_up_in_path() {
    for p in [SETTINGS, KCMSHELL] {
        assert!(p.starts_with("/usr/bin/"), "{p}");
        assert!(!p[1..].contains("//") && !p.contains(".."));
    }
}
