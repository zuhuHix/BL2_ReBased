//! Owner: lane IMPL-PICKUP-RB. `WillowPickup.InitializePickupForRBPhysics()` in `ParityMode::Off`, coded from
//! docs/verification/NATIVE_PICKUP_RB_MINIMAL.md alone (#574, accepted at 7c212484; clean room: lane
//! ANALYST-PICKUP-RB-MINIMAL read the executable, this lane did not), with the hub's rulings on top. It replaces #520's
//! `LayerAbsent` refusal. AI-assisted (Claude Code). **Every rule here is UNVERIFIED in the running game.**
//!
//! **The preflight** (note section 3), write-free, using only loaded objects, in this order and checking only what the
//! chosen path needs:
//! 1. `bUseRBPhysics` (declaration, then value). Clear: nothing at all (#520's answer).
//! 2. C: `CollisionComponent` (declaration, value), a `PrimitiveComponent` or none.
//! 3. The pickup's fields steps 1, 2, 5, 6 and 7 use, and the world's timer service.
//! 4. With C only: G (the world's globals and its definition, with their own refusals), the declarations of C's,
//!    the pickup's and G's fields; with `CollideActors` clear, the deferred reattach check (refusals 3 and 4); the
//!    physics-asset test (refusals 2 and 5; `GetInventoryDefinition` through the write-free
//!    `inventory_definition_of`, its refusals passing through; `PhysicsAsset` read as a handle, never followed).
//!
//! **The hub's rulings** (on #574): the asset branch is answered **at rest**: its non-physics writes run (`PhysicsWeight`,
//! `PhysicsAsset`, `bHasPhysicsAssetInstance`), no rigid body is made, and the run's log says so (UNVERIFIED). That
//! replaces refusal 1. The spin's three draws come from `GameRng::engine_seeded`, a port stand-in stream for the
//! engine's seeded generator (UNVERIFIED: only the number of draws is claimed; the spin itself is an impulse on a body
//! the port does not have). That removes refusal 6. Refusals 2 to 5 stay, with the note's texts.
//!
//! **The writes** (note section 2), in order, once the preflight passes: `bForceRBToSleep` false and the two sleep
//! timers; `RotationRate` zero (fitted); with C: `Scale3D` (1, 1, 1), `BlockRigidBody`, `CollideActors` and the deferred
//! reattach's flag when it was clear, `BlockActors` false, the asset branch's three fields, with G the three impact
//! fields, `bNotifyRigidBodyCollision`, with G the collision threshold, `RBChannel` and the nine channel bits (the
//! container's bool fields of the channels' names, one write keeping every other bit); `Physics` `PHYS_RigidBody`;
//! with C the spin draws (`bTorque` clear, with G) or the torque's `bTorqueTemporary` clear; the
//! `EnableRagdollCollision` timer. Enum values are read by name from the declared enums. Nothing refuses after the
//! first write; a `NotYet` is marked "stopped after partial writes: " only when #518's counters moved.
//!
//! **Left out** (the empty physics layer, as the note and the hub's ruling describe it): the filter updates, the body's
//! wake and position, the physics mode change's own work and the impulses.

use ow_core::NotYet;
use ow_core::rand::frand_f32;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::{REATTACH_OWNER, REATTACH_SCENE, RECORD, Want, bool_of, fits, is_zero, schemas};
use crate::equip::{bad, object_of, required, write};

/// The record of this rule.
pub const RECORD_RB: &str = "docs/verification/NATIVE_PICKUP_RB_MINIMAL.md";
/// Refusal 2 (note section 4).
pub const NO_ASSET: &str = "WillowPickup.InitializeRBPhysics: the pickup's rigid-body set-up without a physics asset";
/// Refusal 5.
pub const UNLOADED_DEFINITION: &str =
    "InitializePickupForRBPhysics: the physics asset of an inventory definition that is not loaded";
const WHAT: &str = "WillowPickup.InitializePickupForRBPhysics";

/// The pickup's fields steps 1, 2, 5, 6 and 7 use (preflight step 3).
const PICKUP_FIELDS: [(&str, Want); 5] = [
    ("bForceRBToSleep", Want::Bool),
    ("RotationRate", Want::Struct),
    ("bTorque", Want::Bool),
    ("bTorqueTemporary", Want::Bool),
    ("Torque", Want::Struct),
];
/// C's fields step 4 reads or writes (the asset branch's checked when it is taken).
const COMPONENT_FIELDS: [(&str, Want); 9] = [
    ("Scale3D", Want::Struct),
    ("BlockRigidBody", Want::Bool),
    ("CollideActors", Want::Bool),
    ("BlockActors", Want::Bool),
    ("Owner", Want::Object),
    ("Scene", Want::Struct),
    ("bNeedsReattach", Want::Bool),
    ("bNotifyRigidBodyCollision", Want::Bool),
    ("RBCollideWithChannels", Want::Struct),
];
const ASSET_FIELDS: [(&str, Want); 2] = [("PhysicsAsset", Want::Object), ("bHasPhysicsAssetInstance", Want::Bool)];
/// The nine channel updates of step 4.8, in order: the channel's container field and its new value.
const CHANNELS: [(&str, bool); 9] = [
    ("WillowPickup", false),
    ("DeadPawn", false),
    ("Default", true),
    ("GameplayPhysics", true),
    ("EffectPhysics", true),
    ("TossedItems", true),
    ("TossedItemsPlayerVehicle", true),
    ("TossedItemsEnemyVehicle", true),
    ("TossedItemsPlayerVehicleEnemyVehicle", true),
];
const SPIN_FIELDS: [&str; 4] = ["PickupSpinVMin", "PickupSpinVMax", "PickupSpinHMin", "PickupSpinHMax"];

/// #518's canonical write counters, snapshot when the native is entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WriteCounters {
    heap: u64,
    draws: u64,
    loads: u64,
}

impl WriteCounters {
    fn of(vm: &Vm) -> Self {
        WriteCounters { heap: vm.space.heap.committed_writes(), draws: vm.rng.draws(), loads: vm.defs.load_transitions() }
    }

    fn mark(&self, vm: &Vm, e: VmError) -> VmError {
        match e {
            VmError::NotYet(n) if WriteCounters::of(vm) != *self => n.after_partial_writes().into(),
            other => other,
        }
    }
}

/// What the preflight settled for the writes.
struct Plan {
    component: Option<ObjRef>,
    globals: Option<ObjRef>,
    reattach: bool,
    asset: Option<ObjRef>,
    awake_time: f32,
}

fn float_of(v: &Value, name: &str) -> Result<f32, VmError> {
    match v {
        Value::Float(f) => Ok(*f),
        other => Err(bad(WHAT, format!("{name} holds {}, not a float", other.kind_name()))),
    }
}

fn is_a(vm: &Vm, o: ObjRef, path: &str) -> Result<bool, VmError> {
    let class = vm.space.heap.get(o)?.class;
    Ok(vm.space.classes.by_path(path).is_some_and(|c| vm.space.classes.is_child_of(class, c)))
}

/// An object the store has not loaded, or a slotless stand-in whose class has slots.
fn unloaded(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    let stand_in = x.slots.is_empty() && vm.space.classes.get_layout(x.class).is_some_and(|l| l.slot_count() > 0);
    Ok(vm.defs.is_unloaded(&vm.space, o) || stand_in)
}

/// The value of enum value `name` in the enum field `field` of `o` declares.
fn enum_value(vm: &Vm, o: ObjRef, field: &str, name: &str) -> Result<u8, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let missing = || bad(WHAT, format!("{field} declares no enum with {name}"));
    let p = vm.space.prop(class, field).ok_or_else(missing)?;
    let Some(PropKind::Byte { enum_: Some(e) }) = vm.space.classes.decl(p).map(|d| d.kind.clone()) else {
        return Err(missing());
    };
    let layout = vm.space.classes.enum_layout(e).ok_or_else(missing)?;
    let i = layout.values().iter().position(|v| vm.space.names.text(*v) == name).ok_or_else(missing)?;
    u8::try_from(i).map_err(|_| missing())
}

/// The value `v` with each named field replaced (a struct value), else a data error.
fn with_fields(vm: &Vm, v: &Value, fields: &[(&str, Value)], what: &str) -> Result<Value, VmError> {
    let mut out = v.clone();
    let ty = out.as_struct().ok_or_else(|| bad(WHAT, format!("{what} is not a struct")))?.ty;
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(WHAT, format!("{what}: an unknown struct")))?;
    let mut slots = Vec::new();
    for (name, _) in fields {
        let id = vm.space.names.find(name).ok_or_else(|| bad(WHAT, format!("{what} has no {name}")))?;
        let (i, _) = layout.field(id).ok_or_else(|| bad(WHAT, format!("{what} has no {name}")))?;
        slots.push(i as usize);
    }
    let s = out.struct_mut().expect("a struct, checked above");
    for (slot, (_, value)) in slots.into_iter().zip(fields) {
        *s.fields.get_mut(slot).ok_or_else(|| bad(WHAT, format!("{what}: past its fields")))? = value.clone();
    }
    Ok(out)
}

/// A struct value with every int and float field zeroed, at any depth (`RotationRate`'s zero constant, fitted).
fn zeroed(v: &Value) -> Value {
    match v {
        Value::Int(_) => Value::Int(0),
        Value::Float(_) => Value::Float(0.0),
        Value::Struct(_) => {
            let mut out = v.clone();
            if let Some(s) = out.struct_mut() {
                for f in &mut s.fields {
                    *f = zeroed(f);
                }
            }
            out
        }
        other => other.clone(),
    }
}

/// The preflight (module documentation). Reads only.
fn preflight(vm: &mut Vm, pickup: ObjRef) -> Result<Option<Plan>, VmError> {
    // 1. bUseRBPhysics.
    schemas(vm, pickup, &[("bUseRBPhysics", Want::Bool)], WHAT)?;
    if !bool_of(&required(vm, pickup, "bUseRBPhysics", WHAT)?, "bUseRBPhysics", WHAT)? {
        return Ok(None);
    }
    // 2. C.
    schemas(vm, pickup, &[("CollisionComponent", Want::Object)], WHAT)?;
    let component = match object_of(&required(vm, pickup, "CollisionComponent", WHAT)?, WHAT)? {
        Some(c) if is_a(vm, c, "Engine.PrimitiveComponent")? => Some(c),
        _ => None,
    };
    // 3. The pickup's fields of steps 1, 2, 5, 6 and 7, and the timer service.
    schemas(vm, pickup, &PICKUP_FIELDS, WHAT)?;
    let awake_time = float_of(&required(vm, pickup, "MaxRBAwakeTime", WHAT)?, "MaxRBAwakeTime")?;
    enum_value(vm, pickup, "Physics", "PHYS_RigidBody")?;
    ow_world::timers::timers_of(vm)?;
    let Some(c) = component else {
        return Ok(Some(Plan { component: None, globals: None, reattach: false, asset: None, awake_time }));
    };
    // 4.1. G, as the pickup world's set-up prepared it.
    let world = crate::globals::world_globals(vm)?;
    let globals = Some(crate::globals::globals_definition(vm, world, WHAT)?);
    // 4.2. The declarations.
    schemas(vm, c, &COMPONENT_FIELDS, WHAT)?;
    schemas(vm, pickup, &[("MaxImpactEffectPlayCount", Want::Int), ("Inventory", Want::Object)], WHAT)?;
    for f in ["Scale3D", "RBCollideWithChannels"] {
        let v = required(vm, c, f, WHAT)?;
        let fields: Vec<(&str, Value)> = match f {
            "Scale3D" => vec![("X", Value::Float(1.0)), ("Y", Value::Float(1.0)), ("Z", Value::Float(1.0))],
            _ => CHANNELS.iter().map(|&(n, b)| (n, Value::Bool(b))).collect(),
        };
        with_fields(vm, &v, &fields, f)?;
    }
    enum_value(vm, c, "RBChannel", "RBCC_WillowPickup")?;
    if let Some(g) = globals {
        for f in ["DropImpactEffectMax", "DropImpactEffectThreshold"] {
            float_of(&required(vm, g, f, WHAT)?, f)?;
        }
        for (from, to) in [("DropImpactEffectDelay", "ImpactEffectDelay"), ("DropImpactEffectVelocityAkRtpc", "ImpactVelocityAkRtpc")] {
            let v = required(vm, g, from, WHAT)?;
            fits(vm, pickup, to, &v, WHAT)?;
        }
        fits(vm, c, "ScriptRigidBodyCollisionThresholdCompiled", &Value::Byte(0), WHAT)?;
        for f in SPIN_FIELDS {
            float_of(&required(vm, g, f, WHAT)?, f)?;
        }
    }
    // 4.3. The deferred reattach, with CollideActors clear (refusals 3 and 4).
    let reattach = !bool_of(&required(vm, c, "CollideActors", WHAT)?, "CollideActors", WHAT)?;
    if reattach {
        match object_of(&required(vm, c, "Owner", WHAT)?, WHAT)? {
            Some(owner) => {
                let flag = |vm: &Vm, n: &str| -> Result<bool, VmError> { bool_of(&required(vm, owner, n, WHAT)?, n, WHAT) };
                if (flag(vm, "bStatic")? || flag(vm, "bTickIsDisabled")?) && !flag(vm, "bDeleteMe")? {
                    return Err(NotYet::not_read(REATTACH_OWNER, RECORD).into());
                }
            }
            None => {
                if !is_zero(&required(vm, c, "Scene", WHAT)?, WHAT)? {
                    return Err(NotYet::not_read(REATTACH_SCENE, RECORD).into());
                }
            }
        }
    }
    // 4.4. The physics-asset test (refusals 2 and 5).
    let no_asset = || -> VmError { NotYet::not_read(NO_ASSET, RECORD_RB).into() };
    if !is_a(vm, c, "Engine.SkeletalMeshComponent")? {
        return Err(no_asset());
    }
    let Some(inventory) = object_of(&required(vm, pickup, "Inventory", WHAT)?, WHAT)? else {
        return Err(no_asset());
    };
    let Some(definition) = crate::definition::inventory_definition_of(vm, inventory)? else {
        return Err(no_asset());
    };
    if unloaded(vm, definition)? {
        return Err(NotYet::not_read(UNLOADED_DEFINITION, RECORD_RB).into());
    }
    let Some(asset) = object_of(&required(vm, definition, "PhysicsAsset", WHAT)?, WHAT)? else {
        return Err(no_asset());
    };
    schemas(vm, c, &ASSET_FIELDS, WHAT)?;
    float_of(&required(vm, c, "PhysicsWeight", WHAT)?, "PhysicsWeight")?;
    fits(vm, c, "PhysicsAsset", &Value::Object(Some(asset)), WHAT)?;
    Ok(Some(Plan { component: Some(c), globals, reattach, asset: Some(asset), awake_time }))
}

/// `InitializePickupForRBPhysics()` on `pickup` in Off (module documentation).
pub fn initialize(vm: &mut Vm, pickup: ObjRef) -> Result<(), VmError> {
    let start = WriteCounters::of(vm);
    let result = (|| -> Result<(), VmError> {
        let Some(plan) = preflight(vm, pickup)? else {
            return Ok(());
        };
        writes(vm, pickup, &plan)
    })();
    result.map_err(|e| start.mark(vm, e))
}

fn set_timer(vm: &mut Vm, pickup: ObjRef, rate: f32, looping: bool, func: &str) -> Result<(), VmError> {
    let name = vm.space.names.intern(func);
    let timers = ow_world::timers::timers_of(vm)?;
    timers.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("Timers"))?.set(pickup, rate, looping, name, pickup);
    Ok(())
}

/// The writes, in note section 2's order (module documentation).
fn writes(vm: &mut Vm, pickup: ObjRef, plan: &Plan) -> Result<(), VmError> {
    // 1. The sleep set-up.
    write(vm, pickup, "bForceRBToSleep", Value::Bool(false), WHAT)?;
    set_timer(vm, pickup, plan.awake_time, false, "MaxRBAwakeTimer")?;
    set_timer(vm, pickup, 1.0, true, "CheckForRigidBodySleepState")?;
    // 2. RotationRate, the zero constant (fitted).
    let rate = zeroed(&required(vm, pickup, "RotationRate", WHAT)?);
    write(vm, pickup, "RotationRate", rate, WHAT)?;
    // 4. With C.
    if let Some(c) = plan.component {
        let scale = required(vm, c, "Scale3D", WHAT)?;
        let one = with_fields(vm, &scale, &[("X", Value::Float(1.0)), ("Y", Value::Float(1.0)), ("Z", Value::Float(1.0))], "Scale3D")?;
        write(vm, c, "Scale3D", one, WHAT)?;
        write(vm, c, "BlockRigidBody", Value::Bool(true), WHAT)?;
        if plan.reattach {
            write(vm, c, "CollideActors", Value::Bool(true), WHAT)?;
            write(vm, c, "bNeedsReattach", Value::Bool(true), WHAT)?;
        }
        write(vm, c, "BlockActors", Value::Bool(false), WHAT)?;
        // 4.5. The asset branch, at rest (the hub's ruling): its non-physics writes, no rigid body.
        if let Some(asset) = plan.asset {
            write(vm, c, "PhysicsWeight", Value::Float(1.0), WHAT)?;
            write(vm, c, "PhysicsAsset", Value::Object(Some(asset)), WHAT)?;
            write(vm, c, "bHasPhysicsAssetInstance", Value::Bool(true), WHAT)?;
            vm.log.push(
                "InitializePickupForRBPhysics: the physics asset branch answered at rest: no rigid body is made (the hub's ruling, UNVERIFIED)"
                    .to_string(),
            );
        }
        // 4.6. The impact fields, with G.
        if let Some(g) = plan.globals {
            let max = float_of(&required(vm, g, "DropImpactEffectMax", WHAT)?, "DropImpactEffectMax")?;
            write(vm, pickup, "MaxImpactEffectPlayCount", Value::Int(ow_core::num::trunc_i32(max)), WHAT)?;
            let delay = required(vm, g, "DropImpactEffectDelay", WHAT)?;
            write(vm, pickup, "ImpactEffectDelay", delay, WHAT)?;
            let rtpc = required(vm, g, "DropImpactEffectVelocityAkRtpc", WHAT)?;
            write(vm, pickup, "ImpactVelocityAkRtpc", rtpc, WHAT)?;
        }
        // 4.7. The collision notify, then the threshold with G.
        write(vm, c, "bNotifyRigidBodyCollision", Value::Bool(true), WHAT)?;
        if let Some(g) = plan.globals {
            let t = float_of(&required(vm, g, "DropImpactEffectThreshold", WHAT)?, "DropImpactEffectThreshold")?;
            let byte = ow_core::num::trunc_i32(t * 0.25) as u8;
            write(vm, c, "ScriptRigidBodyCollisionThresholdCompiled", Value::Byte(byte), WHAT)?;
        }
        // 4.8. The channel and the nine bits (one write keeping every other bit).
        let channel = enum_value(vm, c, "RBChannel", "RBCC_WillowPickup")?;
        write(vm, c, "RBChannel", Value::Byte(channel), WHAT)?;
        let bits = required(vm, c, "RBCollideWithChannels", WHAT)?;
        let fields: Vec<(&str, Value)> = CHANNELS.iter().map(|&(n, b)| (n, Value::Bool(b))).collect();
        let bits = with_fields(vm, &bits, &fields, "RBCollideWithChannels")?;
        write(vm, c, "RBCollideWithChannels", bits, WHAT)?;
    }
    // 5. The physics mode.
    let mode = enum_value(vm, pickup, "Physics", "PHYS_RigidBody")?;
    write(vm, pickup, "Physics", Value::Byte(mode), WHAT)?;
    // 6. The spin or the torque.
    if plan.component.is_some() {
        if bool_of(&required(vm, pickup, "bTorque", WHAT)?, "bTorque", WHAT)? {
            if bool_of(&required(vm, pickup, "bTorqueTemporary", WHAT)?, "bTorqueTemporary", WHAT)? {
                write(vm, pickup, "bTorque", Value::Bool(false), WHAT)?;
            }
        } else if plan.globals.is_some() {
            // Three draws on the port's stand-in for the engine's seeded generator (UNVERIFIED; the values feed only
            // the impulse, which the port does not apply).
            for _ in 0..3 {
                let _ = frand_f32(vm.rng.engine_seeded());
            }
        }
    }
    // 7. The ragdoll timer.
    set_timer(vm, pickup, 0.3, false, "EnableRagdollCollision")
}

#[cfg(test)]
#[path = "pickup_rb_tests.rs"]
mod tests;
