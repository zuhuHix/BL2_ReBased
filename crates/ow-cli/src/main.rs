//! Owner: lane W0-SCAFFOLD. The Rust `ow-package` binary: the port of `src/cli.cpp`, with the same flags and, once
//! each family is ported, byte-identical output and exit codes (architecture rule R7). Until a family is ported its
//! commands print a "not ported to Rust yet" message and exit with code 3; the C++ `ow-package` in `build/Release`
//! is the one to use for them.
//!
//! `args.rs` holds the dispatch table and `cmd/` one file per command family. Lanes edit only their own `cmd/` file.

mod args;
mod cmd;
mod json;

use std::process::ExitCode;

fn main() -> ExitCode {
    // The parity mode is read once, here, at start-up (architecture rule R3). A bad OW_PARITY value is an error like
    // any other bad command line: message on stderr, exit 1. Command families get the mode as an argument of their
    // run(); they never read the environment.
    let mode = match ow_core::parity::init_parity_mode() {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("ow-package: {e}");
            return ExitCode::from(1);
        }
    };
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    match args::dispatch(&args) {
        args::Dispatch::Usage => {
            eprintln!("ow-package: {}", args::USAGE);
            ExitCode::from(1)
        }
        args::Dispatch::Run(command) => cmd::run(command, &args, mode),
    }
}
