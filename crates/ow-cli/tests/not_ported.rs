//! The scaffold's promise was that every command exits 3 with a "not ported" message until its lane ports it. Every
//! command is ported now (lane W6-FIRE ported `--slice-run`, the last, decision 2026-11-16-38), so what is left here are
//! the two checks `main` makes before any command runs: no arguments, and the parity variable's value.
//!
//! This file drives the binary, so it may name the parity variable for the child process (tests/crate_edges_test.py
//! allows the string in `crates/*/tests/`); it never reads the mode itself.

use std::process::Command;

const PARITY_VAR: &str = "OW_PARITY";

fn ow_package_with(args: &[&str], parity: Option<&str>) -> (Option<i32>, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ow-package"));
    command.args(args).env_remove(PARITY_VAR);
    if let Some(value) = parity {
        command.env(PARITY_VAR, value);
    }
    let out = command.output().unwrap();
    assert!(out.stdout.is_empty(), "these error paths print nothing on stdout");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn ow_package(args: &[&str]) -> (Option<i32>, String) {
    ow_package_with(args, None)
}

#[test]
fn no_arguments_print_the_usage_and_exit_1() {
    let (code, err) = ow_package(&[]);
    assert_eq!(code, Some(1));
    assert!(
        err.starts_with("ow-package: usage: ow-package <package> [--exports"),
        "{err}"
    );
}

#[test]
fn a_bad_parity_value_is_an_error_with_exit_1() {
    let (code, err) = ow_package_with(&["x", "--slice-run", "1"], Some("Reference"));
    assert_eq!(code, Some(1));
    assert_eq!(
        err.trim_end(),
        "ow-package: OW_PARITY must be \"reference\" or unset, not \"Reference\""
    );
    // A valid value reaches the command (its package error, not the parity error).
    let (code, err) = ow_package_with(&["x", "--slice-run", "1"], Some("reference"));
    assert_eq!(code, Some(1));
    assert_eq!(err.trim_end(), "ow-package: cannot open input package");
}
