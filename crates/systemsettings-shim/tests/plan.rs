//! The command lines `systemsettings` is started with, and what each becomes.
//! Nothing here starts a program: `plan` only decides.

use settings_registry::kcm::{self, Target};
use settings_registry::launch::{MAX_ARG, MAX_ARGS, MAX_KCM_ARGS};
use systemsettings_shim::{KCMSHELL, Plan, SETTINGS, plan};

fn p(args: &[&str]) -> Plan {
    plan(args)
}

fn settings(args: &[&str]) -> Plan {
    Plan::Exec {
        program: SETTINGS,
        args: args.iter().map(|s| s.to_string()).collect(),
    }
}

fn kcmshell(args: &[&str]) -> Plan {
    Plan::Exec {
        program: KCMSHELL,
        args: args.iter().map(|s| s.to_string()).collect(),
    }
}

fn refused(plan: Plan) -> String {
    match plan {
        Plan::Refuse(why) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn the_two_programs_are_fixed_absolute_paths() {
    assert_eq!(SETTINGS, "/usr/bin/telamon-settings");
    assert_eq!(KCMSHELL, "/usr/bin/kcmshell6");
}

#[test]
fn no_module_opens_settings() {
    assert_eq!(p(&[]), settings(&[]));
    // An unused launcher placeholder asks for nothing.
    assert_eq!(p(&[""]), settings(&[]));
    assert_eq!(p(&["", "  "]), settings(&[]));
    // System Settings' own start page, by its three names.
    for home in [
        "kcm_landingpage",
        "systemsettings",
        "org.kde.systemsettings",
    ] {
        assert_eq!(p(&[home]), settings(&[]), "{home}");
    }
}

#[test]
fn a_kcm_settings_has_a_page_for_goes_to_settings() {
    assert_eq!(p(&["kcm_kscreen"]), settings(&["--kcm", "kcm_kscreen"]));
    assert_eq!(
        p(&["kcm_nightlight"]),
        settings(&["--kcm", "kcm_nightlight"])
    );
    // The other spellings of the same name.
    assert_eq!(
        p(&["kcm_kscreen.desktop"]),
        settings(&["--kcm", "kcm_kscreen"])
    );
    assert_eq!(
        p(&["plasma/kcms/systemsettings/kcm_kscreen"]),
        settings(&["--kcm", "kcm_kscreen"])
    );
    // --args means nothing to a page; it is not passed on.
    assert_eq!(
        p(&["kcm_networkmanagement", "--args", "x"]),
        settings(&["--kcm", "kcm_networkmanagement"])
    );
}

#[test]
fn any_other_kcm_goes_to_kcmshell6() {
    assert_eq!(p(&["kcm_trash"]), kcmshell(&["kcm_trash"]));
    assert_eq!(p(&["kcm_keys"]), kcmshell(&["kcm_keys"]));
    assert_eq!(p(&["kcm_trash.desktop"]), kcmshell(&["kcm_trash"]));
    // A row's KCM, asked for directly, opens the KCM itself.
    assert_eq!(
        p(&["kcm_printer_manager"]),
        kcmshell(&["kcm_printer_manager"])
    );
    // Names Plasma has that the registry does not know are still KCMs.
    assert_eq!(p(&["kcm_recentFiles"]), kcmshell(&["kcm_recentFiles"]));
    assert_eq!(p(&["kcmspellchecking"]), kcmshell(&["kcmspellchecking"]));
}

#[test]
fn arguments_for_a_kcm_are_one_word() {
    assert_eq!(
        p(&["kcm_fonts", "--args", "x=1 y"]),
        kcmshell(&["kcm_fonts", "--args=x=1 y"])
    );
    assert_eq!(
        p(&["kcm_fonts", "--args=x=1"]),
        kcmshell(&["kcm_fonts", "--args=x=1"])
    );
    // Text that looks like an option stays inside the one word.
    assert_eq!(
        p(&["kcm_fonts", "--args", "--rm-rf"]),
        kcmshell(&["kcm_fonts", "--args=--rm-rf"])
    );
    // The module may come after its arguments.
    assert_eq!(
        p(&["--args", "a", "kcm_fonts"]),
        kcmshell(&["kcm_fonts", "--args=a"])
    );
    // After `--`, everything is a module.
    assert!(refused(p(&["--", "--args"])).contains("not a KCM name"));
}

#[test]
fn every_kcm_in_the_map_lands_where_the_app_would() {
    for &(name, _, _) in kcm::MAP {
        match kcm::resolve(name) {
            Target::Kcm => panic!("{name} is mapped but resolves to a KCM"),
            _ => assert_eq!(p(&[name]), settings(&["--kcm", name]), "{name}"),
        }
    }
}

#[test]
fn help_and_version_start_nothing() {
    for flag in ["-h", "--help", "--help-all"] {
        match p(&[flag]) {
            Plan::Print(t) => assert!(t.starts_with("Usage: systemsettings"), "{flag}"),
            other => panic!("{flag}: {other:?}"),
        }
    }
    for flag in ["-v", "--version"] {
        match p(&[flag]) {
            Plan::Print(t) => assert!(t.starts_with("systemsettings 0."), "{flag}: {t}"),
            other => panic!("{flag}: {other:?}"),
        }
    }
    // Whichever comes first wins; neither starts anything.
    assert!(matches!(p(&["kcm_trash", "--help"]), Plan::Print(_)));
}

#[test]
fn hostile_command_lines_are_refused() {
    for bad in [
        "../../bin/sh",
        "/usr/bin/sh",
        "kcm_a b",
        "kcm_a;reboot",
        "kcm_$(id)",
        "kcm_`id`",
        "kcm_x|y",
        "kcm_x&",
        "kcm_x\n",
        "kcm_x.so",
        "kcm_é",
        "kscreen",
        "kcm",
        "kcm_",
        "--kcm",
        "kcm_x/../../y",
    ] {
        let why = refused(p(&[bad]));
        assert!(!why.is_empty(), "{bad:?}");
    }
    // Options that are not ours, however they are spelled.
    for opt in [
        "--evil",
        "-platform",
        "--icon-mode",
        "-",
        "--args-",
        "--argsx=1",
        "--ARGS=1",
    ] {
        match p(&[opt]) {
            Plan::Refuse(_) => {}
            // `-` is a lone dash: a module name, and not a KCM's.
            other => panic!("{opt:?}: {other:?}"),
        }
    }
    assert!(refused(p(&["--args"])).contains("needs text"));
    assert!(refused(p(&["--args", "x"])).contains("needs a module"));
    assert!(refused(p(&["--args=x"])).contains("needs a module"));
    assert!(refused(p(&["kcm_trash", "kcm_keys"])).contains("one module"));
    assert!(refused(p(&["kcm_trash", "--args", "a\u{1b}[2J"])).contains("control"));
}

#[test]
fn limits_hold() {
    let long_name = format!("kcm_{}", "a".repeat(MAX_ARG));
    assert!(refused(p(&[&long_name])).contains("too long"));
    let long_args = "x".repeat(MAX_KCM_ARGS + 1);
    assert!(matches!(
        p(&["kcm_trash", "--args", &long_args]),
        Plan::Refuse(_)
    ));
    let ok_args = "x".repeat(MAX_KCM_ARGS);
    assert!(matches!(
        p(&["kcm_trash", "--args", &ok_args]),
        Plan::Exec { .. }
    ));
    let many: Vec<&str> = std::iter::repeat_n("kcm_trash", MAX_ARGS + 1).collect();
    assert!(refused(plan(&many)).contains("too many"));
    // A flood is reported, not copied whole.
    let huge = "z".repeat(1_000_000);
    let why = refused(p(&[&huge]));
    assert!(why.len() < 200, "{} bytes", why.len());
}

#[test]
fn non_text_arguments_are_refused() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let bad = OsString::from_vec(b"kcm_\xff".to_vec());
    assert!(refused(plan(&[bad])).contains("not text"));
}

/// A very small plan: the budget is for the whole process (see
/// tests/process.rs); the decision itself must be negligible.
#[test]
fn planning_is_quick() {
    let args = ["kcm_kscreen", "--args", "x"];
    let start = std::time::Instant::now();
    for _ in 0..2_000 {
        std::hint::black_box(plan(std::hint::black_box(&args)));
    }
    let each = start.elapsed() / 2_000;
    assert!(
        each < std::time::Duration::from_micros(500),
        "{each:?} per plan"
    );
}
