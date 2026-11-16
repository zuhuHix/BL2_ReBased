//! `WillowGame.PlayThroughCountAttributeValueResolver`, the playthrough count. The class declares one field,
//! `IncludePlaythroughThree` (int), and no script functions. The installed data has one such resolver, the only value
//! resolver of `D_Attributes.Balance.PlayThroughCount` (context chain NoContextNeeded, `IncludePlaythroughThree` 0).
//! **Every rule here is UNVERIFIED in the running game.**
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ reading: **1**, whatever the context, the previous value, the field and the
//!   world's playthrough. `src/progression.cpp` `operandValue` answers an expression operand whose attribute path ends
//!   in `PlayThroughCount` with `AttributeContext::playThroughCount`, which defaults to 1 (`src/progression.hpp`) and
//!   which nothing sets to another value (`src/mission_script.cpp` keeps its own copy at 1 and refuses any other).
//!   The C++ matches the attribute by path and never runs its chain; the port runs the chain and answers at the
//!   resolver. In the installed data the two meet exactly: the one attribute whose path ends that way is the one
//!   attribute served by this class, and its context chain (NoContextNeeded) always resolves. Where the C++ would
//!   refuse (the attribute read outside an expression, as "unsupported attribute value resolver"), this answers 1
//!   too. Get and get-base are the same read.
//! - `ParityMode::Off`: `NotYet { NotPorted }`. The native rule is written down in NATIVE_PLAYTHROUGH_ATTRIBUTES.md
//!   and waits for a clean-room implementation from that note (docs/LANE_BRIEF.md, "Clean room (native code)").
//!
//! **Writes** (add, remove, set-base) answer `NotYet { NotPorted }` in both modes: the C++ has no write for this
//! class, and the native one waits with the Off rule.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;

use crate::api::{AttrError, EvalCx, Storage, ValueResolver};
use crate::registry::AttributeSystem;

/// Where the native rule is written down.
pub(crate) const NOTE: &str = "NATIVE_PLAYTHROUGH_ATTRIBUTES.md";
/// Why the Off rule and the writes are not here yet.
const AWAITING: &str =
    "native rule in NATIVE_PLAYTHROUGH_ATTRIBUTES.md, awaiting clean-room implementation";
/// The C++ reading's value (`src/progression.hpp`, `AttributeContext::playThroughCount`).
const CPP_PLAYTHROUGH_COUNT: f32 = 1.0;

pub(crate) struct PlayThroughCount;

/// The world's parity mode: its store's. A world without a store runs the Rust rules (`Off`).
fn mode(cx: &mut EvalCx<'_>) -> ParityMode {
    cx.world.parts().defs.map_or(ParityMode::Off, |d| d.mode())
}

/// `NotYet { NotPorted }` for the native rule.
fn awaiting(what: &str) -> AttrError {
    NotYet::not_ported(
        format!("PlayThroughCountAttributeValueResolver {what}: {AWAITING}"),
        NOTE,
    )
    .into()
}

/// Get and get-base: the C++ reading in Reference, the awaited native rule in Off.
fn read(cx: &mut EvalCx<'_>, what: &str) -> Result<f32, AttrError> {
    match mode(cx) {
        ParityMode::Reference => Ok(CPP_PLAYTHROUGH_COUNT),
        _ => Err(awaiting(what)),
    }
}

impl ValueResolver for PlayThroughCount {
    fn value(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(cx, "get")
    }

    fn base_value(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(cx, "get-base")
    }

    /// Both modes: `NotYet { NotPorted }` (module documentation), never a quiet `None`.
    fn storage(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        _cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        Err(awaiting("add, remove or set-base"))
    }
}
