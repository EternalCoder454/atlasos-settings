//! Property tests of everything another program can hand to Settings: launch
//! arguments (`launch::parse`), the Launcher's deep links
//! (`launch::action_args`) and KCM names (`kcm::normalize`). They throw
//! arbitrary and option-shaped text at the parsers and check what may come
//! out, not one example of it: only registry pages and settings, KCM names
//! that can't name a path or an option, search text without control
//! characters, and never a panic.
//!
//! `PROPTEST_CASES=20000 cargo test -p settings-registry --test fuzz_launch`
//! is the longer run CI does.

use proptest::prelude::*;
use settings_registry::kcm::{self, Target};
use settings_registry::launch::{self, MAX_ARG, MAX_KCM_ARGS, Request};
use settings_registry::pages;

/// Text a hostile caller might send: anything, option-shaped text, words the
/// parsers know, and long runs.
fn arg() -> impl Strategy<Value = String> {
    let known = proptest::sample::select(vec![
        "--kcm",
        "--kcm=kcm_fonts",
        "--args",
        "--args=--help",
        "--search",
        "--page",
        "--page=system",
        "--",
        "-",
        "-h",
        "kcm_fonts",
        "kcm_kscreen",
        "kcm_landingpage",
        "kcm_x.desktop",
        "plasma/kcms/systemsettings/kcm_x",
        "../kcm_x",
        "displays",
        "night-light",
        "sound",
        "network",
        "wifi",
        "apps",
        "permissions",
        "org.kde.systemsettings",
        "",
        " ",
    ])
    .prop_map(String::from);
    prop_oneof![
        3 => any::<String>(),
        3 => known,
        2 => "[ -~]{0,40}",
        1 => "\\PC{0,300}",
        1 => "[a-z/ -]{0,140}",
        // Longer than every cap.
        1 => (0usize..3).prop_map(|n| "x".repeat([MAX_ARG + 1, MAX_KCM_ARGS + 9, 5000][n])),
    ]
}

fn args() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(arg(), 0..24)
}

fn has_control(s: &str) -> bool {
    s.chars().any(char::is_control)
}

/// `kcm`, an optional `_`, then a letter or digit and up to 64 of
/// `[A-Za-z0-9_-]`.
fn is_kcm_name(name: &str) -> bool {
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

/// What every request out of the parser must satisfy.
fn check_request(r: &Request) -> Result<(), TestCaseError> {
    match r {
        Request::Home => {}
        Request::Page { page, item } => {
            let p = pages::page(page);
            prop_assert!(p.is_some(), "no page {page}");
            if let (Some(p), Some(i)) = (p, item) {
                prop_assert!(p.item(i).is_some(), "no setting {page}/{i}");
            }
        }
        Request::Kcm { name, args } => {
            prop_assert!(is_kcm_name(name), "not a KCM name: {name:?}");
            prop_assert_eq!(kcm::normalize(name), Some(name.clone()));
            // A KCM Settings has a page for never gets here.
            prop_assert_eq!(kcm::resolve(name), Target::Kcm);
            if let Some(a) = args {
                prop_assert!(a.len() <= MAX_KCM_ARGS);
                prop_assert!(!has_control(a));
            }
        }
        Request::Search(text) => {
            prop_assert!(!text.is_empty());
            prop_assert_eq!(text.trim(), text.as_str());
            prop_assert!(text.len() <= MAX_ARG);
            prop_assert!(!has_control(text));
        }
    }
    Ok(())
}

proptest! {
    // 256 cases, or $PROPTEST_CASES (CI runs far more). Failures are printed,
    // not saved next to the source.
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn parse_only_yields_what_the_registry_has(a in args()) {
        let (requests, refused) = launch::parse(&a);
        prop_assert!(!requests.is_empty(), "nothing usable asks for the first page");
        for r in &requests {
            check_request(r)?;
        }
        // What was refused is shown to the person: short and escaped, however
        // long or odd the argument was (at most 64 characters, each at worst
        // `\u{10ffff}`, and the quotes and an ellipsis).
        for r in &refused {
            prop_assert!(r.arg.chars().count() <= 64 * 10 + 4, "{} chars", r.arg.chars().count());
            prop_assert!(!has_control(&r.arg));
        }
    }

    #[test]
    fn parse_is_deterministic(a in args()) {
        prop_assert_eq!(launch::parse(&a), launch::parse(&a));
    }

    #[test]
    fn deep_links_only_open_pages(action in arg(), parameter in proptest::option::of(arg())) {
        let (launch_args, _) = launch::action_args(&action, parameter.as_deref());
        let (requests, _) = launch::parse(&launch_args);
        for r in &requests {
            // No KCM and no search, whatever the text: only a page, or the
            // first page.
            prop_assert!(
                matches!(r, Request::Home | Request::Page { .. }),
                "{action:?} {parameter:?} became {r:?}"
            );
            check_request(r)?;
        }
        // And what it hands on is a link or the fixed Apps page: never an
        // option.
        for a in &launch_args {
            prop_assert!(!a.starts_with('-'), "{a:?}");
        }
    }

    #[test]
    fn links_are_registry_shaped(link in arg()) {
        if launch::valid_link(&link) {
            prop_assert!(link.len() <= launch::MAX_LINK);
            for seg in link.split('/') {
                let b = seg.as_bytes();
                prop_assert!(!b.is_empty());
                prop_assert!(b[0].is_ascii_lowercase() || b[0].is_ascii_digit());
                prop_assert!(b.iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'));
            }
        }
        if launch::valid_desktop_id(&link) {
            prop_assert!(link.len() <= launch::MAX_DESKTOP_ID);
            prop_assert!(!link.starts_with(['-', '.']));
            prop_assert!(link.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+')));
        }
    }

    #[test]
    fn kcm_names_cannot_name_a_path_or_an_option(raw in arg()) {
        if let Some(name) = kcm::normalize(&raw) {
            if !kcm::HOME_NAMES.contains(&name.as_str()) {
                prop_assert!(is_kcm_name(&name), "{raw:?} became {name:?}");
            }
            prop_assert!(!name.starts_with('-'));
            prop_assert!(!name.contains('/'));
            if !kcm::HOME_NAMES.contains(&name.as_str()) {
                prop_assert!(!name.contains('.'));
            }
            // Normal form: normalizing again changes nothing.
            prop_assert_eq!(kcm::normalize(&name), Some(name.clone()));
            // A path was only a plugin path of Plasma's.
            if raw.contains('/') {
                prop_assert!(raw.starts_with("plasma/kcms/"), "{raw:?}");
                prop_assert!(!raw.split('/').any(|p| p == ".." || p == "." || p.is_empty()));
            }
        }
    }
}

#[test]
fn a_deep_link_cannot_carry_the_options_a_command_line_can() {
    // The text of an ActivateAction call is a link, not a command line: this
    // is what the old D-Bus name used to get wrong by splitting it.
    for text in [
        "--kcm kcm_fonts",
        "--kcm=kcm_fonts",
        "kcm_fonts --args x",
        "--search x",
        "displays --kcm kcm_fonts",
        "displays/--kcm",
        "displays night-light",
        "displays\nnight-light",
    ] {
        for action in ["open", "open-app"] {
            let (launch_args, _) = launch::action_args(action, Some(text));
            let (requests, _) = launch::parse(&launch_args);
            assert!(
                requests
                    .iter()
                    .all(|r| matches!(r, Request::Home | Request::Page { .. })),
                "{action} {text:?} -> {requests:?}"
            );
            assert!(
                launch_args.iter().all(|a| !a.starts_with('-')),
                "{action} {text:?} -> {launch_args:?}"
            );
        }
    }
    // The shape the Launcher sends still works.
    let (a, bad) = launch::action_args("open", Some("displays/night-light"));
    assert!(bad.is_empty());
    assert_eq!(
        launch::parse(&a).0,
        vec![Request::Page {
            page: "displays",
            item: Some("night-light")
        }]
    );
}
