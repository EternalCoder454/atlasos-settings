//! `/usr/bin/systemsettings` (docs/DESIGN.md, "Entry points"). The work is in
//! the library; this reads the command line, replaces the process, and says
//! why when it can't.

use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};
use systemsettings_shim::{Plan, plan};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match plan(&args) {
        Plan::Print(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Plan::Refuse(why) => {
            eprintln!("systemsettings: {why}");
            ExitCode::from(2)
        }
        Plan::Exec { program, args } => {
            // exec only returns on failure. The program is an absolute path,
            // so there is no PATH lookup; the environment (the activation
            // token, the display) goes along unchanged.
            let name = program.rsplit('/').next().unwrap_or(program);
            let err = Command::new(program).arg0(name).args(&args).exec();
            eprintln!("systemsettings: cannot start {program}: {err}");
            // 127 as a shell says it for a program that isn't there.
            ExitCode::from(if err.kind() == std::io::ErrorKind::NotFound {
                127
            } else {
                126
            })
        }
    }
}
