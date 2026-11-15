//! Owner: lane W4-INTERP2 (created as a `NotYet` stub by W3-INTERP, architecture section 8). `foreach`: dynamic-array
//! iterators and engine iterators (`EX_DynArrayIterator`, `EX_Iterator`, `IteratorNext`, `IteratorPop`,
//! `startEngineIterator` in `src/interp.cpp`), with the iterator natives the VM registry holds.
//!
//! Until W4-INTERP2 fills it, every iterator statement is `NotYet { NotPorted }` naming the opcode.

use ow_core::NotYet;

use crate::error::VmError;

/// An iterator statement (`op` is the opcode).
pub(crate) fn statement(op: u8) -> VmError {
    NotYet::not_ported(format!("statement 0x{op:02x} (foreach)"), "lane W4-INTERP2").into()
}
