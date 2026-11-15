//! Owner: lane W4-REGISTRY. Commands: `--item-registry --cooked <install folder> [--overlay <file>]` (Rust-only).
//!
//! Prints the `openwillow-item-registry/1` JSON of `ow_items::registry` to stdout: the bytes
//! `tools/item_registry.py` writes to `registry.json` (no final newline), so `ow-package --item-registry ... >
//! local/.../registry.json` gives a file to compare with the tool's. The registry is game data: write it under ignored
//! `local/` only (docs/LEGAL.md). `--cooked` names the install folder (the one holding `WillowGame/CookedPCConsole`
//! and `DLC/`), as the tool's `--game` does; the registry's `game` field is that text as Python's `str(Path(..))`
//! prints it.
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
use ow_items::registry::{RegistryError, build_registry, python_path_text};
use ow_object::heap::ObjectSpace;

use super::NOT_PORTED;
use crate::args::Command;

const USAGE: &str = "usage: ow-package --item-registry --cooked <install folder>";

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let mut cooked: Option<&str> = None;
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
    match build_registry(&mut defs, &mut space, &python_path_text(cooked)) {
        Ok(registry) => {
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            // Ignore a failed write (a closed pipe), as the other commands do.
            let _ = out.write_all(registry.to_json().as_bytes());
            let _ = out.flush();
            ExitCode::SUCCESS
        }
        Err(RegistryError::NotYet(e)) => {
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
        ] {
            assert_eq!(
                run(c, &args(line), ParityMode::Off),
                ExitCode::from(1),
                "{line:?}"
            );
        }
    }

    #[test]
    fn the_store_census_is_not_there_yet() {
        // Flips when the body of DefStore::census() lands (W3-DEFS).
        let dir = std::env::temp_dir().join(format!("ow-cli-items-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let line = args(&["--item-registry", "--cooked", &dir.to_string_lossy()]);
        assert_eq!(
            run(command(), &line, ParityMode::Off),
            ExitCode::from(NOT_PORTED)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
