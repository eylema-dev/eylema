//! Native task CLI and local service entrypoint.
#![forbid(unsafe_code)]

mod cli;

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(cli::entry(std::env::args_os().skip(1)))
}
