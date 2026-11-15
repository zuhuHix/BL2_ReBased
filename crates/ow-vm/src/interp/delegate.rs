//! Owner: lane W4-INTERP2 (created as a `NotYet` stub by W3-INTERP, architecture section 8). Delegates: delegate
//! calls, delegate properties and values, the delegate comparisons and `LetDelegate` (`callDelegate` and the
//! delegate cases of `eval` in `src/interp.cpp`).
//!
//! Until W4-INTERP2 fills it, every delegate expression is `NotYet { NotPorted }` naming the opcode.

use ow_core::NotYet;

use crate::error::VmError;

/// A delegate expression (`op` is the opcode).
pub(crate) fn expression(op: u8) -> VmError {
    NotYet::not_ported(
        format!("expression 0x{op:02x} (delegates)"),
        "lane W4-INTERP2",
    )
    .into()
}
