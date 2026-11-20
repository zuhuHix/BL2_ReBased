//! Owner: lanes IMPL-SPAWN-PICKUP-PARTICLES and IMPL-PARTICLE-ACTIVATION. `WillowPickup.SpawnPickupParticles()` in
//! `ParityMode::Off`, coded from docs/verification/NATIVE_SPAWN_PICKUP_PARTICLES.md (#581, accepted at 164b43d3) and,
//! for `SetTemplate`'s two branches, docs/verification/NATIVE_PARTICLE_ACTIVATION.md (#597, accepted at 3df9cd88),
//! alone (clean room: the analyst lanes read the executable, these lanes did not). It replaces #520's particle-effects
//! `LayerAbsent`. AI-assisted (Claude Code). **Every rule here is UNVERIFIED in the running game.**
//!
//! **Plan, then apply.** The preflight plans every write over an overlay ([`particle_activation::Setup`]) and is
//! write-free, load-free and draw-free; the first refusal or data error met wins, in the native's own order:
//! 1. the pickup's declarations (`PickupParticles`, `PickupParticleComponent`, `InventoryRarityLevel`, `Inventory`,
//!    `Rotation`, `bHasPickupParticleOffset`, `Components`);
//! 2. step 1: `PickupParticles` None is #520's [`PARTICLES_REMOVAL`];
//! 3. step 2, the component C: a held `PickupParticleComponent` must be a `ParticleSystemComponent` (else a data
//!    error) and its own values are the inputs; with none held, the class and default object the pickup world's
//!    set-up preloaded ([`ParticlePreload`]; none is refusal [`G`]) give the inputs, captured once, then the offset is
//!    planned (`GetPickupParticleOffset`'s reads) and the attach planned with part 1's [`attach_plan`] (its guards and
//!    refusals, [`ATTACH`] and [`ATTACH_DETACH`], unchanged);
//! 4. step 4, the rarity choice: with no globals definition held (`TheGlobalsDefinition` None) nothing is read
//!    (review r1 of #593); else every `RarityLevelColors` entry is type-checked and the first holding
//!    `InventoryRarityLevel` rates it; `RARITY_Rainbow` (by name) is refusal [`A`], a rating outside the enum a data
//!    error;
//! 5. step 5, `SetTemplate`: the template not loaded is refusal [`B`]; the engine flags (a harness input, hub ruling 1
//!    on #597); then [`particle_activation::Setup::set_template`], the note's section 7.3 (the activation or the
//!    initialisation, `bAcceptsLights`, the reattach request);
//! 6. #581's steps 6 to 10: the absolute flags and `Rotation`, the colour, the `InstanceParameters` plan ("RColor",
//!    `ParamType` 3), `SetHidden(false)`, the deferred transform update (refusals C2 and D).
//!
//! **The writes** are then applied in the plan's order, with nothing that can refuse: for a new C the allocation
//! (outer the pickup, the captured default slots) comes first, and planned writes on the default object land on it. A
//! late `NotYet` would be marked "stopped after partial writes: " only when #518's counters moved since entry.
//!
//! **Harness inputs** (hub ruling 1 on #581 and on #597): the pickup world's set-up preloads `PartPickup`, the
//! `ParticleSystemComponent` class and its default object ([`ParticlePreload`]), and supplies the engine flags
//! ([`particle_activation::ParticleEngineFlags`]); each is logged as UNVERIFIED.

use ow_core::NotYet;
use ow_object::ids::{ClassId, NameId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::{StructValue, Value};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::particle_activation::{self as pa, K, Setup};
use super::pickup_rb::WriteCounters;
use super::{PARTICLES_REMOVAL, RECORD, Want, attach_plan, schemas};
use crate::equip::{bad, field, object_of, required, write};

/// The record of this rule.
pub use super::particle_activation::RECORD_PARTICLES;
/// Refusal A.
pub const A: &str = "WillowPickup.SpawnPickupParticles: the Rainbow-rarity template, loaded by name from an item-set definition";
/// Refusal B.
pub const B: &str = pa::OLD_NOT_LOADED;
/// Refusal G.
pub const G: &str = "WillowPickup.SpawnPickupParticles: the particle component class, not preloaded";
/// The implementer's guard (not a note refusal): the offset's definition held as a handle the store has not loaded.
pub const UNLOADED_DEFINITION: &str =
    "WillowPickup.SpawnPickupParticles: the particle offset of an inventory definition that is not loaded (a load is a write)";
/// The component class the new C is made from (the field's declared class, fitted by the note).
pub const COMPONENT_CLASS: &str = "Engine.ParticleSystemComponent";
/// The vector parameter's name (#581 step 8; read as text, UNVERIFIED).
pub const COLOR_PARAMETER: &str = "RColor";
/// `PSPT_Vector` (#581 step 8).
pub const PARAM_VECTOR: u8 = 3;
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
const PARAMETER: K = K::StructOf(&[("Name", K::Name), ("ParamType", K::Byte), ("Vector", pa::VECTOR_KIND)]);
const STEP_FIELDS: [(&str, K); 2] = [("InstanceParameters", K::ArrayOf(&PARAMETER)), ("Translation", pa::VECTOR_KIND)];
const COLOR: K = K::StructOf(&[("R", K::Byte), ("G", K::Byte), ("B", K::Byte), ("A", K::Byte)]);

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

/// A slotless stand-in whose class has slots.
pub(super) fn stand_in(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    Ok(x.slots.is_empty() && vm.space.classes.get_layout(x.class).is_some_and(|l| l.slot_count() > 0))
}

/// An object the store has not loaded, or a stand-in.
fn unloaded(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    Ok(vm.defs.is_unloaded(&vm.space, o) || stand_in(vm, o)?)
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

fn byte_in(vm: &Vm, v: &Value, name: &str) -> Result<u8, VmError> {
    match field(vm, v, name)? {
        Value::Byte(b) => Ok(b),
        other => Err(bad(WHAT, format!("{name} holds {}, not a byte", other.kind_name()))),
    }
}

/// The inventory's definition, checked loaded; None without an inventory or a definition.
fn definition(vm: &Vm, pickup: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let Some(inventory) = object_of(&required(vm, pickup, "Inventory", WHAT)?, WHAT)? else {
        return Ok(None);
    };
    let Some(definition) = crate::definition::inventory_definition_of(vm, inventory)? else {
        return Ok(None);
    };
    if unloaded(vm, definition)? {
        return Err(NotYet::not_read(UNLOADED_DEFINITION, RECORD_PARTICLES).into());
    }
    Ok(Some(definition))
}

/// Step 3's offset (`GetPickupParticleOffset`'s reads, NATIVE_PICKUP_TARGETING.md): the definition's
/// `PickupFlagOffset` turned by the pickup's `Rotation`, zero without a definition. Reads only.
fn planned_offset(vm: &Vm, pickup: ObjRef) -> Result<[f32; 3], VmError> {
    let Some(definition) = definition(vm, pickup)? else {
        return Ok([0.0; 3]);
    };
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

/// Step 4: the globals definition's entry for `level` (every entry type-checked, review r1 of #593), and whether it
/// rates `RARITY_Rainbow` (by the declared enum's value names; a rating outside the enum is a data error).
fn rarity_entry(vm: &Vm, definition: ObjRef, level: i32) -> Result<Option<(Value, bool)>, VmError> {
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
    let mut found = None;
    for entry in &entries {
        let (min, max) = (int_in(vm, entry, "MinLevel")?, int_in(vm, entry, "MaxLevel")?);
        let r = byte_in(vm, entry, "RarityRating")?;
        let name = values
            .get(usize::from(r))
            .map(|n| vm.space.names.text(*n).to_string())
            .ok_or_else(|| bad(WHAT, format!("RarityRating holds {r}, outside its enum's {} values", values.len())))?;
        let color = field(vm, entry, "Color")?;
        for channel in ["R", "G", "B", "A"] {
            byte_in(vm, &color, channel)?;
        }
        if found.is_none() && min <= level && level <= max {
            found = Some((color, name == "RARITY_Rainbow"));
        }
    }
    Ok(found)
}

/// A colour struct's R, G and B as floats in 0..1 (byte × 1/255), and whether all four bytes are zero.
fn rgb(vm: &Vm, color: &Value) -> Result<([f32; 3], bool), VmError> {
    let [r, g, b, a] = [byte_in(vm, color, "R")?, byte_in(vm, color, "G")?, byte_in(vm, color, "B")?, byte_in(vm, color, "A")?];
    let unit = |x: u8| f32::from(x) * (1.0 / 255.0);
    Ok(([unit(r), unit(g), unit(b)], [r, g, b, a] == [0; 4]))
}

/// `v`, a struct, with the named fields replaced.
fn with_fields(vm: &Vm, v: &Value, fields: &[(&str, Value)]) -> Result<Value, VmError> {
    let Value::Struct(s) = v else { return Err(bad(WHAT, format!("{} is not a struct", v.kind_name()))) };
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| bad(WHAT, "a struct with no layout"))?;
    let mut out = s.fields.clone();
    for (name, value) in fields {
        let (i, _) = vm.space.names.find(name).and_then(|n| layout.field(n)).ok_or_else(|| bad(WHAT, format!("no field {name}")))?;
        out[i as usize] = value.clone();
    }
    Ok(Value::Struct(std::rc::Rc::new(StructValue { ty: s.ty, fields: out })))
}

/// A new component's allocation: its class, the captured default slots and the default object the plan stands it
/// for.
struct NewComponent {
    class: ClassId,
    defaults: ObjRef,
    slots: Vec<Value>,
}

struct Plan {
    /// The component the plan reads: a held one, or the default object a new one stands for.
    component: ObjRef,
    new: Option<NewComponent>,
    /// The writes up to and including step 7, then the parameter step, then the rest.
    before: Vec<(ObjRef, &'static str, Value)>,
    /// The `InstanceParameters` value, built whole in the plan (review r1 of #609), and the index of an appended entry
    /// whose `Name` is still the placeholder because the name table has no "RColor" yet: the apply phase interns the
    /// name and sets it there before the write (review r2 of #617), so the preflight never interns.
    parameter: Option<(Value, Option<usize>)>,
    after: Vec<(ObjRef, &'static str, Value)>,
}

/// The preflight (module documentation): the whole plan, or the first refusal or data error.
fn preflight(vm: &mut Vm, pickup: ObjRef) -> Result<Plan, VmError> {
    // 1. The pickup's declarations.
    schemas(vm, pickup, &PICKUP_FIELDS, WHAT)?;
    // 2. Step 1.
    let Some(template) = object_of(&required(vm, pickup, "PickupParticles", WHAT)?, WHAT)? else {
        return Err(NotYet::not_read(PARTICLES_REMOVAL, RECORD).into());
    };
    // 3. Step 2, the component C and its inputs.
    let mut early: Vec<(ObjRef, &'static str, Value)> = Vec::new();
    let (c, new) = match object_of(&required(vm, pickup, "PickupParticleComponent", WHAT)?, WHAT)? {
        Some(c) => {
            if !is_a(vm, c, COMPONENT_CLASS)? {
                return Err(bad(
                    WHAT,
                    format!("PickupParticleComponent holds a {}, not a ParticleSystemComponent", ow_attr::eval::class_path(&vm.space, c)),
                ));
            }
            (c, None)
        }
        None => {
            // The preload, looked up once; the defaults' values are the new C's, captured here.
            let preload = match vm.service::<ParticlePreload>() {
                Some(p) => *p.try_borrow().map_err(|_| VmError::ServiceBorrowed("ParticlePreload"))?,
                None => return Err(NotYet::not_read(G, RECORD_PARTICLES).into()),
            };
            let held = vm.space.heap.get(preload.defaults)?.class;
            if held != preload.class || !is_a(vm, preload.defaults, COMPONENT_CLASS)? {
                return Err(bad(WHAT, "the preloaded default object is not of the preloaded ParticleSystemComponent class"));
            }
            let d = preload.defaults;
            pa::check(vm, d, &STEP_FIELDS[1..], WHAT)?;
            let offset = planned_offset(vm, pickup)?;
            early.push((pickup, "PickupParticleComponent", Value::Object(Some(d))));
            let translation = with_fields(
                vm,
                &required(vm, d, "Translation", WHAT)?,
                &[("X", Value::Float(offset[0])), ("Y", Value::Float(offset[1])), ("Z", Value::Float(offset[2]))],
            )?;
            early.push((d, "Translation", translation));
            if offset != [0.0; 3] {
                early.push((pickup, "bHasPickupParticleOffset", Value::Bool(true)));
            }
            // Part 1's attach plan from the default's values: its guards, its refusals, its concrete checks.
            if let Some(attach) = attach_plan(vm, pickup, Some(d))? {
                early.extend(attach);
            }
            let slots = vm.space.heap.get(d)?.slots.clone();
            (d, Some(NewComponent { class: preload.class, defaults: d, slots }))
        }
    };
    // Steps 4 to 10 read only; the parameter value is built after them (review r1 of #617).
    let (before, after, params, at, colour) = {
        // 4. Step 4, the rarity choice (only the choice; the colour is step 7).
        let world = crate::globals::world_globals(vm)?;
        let vm: &Vm = vm;
        crate::globals::declared_reference_loaded(vm, world, "TheGlobalsDefinition", Some(crate::globals::GEARBOX_GLOBALS_DEFINITION), WHAT)?;
        let level = match required(vm, pickup, "InventoryRarityLevel", WHAT)? {
            Value::Int(i) => i,
            other => return Err(bad(WHAT, format!("InventoryRarityLevel holds {}, not an int", other.kind_name()))),
        };
        let entry = match object_of(&required(vm, world, "TheGlobalsDefinition", WHAT)?, WHAT)? {
            None => None,
            Some(_) => {
                let g = crate::globals::globals_definition_loaded(vm, world, WHAT)?;
                let entry = rarity_entry(vm, g, level)?;
                if entry.as_ref().is_some_and(|(_, rainbow)| *rainbow) {
                    return Err(NotYet::not_read(A, RECORD_PARTICLES).into());
                }
                entry
            }
        };
        // 5. Step 5, SetTemplate.
        if unloaded(vm, template)? {
            return Err(NotYet::not_read(B, RECORD_PARTICLES).into());
        }
        let flags = pa::engine_flags(vm)?;
        let mut s = Setup::new(vm, c, new.is_some(), flags);
        for (o, name, v) in early {
            // Each new-component write against the heap's write rules, its slot read (part 1's are checked already).
            super::fits(vm, o, name, &v, WHAT)?;
            required(vm, o, name, WHAT)?;
            s.planned(o, name, v);
        }
        s.set_template(template)?;
        // 6. #581's step 6: the absolute flags and Rotation.
        s.set(c, "AbsoluteTranslation", Value::Bool(false))?;
        s.set(c, "AbsoluteRotation", Value::Bool(true))?;
        s.set(c, "AbsoluteScale", Value::Bool(false))?;
        let zero = with_fields(vm, &s.get(c, "Rotation")?, &[("Pitch", Value::Int(0)), ("Yaw", Value::Int(0)), ("Roll", Value::Int(0))])?;
        s.set(c, "Rotation", zero)?;
        // 7. The colour: the entry's, or opaque white; then a non-zero LootBeamColorOverride.
        let mut colour = match &entry {
            Some((color, _)) => rgb(vm, color)?.0,
            None => [1.0; 3],
        };
        if let Some(definition) = definition(vm, pickup)? {
            pa::check(vm, definition, &[("LootBeamColorOverride", COLOR)], WHAT)?;
            let (over, zero) = rgb(vm, &required(vm, definition, "LootBeamColorOverride", WHAT)?)?;
            if !zero {
                colour = over;
            }
        }
        // 8. The parameter plan.
        pa::check(vm, c, &STEP_FIELDS[..1], WHAT)?;
        let params = s.array(c, "InstanceParameters")?;
        // Found, not interned: the name table keeps the first spelling it meets (decision 2026-11-15-23), so a refused
        // call must not add one (review r1 of #617).
        let name = vm.space.names.find(COLOR_PARAMETER);
        let mut at = None;
        for (i, p) in params.iter().enumerate() {
            let n = match field(vm, p, "Name")? {
                Value::Name(n) => n,
                other => return Err(bad(WHAT, format!("a parameter's Name holds {}, not a name", other.kind_name()))),
            };
            let ty = byte_in(vm, p, "ParamType")?;
            let v = field(vm, p, "Vector")?;
            for axis in ["X", "Y", "Z"] {
                float_in(vm, &v, axis)?;
            }
            if at.is_none() && Some(n) == name && ty == PARAM_VECTOR {
                at = Some(i);
            }
        }
        let before = std::mem::take(&mut s.writes);
        // 9. SetHidden(false): only when HiddenGame is set.
        if s.flag(c, "HiddenGame")? {
            s.set(c, "HiddenGame", Value::Bool(false))?;
            s.deferred_reattach()?;
        }
        // 10. The deferred transform update.
        s.set(c, "bNeedsUpdateTransform", Value::Bool(true))?;
        s.transform_refusals()?;
        let after = std::mem::take(&mut s.writes);
        (before, after, params, at, colour)
    };
    // The parameter value, built whole in the plan (review r1 of #609) after the last NotYet (review r1 of #617). The
    // preflight never interns (review r2 of #617; interning is a side effect, decision 2026-11-15-23): the name is
    // found, or an appended entry holds the placeholder `NameId::NONE` until the apply phase interns the real one.
    // `fits` checks the value with the placeholder, which has the same kind.
    let found = vm.space.names.find(COLOR_PARAMETER);
    let color_name = found.unwrap_or(NameId::NONE);
    let vm: &Vm = vm;
    let mut params = params;
    let vector = |base: &Value| {
        with_fields(vm, base, &[("X", Value::Float(colour[0])), ("Y", Value::Float(colour[1])), ("Z", Value::Float(colour[2]))])
    };
    match at {
        Some(i) => {
            let v = vector(&field(vm, &params[i], "Vector")?)?;
            params[i] = with_fields(vm, &params[i], &[("Vector", v)])?;
        }
        None => {
            let class = vm.space.heap.get(c)?.class;
            let p = vm.space.prop(class, "InstanceParameters").ok_or_else(|| bad(WHAT, "no InstanceParameters"))?;
            let Some(PropKind::Array(inner)) = vm.space.classes.decl(p).map(|d| d.kind.clone()) else {
                return Err(bad(WHAT, "InstanceParameters is not an array"));
            };
            let zero = vm.space.classes.zero_value(&inner);
            let v = vector(&field(vm, &zero, "Vector")?)?;
            params.push(with_fields(vm, &zero, &[("Name", Value::Name(color_name)), ("ParamType", Value::Byte(PARAM_VECTOR)), ("Vector", v)])?);
        }
    }
    let patch = (at.is_none() && found.is_none()).then(|| params.len() - 1);
    let value = Value::array(params);
    super::fits(vm, c, "InstanceParameters", &value, WHAT)?;
    let parameter = Some((value, patch));
    Ok(Plan { component: c, new, before, parameter, after })
}

/// The write phase: the plan applied in order (module documentation).
fn apply(vm: &mut Vm, pickup: ObjRef, plan: Plan) -> Result<(), VmError> {
    let (c, defaults) = match &plan.new {
        Some(n) => {
            let o = vm.instantiate(n.class, "")?;
            let x = vm.space.heap.get_mut(o)?;
            x.outer = Some(pickup);
            x.slots = n.slots.clone();
            (o, Some(n.defaults))
        }
        None => (plan.component, None),
    };
    let remap = |o: ObjRef| if Some(o) == defaults { c } else { o };
    let remap_value = |v: Value| match (v, defaults) {
        (Value::Object(Some(o)), Some(d)) if o == d => Value::Object(Some(c)),
        (Value::Array(a), Some(d)) => Value::array(
            a.iter().map(|e| if *e == Value::Object(Some(d)) { Value::Object(Some(c)) } else { e.clone() }).collect(),
        ),
        (v, _) => v,
    };
    for (o, name, v) in plan.before {
        write(vm, remap(o), name, remap_value(v), WHAT)?;
    }
    if let Some((v, patch)) = plan.parameter {
        // The appended entry's real name, interned only now, in the write phase (review r2 of #617).
        let v = match patch {
            Some(i) => {
                let name = vm.space.names.intern(COLOR_PARAMETER);
                let mut items = v.elements().map(<[Value]>::to_vec).unwrap_or_default();
                let entry = items.get(i).ok_or_else(|| bad(WHAT, "the planned parameter entry is missing"))?;
                items[i] = with_fields(vm, entry, &[("Name", Value::Name(name))])?;
                Value::array(items)
            }
            None => v,
        };
        write(vm, c, "InstanceParameters", v, WHAT)?;
    }
    for (o, name, v) in plan.after {
        write(vm, remap(o), name, remap_value(v), WHAT)?;
    }
    Ok(())
}

/// `SpawnPickupParticles()` on `pickup` in Off (module documentation).
pub fn spawn_pickup_particles(vm: &mut Vm, pickup: ObjRef) -> Result<(), VmError> {
    let start = WriteCounters::of(vm);
    let run = |vm: &mut Vm| -> Result<(), VmError> {
        let plan = preflight(vm, pickup)?;
        apply(vm, pickup, plan)
    };
    run(vm).map_err(|e| start.mark(vm, e))
}

#[cfg(test)]
#[path = "pickup_particles_tests.rs"]
mod tests;
