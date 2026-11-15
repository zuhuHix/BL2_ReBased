//! Owner: lane W2-STORE. Commands: `--resolve <reference> --cooked <directory>`: the port of its branch in
//! `src/cli.cpp` (`resolvedJson`), with the same argument checks in the same order, the same stdout JSON and the same
//! `ow-package: <message>` stderr line and exit code 1 on failure.
//!
//! Order, as the C++: the argument count, the reference, the options (`--cooked <dir>` only, the last one wins), then
//! the source package is loaded, then the store scans the cooked root, then the reference is resolved.
//!
//! The C++ check oracle is switched on by the environment variable `OPENWILLOW_RESOLVE_CHECK` (set at all), with its
//! test seam `OPENWILLOW_RESOLVE_CHECK_FAIL_BUILD` read only then. The binary reads them here and hands the library
//! a `ResolveCheck` value; the `OWRESOLVECHECK` lines the store collects go to stderr before any error message, the
//! order the C++ writes them in.
//!
//! Lines end in `\n` on every platform. The C++ binary's text-mode streams write `\r\n` on Windows. That difference
//! is not mirrored here: the parity harness reads each CRLF as LF when it compares (decision 2026-11-15-09).
//!
//! The command does not depend on the parity mode.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ow_core::ParityMode;
use ow_package::package::Package;
use ow_package::store::{PackageStore, ResolveCheck};

use super::tables::signed_number;
use crate::args::{Command, USAGE};
use crate::json::quote;

/// The C++ check oracle's switch and its test seam.
const CHECK_VAR: &str = "OPENWILLOW_RESOLVE_CHECK";
const CHECK_FAIL_BUILD_VAR: &str = "OPENWILLOW_RESOLVE_CHECK_FAIL_BUILD";

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = (command, mode);
    let mut log = Vec::new();
    let result = execute(args, &mut log);
    let mut err = std::io::stderr().lock();
    for line in &log {
        let _ = writeln!(err, "{line}");
    }
    match result {
        Ok(out) => {
            // Ignore a failed write (a closed pipe), as the C++ binary does.
            let _ = std::io::stdout().lock().write_all(out.as_bytes());
            ExitCode::SUCCESS
        }
        Err(message) => {
            let _ = writeln!(err, "ow-package: {message}");
            ExitCode::from(1)
        }
    }
}

/// The error and each of its causes (`source()`), joined with `: `, as the other command families print them.
fn message(e: &dyn std::error::Error) -> String {
    let mut text = e.to_string();
    let mut cause = e.source();
    while let Some(c) = cause {
        text.push_str(": ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    text
}

/// `args` is the command line without the program name: `args[0]` is the source package, `args[1]` `--resolve`. The
/// C++ `argc` is `args.len() + 1`. Returns stdout's text; the check lines go to `log` whether or not it fails.
fn execute(args: &[String], log: &mut Vec<String>) -> Result<String, String> {
    if args.len() + 1 < 6 {
        return Err(USAGE.to_string());
    }
    let reference = signed_number(&args[2])?;
    let mut cooked = String::new();
    let mut i = 3;
    while i < args.len() {
        if args[i] != "--cooked" {
            return Err(USAGE.to_string());
        }
        i += 1;
        cooked = args.get(i).cloned().ok_or("missing value for --cooked")?;
        i += 1;
    }
    if cooked.is_empty() {
        return Err(USAGE.to_string());
    }
    let source = Package::load(Path::new(&args[0])).map_err(|e| message(&e))?;
    let mut store = PackageStore::open(PathBuf::from(cooked)).map_err(|e| message(&e))?;
    if std::env::var_os(CHECK_VAR).is_some() {
        let fail_in_build =
            std::env::var_os(CHECK_FAIL_BUILD_VAR).map(|v| v.to_string_lossy().into_owned());
        store.set_resolve_check(Some(ResolveCheck { fail_in_build }));
    }
    let resolved = store
        .adopt(source)
        .and_then(|id| Ok((id, store.resolve(id, reference)?)));
    log.extend(store.take_check_log());
    let (source_id, result) = resolved.map_err(|e| message(&e))?;
    resolved_json(&store, source_id, reference, result).map_err(|e| message(&e))
}

/// The C++ `resolvedJson`, with its line end.
fn resolved_json(
    store: &PackageStore,
    source_id: ow_package::PackageId,
    reference: i32,
    result: Option<(ow_package::PackageId, i32)>,
) -> Result<String, ow_package::package::PackageError> {
    let source = store.get(source_id);
    let reference_path = if reference != 0 {
        quote(&source.path_of(reference)?)
    } else {
        "null".to_string()
    };
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"source_package\":{},\"reference\":{reference},\"reference_path\":{reference_path},\"indexed_packages\":{}",
        quote(&source.name),
        store.indexed_package_count()
    );
    match result {
        Some((id, index)) => {
            let target = store.get(id);
            // C++ `ResolvedObject::path`: the export's path (a resolved object always has a non-zero index).
            let _ = write!(
                out,
                ",\"resolved_package\":{},\"resolved_index\":{index},\"resolved_path\":{}",
                quote(&target.name),
                quote(&target.path_of(index)?)
            );
        }
        None => {
            out.push_str(",\"resolved_package\":null,\"resolved_index\":0,\"resolved_path\":null")
        }
    }
    out.push_str("}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// The argument checks come before any file is read, in the C++ order.
    #[test]
    fn bad_command_lines_fail_before_loading() {
        let mut log = Vec::new();
        let usage = Err(USAGE.to_string());
        assert_eq!(
            execute(&args(&["x", "--resolve", "-1", "--cooked"]), &mut log),
            usage
        );
        assert_eq!(
            execute(&args(&["x", "--resolve", "-1", "--other", "d"]), &mut log),
            usage
        );
        assert_eq!(
            execute(&args(&["x", "--resolve", "1x", "--cooked", "d"]), &mut log),
            Err("numeric argument out of range".into())
        );
        assert_eq!(
            execute(
                &args(&["x", "--resolve", "-1", "--cooked", "d", "--cooked"]),
                &mut log
            ),
            Err("missing value for --cooked".into())
        );
        assert_eq!(
            execute(&args(&["x", "--resolve", "-1", "--cooked", ""]), &mut log),
            usage
        );
        assert_eq!(
            execute(
                &args(&["no-such-file.upk", "--resolve", "-1", "--cooked", "d"]),
                &mut log
            ),
            Err("cannot open input package".into())
        );
        assert!(log.is_empty());
    }
}
