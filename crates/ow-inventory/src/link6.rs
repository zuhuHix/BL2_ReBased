//! Owner: lane IMPL-LINK6-AFTER-TAKE (decision 2026-11-26-NNN). Link 6's take after `SetBase`, in `ParityMode::Off`.
//! AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_ACTOR_SETBASE.md section 4 (#795) and
//! NATIVE_LINK6_AFTER_TAKE.md section 2 (#797), as landed, alone; this lane did not read the executable. **Every rule
//! is UNVERIFIED in the running game.** Reference binds nothing. Each reads only unless it says so; each field read with
//! its declared kind, else `VmError::Script`; each object read resident, else `NotRead`.
//!
//! - **`WillowWeapon.GetAmmoCount()`**: the `InvManager`'s `bInfiniteAmmo` gives `ClipSize`; else a live `AmmoPool`
//!   (resolved on a copy, the cache not written back: a gap) its `CurrentValue` truncated; else `bUseOverheatBehavior`
//!   gives `OverheatAmmo`; else `StoredAmmo`.
//! - **`AttributeExpression.SelectAkEvent(ContextSource, SoundList)`** (static): the first entry whose non-empty
//!   `Expressions` hold (And, the source, no override) is the answer; an entry with an empty list is the fallback, a
//!   later one replacing it; the answer is the last fallback, or None. `Cue` is not read.
//! - **`Actor.PlayAkEvent(InSoundCue, …)`**: a None event, nothing; a set event, [`AUDIO`] (`LayerAbsent`), unless the
//!   world turned on the audio stand-in ([`enable_audio_stand_in`], Off only, a harness setting as `PhysXLevel` and
//!   `AtRestBodyPose` are; the hub's ruling, UNVERIFIED): then nothing is played, the default handle is returned, and
//!   [`AUDIO_LINE`] is logged per call.
//! - **`WillowGlobals.RemovePickup(Pickup)`**: a None pickup is `VmError::Script`; with a `Base`, whether the base's
//!   class provides `GearboxFramework.IBehaviorConsumer` (by names and classes already in the tables, nothing interned
//!   or loaded): unresolved, [`CONSUMER_UNRESOLVED`]; provided, [`CONSUMER_EVENTS`] (both `NotRead`, before any write;
//!   the consumer's event call is not ported); then every entry equal to the pickup removed from `PickupList`, the
//!   others kept in order (one write).

use std::rc::Rc;

use ow_attr::api::ExprMode;
use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

/// The records of the rules.
pub const RECORD_SETBASE: &str = "docs/verification/NATIVE_ACTOR_SETBASE.md (section 4)";
pub const RECORD_AFTER: &str = "docs/verification/NATIVE_LINK6_AFTER_TAKE.md (section 2)";
pub const GET_AMMO_COUNT: &str = "WillowGame.WillowWeapon.GetAmmoCount";
pub const SELECT_AK_EVENT: &str = "Engine.AttributeExpression.SelectAkEvent";
pub const PLAY_AK_EVENT: &str = "Engine.Actor.PlayAkEvent";
pub const REMOVE_PICKUP: &str = "WillowGame.WillowGlobals.RemovePickup";
pub const NOT_LOADED: &str = "an object the store has not loaded (a load is a write)";
pub const AUDIO: &str = "PlayAkEvent: an audio event played on an actor";
pub const AUDIO_LAYER: &str = "an audio layer";
pub const AUDIO_LINE: &str = "harness input: PlayAkEvent: audio is not modelled: the event is not played (hub ruling; UNVERIFIED)";
pub const AUDIO_INPUT: &str =
    "harness input: the world turns on PlayAkEvent's audio stand-in (the hub's ruling on NATIVE_ACTOR_SETBASE.md 4.2; UNVERIFIED)";
pub const CONSUMER_UNRESOLVED: &str = "RemovePickup: the behavior-consumer interface is not resolved";
pub const CONSUMER_EVENTS: &str = "RemovePickup: the base's behavior-consumer events";
const CONSUMER: &str = "GearboxFramework.IBehaviorConsumer";

fn bad(what: &str, message: impl Into<String>) -> VmError {
    VmError::Script { function: what.to_string(), message: message.into() }
}

fn read(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<(Value, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(what, format!("declares no {name}")))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(what, format!("{name} undeclared")))?;
    Ok((vm.space.read(o, PropAddr::scalar(p))?.clone(), kind))
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

/// An int or float field as an int (a float truncated).
fn number(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<i32, VmError> {
    match read(vm, o, name, what)? {
        (Value::Int(i), PropKind::Int | PropKind::IntAttribute) => Ok(i),
        (Value::Float(f), PropKind::Float | PropKind::FloatAttribute) => Ok(f as i32),
        (Value::Byte(b), PropKind::Byte { .. }) => Ok(i32::from(b)),
        _ => Err(bad(what, format!("{name} is not declared as a number"))),
    }
}

fn resident(vm: &Vm, o: ObjRef, record: &'static str) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(NOT_LOADED, record).into());
    }
    Ok(())
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without a receiver"))
}

// --- GetAmmoCount ---------------------------------------------------------------------------------------------------

/// `WillowWeapon.GetAmmoCount()` (module documentation). Reads only.
pub fn ammo_count(vm: &Vm, weapon: ObjRef) -> Result<i32, VmError> {
    const WHAT: &str = "WillowWeapon.GetAmmoCount";
    resident(vm, weapon, RECORD_SETBASE)?;
    for f in ["StoredAmmo", "OverheatAmmo", "ClipSize"] {
        number(vm, weapon, f, WHAT)?;
    }
    boolean(vm, weapon, "bUseOverheatBehavior", WHAT)?;
    let (reference, _) = read(vm, weapon, "AmmoPool", WHAT)?;
    if let Some(manager) = object(vm, weapon, "InvManager", WHAT)? {
        resident(vm, manager, RECORD_SETBASE)?;
        if boolean(vm, manager, "bInfiniteAmmo", WHAT)? {
            return number(vm, weapon, "ClipSize", WHAT);
        }
    }
    let r = ow_world::pools::read_ref(&vm.space, &reference)?;
    if let (_, Some(pool)) = ow_world::pools::resolve(&vm.space, r)? {
        resident(vm, pool, RECORD_SETBASE)?;
        return number(vm, pool, "CurrentValue", WHAT);
    }
    if boolean(vm, weapon, "bUseOverheatBehavior", WHAT)? {
        return number(vm, weapon, "OverheatAmmo", WHAT);
    }
    number(vm, weapon, "StoredAmmo", WHAT)
}

// --- SelectAkEvent --------------------------------------------------------------------------------------------------

fn struct_field(vm: &Vm, v: &Value, name: &str, what: &str) -> Result<Value, VmError> {
    let s = v.as_struct().ok_or_else(|| bad(what, "a sound entry is not a struct"))?;
    let id = vm.space.names.find(name).ok_or_else(|| bad(what, format!("no {name}")))?;
    let (i, d) = vm
        .space
        .classes
        .struct_layout(s.ty)
        .and_then(|l| l.field(id).map(|(i, d)| (i, d.kind.clone())))
        .ok_or_else(|| bad(what, format!("the sound entry declares no {name}")))?;
    let v = s.fields.get(i as usize).cloned().ok_or_else(|| bad(what, format!("no {name} slot")))?;
    match (name, &d, &v) {
        ("Event", PropKind::Object { .. }, Value::Object(_)) => Ok(v),
        ("Expressions", PropKind::Array(inner), Value::Array(_)) if matches!(**inner, PropKind::Struct(_)) => Ok(v),
        _ => Err(bad(what, format!("{name} is not declared as expected"))),
    }
}

/// `SelectAkEvent` over the script list `list` (module documentation).
pub fn select_ak_event(vm: &mut Vm, source: Option<ObjRef>, list: &[Value]) -> Result<Option<ObjRef>, VmError> {
    const WHAT: &str = "AttributeExpression.SelectAkEvent";
    let mut fallback = None;
    for entry in list {
        let event = match struct_field(vm, entry, "Event", WHAT)? {
            Value::Object(o) => o,
            _ => None,
        };
        let rows = match struct_field(vm, entry, "Expressions", WHAT)? {
            Value::Array(rows) => rows,
            _ => unreachable!(),
        };
        if rows.is_empty() {
            fallback = event;
            continue;
        }
        if crate::pickup_take::expressions(vm, ExprMode::And, source, &rows, None)? {
            return Ok(event);
        }
    }
    Ok(fallback)
}

fn select_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "AttributeExpression.SelectAkEvent";
    let source = match c.arg(0)? {
        Value::Object(o) => *o,
        Value::None => None,
        other => return Err(bad(WHAT, format!("ContextSource holds {}", other.kind_name()))),
    };
    let list = match c.arg(1)? {
        Value::Array(items) => items.clone(),
        other => return Err(bad(WHAT, format!("SoundList holds {}", other.kind_name()))),
    };
    Ok(Value::Object(select_ak_event(c.vm, source, &list)?))
}

// --- PlayAkEvent ----------------------------------------------------------------------------------------------------

/// The audio stand-in's harness setting (a VM service; its presence turns it on).
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioStandIn;

/// Turns the audio stand-in on (world set-up; Off only, else a registration refusal; twice is a refusal too). Logs
/// [`AUDIO_INPUT`].
pub fn enable_audio_stand_in(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Err(VmError::Registration {
            key: "AudioStandIn".to_string(),
            reason: "the audio stand-in is an Off harness setting; Reference keeps the LayerAbsent",
        });
    }
    vm.add_service(AudioStandIn)?;
    vm.log.push(AUDIO_INPUT.to_string());
    Ok(())
}

fn play_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "Actor.PlayAkEvent";
    this(c, WHAT)?;
    let event = match c.arg(0)? {
        Value::Object(o) => *o,
        Value::None => None,
        other => return Err(bad(WHAT, format!("InSoundCue holds {}", other.kind_name()))),
    };
    let default = || -> Result<Value, VmError> {
        match c.vm.function(c.function).and_then(|d| d.result.as_ref()).map(|r| r.kind.clone()) {
            Some(kind) => Ok(c.vm.space.classes.default_value(&kind).map_err(ow_object::heap::HeapError::from)?),
            None => Ok(Value::None),
        }
    };
    if event.is_none() {
        return default();
    }
    if c.vm.service::<AudioStandIn>().is_none() {
        return Err(NotYet::layer_absent(AUDIO, AUDIO_LAYER, RECORD_SETBASE).into());
    }
    let v = default()?;
    c.vm.log.push(AUDIO_LINE.to_string());
    Ok(v)
}

// --- RemovePickup ---------------------------------------------------------------------------------------------------

fn remove_pickup(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowGlobals.RemovePickup";
    let globals = this(c, WHAT)?;
    let pickup = match c.arg(0)? {
        Value::Object(Some(p)) => *p,
        Value::Object(None) | Value::None => return Err(bad(WHAT, "a None pickup (the game reads its Base unchecked)")),
        other => return Err(bad(WHAT, format!("Pickup holds {}", other.kind_name()))),
    };
    let vm: &Vm = c.vm;
    crate::globals::pickup_list_declared(vm, globals, WHAT)?;
    resident(vm, pickup, RECORD_AFTER)?;
    if let Some(base) = object(vm, pickup, "Base", WHAT)? {
        resident(vm, base, RECORD_AFTER)?;
        match read(vm, base, "Attached", WHAT)? {
            (_, PropKind::Array(inner)) if matches!(*inner, PropKind::Object { .. }) => {}
            _ => return Err(bad(WHAT, "Attached is not declared as an array of objects")),
        }
        // The interface by names and classes already in the tables (nothing interned, no class code loaded).
        let iface = vm.space.names.find("IBehaviorConsumer").and_then(|_| vm.space.classes.by_path(CONSUMER));
        let Some(iface) = iface else {
            return Err(NotYet::not_read(CONSUMER_UNRESOLVED, RECORD_AFTER).into());
        };
        let class = vm.space.heap.get(base)?.class;
        match vm.implements_read_only(class, iface)? {
            Some(true) => return Err(NotYet::not_read(CONSUMER_EVENTS, RECORD_AFTER).into()),
            Some(false) => {}
            None => return Err(NotYet::not_read(CONSUMER_UNRESOLVED, RECORD_AFTER).into()),
        }
    }
    let list = crate::globals::pickup_list(vm, globals, WHAT)?;
    let kept: Vec<Value> = list.into_iter().filter(|v| *v != Value::Object(Some(pickup))).collect();
    let class = c.vm.space.heap.get(globals)?.class;
    let p = c.vm.space.prop(class, "PickupList").ok_or_else(|| bad(WHAT, "no PickupList"))?;
    c.vm.space.write(globals, PropAddr::scalar(p), Value::array(kept))?;
    Ok(Value::None)
}

/// Binds `f` to the native at `path` when the packages declare it native and nothing bound it yet.
fn bind(vm: &mut Vm, path: &str, f: NativeFn) -> Result<(), VmError> {
    let Ok(id) = vm.find_function(path) else { return Ok(()) };
    if !vm.function(id).is_some_and(|d| d.is_native()) || vm.is_bound(id) {
        return Ok(());
    }
    vm.register_native_for(id, f)
}

/// Binds the four natives in Off (the caller checks the mode; keep-existing).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    bind(vm, GET_AMMO_COUNT, Rc::new(|c: &mut NativeCall<'_>| {
        let weapon = this(c, "WillowWeapon.GetAmmoCount")?;
        Ok(Value::Int(ammo_count(c.vm, weapon)?))
    }))?;
    bind(vm, SELECT_AK_EVENT, Rc::new(select_native))?;
    bind(vm, PLAY_AK_EVENT, Rc::new(play_native))?;
    bind(vm, REMOVE_PICKUP, Rc::new(remove_pickup))?;
    Ok(())
}

#[cfg(test)]
#[path = "link6_tests.rs"]
mod tests;
