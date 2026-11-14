//! The scaffold's promise: every command exits 3 with a "not ported" message until its lane ports it.

use std::process::Command;

fn ow_package(args: &[&str]) -> (Option<i32>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ow-package"))
        .args(args)
        .output()
        .unwrap();
    assert!(out.stdout.is_empty(), "a stub prints nothing on stdout");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn a_command_says_not_ported_and_exits_3() {
    let (code, err) = ow_package(&["x", "--names"]);
    assert_eq!(code, Some(3));
    assert!(
        err.contains("--names is not ported to Rust yet (lane W1-PKG)"),
        "{err}"
    );
    // As in the C++ binary, "--names" in the package position is read as a package path, so the reader is missing.
    let (code, err) = ow_package(&["--names", "x"]);
    assert_eq!(code, Some(3));
    assert!(
        err.contains("<package> is not ported to Rust yet (lane W1-PKG)"),
        "{err}"
    );
    let (code, err) = ow_package(&["--native-selftest"]);
    assert_eq!(code, Some(3));
    assert!(err.contains("(lane W4-VMTOOLS)"), "{err}");
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
