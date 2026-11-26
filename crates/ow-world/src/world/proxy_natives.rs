//! Owner: lane IMPL-PAWN-INTERACTION-PROXY (decision 2026-11-26-62). `Actor.SetCollisionSize(Radius, Height)` and
//! `Actor.SetHardAttach(optional bNewHardAttach)` in `ParityMode::Off`, for the player's trade interaction proxy.
//! AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_PAWN_INTERACTION_PROXY.md sections 3 and 4 (#802 as
//! landed, r1) alone; this lane did not read the executable. **Every rule is UNVERIFIED in the running game.** Reference
//! binds nothing. Section 5 (`SetBase` on the proxy) is in [`super::set_base`].
//!
//! **`SetCollisionSize`** (one body for every class). The preflight, write-free and load-free: `CollisionComponent`
//! declared as an object; on a `CylinderComponent` its `CollisionHeight` and `CollisionRadius` (floats) and the
//! deferred reattach's declarations and refusals (`component_setters`); `bNetDirty` a bool; the `CollisionChanged`
//! event resolved as Off event dispatch resolves it, in loaded code only (code not loaded is [`EVENT_NOT_LOADED`]); then
//! **the touch update**: a world with no collision service is [`TOUCH_UPDATE`] (`LayerAbsent`), unless the Off-only
//! harness setting [`NothingTouched`] is on (the hub's ruling on review of #802: no touch begins or ends, one
//! harness-input line, [`NOTHING_TOUCHED_LINE`]); a world with one is [`TOUCH_RULE`] (`NotRead`, the overlap rule is
//! not read). The commit, in the note's order: `CollisionHeight` := Height, `CollisionRadius` := Radius (by fit), the
//! deferred reattach (`bNeedsReattach`), the event, then `bNetDirty` := true. A refusal after the first write is
//! prefixed.
//!
//! **`SetHardAttach`**: bound for a receiver whose first native class is `Actor`, `Trigger`, `InteractionProxy` or
//! `PawnInteractionProxy` ([`HARD_ATTACH_CLASSES`]); any other is [`HARD_ATTACH_UNRESOLVED`] (`NotRead`), decided
//! first. `bHardAttach` declared as a bool; an omitted argument is the current value. The same value: nothing. A change
//! with no `Base`: the flag. A change with a `Base`: the first `SetBase(None)`'s preflight and the flag's declaration
//! decide its refusals (no prefix); then `SetBase(None)`, the flag, and, if the receiver's `bDeleteMe` is now set (an
//! `InteractionProxy`'s `BaseChange` destroys a base-less proxy), [`DESTROYED`] with the prefix; otherwise
//! `SetBase(the old base, , the kept BaseSkelComponent and BaseBoneName)` with its full preflight, a refusal prefixed.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::component_setters::{self as setters, Slots, Want};

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_PAWN_INTERACTION_PROXY.md";
pub const SET_COLLISION_SIZE: &str = "Actor.SetCollisionSize(float,float)";
pub const SET_HARD_ATTACH: &str = "Actor.SetHardAttach(bool)";
const PACKAGE: &str = "Engine";
pub const TOUCH_UPDATE: &str = "Actor.SetCollisionSize: the touch update";
pub const TOUCH_LAYER: &str = "collision (the world has no collision service)";
pub const TOUCH_RULE: &str = "Actor.SetCollisionSize: the touch update (its overlap rule is not read)";
pub const EVENT_NOT_LOADED: &str = "Actor.SetCollisionSize: the code of CollisionChanged is not loaded";
pub const HARD_ATTACH_UNRESOLVED: &str = "Actor.SetHardAttach: the class's own implementation is not resolved";
pub const DESTROYED: &str = "Actor.SetHardAttach: the receiver was destroyed by its BaseChange";
/// The first native classes whose `SetHardAttach` the note read (section 4).
pub const HARD_ATTACH_CLASSES: [&str; 4] =
    ["Engine.Actor", "Engine.Trigger", "WillowGame.InteractionProxy", "WillowGame.PawnInteractionProxy"];
/// The harness line of the touch update's stand-in, once per call.
pub const NOTHING_TOUCHED_LINE: &str = "harness input: Actor.SetCollisionSize's touch update found nothing (a world \
     with no collision service; UNVERIFIED stand-in, the hub's ruling on review of #802)";
/// The set-up line of [`enable_nothing_touched`].
pub const NOTHING_TOUCHED_INPUT: &str = "harness input: the world turns on SetCollisionSize's nothing-touched stand-in \
     (the hub's ruling on review of #802; UNVERIFIED)";
const SIZE: &str = "Actor.SetCollisionSize";
const HARD: &str = "Actor.SetHardAttach";

/// The Off-only harness setting of the touch update's stand-in (a VM service; its presence turns it on).
#[derive(Debug, Clone, Copy, Default)]
pub struct NothingTouched;

/// Turns [`NothingTouched`] on, logging [`NOTHING_TOUCHED_INPUT`] (Off only, else a registration refusal; twice is a
/// registration refusal too).
pub fn enable_nothing_touched(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Err(VmError::Registration {
            key: "NothingTouched".to_string(),
            reason: "the touch update's stand-in is an Off harness setting; Reference binds nothing",
        });
    }
    vm.add_service(NothingTouched)?;
    vm.log.push(NOTHING_TOUCHED_INPUT.to_string());
    Ok(())
}

fn bad(what: &str, message: impl Into<String>) -> VmError {
    VmError::Script { function: what.to_string(), message: message.into() }
}

fn read(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<(Value, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(what, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(what, format!("{name} undeclared")))?;
    Ok((vm.space.read(o, ow_object::ids::PropAddr::scalar(p))?.clone(), kind))
}

fn object(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    match read(vm, o, name, what)? {
        (Value::Object(x), PropKind::Object { .. }) => Ok(x),
        _ => Err(bad(what, format!("{name} is not declared as an object"))),
    }
}

fn boolean(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<bool, VmError> {
    match read(vm, o, name, what)? {
        (Value::Bool(b), PropKind::Bool) => Ok(b),
        _ => Err(bad(what, format!("{name} is not declared as a bool"))),
    }
}

fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(format!("{}: an object the store has not loaded (a load is a write)", ow_attr::eval::class_path(&vm.space, o)), RECORD)
            .into());
    }
    Ok(())
}

fn is_a(vm: &Vm, o: ObjRef, wanted: &str) -> Result<bool, VmError> {
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    let mut steps = 0;
    while let Some(c) = cursor.filter(|_| steps < 256) {
        steps += 1;
        let Some(l) = vm.space.classes.get_layout(c) else { return Ok(false) };
        if l.path().as_str().eq_ignore_ascii_case(wanted) {
            return Ok(true);
        }
        cursor = l.super_class();
    }
    Ok(false)
}

fn marks(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

fn float_arg(c: &NativeCall<'_>, i: usize, what: &str) -> Result<f32, VmError> {
    match c.arg(i)? {
        Value::Float(f) => Ok(*f),
        other => Err(bad(what, format!("argument {i} holds {}, not a float", other.kind_name()))),
    }
}

// --- SetCollisionSize -----------------------------------------------------------------------------------------------

/// What `SetCollisionSize` will do, decided write-free and load-free (module documentation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizePlan {
    /// The cylinder to size, when `CollisionComponent` is one.
    pub cylinder: Option<ObjRef>,
    /// Whether `CollisionChanged` resolves to a function (else no event).
    pub event: bool,
    /// Whether the touch update is the stand-in (log its line).
    pub stand_in: bool,
}

/// [`SizePlan`] for `actor` (module documentation). Reads only.
pub fn plan_collision_size(vm: &Vm, actor: ObjRef) -> Result<SizePlan, VmError> {
    resident(vm, actor)?;
    let component = object(vm, actor, "CollisionComponent", SIZE)?;
    boolean(vm, actor, "bNetDirty", SIZE)?;
    let mut cylinder = None;
    if let Some(c) = component {
        resident(vm, c)?;
        if is_a(vm, c, "Engine.CylinderComponent")? {
            setters::schemas(vm, c, &[("CollisionHeight", Want::Float), ("CollisionRadius", Want::Float)], SIZE)?;
            setters::schemas(vm, c, &setters::REATTACH_FIELDS, SIZE)?;
            setters::reattach_check(vm, Slots::of(c), SIZE)?;
            cylinder = Some(c);
        }
    }
    let event = match ow_vm::states::find_loaded_event(vm, actor, "CollisionChanged")? {
        Some(f) => f.is_some(),
        None => return Err(NotYet::not_read(EVENT_NOT_LOADED, RECORD).into()),
    };
    let stand_in = match crate::placement::collision_model(vm)? {
        Some(_) => return Err(NotYet::not_read(TOUCH_RULE, RECORD).into()),
        None if vm.service::<NothingTouched>().is_some() => true,
        None => return Err(NotYet::layer_absent(TOUCH_UPDATE, TOUCH_LAYER, RECORD).into()),
    };
    Ok(SizePlan { cylinder, event, stand_in })
}

/// `SetCollisionSize(radius, height)` on `actor` (module documentation).
pub fn set_collision_size(vm: &mut Vm, actor: ObjRef, radius: f32, height: f32) -> Result<(), VmError> {
    let plan = plan_collision_size(vm, actor)?;
    let entry = marks(vm);
    let run = (|| -> Result<(), VmError> {
        if let Some(c) = plan.cylinder {
            setters::write(vm, c, "CollisionHeight", Value::Float(height), SIZE)?;
            setters::write(vm, c, "CollisionRadius", Value::Float(radius), SIZE)?;
            setters::write(vm, c, "bNeedsReattach", Value::Bool(true), SIZE)?;
        }
        if plan.stand_in {
            vm.log.push(NOTHING_TOUCHED_LINE.to_string());
        }
        if plan.event {
            crate::actors::send_event(vm, actor, "CollisionChanged", vec![])?;
        }
        setters::write(vm, actor, "bNetDirty", Value::Bool(true), SIZE)
    })();
    match run {
        Err(e) if marks(vm) != entry => Err(crate::pools::after_partial_writes(e)),
        other => other,
    }
}

fn collision_size_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let actor = c.this.ok_or_else(|| bad(SIZE, "called without an actor"))?;
    let (radius, height) = (float_arg(c, 0, SIZE)?, float_arg(c, 1, SIZE)?);
    set_collision_size(c.vm, actor, radius, height)?;
    Ok(Value::None)
}

// --- SetHardAttach --------------------------------------------------------------------------------------------------

/// `SetHardAttach(value)` on `actor` (module documentation); `None` is an omitted argument.
pub fn set_hard_attach(vm: &mut Vm, actor: ObjRef, value: Option<bool>) -> Result<(), VmError> {
    let native = super::set_base::first_native(vm, actor)?;
    if !native.as_deref().is_some_and(|n| HARD_ATTACH_CLASSES.iter().any(|c| c.eq_ignore_ascii_case(n))) {
        return Err(NotYet::not_read(HARD_ATTACH_UNRESOLVED, RECORD).into());
    }
    resident(vm, actor)?;
    let current = boolean(vm, actor, "bHardAttach", HARD)?;
    let new = value.unwrap_or(current);
    if new == current {
        return Ok(());
    }
    let Some(base) = object(vm, actor, "Base", HARD)? else {
        return setters::write(vm, actor, "bHardAttach", Value::Bool(new), HARD);
    };
    // The kept attachment, the first SetBase(None)'s preflight and the flag's declaration decide the refusals up front.
    let skel = object(vm, actor, "BaseSkelComponent", HARD)?;
    let bone = match read(vm, actor, "BaseBoneName", HARD)? {
        (v @ Value::Name(_), PropKind::Name) => v,
        _ => return Err(bad(HARD, "BaseBoneName is not declared as a name")),
    };
    boolean(vm, actor, "bDeleteMe", HARD)?;
    let none = Value::Name(ow_object::ids::NameId::NONE);
    super::set_base::preflight(vm, actor, None, None, &none)?;
    let entry = marks(vm);
    let run = (|| -> Result<(), VmError> {
        super::set_base::run(vm, actor, None, None, &none)?;
        setters::write(vm, actor, "bHardAttach", Value::Bool(new), HARD)?;
        if boolean(vm, actor, "bDeleteMe", HARD)? || crate::actors::is_destroyed(&vm.space, actor)? {
            return Err(NotYet::not_read(DESTROYED, RECORD).into());
        }
        super::set_base::run(vm, actor, Some(base), skel, &bone)
    })();
    match run {
        Err(e) if marks(vm) != entry => Err(crate::pools::after_partial_writes(e)),
        other => other,
    }
}

fn hard_attach_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let actor = c.this.ok_or_else(|| bad(HARD, "called without an actor"))?;
    let value = if c.arg_count() > 0 && c.has(0) {
        match c.arg(0)? {
            Value::Bool(b) => Some(*b),
            other => return Err(bad(HARD, format!("bNewHardAttach holds {}, not a bool", other.kind_name()))),
        }
    } else {
        None
    };
    set_hard_attach(c.vm, actor, value)?;
    Ok(Value::None)
}

/// Binds the two keys in Off (the caller binds in Off only; keep-existing).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if !vm.is_bound_in(PACKAGE, SET_COLLISION_SIZE) {
        vm.register_native_in(PACKAGE, SET_COLLISION_SIZE, std::rc::Rc::new(collision_size_native))?;
    }
    if !vm.is_bound_in(PACKAGE, SET_HARD_ATTACH) {
        vm.register_native_in(PACKAGE, SET_HARD_ATTACH, std::rc::Rc::new(hard_attach_native))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "proxy_natives_tests.rs"]
mod tests;
