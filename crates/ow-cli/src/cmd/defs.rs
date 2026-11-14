//! Owner: lane W3-DEFS. Commands: `--def-dump` with `--overlay` (Rust-only).
//!
//! Not ported yet: a scaffold stub (W0-SCAFFOLD) that exits with code 3. The owner lane replaces `run`'s body,
//! parses its own trailing modifiers from `args` (the options listed in `args.rs` are only the usage record) and
//! prints what the C++ binary prints for the same command line. `mode` is the run's parity mode, read once by
//! `main`; pass it down to the library calls that take one (never read the environment here).

use std::process::ExitCode;

use ow_core::ParityMode;

use crate::args::Command;

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = (args, mode);
    super::not_ported(command)
}
