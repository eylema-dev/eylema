//! Native workspace bootstrap only; no task service or lifecycle CLI yet.
#![forbid(unsafe_code)]

use std::ffi::OsStr;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let first = arguments.next();
    if arguments.next().is_some() {
        eprintln!("unsupported bootstrap arguments; use --help or --version");
        return ExitCode::from(2);
    }
    match first.as_deref() {
        None => help(),
        Some(value) if value == OsStr::new("--help") => help(),
        Some(value) if value == OsStr::new("--version") => {
            println!(
                "eylema {} (workspace bootstrap)",
                eylema_contracts::PACKAGE_VERSION
            );
            ExitCode::SUCCESS
        }
        Some(_) => {
            eprintln!("unsupported bootstrap argument; use --help or --version");
            ExitCode::from(2)
        }
    }
}

fn help() -> ExitCode {
    println!(
        "Eylema workspace bootstrap\nUsage: eylema [--help|--version]\nTask and service operations are not implemented."
    );
    ExitCode::SUCCESS
}
