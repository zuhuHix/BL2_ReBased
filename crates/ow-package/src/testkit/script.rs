//! Owner: lane W2-SCRIPT. Not started: this file is a placeholder (W1-TESTKIT); the owner lane fills it.
//!
//! Function exports and hand-assembled UnrealScript bodies for synthetic packages, built on
//! [`super::PackageBuilder`] (invented values only, AGENTS.md rule 9). It will re-express these Python helpers, byte
//! for byte, the way `testkit/tests.rs` pins the package helpers:
//!
//! - The `Asm` writer (`tests/combat_test.py:138`, `tests/mission_script_test.py:161`, `tests/vm_test.py:63`):
//!   opcodes, object references that are 4 bytes in the file and 8 in memory, and `here()`, the in-memory offset
//!   that jump targets use.
//! - `Toy.function()` of `tests/combat_test.py` (lines 192-207) and of `tests/mission_script_test.py` (from
//!   line 222): parameters and the return value exported first in reverse declaration order, then the
//!   Function export (script sizes in memory and in the file, native index, flags, friendly name).
//! - `func()` (from line 96) and `make_function()` (from line 132) of `tests/vm_test.py`.
//!
//! The function index is reserved before its children are added: [`super::PackageBuilder::next_export`] and
//! [`super::PackageBuilder::export_count`].
