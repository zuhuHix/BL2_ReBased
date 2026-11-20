//! Tests of `SpawnPickupParticles` in Off (lane IMPL-SPAWN-PICKUP-PARTICLES) on three invented packages built with
//! `ow_package::testkit` (AGENTS.md rule 9). `Engine` has the actor and component classes, the particle system and its
//! LOD struct, the inventory, its definition and a holder whose load makes an unloaded template handle;
//! `GearboxFramework` the globals definition's base; `WillowGame` the weapon, the pickup with its native, the globals and
//! the globals definition with a rarity enum in an order unlike the game's. Every value is invented. AI-assisted
//! (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::PropAddr;
use ow_object::value::StructValue;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, ScriptToy};
use ow_package::testkit::{Export, ints};
use ow_vm::api::StubPolicy;

use super::*;
use crate::attach_mesh::{ATTACH, ATTACH_DETACH, SPAWN_PICKUP_PARTICLES, add_empty_scene, register};

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);
const PARTICLES_FN: &str = "WillowGame.WillowPickup.SpawnPickupParticles";

const IMPORTS: [&str; 11] = [
    "Class",
    "Function",
    "ScriptStruct",
    "Enum",
    "ObjectProperty",
    "BoolProperty",
    "IntProperty",
    "FloatProperty",
    "StructProperty",
    "ByteProperty",
    "ArrayProperty",
];

fn toy_enum(t: &mut ScriptToy, owner: i32, name: &str, values: &[&str]) -> i32 {
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    payload.extend(ints(&[0, values.len() as i32]));
    for v in values {
        let bytes = t.p.fname(v, 0);
        payload.extend_from_slice(&bytes);
    }
    let mut e = Export::new(t.import("Enum"), name, payload);
    e.outer = owner;
    t.p.add_export_with(e)
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let vector = toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    let rotator = toy_struct(&mut t, object, "Rotator", None);
    for f in ["Pitch", "Yaw", "Roll"] {
        t.prop("Int", rotator, f, 0, 0);
    }
    let color = toy_struct(&mut t, object, "Color", None);
    for f in ["B", "G", "R", "A"] {
        t.prop("Byte", color, f, 0, 0);
    }
    let pointer = toy_struct(&mut t, object, "Pointer", None);
    t.prop("Int", pointer, "Dummy", 0, 0);
    let actor = t.class("Actor", object);
    let component = t.class("ActorComponent", object);
    toy_array_of(&mut t, actor, "Components", "Object", component);
    t.prop("Bool", actor, "bDeleteMe", 0, 0);
    t.prop("Struct", actor, "Rotation", 0, rotator);
    t.prop("Struct", component, "Scene", 0, pointer);
    t.prop("Object", component, "Owner", 0, actor);
    for b in ["bAttached", "bNeedsReattach", "bNeedsUpdateTransform"] {
        t.prop("Bool", component, b, 0, 0);
    }
    let primitive = t.class("PrimitiveComponent", component);
    let system = t.class("ParticleSystem", object);
    t.prop("Float", system, "WarmupTime", 0, 0);
    let lod = toy_struct(&mut t, system, "ParticleSystemLOD", None);
    t.prop("Bool", lod, "bLit", 0, 0);
    toy_array_of(&mut t, system, "LODSettings", "Struct", lod);
    let psc = t.class("ParticleSystemComponent", primitive);
    t.prop("Object", psc, "Template", 0, system);
    for b in ["bAutoActivate", "bIsActive", "bWasDeactivated"] {
        t.prop("Bool", psc, b, 0, 0);
    }
    t.class("WillowInventory", actor);
    let definition = t.class("WillowInventoryDefinition", object);
    t.prop("Struct", definition, "PickupFlagOffset", 0, vector);
    t.prop("Struct", definition, "LootBeamColorOverride", 0, color);
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Target", 0, system);
    let target = t.p.add_export(system, "ToyTemplate", {
        let mut p = ints(&[0]);
        p.extend_from_slice(&t.none);
        p
    });
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Target", target));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let core = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", core, "Object");
    t.class("GearboxGlobalsDefinition", object);
    t.build()
}

fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let inventory = t.p.add_import("Core", "Class", engine, "WillowInventory");
    let definition = t.p.add_import("Core", "Class", engine, "WillowInventoryDefinition");
    let system = t.p.add_import("Core", "Class", engine, "ParticleSystem");
    let psc = t.p.add_import("Core", "Class", engine, "ParticleSystemComponent");
    let color = t.p.add_import("Core", "ScriptStruct", object, "Color");
    let gearbox = t.p.add_import("Core", "Package", 0, "GearboxFramework");
    let gbx_definition = t.p.add_import("Core", "Class", gearbox, "GearboxGlobalsDefinition");
    let weapon = t.class("WillowWeapon", inventory);
    let data = toy_struct(&mut t, weapon, "WeaponDefinitionData", None);
    t.prop("Object", data, "WeaponTypeDefinition", 0, object);
    t.prop("Struct", weapon, "DefinitionData", 0, data);
    t.class("WeaponTypeDefinition", definition);
    let pickup = t.class("WillowPickup", actor);
    t.prop("Object", pickup, "PickupParticles", 0, system);
    t.prop("Object", pickup, "PickupParticleComponent", 0, psc);
    t.prop("Int", pickup, "InventoryRarityLevel", 0, 0);
    t.prop("Object", pickup, "Inventory", 0, inventory);
    t.prop("Bool", pickup, "bHasPickupParticleOffset", 0, 0);
    let spec = FunctionSpec::new(NATIVE, "SpawnPickupParticles");
    t.function(pickup, "SpawnPickupParticles", &[], None, &[], &spec, |_| None);
    let globals = t.class("WillowGlobals", object);
    t.prop("Object", globals, "TheGlobalsDefinition", 0, gbx_definition);
    let gd = t.class("GlobalsDefinition", gbx_definition);
    // The rarity enum in an invented order: RARITY_Rainbow is 1 here.
    let rarity = toy_enum(&mut t, gd, "EItemRarity", &["RARITY_Unknown", "RARITY_Rainbow", "RARITY_Common"]);
    let entry = toy_struct(&mut t, gd, "RarityLevelColor", None);
    t.prop("Int", entry, "MinLevel", 0, 0);
    t.prop("Int", entry, "MaxLevel", 0, 0);
    t.prop("Byte", entry, "RarityRating", 0, rarity);
    t.prop("Struct", entry, "Color", 0, color);
    toy_array_of(&mut t, gd, "RarityLevelColors", "Struct", entry);
    t.build()
}

/// A world with a pickup holding a loaded template, no component yet, a weapon with a definition (zero offset), the
/// globals with one rarity band (levels 0 to 9, common), an empty scene and the particle preload.
struct Fx {
    vm: Vm,
    dir: PathBuf,
    pickup: ObjRef,
    template: ObjRef,
    unloaded: ObjRef,
    defaults: ObjRef,
    gd: ObjRef,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn make(vm: &mut Vm, class: &str) -> ObjRef {
    let c = vm.find_class(class).unwrap();
    ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
}

fn set(vm: &mut Vm, o: ObjRef, name: &str, v: Value) {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.write(o, PropAddr::scalar(p), v).unwrap();
}

fn get(vm: &Vm, o: ObjRef, name: &str) -> Value {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
}

/// `v`, a struct, with the named fields replaced.
fn with(vm: &Vm, v: &Value, fields: &[(&str, Value)]) -> Value {
    let Value::Struct(s) = v else { panic!("not a struct") };
    let layout = vm.space.classes.struct_layout(s.ty).unwrap();
    let mut out = s.fields.clone();
    for (name, value) in fields {
        let (i, _) = layout.field(vm.space.names.find(name).unwrap()).unwrap();
        out[i as usize] = value.clone();
    }
    Value::Struct(Rc::new(StructValue { ty: s.ty, fields: out }))
}

fn fx_with(mode: ParityMode, preload: bool, scene: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-pickup-particles-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g, w) = (engine(), gearbox(), willow_game());
    let files = [("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice()), ("WillowGame.upk", w.as_slice())];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    for c in ["Engine.ParticleSystemComponent", "WillowGame.WillowPickup", "WillowGame.GlobalsDefinition"] {
        vm.find_class(c).unwrap();
    }
    register(&mut vm).unwrap();
    if scene {
        add_empty_scene(&mut vm).unwrap();
    }
    let class = vm.find_class(COMPONENT_CLASS).unwrap();
    let defaults = vm.defs.class_default(&mut vm.space, COMPONENT_CLASS).unwrap();
    if preload {
        add_particle_preload(&mut vm, ParticlePreload { class, defaults }).unwrap();
    }
    let holder = vm.defs.find(&mut vm.space, &ObjectPath::parse("Engine.ToyHolderObject").unwrap()).unwrap().unwrap();
    let Value::Object(Some(unloaded)) = get(&vm, holder, "Target") else { panic!("the holder's target") };
    assert!(vm.defs.is_unloaded(&vm.space, unloaded), "an unloaded template handle");
    let template = make(&mut vm, "Engine.ParticleSystem");
    let pickup = make(&mut vm, "WillowGame.WillowPickup");
    set(&mut vm, pickup, "PickupParticles", Value::Object(Some(template)));
    let definition = make(&mut vm, "WillowGame.WeaponTypeDefinition");
    let weapon = make(&mut vm, "WillowGame.WillowWeapon");
    let data = with(&vm, &get(&vm, weapon, "DefinitionData"), &[("WeaponTypeDefinition", Value::Object(Some(definition)))]);
    set(&mut vm, weapon, "DefinitionData", data);
    set(&mut vm, pickup, "Inventory", Value::Object(Some(weapon)));
    let globals = make(&mut vm, "WillowGame.WillowGlobals");
    let gd = make(&mut vm, "WillowGame.GlobalsDefinition");
    set(&mut vm, globals, "TheGlobalsDefinition", Value::Object(Some(gd)));
    let q = ow_world::query::install(&mut vm).unwrap();
    q.borrow_mut().set_globals(globals);
    let mut x = Fx { vm, dir, pickup, template, unloaded, defaults, gd };
    x.bands(&[(0, 9, 2), (10, 19, 1)]);
    x
}

fn fx() -> Fx {
    fx_with(ParityMode::Off, true, true)
}

impl Fx {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        set(&mut self.vm, o, name, v);
    }

    /// The globals definition's bands: (`MinLevel`, `MaxLevel`, `RarityRating`).
    fn bands(&mut self, bands: &[(i32, i32, u8)]) {
        let class = self.vm.space.heap.get(self.gd).unwrap().class;
        let p = self.vm.space.prop(class, "RarityLevelColors").unwrap();
        let PropKind::Array(inner) = self.vm.space.classes.decl(p).unwrap().kind.clone() else { panic!() };
        let zero = self.vm.space.classes.zero_value(&inner);
        let entries: Vec<Value> = bands
            .iter()
            .map(|&(min, max, r)| {
                with(&self.vm, &zero, &[("MinLevel", Value::Int(min)), ("MaxLevel", Value::Int(max)), ("RarityRating", Value::Byte(r))])
            })
            .collect();
        let gd = self.gd;
        self.set(gd, "RarityLevelColors", Value::array(entries));
    }

    fn call(&mut self) -> Result<Value, VmError> {
        let f = self.vm.find_function(PARTICLES_FN)?;
        let p = self.pickup;
        Ok(self.vm.call(f, Some(p), vec![])?.ret)
    }

    /// The pickup's and the defaults' values, the heap size and #518's three counters.
    fn state(&self) -> (Vec<Value>, Vec<Value>, usize, [u64; 3]) {
        let all = |o: ObjRef| -> Vec<Value> { self.vm.space.heap.get(o).unwrap().slots.to_vec() };
        let counters = [self.vm.space.heap.committed_writes(), self.vm.rng.draws(), self.vm.defs.load_transitions()];
        (all(self.pickup), all(self.defaults), self.vm.space.heap.handles().len(), counters)
    }
}

/// The call refuses with `what` (kind `missing`, record `record`), unmarked, and changes nothing: the pickup, the
/// defaults, the heap's object count and #518's three counters.
fn refused(x: &mut Fx, missing: Missing, what: &str, record: &str) {
    let before = x.state();
    let n = match x.call() {
        Err(VmError::NotYet(n)) => n,
        other => panic!("{what}: expected NotYet, got {other:?}"),
    };
    assert_eq!((n.missing, n.what.as_ref(), n.record), (missing, what, record));
    assert!(!ow_core::notyet::is_partial_writes(&n.what), "a preflight refusal is unmarked");
    assert_eq!(x.state(), before, "{what}: nothing written, allocated, loaded or drawn");
}

/// Step 1: no particle system is #520's removal refusal.
#[test]
fn no_particle_system_is_the_removal_refusal() {
    let mut x = fx();
    let p = x.pickup;
    x.set(p, "PickupParticles", Value::Object(None));
    refused(&mut x, Missing::NotRead, PARTICLES_REMOVAL, RECORD);
}

/// The slice's case: a new component from the preloaded default (whose `bAutoActivate` is clear here), a common
/// rating, a loaded template: refusal F; with the default's `bAutoActivate` set, refusal E.
#[test]
fn a_new_component_stops_at_the_initialisation_or_the_activation() {
    let mut x = fx();
    refused(&mut x, Missing::NotRead, F, RECORD_PARTICLES);
    let d = x.defaults;
    x.set(d, "bAutoActivate", Value::Bool(true));
    refused(&mut x, Missing::NotRead, E, RECORD_PARTICLES);
}

/// A held component's own values choose: active and not deactivated is E; active and deactivated is F.
#[test]
fn a_held_component_chooses_by_its_own_flags() {
    let mut x = fx();
    let c = make(&mut x.vm, COMPONENT_CLASS);
    let p = x.pickup;
    x.set(p, "PickupParticleComponent", Value::Object(Some(c)));
    x.set(c, "bIsActive", Value::Bool(true));
    refused(&mut x, Missing::NotRead, E, RECORD_PARTICLES);
    x.set(c, "bWasDeactivated", Value::Bool(true));
    refused(&mut x, Missing::NotRead, F, RECORD_PARTICLES);
    // A held component skips the preload: without it, still F.
    let mut y = fx_with(ParityMode::Off, false, false);
    let c = make(&mut y.vm, COMPONENT_CLASS);
    let p = y.pickup;
    y.set(p, "PickupParticleComponent", Value::Object(Some(c)));
    refused(&mut y, Missing::NotRead, F, RECORD_PARTICLES);
}

/// Refusal G: a new component with no preload in the world.
#[test]
fn a_new_component_without_the_preload_is_refusal_g() {
    let mut x = fx_with(ParityMode::Off, false, true);
    refused(&mut x, Missing::NotRead, G, RECORD_PARTICLES);
}

/// The attach plan from the default's values: no scene layer is #520's `ATTACH`; the default `bAttached` is
/// `ATTACH_DETACH`; a pickup being destroyed attaches nothing, so neither applies and the native goes on to F.
#[test]
fn the_attach_plan_uses_part_1s_guards_and_refusals() {
    let mut x = fx_with(ParityMode::Off, true, false);
    refused(&mut x, Missing::LayerAbsent("a scene that components attach to: render, collision and physics registration (the port has none)"), ATTACH, RECORD);
    let p = x.pickup;
    x.set(p, "bDeleteMe", Value::Bool(true));
    refused(&mut x, Missing::NotRead, F, RECORD_PARTICLES);
    let mut x = fx();
    let d = x.defaults;
    x.set(d, "bAttached", Value::Bool(true));
    refused(&mut x, Missing::NotRead, ATTACH_DETACH, RECORD);
}

/// Refusal A: the band holding the level rates `RARITY_Rainbow` (by name; 1 in this toy's enum). It comes after G and
/// the attach plan, and before B.
#[test]
fn a_rainbow_rating_is_refusal_a_in_its_place() {
    let mut x = fx();
    let p = x.pickup;
    x.set(p, "InventoryRarityLevel", Value::Int(12));
    refused(&mut x, Missing::NotRead, A, RECORD_PARTICLES);
    let u = x.unloaded;
    x.set(p, "PickupParticles", Value::Object(Some(u)));
    refused(&mut x, Missing::NotRead, A, RECORD_PARTICLES);
    let mut y = fx_with(ParityMode::Off, false, true);
    let p = y.pickup;
    y.set(p, "InventoryRarityLevel", Value::Int(12));
    refused(&mut y, Missing::NotRead, G, RECORD_PARTICLES);
    // A level no band holds is RARITY_Unknown: not Rainbow.
    let mut z = fx();
    let p = z.pickup;
    z.set(p, "InventoryRarityLevel", Value::Int(40));
    refused(&mut z, Missing::NotRead, F, RECORD_PARTICLES);
}

/// Refusal B: a template the store has not loaded, refused without loading it, before E or F.
#[test]
fn an_unloaded_template_is_refusal_b_and_stays_unloaded() {
    let mut x = fx();
    let d = x.defaults;
    x.set(d, "bAutoActivate", Value::Bool(true));
    let (p, u) = (x.pickup, x.unloaded);
    x.set(p, "PickupParticles", Value::Object(Some(u)));
    refused(&mut x, Missing::NotRead, B, RECORD_PARTICLES);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, u), "not loaded");
}

/// The template's `LODSettings` entries are checked for a bool `bLit` before E or F.
#[test]
fn the_template_lod_settings_are_checked_before_the_choice() {
    let mut x = fx();
    let t = x.template;
    let class = x.vm.space.heap.get(t).unwrap().class;
    let p = x.vm.space.prop(class, "LODSettings").unwrap();
    let PropKind::Array(inner) = x.vm.space.classes.decl(p).unwrap().kind.clone() else { panic!() };
    let lod = with(&x.vm, &x.vm.space.classes.zero_value(&inner), &[("bLit", Value::Bool(true))]);
    x.set(t, "LODSettings", Value::array(vec![lod]));
    refused(&mut x, Missing::NotRead, F, RECORD_PARTICLES);
}

/// Reference binds nothing.
#[test]
fn reference_binds_nothing() {
    let x = fx_with(ParityMode::Reference, true, true);
    assert!(!x.vm.has_native(SPAWN_PICKUP_PARTICLES));
}
