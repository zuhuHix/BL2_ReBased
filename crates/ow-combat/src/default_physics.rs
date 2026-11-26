//! Owner: lane IMPL-SETDEFAULTPHYSICS-AND-MIND-RECALC (decision 2026-11-26-NNN). `WillowPawn.SetDefaultPhysics()` past
//! its gates, and `WillowPawn.StaticCanCloak()` / `StaticSetCanCloak(bool)`, in `ParityMode::Off`. AI-assisted (Claude
//! Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md section 2 (lane
//! ANALYST-SETDEFAULTPHYSICS-AND-SPAWN-BALANCE, #788 at its r1 fix 7494393) and NATIVE_MIND_ROUTE.md section 1 alone;
//! this lane did not read the executable. **Every rule is UNVERIFIED in the running game.**
//!
//! **The gates** are [`crate::hit::pools::set_default_physics`]'s, unchanged. Past them, **the preflight** (write-free
//! and load-free; each field read with its declared kind, else `VmError::Script`; each object read resident, else
//! [`NOT_LOADED`]):
//! 1. the archetype: the untouched copy the spawn recorded for this pawn ([`crate::hit::archetype_of`]); none is
//!    [`NO_ARCHETYPE`] (`NotRead`);
//! 2. the default physics, `WillowAIPawn`'s rule (another pawn class's default physics is not read: [`NOT_AI`]):
//!    no `AIClass` `PHYS_Falling`; the class's `bIsTurret` `PHYS_None`; no `MyWillowMind`, or a mind with no
//!    `WillowNav`, the class's `Physics`; else, the handle "flying" (its `FlightMode` neither 0 nor 4) or the class's
//!    `Physics` `PHYS_Flying`, `PHYS_Flying`, otherwise `PHYS_NavMeshWalking`. The modes are named by the pawn's
//!    declared `EPhysics`;
//! 3. `Actor.SetPhysics`' own preflight for that mode ([`ow_world::world::set_physics::preflight_check`]): its
//!    refusals (the base search, the nav-mesh check) come here, before any write;
//! 4. the collision copy is skipped only when both queries hold: query 1 the pawn's `bCloaked` (a byte, set when not
//!    0), asked first; query 2 only when it is set: false with the can-cloak flag clear or no `BodyClass`; else the
//!    cloak's body class (`BodyClass`, then `AnimationParentBody` while `bCloakUseParentData` and a parent), and with
//!    `bCloakingRequiresShields` false when the maximum shield is above 0 and the current below 0.0001; otherwise true.
//!    The shield: the `ShieldArmor` pool's `CurrentValue` / `MaxValue` when the reference names a live pool (the port's
//!    pool handle rule), else `ShieldVar` / `ShieldMaxVar`;
//! 5. the copy itself: the three flags (`bCollideActors`, `bBlockActors`, `bIgnoreEncroachers`) equal to the
//!    archetype's, nothing; otherwise `Actor.SetCollision`'s `NotRead` ([`SET_COLLISION`]).
//!
//! **The commit:** the physics set through the bound `Actor.SetPhysics` (its own rule and its own prefix), then
//! `bCollideWorld` from the archetype. `SetDefaultPhysics` writes nothing before the physics set, so a refusal decided
//! in the preflight carries no prefix.
//!
//! **The can-cloak flag:** a VM-wide value in the combat state, initially true; `StaticCanCloak` reads it and
//! `StaticSetCanCloak` writes it.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::hit::bind_fn;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md (section 2)";
pub const NO_ARCHETYPE: &str = "SetDefaultPhysics: the pawn's archetype";
pub const NOT_LOADED: &str = "SetDefaultPhysics: an object the store has not loaded (a load is a write)";
pub const NOT_AI: &str = "SetDefaultPhysics: the default physics of a pawn that is not a WillowAIPawn";
pub const SET_COLLISION: &str = "SetDefaultPhysics: the collision copy (Actor.SetCollision with a change)";
pub const CAN_CLOAK: &str = "WillowGame.WillowPawn.StaticCanCloak";
pub const SET_CAN_CLOAK: &str = "WillowGame.WillowPawn.StaticSetCanCloak";
const WHAT: &str = "WillowPawn.SetDefaultPhysics";
/// The two `EFlightMode` values that are not "flying" (the note's section 2).
const NOT_FLYING: [u8; 2] = [0, 4];

fn bad(message: impl Into<String>) -> VmError {
    VmError::Script { function: WHAT.to_string(), message: message.into() }
}

fn read(vm: &Vm, o: ObjRef, name: &str) -> Result<(Value, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(format!("{} declares no {name}", path(vm, o))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(format!("{name} undeclared")))?;
    Ok((vm.space.read(o, PropAddr::scalar(p))?.clone(), kind))
}

fn path(vm: &Vm, o: ObjRef) -> String {
    vm.space.heap.get(o).ok().and_then(|x| vm.space.classes.get_layout(x.class)).map_or_else(String::new, |l| l.path().to_string())
}

fn object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match read(vm, o, name)? {
        (Value::Object(x), PropKind::Object { .. }) => Ok(x),
        _ => Err(bad(format!("{name} is not declared as an object"))),
    }
}

fn boolean(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    match read(vm, o, name)? {
        (Value::Bool(b), PropKind::Bool) => Ok(b),
        _ => Err(bad(format!("{name} is not declared as a bool"))),
    }
}

fn byte(vm: &Vm, o: ObjRef, name: &str) -> Result<u8, VmError> {
    match read(vm, o, name)? {
        (Value::Byte(b), PropKind::Byte { .. }) => Ok(b),
        _ => Err(bad(format!("{name} is not declared as a byte"))),
    }
}

fn float(vm: &Vm, o: ObjRef, name: &str) -> Result<f32, VmError> {
    match read(vm, o, name)? {
        (Value::Float(f), PropKind::Float | PropKind::FloatAttribute) => Ok(f),
        _ => Err(bad(format!("{name} is not declared as a float"))),
    }
}

fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
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

/// The value of `EPhysics` entry `name`, by its place in the enum `Physics` is declared with on `o`.
fn physics_mode(vm: &Vm, o: ObjRef, name: &str) -> Result<u8, VmError> {
    let (_, kind) = read(vm, o, "Physics")?;
    let PropKind::Byte { enum_: Some(e) } = kind else {
        return Err(bad("Physics is not declared as a byte of an enum"));
    };
    let at = vm
        .space
        .classes
        .enum_layout(e)
        .and_then(|l| l.values().iter().position(|n| vm.space.names.text(*n).eq_ignore_ascii_case(name)))
        .ok_or_else(|| bad(format!("EPhysics has no {name}")))?;
    u8::try_from(at).map_err(|_| bad(format!("{name} is past a byte")))
}

/// The default physics (module documentation, step 2). Reads only.
fn default_physics(vm: &Vm, pawn: ObjRef) -> Result<u8, VmError> {
    if !is_a(vm, pawn, "WillowGame.WillowAIPawn")? {
        return Err(NotYet::not_read(format!("{NOT_AI} ({})", path(vm, pawn)), RECORD).into());
    }
    let mode = |name: &str| physics_mode(vm, pawn, name);
    let Some(class) = object(vm, pawn, "AIClass")? else {
        return mode("PHYS_Falling");
    };
    resident(vm, class)?;
    if boolean(vm, class, "bIsTurret")? {
        return mode("PHYS_None");
    }
    let class_physics = byte(vm, class, "Physics")?;
    let Some(mind) = object(vm, pawn, "MyWillowMind")? else {
        return Ok(class_physics);
    };
    resident(vm, mind)?;
    let Some(nav) = object(vm, mind, "WillowNav")? else {
        return Ok(class_physics);
    };
    resident(vm, nav)?;
    let flying = !NOT_FLYING.contains(&byte(vm, nav, "FlightMode")?);
    if !flying && class_physics != mode("PHYS_Flying")? {
        mode("PHYS_NavMeshWalking")
    } else {
        mode("PHYS_Flying")
    }
}

/// The shield's current and maximum (module documentation, step 4).
fn shield(vm: &Vm, pawn: ObjRef) -> Result<(f32, f32), VmError> {
    let (reference, _) = read(vm, pawn, "ShieldArmor")?;
    if let Ok(r) = ow_world::pools::read_ref(&vm.space, &reference)
        && let (_, Some(pool)) = ow_world::pools::resolve(&vm.space, r)?
    {
        resident(vm, pool)?;
        return Ok((float(vm, pool, "CurrentValue")?, float(vm, pool, "MaxValue")?));
    }
    Ok((float(vm, pawn, "ShieldVar")?, float(vm, pawn, "ShieldMaxVar")?))
}

/// Query 2 (module documentation, step 4): the cloak's shield requirement is met.
fn cloak_allowed(vm: &Vm, pawn: ObjRef) -> Result<bool, VmError> {
    if !crate::hit::with_state(vm, |s| s.can_cloak())? {
        return Ok(false);
    }
    let Some(mut body) = object(vm, pawn, "BodyClass")? else {
        return Ok(false);
    };
    resident(vm, body)?;
    for _ in 0..64 {
        if !boolean(vm, body, "bCloakUseParentData")? {
            break;
        }
        let Some(parent) = object(vm, body, "AnimationParentBody")? else { break };
        resident(vm, parent)?;
        body = parent;
    }
    if boolean(vm, body, "bCloakingRequiresShields")? {
        let (current, max) = shield(vm, pawn)?;
        if max > 0.0 && current < 0.0001 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// What the commit does.
struct Plan {
    mode: u8,
    collide_world: bool,
}

/// The preflight past the gates (module documentation). Reads only.
fn plan(vm: &Vm, pawn: ObjRef) -> Result<Plan, VmError> {
    let archetype = crate::hit::archetype_of(vm, pawn)?.ok_or_else(|| VmError::from(NotYet::not_read(NO_ARCHETYPE, RECORD)))?;
    resident(vm, archetype)?;
    let mode = default_physics(vm, pawn)?;
    ow_world::world::set_physics::preflight_check(vm, pawn, mode)?;
    let collide_world = boolean(vm, archetype, "bCollideWorld")?;
    boolean(vm, pawn, "bCollideWorld")?;
    let cloaked = byte(vm, pawn, "bCloaked")? != 0;
    let skip = cloaked && cloak_allowed(vm, pawn)?;
    if !skip {
        for f in ["bCollideActors", "bBlockActors", "bIgnoreEncroachers"] {
            if boolean(vm, pawn, f)? != boolean(vm, archetype, f)? {
                return Err(NotYet::not_read(SET_COLLISION, RECORD).into());
            }
        }
    }
    Ok(Plan { mode, collide_world })
}

/// `SetDefaultPhysics` past its gates (module documentation), Off.
pub fn past_the_gates(vm: &mut Vm, pawn: ObjRef) -> Result<(), VmError> {
    let plan = plan(vm, pawn)?;
    let set_physics = vm.find_function("Engine.Actor.SetPhysics")?;
    vm.call(set_physics, Some(pawn), vec![Value::Byte(plan.mode)])?;
    let class = vm.space.heap.get(pawn)?.class;
    let p = vm.space.prop(class, "bCollideWorld").ok_or_else(|| bad("no bCollideWorld"))?;
    vm.space.write(pawn, PropAddr::scalar(p), Value::Bool(plan.collide_world))?;
    Ok(())
}

/// Binds the can-cloak natives in Off (keep-existing).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    bind_fn(vm, CAN_CLOAK, |c| Ok(Value::Bool(crate::hit::with_state(c.vm, |s| s.can_cloak())?)))?;
    bind_fn(vm, SET_CAN_CLOAK, |c| {
        let on = match c.arg(0)? {
            Value::Bool(b) => *b,
            other => return Err(VmError::Script { function: SET_CAN_CLOAK.to_string(), message: format!("the flag is {}", other.kind_name()) }),
        };
        crate::hit::with_state(c.vm, |s| s.set_can_cloak(on))?;
        Ok(Value::None)
    })?;
    Ok(())
}

#[cfg(test)]
#[path = "default_physics_tests.rs"]
mod tests;
