//! Owner: lane W2-SCRIPT. Commands: `--script-check [--failures]`, `--disasm <index|Class.Function>`: the port of
//! their branches in `src/cli.cpp`, with the same argument counts, the same stdout bytes and the same
//! `ow-package: <message>` stderr line and exit code 1 on failure.
//!
//! As in the C++ binary, the package is loaded before the command's own arguments are checked. Lines end in a plain
//! `\n` on every platform (the hub's decision for the Rust binary: `tools/rust_parity.py` compares with CRLF
//! normalised to LF, so the MSVC build's text-mode `\r\n` is not mirrored).
//!
//! Neither command depends on the parity mode.

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use ow_core::ParityMode;
use ow_core::fmt::json_quote;
use ow_package::package::{Package, PackageError};

use crate::args::{Command, USAGE};

/// A failed command: the message printed after `ow-package: `.
struct Failure(String);

impl From<PackageError> for Failure {
    /// The error and each of its causes (`source()`), joined with `: `, as `cmd/tables.rs` prints them.
    fn from(e: PackageError) -> Self {
        let mut text = e.to_string();
        let mut cause = std::error::Error::source(&e);
        while let Some(c) = cause {
            text.push_str(": ");
            text.push_str(&c.to_string());
            cause = c.source();
        }
        Failure(text)
    }
}

impl From<ow_script::decode::DecodeError> for Failure {
    fn from(e: ow_script::decode::DecodeError) -> Self {
        Failure(e.to_string())
    }
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = mode;
    let mut out = Vec::new();
    let result = execute(command.flag, args, &mut out);
    // Ignore a failed write (a closed pipe), as the C++ binary does.
    let _ = std::io::stdout().lock().write_all(&out);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure(message)) => {
            eprintln!("ow-package: {message}");
            ExitCode::from(1)
        }
    }
}

/// `args` is the command line without the program name: `args[0]` is the package, `args[1]` the command flag. The
/// C++ `argc` is `args.len() + 1`.
fn execute(flag: &str, args: &[String], out: &mut Vec<u8>) -> Result<(), Failure> {
    let Some(source) = args.first() else {
        return Err(Failure(USAGE.to_string()));
    };
    let package = Package::load(Path::new(source))?;
    match flag {
        "--script-check" => script_check(&package, args, out),
        _ => disasm(&package, args, out),
    }
}

/// `--script-check [--failures]`: the counts of `check_package` as one JSON line, and with `--failures` the list
/// of failing functions with their reasons.
fn script_check(package: &Package, args: &[String], out: &mut Vec<u8>) -> Result<(), Failure> {
    let list_failures = args.len() == 3 && args[2] == "--failures";
    if args.len() != 2 && !list_failures {
        return Err(Failure(USAGE.to_string()));
    }
    let result = ow_script::check::check_package(package)?;
    let mut text = format!(
        "{{\"package\":{},\"functions\":{},\"native\":{},\"script\":{},\"decoded\":{},\"failed\":{}",
        json_quote(&package.name),
        result.functions,
        result.native,
        result.functions - result.native,
        result.decoded,
        result.failures.len()
    );
    if list_failures {
        text.push_str(",\"failures\":[");
        for (i, failure) in result.failures.iter().enumerate() {
            if i > 0 {
                text.push(',');
            }
            text.push_str(&json_quote(failure));
        }
        text.push(']');
    }
    text.push_str("}\n");
    out.extend_from_slice(text.as_bytes());
    Ok(())
}

/// `--disasm <index|Class.Function>`: a target made only of digits (the empty text too, as in the C++) is an export
/// index read with `signedNumber`, anything else an object path looked up with `findExport`.
fn disasm(package: &Package, args: &[String], out: &mut Vec<u8>) -> Result<(), Failure> {
    if args.len() != 3 {
        return Err(Failure(USAGE.to_string()));
    }
    let target = &args[2];
    let index = if target.bytes().all(|b| b.is_ascii_digit()) {
        super::tables::signed_number(target).map_err(Failure)?
    } else {
        package.find_export(target)?.unwrap_or(0)
    };
    if index == 0 {
        return Err(Failure(format!("function not found: {target}")));
    }
    let info = ow_script::function::read_function(package, index)?;
    let code = ow_script::decode::decode(package, &info)?;
    out.extend_from_slice(&ow_script::disasm::disassemble(package, &info, &code)?);
    Ok(())
}
