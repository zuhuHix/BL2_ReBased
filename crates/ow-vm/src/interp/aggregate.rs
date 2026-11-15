//! Owner: lane W4-INTERP2 (created as a `NotYet` stub by W3-INTERP, architecture section 8). Struct and
//! dynamic-array operations: struct members, array elements (static and dynamic), the dynamic-array natives
//! (length, add, insert, remove, find, sort) and struct comparison (`src/interp.cpp`).
//!
//! Until W4-INTERP2 fills it, every operation here is `NotYet { NotPorted }` naming the opcode.

use ow_core::NotYet;

use crate::error::VmError;

/// An element, member or dynamic-array operation (`op` is the opcode).
pub(crate) fn element(op: u8) -> VmError {
    NotYet::not_ported(
        format!("expression 0x{op:02x} (struct and array operations)"),
        "lane W4-INTERP2",
    )
    .into()
}

/// A static array (`array_dim > 1`) used as a whole, or through one of its elements.
pub(crate) fn static_array() -> VmError {
    NotYet::not_ported(
        "a static-array variable (struct and array operations)",
        "lane W4-INTERP2",
    )
    .into()
}
