//! What `/usr/bin/systemsettings` does on Telamon OS.
//!
//! KDE's System Settings is replaced by Settings, but Plasma still starts
//! `systemsettings <kcm>` (KCMLauncher, the tray applets), and so do old
//! scripts. This answers the same command line: it validates the arguments
//! with the registry's code (the same as `telamon-settings --kcm`), then
//! replaces itself with
//!
//! - `/usr/bin/telamon-settings --kcm <name>` for a KCM Settings has a page
//!   for, or with no arguments for System Settings' own start page;
//! - `/usr/bin/kcmshell6 <name> [--args=<text>]` for any other.
//!
//! Both programs are named by absolute path: no shell, no `PATH` lookup.
//! Nothing but the plan is made here, so tests can check it without starting
//! anything ([`plan`]).

use settings_registry::kcm;
use settings_registry::launch::{self, MAX_ARG, MAX_ARGS, MAX_KCM_ARGS, Request};

/// Settings.
pub const SETTINGS: &str = "/usr/bin/telamon-settings";
/// Plasma's KCM launcher, for the KCMs Settings has no page for.
pub const KCMSHELL: &str = "/usr/bin/kcmshell6";

/// What to do with a command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Replace this process with `program` and `args` (`args` without the
    /// program name).
    Exec {
        program: &'static str,
        args: Vec<String>,
    },
    /// Print this to the standard output and succeed (`--help`, `--version`).
    Print(String),
    /// Print this to the standard error and fail with status 2: the command
    /// line is not one this program answers.
    Refuse(String),
}

impl Plan {
    fn refuse(why: impl Into<String>) -> Plan {
        Plan::Refuse(why.into())
    }
}

/// The `--help` text.
pub fn help() -> String {
    "Usage: systemsettings [options] [module]\n\
     \n\
     Opens Telamon Settings, or what a Plasma settings module (KCM) opens.\n\
     \n\
     Options:\n\
     \x20 -h, --help         Displays this help.\n\
     \x20 -v, --version      Displays version information.\n\
     \x20 --args <text>      Arguments for the module.\n\
     \n\
     Arguments:\n\
     \x20 module             The module to open, such as kcm_kscreen.\n"
        .to_string()
}

/// The `--version` text.
pub fn version() -> String {
    format!(
        "systemsettings {} (Telamon Settings)\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// `arg` shortened and escaped for a message.
fn shown(arg: &str) -> String {
    let short: String = arg.chars().take(64).collect();
    let mut s = format!("{short:?}");
    if short.len() < arg.len() {
        s.push('…');
    }
    s
}

/// What `args` (the command line without the program name) ask for.
///
/// The accepted forms are those of KDE's `systemsettings`: an optional
/// module (`kcm_x`, `kcm_x.desktop`, a plugin path, or `systemsettings`
/// itself), `--args <text>` or `--args=<text>` for it, `-h`/`--help` and
/// `-v`/`--version`. Anything else is refused, never guessed at.
pub fn plan<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> Plan {
    if args.len() > MAX_ARGS {
        return Plan::refuse(format!("too many arguments ({})", args.len()));
    }
    let mut text = Vec::with_capacity(args.len());
    for a in args {
        match a.as_ref().to_str() {
            Some(s) => text.push(s),
            None => return Plan::refuse("an argument is not text"),
        }
    }

    let mut module: Option<&str> = None;
    let mut module_args: Option<&str> = None;
    let mut only_positional = false;
    let mut it = text.iter().copied();
    while let Some(arg) = it.next() {
        if let Err(why) = launch::clean(arg, MAX_ARG.max(MAX_KCM_ARGS + 7)) {
            return Plan::refuse(format!("{}: {why}", shown(arg)));
        }
        if !only_positional && arg.starts_with('-') && arg != "-" {
            match arg {
                "-h" | "--help" | "--help-all" => return Plan::Print(help()),
                "-v" | "--version" => return Plan::Print(version()),
                "--" => only_positional = true,
                "--args" => match it.next() {
                    Some(t) => module_args = Some(t),
                    None => return Plan::refuse("--args needs text"),
                },
                _ => match arg.strip_prefix("--args=") {
                    Some(t) => module_args = Some(t),
                    None => return Plan::refuse(format!("unknown option {}", shown(arg))),
                },
            }
            continue;
        }
        // An empty argument (a launcher's unused %u) asks for nothing.
        if arg.trim().is_empty() {
            continue;
        }
        if module.replace(arg).is_some() {
            return Plan::refuse("one module at a time");
        }
    }
    if let Some(t) = module_args {
        if let Err(why) = launch::clean(t, MAX_KCM_ARGS) {
            return Plan::refuse(format!("--args {}: {why}", shown(t)));
        }
        if module.is_none() {
            return Plan::refuse("--args needs a module");
        }
    }
    let Some(raw) = module else {
        // `systemsettings` alone: System Settings' start page.
        return Plan::Exec {
            program: SETTINGS,
            args: Vec::new(),
        };
    };

    // The registry's own parser decides what is a KCM name and where it
    // leads, as it does for `telamon-settings --kcm`.
    let mut launch_args = vec!["--kcm".to_string(), raw.to_string()];
    if let Some(t) = module_args {
        launch_args.push("--args".to_string());
        launch_args.push(t.to_string());
    }
    let (requests, refused) = launch::parse(&launch_args);
    if let Some(r) = refused.first() {
        return Plan::refuse(format!("{}: {}", r.arg, r.reason));
    }
    match requests.as_slice() {
        [Request::Home] => Plan::Exec {
            program: SETTINGS,
            args: Vec::new(),
        },
        [Request::Page { .. }] => {
            // The name is known to be fine: parse() took it. Settings reads
            // it again and lands on the same page; --args has no use there.
            let name = kcm::normalize(raw).unwrap_or_default();
            Plan::Exec {
                program: SETTINGS,
                args: vec!["--kcm".to_string(), name],
            }
        }
        [Request::Kcm { name, args }] => {
            let mut argv = vec![name.clone()];
            if let Some(t) = args {
                // One word, so text that starts with `-` can't be read as an
                // option of kcmshell6.
                argv.push(format!("--args={t}"));
            }
            Plan::Exec {
                program: KCMSHELL,
                args: argv,
            }
        }
        _ => Plan::refuse("not a KCM name"),
    }
}
