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
//! - `ParityMode::Off`, the native rule, implemented from NATIVE_PLAYTHROUGH_ATTRIBUTES.md alone (the implementer
//!   half of the clean room, docs/LANE_BRIEF.md). Get reads only the world's game replication info
//!   ([`EngineQuery::GameReplicationInfo`]) and the resolver's `IncludePlaythroughThree`; the context and the chain's
//!   previous value are ignored (note section 3, checklist item 1). With CP the replication info's
//!   `CurrentPlaythrough` as stored (note section 3):
//!
//!   | Case | Result |
//!   |---|---|
//!   | no replication info (`Ok(None)`), or one that is not a `WillowGameReplicationInfo` (a subclass counts) | 1.0 |
//!   | CP < 0 | 1.0 |
//!   | CP >= 0, `IncludePlaythroughThree` = 0 | CP + 1, clamped to 1..2 |
//!   | CP >= 0, `IncludePlaythroughThree` not 0 | CP + 1 |
//!
//!   CP + 1 is the 32-bit wrapping sum (note section 4, checklist item 4): CP = `i32::MAX` gives `i32::MIN`, so the
//!   clamped row reads 1.0 and the other -2147483648.0. The int becomes a float rounded to nearest
//!   ([`crate::eval::number`]). A world that cannot answer the query (`NotYet`) stops the read with that `NotYet`:
//!   "this world does not say" is not "this world has no replication info". Get-base is get (note section 2).
//!
//! **Writes** (add, remove, set-base): in Off, the class default (note section 2, checklist item 6): `storage` is
//! `Ok(None)`, so each answers false and nothing changes. In Reference, `NotYet { NotPorted }`, as before: the C++ has
//! no write for this class.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use crate::api::{AttrError, EngineQuery, EvalCx, Storage, ValueResolver};
use crate::context_resolvers::is_a;
use crate::eval::{number, required};
use crate::registry::AttributeSystem;

/// Where the native rule is written down.
pub(crate) const NOTE: &str = "NATIVE_PLAYTHROUGH_ATTRIBUTES.md";
/// The C++ reading's value (`src/progression.hpp`, `AttributeContext::playThroughCount`).
const CPP_PLAYTHROUGH_COUNT: f32 = 1.0;
/// The count when the world has no usable replication info, or a negative playthrough (note section 3).
const NO_PLAYTHROUGH: f32 = 1.0;
/// The class the replication info must be of, a subclass included (note section 3).
const GAME_REPLICATION_INFO: &str = "WillowGame.WillowGameReplicationInfo";

pub(crate) struct PlayThroughCount;

/// The world's parity mode: its store's. A world without a store runs the Rust rules (`Off`).
fn mode(cx: &mut EvalCx<'_>) -> ParityMode {
    cx.world.parts().defs.map_or(ParityMode::Off, |d| d.mode())
}

/// An int field of `obj` (note section 1 reads both fields as ints). Any other kind, a byte included, is bad data.
fn int_field(cx: &mut EvalCx<'_>, obj: ObjRef, name: &str) -> Result<i32, AttrError> {
    match required(cx, obj, name)? {
        Value::Int(i) => Ok(i),
        other => Err(AttrError::BadData {
            reason: format!(
                "PlayThroughCountAttributeValueResolver: {name} is a {}, not an int",
                other.kind_name()
            )
            .into(),
        }),
    }
}

/// The note's section 3 table on its two inputs: `current` is the replication info's `CurrentPlaythrough` (`None`
/// for the first row), `include_three` the resolver's field. CP + 1 wraps (section 4).
pub(crate) fn count(current: Option<i32>, include_three: i32) -> i32 {
    match current {
        None => 1,
        Some(cp) if cp < 0 => 1,
        Some(cp) => {
            let n = cp.wrapping_add(1);
            if include_three == 0 { n.clamp(1, 2) } else { n }
        }
    }
}

/// The Off-mode get (module documentation). `context` is passed to the query as its source, which the query does
/// not read.
fn native(cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    let gri = cx
        .world
        .engine_query(EngineQuery::GameReplicationInfo, context)?;
    let current = match gri {
        Some(g) if is_a(cx.world.parts().space, g, GAME_REPLICATION_INFO)? => {
            Some(int_field(cx, g, "CurrentPlaythrough")?)
        }
        _ => None,
    };
    if current.is_none_or(|cp| cp < 0) {
        // The first two rows give 1 whatever the field.
        return Ok(NO_PLAYTHROUGH);
    }
    let include_three = int_field(cx, resolver, "IncludePlaythroughThree")?;
    number(
        "PlayThroughCount",
        &Value::Int(count(current, include_three)),
    )
}

/// Get and get-base: the C++ reading in Reference, the native rule in Off.
fn read(cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    match mode(cx) {
        ParityMode::Reference => Ok(CPP_PLAYTHROUGH_COUNT),
        _ => native(cx, resolver, context),
    }
}

impl ValueResolver for PlayThroughCount {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(cx, resolver, context)
    }

    /// Get-base is get in both modes (note section 2).
    fn base_value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(cx, resolver, context)
    }

    /// Off: the class default, `None` (each write answers false). Reference: `NotYet { NotPorted }`, never a quiet
    /// `None` (module documentation).
    fn storage(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        match mode(cx) {
            ParityMode::Reference => Err(NotYet::not_ported(
                "PlayThroughCountAttributeValueResolver add, remove or set-base: the C++ has no write for this class",
                NOTE,
            )
            .into()),
            _ => Ok(None),
        }
    }
}
