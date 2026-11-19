//! Owner: lane IMPL-IS-FRIENDLY. AI-assisted (Claude Code). A pawn's opinion of another and its friendliness test, in
//! `ParityMode::Off`, coded from NATIVE_IS_FRIENDLY.md alone (decision 2026-11-19-60). This lane did not read the
//! natives. Every rule is the note's reading of the executable, **UNVERIFIED** in the running game.
//!
//! - **`Pawn.IsFriendly(Other)`** (section 1): true exactly when the pawn's own opinion of `Other` is Friendly. The
//!   opinion is the pawn's class's: the charm layer for a `WillowPawn` and its subclasses, the generic rule for any
//!   other pawn (section 5).
//! - **`WillowPawn.GetOpinion(Other)`** (section 2), first match wins:
//!   1. `Other` is this pawn's `CharmedChild`: Enemy;
//!   2. `Other` is this pawn's `CharmedParent`: Friendly;
//!   3. this pawn has no `CharmedParent`, `Other` is a `WillowPawn` with a `CharmedParent`, and that parent
//!      `IsFriendly` toward this pawn (by these same rules): Enemy. That second question cannot reach step 3 again:
//!      it asks about this pawn, which has no `CharmedParent`, so no chain or cycle can follow;
//!   4. otherwise the generic rule.
//! - **`Pawn.GetOpinion(Other)`** (section 3), the generic rule:
//!   1. no `Allegiance` on this pawn (A): Neutral;
//!   2. the other side's allegiance B: a `WillowPawn`'s `Allegiance` field; `Other` itself when it is a
//!      `PawnAllegiance`; none for a controller (section 4: it is not read through its pawn) and for None;
//!   3. no B: Neutral;
//!   4. B is A (the same object): A's `SelfOpinion`;
//!   5. the first entry of A's `MyOpinions` whose `Allegiance` is B: its `Opinion`;
//!   6. the first entry of B's `OtherOpinions` whose `Allegiance` is A: its `Opinion`;
//!   7. B's `ForcedOtherOpinion` when B has `bForceAllOtherOpinions`, otherwise A's `DefaultOpinion`.
//!
//!   Allegiances are compared as objects. Health and death are not read: a dead pawn answers like a live one.
//!
//! **`NotYet { NotRead }`, each decided before anything is answered (nothing here changes state):**
//! - an `Other` that is a pawn but not a `WillowPawn`, or an object that is none of a pawn, a `PawnAllegiance` and a
//!   controller: the note says B is what the targetable interface reports, and reads that only for the `WillowPawn`
//!   classes of link 9 (their `Allegiance` field);
//! - a pawn whose `CharmedChild` or `CharmedParent` is the pawn itself: the note leaves that case UNVERIFIED (section
//!   4).
//!
//! A pawn class that lacks a field these rules read is a data error.
//!
//! **Not here.** `IsEnemy` and `IsNeutral` keep the combat pack's binding (NATIVE_COMBAT_GROUNDWORK.md section 6): the
//! note does not read `WillowAIPawn`'s replacements of them.
//!
//! **Reference.** Unchanged: the combat pack's own opinion rule (`super::opinion`, the C++ stand-in) answers
//! `GetOpinion` and `IsFriendly`, and `WillowPawn.GetOpinion` stays unbound (R3).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::access::{class_name, error, is_a, required};
use super::{arg_object, bind, this};

/// The record these rules follow.
pub const RECORD: &str = "NATIVE_IS_FRIENDLY.md";

/// The opinion enum (section 1).
pub const ENEMY: u8 = 0;
pub const NEUTRAL: u8 = 1;
pub const FRIENDLY: u8 = 2;

/// The function paths [`register`] binds in Off.
pub const PATHS: [&str; 3] = [
    "Engine.Pawn.IsFriendly",
    "Engine.Pawn.GetOpinion",
    "WillowGame.WillowPawn.GetOpinion",
];

const WILLOW_PAWN: &str = "WillowGame.WillowPawn";

fn object_field(vm: &mut Vm, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    match required(vm, o, name, what)? {
        Value::Object(x) | Value::Interface(x) => Ok(x),
        Value::None => Ok(None),
        other => Err(error(format!("{what}: {name} is {}, not an object", other.kind_name()))),
    }
}

fn opinion_byte(vm: &mut Vm, o: ObjRef, name: &str, what: &str) -> Result<u8, VmError> {
    match required(vm, o, name, what)? {
        Value::Byte(b) => Ok(b),
        other => Err(error(format!("{what}: {name} is {}, not a byte", other.kind_name()))),
    }
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

/// The other side's allegiance B (section 3 step 2, and section 4).
fn other_allegiance(vm: &mut Vm, other: Option<ObjRef>, what: &str) -> Result<Option<ObjRef>, VmError> {
    let Some(other) = other else { return Ok(None) };
    if is_a(vm, other, WILLOW_PAWN)? {
        let b = object_field(vm, other, "Allegiance", what)?;
        return b.map(|b| super::access::loaded(vm, b)).transpose();
    }
    if is_a(vm, other, "Engine.Pawn")? {
        return Err(not_read(format!(
            "the opinion of a {} (a pawn that is not a WillowPawn: what its targetable interface reports as its \
             allegiance is read only for link 9's WillowPawn classes)",
            class_name(vm, other)
        )));
    }
    if is_a(vm, other, "Engine.PawnAllegiance")? {
        return Ok(Some(other));
    }
    if is_a(vm, other, "Engine.Controller")? {
        return Ok(None);
    }
    Err(not_read(format!(
        "the opinion of a {} (neither a pawn, a PawnAllegiance nor a controller: whether it implements the targetable \
         interface, and what it reports, is not read)",
        class_name(vm, other)
    )))
}

/// The first entry of `list` on `owner` whose `Allegiance` is `wanted`: its `Opinion`.
fn listed(vm: &mut Vm, owner: ObjRef, list: &str, wanted: ObjRef, what: &str) -> Result<Option<u8>, VmError> {
    let entries = required(vm, owner, list, what)?;
    let Some(entries) = entries.elements().map(<[Value]>::to_vec) else {
        return Err(error(format!("{what}: {list} is not an array")));
    };
    for entry in entries {
        let named = match super::access::field(vm, &entry, "Allegiance") {
            Some(Value::Object(o)) => o,
            Some(Value::None) | None => None,
            Some(other) => return Err(error(format!("{what}: a {list} Allegiance is {}", other.kind_name()))),
        };
        let Some(named) = named else { continue };
        let named = super::access::loaded(vm, named)?;
        if named == wanted {
            return match super::access::field(vm, &entry, "Opinion") {
                Some(Value::Byte(b)) => Ok(Some(b)),
                other => Err(error(format!("{what}: a {list} Opinion is {other:?}"))),
            };
        }
    }
    Ok(None)
}

/// `Pawn.GetOpinion(Other)`, the generic rule (module documentation).
pub fn generic_opinion(vm: &mut Vm, me: ObjRef, other: Option<ObjRef>) -> Result<u8, VmError> {
    const WHAT: &str = "Pawn.GetOpinion";
    let Some(own) = object_field(vm, me, "Allegiance", WHAT)? else {
        // Section 3 step 1 answers before the other side is read.
        return Ok(NEUTRAL);
    };
    let own = super::access::loaded(vm, own)?;
    let Some(theirs) = other_allegiance(vm, other, WHAT)? else {
        return Ok(NEUTRAL);
    };
    if theirs == own {
        return opinion_byte(vm, own, "SelfOpinion", WHAT);
    }
    if let Some(o) = listed(vm, own, "MyOpinions", theirs, WHAT)? {
        return Ok(o);
    }
    if let Some(o) = listed(vm, theirs, "OtherOpinions", own, WHAT)? {
        return Ok(o);
    }
    let forced = match required(vm, theirs, "bForceAllOtherOpinions", WHAT)? {
        Value::Bool(b) => b,
        other => return Err(error(format!("{WHAT}: bForceAllOtherOpinions is {}", other.kind_name()))),
    };
    if forced {
        opinion_byte(vm, theirs, "ForcedOtherOpinion", WHAT)
    } else {
        opinion_byte(vm, own, "DefaultOpinion", WHAT)
    }
}

/// `WillowPawn.GetOpinion(Other)`, the charm layer (module documentation).
fn willow_opinion(vm: &mut Vm, me: ObjRef, other: Option<ObjRef>) -> Result<u8, VmError> {
    const WHAT: &str = "WillowPawn.GetOpinion";
    let child = object_field(vm, me, "CharmedChild", WHAT)?;
    let parent = object_field(vm, me, "CharmedParent", WHAT)?;
    if child == Some(me) || parent == Some(me) {
        return Err(not_read(
            "a WillowPawn whose CharmedChild or CharmedParent is itself (UNVERIFIED in the note, section 4)",
        ));
    }
    if let Some(other) = other {
        if child == Some(other) {
            return Ok(ENEMY);
        }
        if parent == Some(other) {
            return Ok(FRIENDLY);
        }
        if parent.is_none() && is_a(vm, other, WILLOW_PAWN)? {
            if let Some(their_parent) = object_field(vm, other, "CharmedParent", WHAT)?
                && opinion_of(vm, their_parent, Some(me))? == FRIENDLY
            {
                return Ok(ENEMY);
            }
        }
    }
    generic_opinion(vm, me, other)
}

/// The pawn's own opinion rule: the charm layer for a `WillowPawn`, the generic rule otherwise (section 5).
fn opinion_of(vm: &mut Vm, me: ObjRef, other: Option<ObjRef>) -> Result<u8, VmError> {
    if is_a(vm, me, WILLOW_PAWN)? {
        willow_opinion(vm, me, other)
    } else {
        generic_opinion(vm, me, other)
    }
}

/// `IsFriendly` of `me` toward `other` (module documentation).
pub fn is_friendly(vm: &mut Vm, me: ObjRef, other: Option<ObjRef>) -> Result<bool, VmError> {
    Ok(opinion_of(vm, me, other)? == FRIENDLY)
}

/// Binds the three natives in `ParityMode::Off`; Reference binds nothing here (module documentation). By path, the
/// first binding of a key winning, so call this before any pack that binds a stand-in of the same key.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let [friendly, generic, willow] = PATHS;
    let natives: [(&str, NativeFn); 3] = [
        (
            friendly,
            Rc::new(|c: &mut NativeCall<'_>| {
                let me = this(c, "Pawn.IsFriendly")?;
                let other = arg_object(c, 0)?;
                Ok(Value::Bool(is_friendly(c.vm, me, other)?))
            }),
        ),
        (
            generic,
            Rc::new(|c: &mut NativeCall<'_>| {
                let me = this(c, "Pawn.GetOpinion")?;
                let other = arg_object(c, 0)?;
                Ok(Value::Byte(generic_opinion(c.vm, me, other)?))
            }),
        ),
        (
            willow,
            Rc::new(|c: &mut NativeCall<'_>| {
                let me = this(c, "WillowPawn.GetOpinion")?;
                let other = arg_object(c, 0)?;
                Ok(Value::Byte(willow_opinion(c.vm, me, other)?))
            }),
        ),
    ];
    for (path, f) in natives {
        bind(vm, path, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "opinion_tests.rs"]
mod tests;
