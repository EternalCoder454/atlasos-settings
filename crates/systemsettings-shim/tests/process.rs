//! The built `systemsettings` as a process: what it prints, how it exits and
//! how long it takes to get to the hand-off. It never starts Settings or
//! kcmshell6: a run that would reach one only happens when that program is
//! absent from this machine (the exec then fails with 127).

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use systemsettings_shim::{KCMSHELL, SETTINGS};

fn shim() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_systemsettings"));
    c.stdin(Stdio::null());
    c
}

/// The module to ask for, and the program it hands over to, for each program
/// that is not installed here.
fn absent_targets() -> Vec<(&'static str, &'static str)> {
    [("kcm_kscreen", SETTINGS), ("kcm_trash", KCMSHELL)]
        .into_iter()
        .filter(|(_, program)| !Path::new(program).exists())
        .collect()
}

#[test]
fn help_and_version() {
    let out = shim().arg("--help").output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("Usage: systemsettings"));
    let out = shim().arg("--version").output().unwrap();
    assert!(out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .starts_with(&format!("systemsettings {}", env!("CARGO_PKG_VERSION")))
    );
}

#[test]
fn a_bad_command_line_fails_with_2_and_says_why() {
    for args in [&["../../bin/sh"][..], &["--evil"], &["kcm_a", "kcm_b"]] {
        let out = shim().args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty());
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.starts_with("systemsettings: "), "{err}");
        assert_eq!(err.lines().count(), 1, "{err}");
    }
}

#[test]
fn a_missing_program_is_reported_with_127() {
    let absent = absent_targets();
    if absent.is_empty() {
        eprintln!("skipped: {SETTINGS} and {KCMSHELL} are both installed here");
    }
    for (arg, program) in absent {
        let out = shim().arg(arg).output().unwrap();
        assert_eq!(out.status.code(), Some(127), "{arg}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains(&format!("cannot start {program}")), "{err}");
    }
}

fn median(mut t: Vec<Duration>) -> Duration {
    t.sort();
    t[t.len() / 2]
}

/// docs/DESIGN.md: "`systemsettings` shim, exec to hand-off: at most 5 ms".
/// Measured from the spawn to the exit of a run that ends at the hand-off
/// (the target is missing, so the exec fails at once), which also counts the
/// spawn and the process's own start; the median of many runs.
#[test]
fn the_handoff_is_within_5_ms() {
    let runs = 60;
    let mut times = Vec::new();
    // A run that stops before exec: parse, validate, print.
    for _ in 0..runs {
        let start = Instant::now();
        let status = shim()
            .arg("--version")
            .stdout(Stdio::null())
            .status()
            .unwrap();
        times.push(start.elapsed());
        assert!(status.success());
    }
    let version = median(times);
    eprintln!("--version: median {version:?} over {runs} runs");
    assert!(version < Duration::from_millis(5), "{version:?}");

    for (arg, _) in absent_targets() {
        let mut times = Vec::new();
        for _ in 0..runs {
            let start = Instant::now();
            let status = shim().arg(arg).stderr(Stdio::null()).status().unwrap();
            times.push(start.elapsed());
            assert_eq!(status.code(), Some(127));
        }
        let handoff = median(times);
        eprintln!("{arg} to the failed hand-off: median {handoff:?} over {runs} runs");
        assert!(handoff < Duration::from_millis(5), "{arg}: {handoff:?}");
    }
}
