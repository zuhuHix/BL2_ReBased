//! Owner: lane W2-REFLECT. Not started: this file is a placeholder (W1-TESTKIT); the owner lane fills it.
//!
//! Reflection exports for synthetic packages, built on [`super::PackageBuilder`] (invented values only, AGENTS.md
//! rule 9). It will re-express these Python helpers, byte for byte, the way `testkit/tests.rs` pins the package
//! helpers:
//!
//! - `declarations()` of `tests/behavior_test.py` (line 99): `prop` (the 40-byte property header, then the type
//!   reference), `array_of` (the inner declaration exported right after the array), `struct_` (a `ScriptStruct`
//!   export, optionally the 52-byte header and default tags), `cls`, `enum`.
//! - The same helpers in `tests/kismet_test.py` (lines 84-96), with no flags.
//! - The `Toy` class of `tests/combat_test.py` (lines 176-205) and the one of `tests/mission_script_test.py`
//!   (lines 207-222): `cls`, `prop` with flags (and a custom `tail`), `array_of`, `struct_` with an owner.
//! - `build_package()`'s `prop` (with `dim`) and `class_body()` of `tests/vm_test.py` (lines 92-135): the Class
//!   export body of `docs/verification/NATIVE_CLASS_SERIAL_LAYOUT.md`.
//!
//! `array_of` and the function helpers need the reference of an export before it is added:
//! [`super::PackageBuilder::next_export`].
