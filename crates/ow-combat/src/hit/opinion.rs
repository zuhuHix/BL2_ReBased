//! Owner: lane IMPL-IS-FRIENDLY. AI-assisted (Claude Code). A pawn's opinion of another and its friendliness test, in
//! `ParityMode::Off`, coded from NATIVE_IS_FRIENDLY.md alone (decision 2026-11-19-60). This lane did not read the
//! natives. Every rule is the note's reading of the executable, **UNVERIFIED** in the running game.
//!
//! - **`Pawn.IsFriendly(Other)`** (section 1): true exactly when the pawn's own opinion of `Other` is Friendly. The
//!   opinion is the receiver's `GetOpinion`, resolved virtually through the VM (review r2 of #449, the ruling of
//!   #444's `OnPlayerClassChange`): the stock natives below give the note's rules, a script override runs as script,
//!   and a class that declares no `GetOpinion` is a named `NotYet`.
//! - **`WillowPawn.GetOpinion(Other)`** (section 2), first match wins:
//!   1. `Other` is this pawn's `CharmedChild`: Enemy;
//!   2. `Other` is this pawn's `CharmedParent`: Friendly;
//!   3. this pawn has no `CharmedParent`, `Other` is a `WillowPawn` with a `CharmedParent`, and that parent's own
//!      `IsFriendly` toward this pawn (dispatched through the VM as above) is true: Enemy;
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
//!   A pawn asking about itself answers its own allegiance's `SelfOpinion` (Neutral without one) before the other side
//!   is classified (section 4). Allegiances are compared as objects; a definition is loaded only when its fields are
//!   read. Health and death are not read: a dead pawn answers like a live one.
//!
//! **`NotYet { NotRead }`, each decided before anything is answered (nothing here changes state):**
//! - an `Other` that is a pawn but not a `WillowPawn`, or an object that is none of a pawn, a `PawnAllegiance` and a
//!   controller: the note says B is what the targetable interface reports, and reads that only for the `WillowPawn`
//!   classes of link 9 (their `Allegiance` field);
//! - a `WillowPawn` asking about itself while a charm field points at itself: whether the charm steps then match is
//!   the case the note leaves UNVERIFIED (section 4). A self link that the result does not depend on is not refused:
//!   the steps run in the note's order, so a `CharmedChild` match answers first;
//! - an `Other` that is a class object: the note reads the rule for objects only.
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
use super::{bind, this};

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
    match field_of(vm, o, name, what)? {
        Value::Byte(b) => Ok(b),
        other => Err(error(format!("{what}: {name} is {}, not a byte", other.kind_name()))),
    }
}

/// The `Other` argument: an object, an interface's object or None; a class object is a named `NotYet` (module
/// documentation), any other kind a data error.
fn other_arg(c: &NativeCall<'_>, what: &str) -> Result<Option<ObjRef>, VmError> {
    if c.arg_count() == 0 {
        return Ok(None);
    }
    match c.arg(0)? {
        Value::Object(o) | Value::Interface(o) => Ok(*o),
        Value::None | Value::Class(None) => Ok(None),
        Value::Class(Some(_)) => Err(not_read(format!(
            "{what} of a class object (the note reads the opinion rule for objects only)"
        ))),
        other => Err(error(format!("{what}: Other is {}, not an object", other.kind_name()))),
    }
}

/// `name` on `me`, resolved virtually through the VM (its class and state), called with `Other`.
fn call_virtual(vm: &mut Vm, me: ObjRef, name: &str, other: Option<ObjRef>) -> Result<Value, VmError> {
    let (class, state) = {
        let h = vm.space.heap.get(me)?;
        (h.class, h.state)
    };
    let Some(f) = vm.find_method(class, name, state)? else {
        return Err(not_read(format!("{name} on a {} (its class declares none)", class_name(vm, me))));
    };
    // An override that resolves but cannot run (an unbound native, under any stub policy, or a script that does not
    // prepare) is refused before the call, naming the method (review r3 of #449). Errors of a script that runs pass
    // through unchanged.
    if let Err(e) = vm.prepare_call(f) {
        let path = vm.function(f).map(|d| d.path.as_str().to_string()).unwrap_or_default();
        return Err(not_read(format!("{name} on a {} resolves to {path}, which cannot run here: {e}", class_name(vm, me))));
    }
    Ok(vm.call(f, Some(me), vec![Value::Object(other)])?.ret)
}

/// A definition's field, the definition loaded first (it may be an unloaded handle read from data).
fn field_of(vm: &mut Vm, o: ObjRef, name: &str, what: &str) -> Result<Value, VmError> {
    let o = super::access::loaded(vm, o)?;
    required(vm, o, name, what)
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

/// The other side's allegiance B (section 3 step 2, and section 4).
fn other_allegiance(vm: &mut Vm, other: Option<ObjRef>, what: &str) -> Result<Option<ObjRef>, VmError> {
    let Some(other) = other else { return Ok(None) };
    if is_a(vm, other, WILLOW_PAWN)? {
        return object_field(vm, other, "Allegiance", what);
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
    let entries = field_of(vm, owner, list, what)?;
    let Some(entries) = entries.elements().map(<[Value]>::to_vec) else {
        return Err(error(format!("{what}: {list} is not an array")));
    };
    for entry in entries {
        let named = match super::access::field(vm, &entry, "Allegiance") {
            Some(Value::Object(o)) => o,
            Some(Value::None) | None => None,
            Some(other) => return Err(error(format!("{what}: a {list} Allegiance is {}", other.kind_name()))),
        };
        if named == Some(wanted) {
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
    if other == Some(me) {
        // Itself (section 4), answered before the other side is classified.
        return opinion_byte(vm, own, "SelfOpinion", WHAT);
    }
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
    let forced = match field_of(vm, theirs, "bForceAllOtherOpinions", WHAT)? {
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
    let self_link = |field: &str| {
        not_read(format!(
            "a WillowPawn asking about itself while its {field} is itself (whether the charm step then matches is \
             UNVERIFIED in the note, section 4)"
        ))
    };
    // Each field is read just before its step (review r3 of #449): a matching child answers even when the parent field
    // is missing or invalid.
    let child = object_field(vm, me, "CharmedChild", WHAT)?;
    if other == Some(me) && child == Some(me) {
        return Err(self_link("CharmedChild"));
    }
    if other.is_some() && child == other {
        return Ok(ENEMY);
    }
    let parent = object_field(vm, me, "CharmedParent", WHAT)?;
    if other == Some(me) && parent == Some(me) {
        return Err(self_link("CharmedParent"));
    }
    if let Some(other) = other {
        if parent == Some(other) {
            return Ok(FRIENDLY);
        }
        if parent.is_none()
            && is_a(vm, other, WILLOW_PAWN)?
            && let Some(their_parent) = object_field(vm, other, "CharmedParent", WHAT)?
            && is_friendly(vm, their_parent, Some(me))?
        {
            return Ok(ENEMY);
        }
    }
    generic_opinion(vm, me, other)
}

/// `IsFriendly` of `me` toward `other` (module documentation): `me`'s `GetOpinion`, dispatched through the VM.
fn opinion_is_friendly(vm: &mut Vm, me: ObjRef, other: Option<ObjRef>) -> Result<bool, VmError> {
    match call_virtual(vm, me, "GetOpinion", other)? {
        Value::Byte(b) => Ok(b == FRIENDLY),
        v => Err(error(format!("GetOpinion answered {}, not a byte", v.kind_name()))),
    }
}

/// `me`'s own `IsFriendly` toward `other`, dispatched through the VM (step 3 of the charm layer).
pub fn is_friendly(vm: &mut Vm, me: ObjRef, other: Option<ObjRef>) -> Result<bool, VmError> {
    match call_virtual(vm, me, "IsFriendly", other)? {
        Value::Bool(b) => Ok(b),
        v => Err(error(format!("IsFriendly answered {}, not a bool", v.kind_name()))),
    }
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
                let other = other_arg(c, "Pawn.IsFriendly")?;
                Ok(Value::Bool(opinion_is_friendly(c.vm, me, other)?))
            }),
        ),
        (
            generic,
            Rc::new(|c: &mut NativeCall<'_>| {
                let me = this(c, "Pawn.GetOpinion")?;
                let other = other_arg(c, "Pawn.GetOpinion")?;
                Ok(Value::Byte(generic_opinion(c.vm, me, other)?))
            }),
        ),
        (
            willow,
            Rc::new(|c: &mut NativeCall<'_>| {
                let me = this(c, "WillowPawn.GetOpinion")?;
                let other = other_arg(c, "WillowPawn.GetOpinion")?;
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
