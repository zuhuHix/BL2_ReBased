//! Owner: lane W3-INTERP. Commands: `--run`, `--run-batch`, `--native`.
//!
//! Not ported yet: a scaffold stub (W0-SCAFFOLD) that exits with code 3. The owner lane replaces `run`'s body,
//! parses its own trailing modifiers from `args` (the options listed in `args.rs` are only the usage record) and
//! prints what the C++ binary prints for the same command line.

use std::process::ExitCode;

use crate::args::Command;

pub fn run(command: &Command, args: &[String]) -> ExitCode {
    let _ = args;
    super::not_ported(command)
}
