//! Owner: lane W2-SCRIPT. The whole-package structural check: the port of `checkPackage` of `src/script.cpp`, the
//! counterpart of `research/script_disasm.py --check`, which `--script-check` prints.

use ow_package::package::{Package, PackageError};

use crate::function::{MIN_FUNCTION_SIZE, is_function_export, read_function};
use crate::ops::flags::FUNC_NATIVE;
use crate::decode::decode;

/// What [`check_package`] found (C++ `script::CheckResult`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckResult {
    /// Function exports whose tail could be read.
    pub functions: usize,
    /// Of those, the natives (no script).
    pub native: usize,
    /// Script functions that decoded.
    pub decoded: usize,
    /// `<function path>: <reason>` for each script function that did not decode, in export order.
    pub failures: Vec<String>,
}

/// Reads and decodes every function export of `package`. A function export under 40 bytes, or one whose tail
/// cannot be read, is skipped and not counted, as the Python oracle does. Fails only when the path of a failing
/// function cannot be built (a cyclic outer chain), where the C++ throws out of the check.
pub fn check_package(package: &Package) -> Result<CheckResult, PackageError> {
    let mut result = CheckResult::default();
    for index in 1..=package.exports.len() as i32 {
        if !is_function_export(package, index) {
            continue;
        }
        if package.exports[index as usize - 1].size < MIN_FUNCTION_SIZE {
            continue;
        }
        let Ok(info) = read_function(package, index) else {
            continue;
        };
        result.functions += 1;
        if info.flags & FUNC_NATIVE != 0 {
            result.native += 1;
            continue;
        }
        match decode(package, &info) {
            Ok(_) => result.decoded += 1,
            Err(error) => result.failures.push(format!("{}: {error}", package.path_of(index)?)),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support::{add, load, raw, toy};
    use ow_package::testkit::Export;
    use ow_package::testkit::script::{DEFAULT_FLAGS, FunctionSpec};

    #[test]
    fn counts_natives_failures_and_skips() {
        let mut t = toy();
        let spec = FunctionSpec::new(DEFAULT_FLAGS, "F");
        add(&mut t, "Good", Some(raw(&[0x04, 0x25, 0x53])), &spec);
        add(&mut t, "Native", None, &FunctionSpec { native: 7, ..FunctionSpec::new(0x400, "N") });
        add(&mut t, "Bad", Some(raw(&[0x2B, 0x53])), &spec);
        add(&mut t, "Short", Some(raw(&[0x04, 0x25, 0x53])), &FunctionSpec { memory_size: Some(9), ..spec.clone() });
        // Skipped without being counted: a Function export under 40 bytes, and one whose tail has no FriendlyName.
        let function = t.import("Function");
        t.p.add_export_with(Export::new(function, "Tiny", vec![0u8; 39]));
        t.p.add_export_with(Export::new(function, "NoTail", vec![0xFFu8; 64]));
        let package = load(&mut t);
        assert_eq!(check_package(&package).unwrap(), CheckResult {
            functions: 4,
            native: 1,
            decoded: 1,
            failures: vec![
                "Thing.Bad: unknown opcode 0x2b".to_string(),
                "Thing.Short: memory size 3 != header ScriptBytecodeSize 9".to_string(),
            ],
        });
    }

    #[test]
    fn a_failing_function_without_a_path_fails_the_check() {
        let mut t = toy();
        let bad = add(&mut t, "Bad", Some(raw(&[0x2B, 0x53])), &FunctionSpec::new(DEFAULT_FLAGS, "F"));
        t.p.set_outer(bad, bad);
        assert_eq!(check_package(&load(&mut t)), Err(PackageError::OuterChain));
    }
}
