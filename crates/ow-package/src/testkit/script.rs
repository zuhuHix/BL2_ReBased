//! Owner: lane W2-SCRIPT. Not started: this file is a placeholder (W1-TESTKIT); the owner lane fills it.
//!
//! Function exports and hand-assembled UnrealScript bodies for synthetic packages, built on
//! [`super::PackageBuilder`] (invented values only, AGENTS.md rule 9). It will re-express these Python helpers, byte
//! for byte, the way `testkit/tests.rs` pins the package helpers:
//!
//! - The `Asm` writer of `tests/combat_test.py` (as in `tests/vm_test.py`): opcodes, object references that are 4
//!   bytes in the file and 8 in memory, and `here()`, the in-memory offset that jump targets use.
//! - `Toy.function()` of `tests/combat_test.py` (lines 176-205) and of `tests/mission_script_test.py`
//!   (lines 207-222): parameters and the return value exported first in reverse declaration order, then the
//!   Function export (script sizes in memory and in the file, native index, flags, friendly name).
//! - `func()` and `make_function()` of `tests/vm_test.py` (lines 92-135 and after).
//!
//! The function index is reserved before its children are added: [`super::PackageBuilder::next_export`] and
//! [`super::PackageBuilder::export_count`].
