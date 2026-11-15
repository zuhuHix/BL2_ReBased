//! The scaffold's promise: every command exits 3 with a "not ported" message until its lane ports it.
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
    assert!(out.stdout.is_empty(), "a stub prints nothing on stdout");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn ow_package(args: &[&str]) -> (Option<i32>, String) {
    ow_package_with(args, None)
}

// A lane that ports the example command switches it to another unported one.
#[test]
fn a_command_says_not_ported_and_exits_3() {
    let (code, err) = ow_package(&["x", "--combat-run", "1"]);
    assert_eq!(code, Some(3));
    assert!(
        err.contains("--combat-run is not ported to Rust yet (lane W6-COMBAT)"),
        "{err}"
    );
    let (code, err) = ow_package(&["x", "--mover-probe"]);
    assert_eq!(code, Some(3));
    assert!(err.contains("(lane W5-DIALOG)"), "{err}");
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
    let (code, err) = ow_package_with(&["x", "--combat-run", "1"], Some("Reference"));
    assert_eq!(code, Some(1));
    assert_eq!(
        err.trim_end(),
        "ow-package: OW_PARITY must be \"reference\" or unset, not \"Reference\""
    );
    let (code, _) = ow_package_with(&["x", "--combat-run", "1"], Some("reference"));
    assert_eq!(code, Some(3));
}
