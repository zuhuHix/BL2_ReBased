//! Owner: lane W4-REGISTRY. Commands: `--item-registry --cooked <install folder> [--census <file>] [--overlay
//! <file>]` (Rust-only).
//!
//! Prints the `openwillow-item-registry/1` JSON of `ow_items::registry` to stdout: the bytes
//! `tools/item_registry.py` writes to `registry.json` (no final newline), so `ow-package --item-registry ... >
//! local/.../registry.json` gives a file to compare with the tool's. `--census <file>` also writes the census report
//! (the tool's `census.json` without its timestamp, run time and cross-checks). Both are game data: write them under
//! ignored `local/` only (docs/LEGAL.md).
//!
//! `--cooked` names the install folder (the one holding `WillowGame/CookedPCConsole` and `DLC/`), as the tool's
//! `--game` does, for every command of the items family. The registry's `game` field is the store's root (that folder,
//! absolute) as Python's `str(Path(..))` prints it, so the tool run with the same absolute `--game` writes the same
//! text.
//!
//! `--overlay` is refused for now: the registry reads tag streams, which overlays do not patch (see
//! `ow_items::registry`, "Where values come from"). Exit codes: 0 success, 1 a bad command line or a failed build, 3
//! a layer not there yet (`NotYet`, as for an unported command).

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_items::ItemError;
use ow_items::registry::build_registry;
use ow_object::heap::ObjectSpace;

use super::NOT_PORTED;
use crate::args::Command;

const USAGE: &str = "usage: ow-package --item-registry --cooked <install folder> [--census <file>]";

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let mut cooked: Option<&str> = None;
    let mut census_file: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == command.flag {
            i += 1;
            continue;
        }
        match arg {
            "--cooked" => match args.get(i + 1) {
                Some(value) => {
                    cooked = Some(value);
                    i += 2;
                }
                None => return fail("missing value for --cooked"),
            },
            "--census" => match args.get(i + 1) {
                Some(value) => {
                    census_file = Some(value);
                    i += 2;
                }
                None => return fail("missing value for --census"),
            },
            "--overlay" => {
                return fail(
                    "--overlay is not supported by --item-registry yet: the registry reads tag streams, which \
                     overlays do not patch",
                );
            }
            _ => return fail(USAGE),
        }
    }
    let Some(cooked) = cooked else {
        return fail(USAGE);
    };
    let mut defs = match DefStore::open(
        PathBuf::from(cooked),
        IdentityPolicy::FirstInScanOrder,
        mode,
    ) {
        Ok(defs) => defs,
        Err(e) => return fail(&super::properties::error_text(&e)),
    };
    let mut space = ObjectSpace::new();
    match build_registry(&mut defs, &mut space) {
        Ok(registry) => {
            if let Some(file) = census_file
                && let Err(e) = std::fs::write(file, registry.report().to_json())
            {
                return fail(&format!("{file}: {e}"));
            }
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            // Ignore a failed write (a closed pipe), as the other commands do.
            let _ = out.write_all(registry.to_json().as_bytes());
            let _ = out.flush();
            ExitCode::SUCCESS
        }
        Err(ItemError::NotYet(e)) => {
            eprintln!("ow-package (Rust): --item-registry: {e}");
            ExitCode::from(NOT_PORTED)
        }
        Err(e) => fail(&super::properties::error_text(&e)),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("ow-package: {message}");
    ExitCode::from(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Dispatch, dispatch};

    fn command() -> &'static Command {
        let args = vec!["--item-registry".to_string()];
        match dispatch(&args) {
            Dispatch::Run(c) => c,
            Dispatch::Usage => panic!("--item-registry is a command"),
        }
    }

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn bad_command_lines_fail_with_code_1() {
        let c = command();
        for line in [
            &["--item-registry"][..],
            &["--item-registry", "--cooked"],
            &["--item-registry", "--bogus"],
            &["--item-registry", "--cooked", ".", "--overlay", "x.json"],
            &["--item-registry", "--cooked", ".", "--census"],
        ] {
            assert_eq!(
                run(c, &args(line), ParityMode::Off),
                ExitCode::from(1),
                "{line:?}"
            );
        }
    }

    #[test]
    fn an_empty_cooked_folder_builds_an_empty_registry() {
        // DefStore::census() has its body (W3-DEFS): an empty synthetic folder scans no package and builds an empty
        // registry, whose census report says so.
        let dir = std::env::temp_dir().join(format!("ow-cli-items-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cooked = dir.join("cooked");
        std::fs::create_dir_all(&cooked).unwrap();
        let census = dir.join("census.json");
        let line = args(&[
            "--item-registry",
            "--cooked",
            &cooked.to_string_lossy(),
            "--census",
            &census.to_string_lossy(),
        ]);
        assert_eq!(run(command(), &line, ParityMode::Off), ExitCode::SUCCESS);
        let report = std::fs::read_to_string(&census).unwrap();
        for empty in [
            "\"packages_scanned\":0,",
            "\"scan_errors\":{}",
            "\"objects_by_class\":{}",
        ] {
            assert!(report.contains(empty), "{empty} not in {report}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
