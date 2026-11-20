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
//! 4. With C only: G (the world's globals and its definition, with their own refusals), the declarations of C's and
//!    the pickup's fields; with `CollideActors` clear, the deferred reattach check (refusals 3 and 4); the
//!    physics-asset test (refusals 2 and 5; `GetInventoryDefinition` through the write-free
//!    `inventory_definition_of`, its refusals passing through; `PhysicsAsset` read as a handle, never followed); then
//!    the declarations of G's four impact fields the path copies or reads (`DropImpactEffectMax`,
//!    `DropImpactEffectDelay`, `DropImpactEffectVelocityAkRtpc`, `DropImpactEffectThreshold`), and on the spin's
//!    branch its four range fields.
//!
//! Each path's declarations are checked before that path's first write, and only on the path that reads them (the
//! hub's ruling on #625 r1, following note section 3): a malformed declaration on the chosen path is a data error
//! with nothing written, and one on a path not taken is never read.
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
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::{REATTACH_OWNER, REATTACH_SCENE, RECORD, Want, bool_of, is_zero, schemas};
use crate::equip::{bad, object_of, required};

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
const COMPONENT_FIELDS: [(&str, Want); 6] = [
    ("Scale3D", Want::Struct),
    ("BlockRigidBody", Want::Bool),
    ("CollideActors", Want::Bool),
    ("BlockActors", Want::Bool),
    ("bNotifyRigidBodyCollision", Want::Bool),
    ("RBCollideWithChannels", Want::Struct),
];
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
/// G's fields the path with C copies or reads, checked once its refusals have passed (#625 r1).
const IMPACT_FIELDS: [(&str, Want); 4] = [
    ("DropImpactEffectMax", Want::Float),
    ("DropImpactEffectDelay", Want::Float),
    ("DropImpactEffectVelocityAkRtpc", Want::Object),
    ("DropImpactEffectThreshold", Want::Float),
];

/// #518's canonical write counters, snapshot when the native is entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WriteCounters {
    heap: u64,
    draws: u64,
    loads: u64,
}

impl WriteCounters {
    pub(super) fn of(vm: &Vm) -> Self {
        WriteCounters { heap: vm.space.heap.committed_writes(), draws: vm.rng.draws(), loads: vm.defs.load_transitions() }
    }

    pub(super) fn mark(&self, vm: &Vm, e: VmError) -> VmError {
        match e {
            VmError::NotYet(n) if WriteCounters::of(vm) != *self => n.after_partial_writes().into(),
            other => other,
        }
    }
}

/// One step of the commit, each settled and checked by the preflight (review r1 of #587): a write whose address and
/// value the heap's own rules accepted, a timer, the spin's draws, or the asset branch's log line.
enum Step {
    Write(ObjRef, PropAddr, Value),
    Timer(f32, bool, &'static str),
    Draws(u32),
    Log(&'static str),
}

const AT_REST: &str = "InitializePickupForRBPhysics: the physics asset branch answered at rest: no rigid body is made (the hub's ruling, UNVERIFIED)";

/// A planned write of `v` to `name` on `o`: the object's slot read at that address and `v` checked against the
/// declaration by the heap's own write rules, so the commit's write cannot fail (review r1 of #587, item 2).
fn planned(vm: &Vm, o: ObjRef, name: &str, v: Value) -> Result<Step, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| bad(WHAT, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    let a = PropAddr::scalar(p);
    vm.space.read(o, a).map_err(|e| bad(WHAT, format!("{name} has no slot to write: {e}")))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(WHAT, format!("{name} has no declaration")))?;
    vm.space
        .heap
        .check_value(&vm.space.classes, &kind, &v)
        .map_err(|e| bad(WHAT, format!("{name} cannot take the value: {e}")))?;
    Ok(Step::Write(o, a, v))
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

fn flag(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    bool_of(&required(vm, o, name, WHAT)?, name, WHAT)
}

/// What the preflight settled with C: its globals definition, whether the reattach runs, the asset.
struct WithComponent {
    c: ObjRef,
    g: ObjRef,
    reattach: bool,
    asset: ObjRef,
}

/// The refusal checks with C, in the note's order (section 3, step 4), each field checked only on the branch that
/// uses it (review r1 of #587, item 3).
fn component_checks(vm: &mut Vm, pickup: ObjRef, c: ObjRef) -> Result<WithComponent, VmError> {
    // 4.1. G, as the pickup world's set-up prepared it, with no class load (review r1 of #587, item 1).
    let world = crate::globals::world_globals(vm)?;
    let g = crate::globals::globals_definition_loaded(vm, world, WHAT)?;
    // 4.2. The declarations of the fields every path with C uses.
    schemas(vm, c, &COMPONENT_FIELDS, WHAT)?;
    schemas(vm, pickup, &[("MaxImpactEffectPlayCount", Want::Int), ("Inventory", Want::Object)], WHAT)?;
    // 4.3. The deferred reattach, with CollideActors clear (refusals 3 and 4); its fields only then.
    let reattach = !flag(vm, c, "CollideActors")?;
    if reattach {
        schemas(vm, c, &[("Owner", Want::Object), ("Scene", Want::Struct), ("bNeedsReattach", Want::Bool)], WHAT)?;
        match object_of(&required(vm, c, "Owner", WHAT)?, WHAT)? {
            Some(owner) => {
                if (flag(vm, owner, "bStatic")? || flag(vm, owner, "bTickIsDisabled")?) && !flag(vm, owner, "bDeleteMe")? {
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
    // 4.5. The declarations of G's fields this path copies or reads, before its first write (#625 r1).
    schemas(vm, g, &IMPACT_FIELDS, WHAT)?;
    Ok(WithComponent { c, g, reattach, asset })
}

/// The preflight (module documentation): every refusal, then the whole commit planned and checked. Reads only.
fn preflight(vm: &mut Vm, pickup: ObjRef) -> Result<Option<Vec<Step>>, VmError> {
    // 1. bUseRBPhysics.
    schemas(vm, pickup, &[("bUseRBPhysics", Want::Bool)], WHAT)?;
    if !flag(vm, pickup, "bUseRBPhysics")? {
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
    let mode = enum_value(vm, pickup, "Physics", "PHYS_RigidBody")?;
    ow_world::timers::timers_of(vm)?;
    let with = match component {
        Some(c) => Some(component_checks(vm, pickup, c)?),
        None => None,
    };
    // The plan, in note section 2's order.
    let mut plan = vec![
        planned(vm, pickup, "bForceRBToSleep", Value::Bool(false))?,
        Step::Timer(awake_time, false, "MaxRBAwakeTimer"),
        Step::Timer(1.0, true, "CheckForRigidBodySleepState"),
        planned(vm, pickup, "RotationRate", zeroed(&required(vm, pickup, "RotationRate", WHAT)?))?,
    ];
    if let Some(WithComponent { c, g, reattach, asset }) = with {
        let one = [("X", Value::Float(1.0)), ("Y", Value::Float(1.0)), ("Z", Value::Float(1.0))];
        plan.push(planned(vm, c, "Scale3D", with_fields(vm, &required(vm, c, "Scale3D", WHAT)?, &one, "Scale3D")?)?);
        plan.push(planned(vm, c, "BlockRigidBody", Value::Bool(true))?);
        if reattach {
            plan.push(planned(vm, c, "CollideActors", Value::Bool(true))?);
            plan.push(planned(vm, c, "bNeedsReattach", Value::Bool(true))?);
        }
        plan.push(planned(vm, c, "BlockActors", Value::Bool(false))?);
        // 4.5. The asset branch, at rest (the hub's ruling): its non-physics writes, no rigid body.
        plan.push(planned(vm, c, "PhysicsWeight", Value::Float(1.0))?);
        plan.push(planned(vm, c, "PhysicsAsset", Value::Object(Some(asset)))?);
        plan.push(planned(vm, c, "bHasPhysicsAssetInstance", Value::Bool(true))?);
        plan.push(Step::Log(AT_REST));
        // 4.6. The impact fields.
        let max = float_of(&required(vm, g, "DropImpactEffectMax", WHAT)?, "DropImpactEffectMax")?;
        plan.push(planned(vm, pickup, "MaxImpactEffectPlayCount", Value::Int(ow_core::num::trunc_i32(max)))?);
        plan.push(planned(vm, pickup, "ImpactEffectDelay", required(vm, g, "DropImpactEffectDelay", WHAT)?)?);
        plan.push(planned(vm, pickup, "ImpactVelocityAkRtpc", required(vm, g, "DropImpactEffectVelocityAkRtpc", WHAT)?)?);
        // 4.7. The collision notify, then the threshold.
        plan.push(planned(vm, c, "bNotifyRigidBodyCollision", Value::Bool(true))?);
        let t = float_of(&required(vm, g, "DropImpactEffectThreshold", WHAT)?, "DropImpactEffectThreshold")?;
        let byte = ow_core::num::trunc_i32(t * 0.25) as u8;
        plan.push(planned(vm, c, "ScriptRigidBodyCollisionThresholdCompiled", Value::Byte(byte))?);
        // 4.8. The channel and the nine bits (one write keeping every other bit).
        let channel = enum_value(vm, c, "RBChannel", "RBCC_WillowPickup")?;
        plan.push(planned(vm, c, "RBChannel", Value::Byte(channel))?);
        let fields: Vec<(&str, Value)> = CHANNELS.iter().map(|&(n, b)| (n, Value::Bool(b))).collect();
        let bits = with_fields(vm, &required(vm, c, "RBCollideWithChannels", WHAT)?, &fields, "RBCollideWithChannels")?;
        plan.push(planned(vm, c, "RBCollideWithChannels", bits)?);
        // 5. The physics mode.
        plan.push(planned(vm, pickup, "Physics", Value::Byte(mode))?);
        // 6. The torque, or the spin: three draws on the port's stand-in for the engine's seeded generator
        // (UNVERIFIED; the values feed only the impulse, which the port does not apply). The spin range is read only
        // on the spin's branch.
        if flag(vm, pickup, "bTorque")? {
            if flag(vm, pickup, "bTorqueTemporary")? {
                plan.push(planned(vm, pickup, "bTorque", Value::Bool(false))?);
            }
        } else {
            for f in SPIN_FIELDS {
                schemas(vm, g, &[(f, Want::Float)], WHAT)?;
                float_of(&required(vm, g, f, WHAT)?, f)?;
            }
            plan.push(Step::Draws(3));
        }
    } else {
        plan.push(planned(vm, pickup, "Physics", Value::Byte(mode))?);
    }
    // 7. The ragdoll timer.
    plan.push(Step::Timer(0.3, false, "EnableRagdollCollision"));
    Ok(Some(plan))
}

/// `InitializePickupForRBPhysics()` on `pickup` in Off (module documentation).
pub fn initialize(vm: &mut Vm, pickup: ObjRef) -> Result<(), VmError> {
    let start = WriteCounters::of(vm);
    let result = (|| -> Result<(), VmError> {
        let Some(plan) = preflight(vm, pickup)? else {
            return Ok(());
        };
        // The timer borrow is taken before the first write (review r1 of #587, item 2).
        let timers = ow_world::timers::timers_of(vm)?;
        let mut timers = timers.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("Timers"))?;
        commit(vm, pickup, plan, &mut timers)
    })();
    result.map_err(|e| start.mark(vm, e))
}

/// The planned steps, in order. Every write was checked by the preflight.
fn commit(vm: &mut Vm, pickup: ObjRef, plan: Vec<Step>, timers: &mut ow_world::timers::Timers) -> Result<(), VmError> {
    for step in plan {
        match step {
            Step::Write(o, a, v) => vm.space.write(o, a, v)?,
            Step::Timer(rate, looping, func) => {
                let name = vm.space.names.intern(func);
                timers.set(pickup, rate, looping, name, pickup);
            }
            Step::Draws(n) => {
                for _ in 0..n {
                    let _ = frand_f32(vm.rng.engine_seeded());
                }
            }
            Step::Log(line) => vm.log.push(line.to_string()),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "pickup_rb_tests.rs"]
mod tests;
