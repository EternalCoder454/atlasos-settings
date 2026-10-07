//! Writes the Launcher's search index (settings_registry::index) to the file
//! named by the one argument, or to the standard output with `-`. The
//! package build runs it; it refuses to write an index the Launcher would
//! skip entries of.
//!
//!   gen-search-index <file | ->

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(target), None) = (args.next(), args.next()) else {
        eprintln!("usage: gen-search-index <file | ->");
        return ExitCode::from(2);
    };
    if !settings_registry::index::links_valid() {
        eprintln!("gen-search-index: a page or setting ID is not a valid link");
        return ExitCode::FAILURE;
    }
    let json = settings_registry::index::to_json();
    let result = if target == "-" {
        std::io::stdout().write_all(json.as_bytes())
    } else {
        std::fs::write(&target, json.as_bytes())
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gen-search-index: {target}: {e}");
            ExitCode::FAILURE
        }
    }
}
