//! Owner: lane IMPL-SPAWN-PICKUP-PARTICLES. `WillowPickup.SpawnPickupParticles()` in `ParityMode::Off`, coded from
//! docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md alone (#581, accepted at 164b43d3; clean room: lane
//! ANALYST-SPAWN-PICKUP-PARTICLES read the executable, this lane did not). It replaces #520's particle-effects
//! `LayerAbsent`. AI-assisted (Claude Code). **Every rule here is UNVERIFIED in the running game.**
//!
//! **The preflight** (note section 4), write-free, load-free and draw-free, in the native's own order; the first
//! refusal or data error met wins:
//! 1. the pickup's declarations (`PickupParticles`, `PickupParticleComponent`, `InventoryRarityLevel`, `Inventory`,
//!    `Rotation`, `bHasPickupParticleOffset`, `Components`);
//! 2. step 1: `PickupParticles` None is #520's [`PARTICLES_REMOVAL`];
//! 3. step 2, the component C: a held `PickupParticleComponent` must be a `ParticleSystemComponent` (else a data
//!    error) and its own values are the inputs; with none held, the class and default object the pickup world's
//!    set-up preloaded ([`ParticlePreload`]; none is refusal [`G`]) give the inputs, then the offset is planned
//!    (`GetPickupParticleOffset`'s reads) and the attach planned from the default's values with part 1's
//!    [`attach_plan`] (its guards and refusals, [`ATTACH`] and [`ATTACH_DETACH`], unchanged);
//! 4. step 4, the rarity choice: the first `RarityLevelColors` entry of the prepared globals definition holding
//!    `InventoryRarityLevel`; `RARITY_Rainbow` (by name) is refusal [`A`];
//! 5. step 5, `SetTemplate`: the template `PickupParticles` not loaded is refusal [`B`]; its `WarmupTime` and
//!    `LODSettings` are read and checked; then C's `bAutoActivate`, `bIsActive` and `bWasDeactivated` choose the
//!    activation (refusal [`E`]) or the initialisation (refusal [`F`]).
//!
//! **One of E and F always applies, so the native stops there and writes nothing today** (the note's section 4).
//! Steps 5.3 to 10 of the preflight (`bAcceptsLights`, the reattach requests and refusals C1, C2 and D, the absolute
//! flags, the colour, the `InstanceParameters` plan, the transform update) and the write plan are gated on E and F's
//! answer and are not coded here: they would be unreachable. A late `NotYet` would be marked "stopped after partial
//! writes: " only when #518's counters moved since entry.
//!
//! **Harness inputs** (hub ruling 1 on #581): the pickup world's set-up preloads `PartPickup`, the
//! `ParticleSystemComponent` class and its default object, recorded in [`ParticlePreload`] and logged as UNVERIFIED.

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::pickup_rb::WriteCounters;
use super::{PARTICLES_REMOVAL, RECORD, Want, attach_plan, bool_of, schemas};
use crate::equip::{bad, field, object_of, required};

/// The record of this rule.
pub const RECORD_PARTICLES: &str = "docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md";
/// Refusal A.
pub const A: &str = "WillowPickup.SpawnPickupParticles: the Rainbow-rarity template, loaded by name from an item-set definition";
/// Refusal B.
pub const B: &str = "WillowPickup.SpawnPickupParticles: a particle template that is not loaded (a load is a write)";
/// Refusal E.
pub const E: &str = "ParticleSystemComponent.ActivateSystem: the system's activation (script-visible writes, a possible random draw and warm-up ticks)";
/// Refusal F.
pub const F: &str = "ParticleSystemComponent.SetTemplate: the system's initialisation (not read)";
/// Refusal G.
pub const G: &str = "WillowPickup.SpawnPickupParticles: the particle component class, not preloaded";
/// The implementer's guard (not a note refusal): the offset's definition held as a handle the store has not loaded.
pub const UNLOADED_DEFINITION: &str =
    "WillowPickup.SpawnPickupParticles: the particle offset of an inventory definition that is not loaded (a load is a write)";
/// The component class the new C is made from (the field's declared class, fitted by the note).
pub const COMPONENT_CLASS: &str = "Engine.ParticleSystemComponent";
const WHAT: &str = "WillowPickup.SpawnPickupParticles";

const PICKUP_FIELDS: [(&str, Want); 7] = [
    ("PickupParticles", Want::Object),
    ("PickupParticleComponent", Want::Object),
    ("InventoryRarityLevel", Want::Int),
    ("Inventory", Want::Object),
    ("Rotation", Want::Struct),
    ("bHasPickupParticleOffset", Want::Bool),
    ("Components", Want::Array),
];
const ACTIVATION_FIELDS: [(&str, Want); 3] =
    [("bAutoActivate", Want::Bool), ("bIsActive", Want::Bool), ("bWasDeactivated", Want::Bool)];

/// The particle component class and its default object, preloaded by the pickup world's set-up (hub ruling 1 on #581,
/// an UNVERIFIED harness input; in the game the packages are already loaded). A world without it is refusal [`G`] for
/// a pickup that needs a new component.
#[derive(Debug, Clone, Copy)]
pub struct ParticlePreload {
    pub class: ClassId,
    pub defaults: ObjRef,
}

/// Records the preload in `vm`'s world (world set-up).
pub fn add_particle_preload(vm: &mut Vm, preload: ParticlePreload) -> Result<(), VmError> {
    vm.add_service(preload)?;
    Ok(())
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

fn flag(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    bool_of(&required(vm, o, name, WHAT)?, name, WHAT)
}

fn float_in(vm: &Vm, v: &Value, name: &str) -> Result<f32, VmError> {
    match field(vm, v, name)? {
        Value::Float(f) => Ok(f),
        other => Err(bad(WHAT, format!("{name} holds {}, not a float", other.kind_name()))),
    }
}

fn int_in(vm: &Vm, v: &Value, name: &str) -> Result<i32, VmError> {
    match field(vm, v, name)? {
        Value::Int(i) => Ok(i),
        other => Err(bad(WHAT, format!("{name} holds {}, not an int", other.kind_name()))),
    }
}

/// Step 3's offset (`GetPickupParticleOffset`'s reads, NATIVE_PICKUP_TARGETING.md): the definition's
/// `PickupFlagOffset` turned by the pickup's `Rotation`, zero without a definition. Reads only.
fn planned_offset(vm: &Vm, pickup: ObjRef) -> Result<[f32; 3], VmError> {
    let Some(inventory) = object_of(&required(vm, pickup, "Inventory", WHAT)?, WHAT)? else {
        return Ok([0.0; 3]);
    };
    let Some(definition) = crate::definition::inventory_definition_of(vm, inventory)? else {
        return Ok([0.0; 3]);
    };
    if unloaded(vm, definition)? {
        return Err(NotYet::not_read(UNLOADED_DEFINITION, RECORD_PARTICLES).into());
    }
    schemas(vm, definition, &[("PickupFlagOffset", Want::Struct)], WHAT)?;
    let v = required(vm, definition, "PickupFlagOffset", WHAT)?;
    let offset = [float_in(vm, &v, "X")?, float_in(vm, &v, "Y")?, float_in(vm, &v, "Z")?];
    let r = required(vm, pickup, "Rotation", WHAT)?;
    let (p, y, roll) = (int_in(vm, &r, "Pitch")?, int_in(vm, &r, "Yaw")?, int_in(vm, &r, "Roll")?);
    if offset == [0.0; 3] {
        return Ok(offset);
    }
    Ok(crate::pickup_targeting::rotate(offset, p, y, roll))
}

/// Step 4: whether the first `RarityLevelColors` entry holding `level` rates `RARITY_Rainbow` (by the declared enum's
/// value names). Each entry's `MinLevel`, `MaxLevel` and `RarityRating` are checked for type.
fn is_rainbow(vm: &Vm, definition: ObjRef, level: i32) -> Result<bool, VmError> {
    let class = vm.space.heap.get(definition)?.class;
    let p = vm.space.prop(class, "RarityLevelColors").ok_or_else(|| bad(WHAT, "the globals definition declares no RarityLevelColors"))?;
    let ty = match vm.space.classes.decl(p).map(|d| d.kind.clone()) {
        Some(PropKind::Array(inner)) => match *inner {
            PropKind::Struct(ty) => ty,
            k => return Err(bad(WHAT, format!("RarityLevelColors holds {k:?}, not a struct"))),
        },
        other => return Err(bad(WHAT, format!("RarityLevelColors is declared as {other:?}, not an array"))),
    };
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(WHAT, "RarityLevelColor has no layout"))?;
    let rating = vm.space.names.find("RarityRating").and_then(|n| layout.field(n)).map(|(_, d)| d.kind.clone());
    let Some(PropKind::Byte { enum_: Some(e) }) = rating else {
        return Err(bad(WHAT, "RarityLevelColor.RarityRating is not an enum byte"));
    };
    let values = vm.space.classes.enum_layout(e).ok_or_else(|| bad(WHAT, "RarityRating's enum has no layout"))?.values().to_vec();
    let entries = match required(vm, definition, "RarityLevelColors", WHAT)? {
        Value::Array(a) => a.as_ref().clone(),
        other => return Err(bad(WHAT, format!("RarityLevelColors holds {}, not an array", other.kind_name()))),
    };
    for entry in &entries {
        let (min, max) = (int_in(vm, entry, "MinLevel")?, int_in(vm, entry, "MaxLevel")?);
        let r = match field(vm, entry, "RarityRating")? {
            Value::Byte(b) => b,
            other => return Err(bad(WHAT, format!("RarityRating holds {}, not a byte", other.kind_name()))),
        };
        if min <= level && level <= max {
            let name = values.get(usize::from(r)).map(|n| vm.space.names.text(*n).to_string());
            return Ok(name.as_deref() == Some("RARITY_Rainbow"));
        }
    }
    Ok(false)
}

/// The preflight (module documentation). Always an error today: a refusal, E or F, or a data error.
fn preflight(vm: &mut Vm, pickup: ObjRef) -> Result<(), VmError> {
    // 1. The pickup's declarations.
    schemas(vm, pickup, &PICKUP_FIELDS, WHAT)?;
    // 2. Step 1.
    let Some(template) = object_of(&required(vm, pickup, "PickupParticles", WHAT)?, WHAT)? else {
        return Err(NotYet::not_read(PARTICLES_REMOVAL, RECORD).into());
    };
    // 3. Step 2, the component C and its inputs.
    let inputs = match object_of(&required(vm, pickup, "PickupParticleComponent", WHAT)?, WHAT)? {
        Some(c) => {
            if !is_a(vm, c, COMPONENT_CLASS)? {
                return Err(bad(
                    WHAT,
                    format!("PickupParticleComponent holds a {}, not a ParticleSystemComponent", ow_attr::eval::class_path(&vm.space, c)),
                ));
            }
            c
        }
        None => {
            // The preload, looked up once; the defaults' values are the new C's.
            let preload = match vm.service::<ParticlePreload>() {
                Some(p) => *p.try_borrow().map_err(|_| VmError::ServiceBorrowed("ParticlePreload"))?,
                None => return Err(NotYet::not_read(G, RECORD_PARTICLES).into()),
            };
            let held = vm.space.heap.get(preload.defaults)?.class;
            if held != preload.class || !is_a(vm, preload.defaults, COMPONENT_CLASS)? {
                return Err(bad(WHAT, "the preloaded default object is not of the preloaded ParticleSystemComponent class"));
            }
            planned_offset(vm, pickup)?;
            // Part 1's attach plan from the default's values: its guards, its refusals, its concrete checks.
            attach_plan(vm, pickup, Some(preload.defaults))?;
            preload.defaults
        }
    };
    // 4. Step 4, the rarity choice.
    let world = crate::globals::world_globals(vm)?;
    let g = crate::globals::globals_definition_loaded(vm, world, WHAT)?;
    let level = match required(vm, pickup, "InventoryRarityLevel", WHAT)? {
        Value::Int(i) => i,
        other => return Err(bad(WHAT, format!("InventoryRarityLevel holds {}, not an int", other.kind_name()))),
    };
    if is_rainbow(vm, g, level)? {
        return Err(NotYet::not_read(A, RECORD_PARTICLES).into());
    }
    // 5. Step 5, SetTemplate: the template, then the activation or the initialisation.
    if unloaded(vm, template)? {
        return Err(NotYet::not_read(B, RECORD_PARTICLES).into());
    }
    schemas(vm, template, &[("LODSettings", Want::Array)], WHAT)?;
    match required(vm, template, "WarmupTime", WHAT)? {
        Value::Float(_) => {}
        other => return Err(bad(WHAT, format!("WarmupTime holds {}, not a float", other.kind_name()))),
    }
    let lods = match required(vm, template, "LODSettings", WHAT)? {
        Value::Array(a) => a.as_ref().clone(),
        other => return Err(bad(WHAT, format!("LODSettings holds {}, not an array", other.kind_name()))),
    };
    for lod in &lods {
        bool_of(&field(vm, lod, "bLit")?, "bLit", WHAT)?;
    }
    schemas(vm, inputs, &ACTIVATION_FIELDS, WHAT)?;
    let activate = flag(vm, inputs, "bAutoActivate")? || (flag(vm, inputs, "bIsActive")? && !flag(vm, inputs, "bWasDeactivated")?);
    // One of the two always applies today (module documentation).
    let refusal = if activate { E } else { F };
    Err(NotYet::not_read(refusal, RECORD_PARTICLES).into())
}

/// `SpawnPickupParticles()` on `pickup` in Off (module documentation). Writes nothing today.
pub fn spawn_pickup_particles(vm: &mut Vm, pickup: ObjRef) -> Result<(), VmError> {
    let start = WriteCounters::of(vm);
    preflight(vm, pickup).map_err(|e| start.mark(vm, e))
}

#[cfg(test)]
#[path = "pickup_particles_tests.rs"]
mod tests;
