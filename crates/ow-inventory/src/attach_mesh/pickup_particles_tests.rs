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
use ow_vm::api::{StubPolicy, WorldTimeQuery};

use super::*;
use crate::attach_mesh::particle_activation::*;
use crate::attach_mesh::{ATTACH, ATTACH_DETACH, REATTACH_OWNER, REATTACH_SCENE, SCENE_LAYER, SPAWN_PICKUP_PARTICLES, add_empty_scene, register};

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);
const PARTICLES_FN: &str = "WillowGame.WillowPickup.SpawnPickupParticles";

const IMPORTS: [&str; 12] = [
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
    "NameProperty",
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
    for b in ["bDeleteMe", "bStatic", "bTickIsDisabled"] {
        t.prop("Bool", actor, b, 0, 0);
    }
    t.prop("Struct", actor, "Rotation", 0, rotator);
    t.prop("Struct", component, "Scene", 0, pointer);
    t.prop("Object", component, "Owner", 0, actor);
    for b in ["bAttached", "bNeedsReattach", "bNeedsUpdateTransform"] {
        t.prop("Bool", component, b, 0, 0);
    }
    let primitive = t.class("PrimitiveComponent", component);
    t.prop("Byte", primitive, "DetailMode", 0, 0);
    for b in ["HiddenGame", "bIgnoreOwnerHidden", "bAcceptsLights", "AbsoluteTranslation", "AbsoluteRotation", "AbsoluteScale"] {
        t.prop("Bool", primitive, b, 0, 0);
    }
    t.prop("Float", primitive, "LastRenderTime", 0, 0);
    t.prop("Struct", primitive, "Translation", 0, vector);
    t.prop("Struct", primitive, "Rotation", 0, rotator);
    let ak_event = t.class("AkEvent", object);
    let ak_component = t.class("AkComponent", component);
    let system = t.class("ParticleSystem", object);
    t.prop("Object", system, "PhysxParticleSystemRef", 0, system);
    for b in ["bLoadIfPhysXLevel0", "bLoadIfPhysXLevel1", "bLoadIfPhysXLevel2", "bUseDelayRange", "bSkipSpawnCountCheck"] {
        t.prop("Bool", system, b, 0, 0);
    }
    for f in ["Delay", "DelayLow", "fAudioDelaySeconds", "WarmupTime"] {
        t.prop("Float", system, f, 0, 0);
    }
    t.prop("Byte", system, "LODMethod", 0, 0);
    toy_array_of(&mut t, system, "LODDistances", "Float", 0);
    let lod = toy_struct(&mut t, system, "ParticleSystemLOD", None);
    t.prop("Bool", lod, "bLit", 0, 0);
    toy_array_of(&mut t, system, "LODSettings", "Struct", lod);
    toy_array_of(&mut t, system, "Emitters", "Object", object);
    for e in ["StartAudioEvent", "StopAudioEvent", "StartLoopingAudioEvent", "StopLoopingAudioEvent"] {
        t.prop("Object", system, e, 0, ak_event);
    }
    let psc = t.class("ParticleSystemComponent", primitive);
    t.prop("Object", psc, "Template", 0, system);
    for b in [
        "bAutoActivate", "bIsActive", "bWasDeactivated", "bJustAttached", "bWasCompleted", "bSuppressSpawning",
        "bHasBeenActivated", "bLODUpdatePending", "bOverrideLODMethod", "bWarmingUp", "bSkipUpdateDynamicDataDuringTick",
        "bRecacheViewRelevance", "bSkipSpawnCountCheck", "bIsViewRelevanceDirty", "AudioEnabled", "bStartEventPlayed",
    ] {
        t.prop("Bool", psc, b, 0, 0);
    }
    for f in ["EmitterDelay", "AudioEventDelay", "AccumTickTime", "WarmupTime"] {
        t.prop("Float", psc, f, 0, 0);
    }
    t.prop("Int", psc, "LODLevel", 0, 0);
    t.prop("Byte", psc, "LODMethod", 0, 0);
    let playing = toy_struct(&mut t, psc, "AkPlayingInfo", None);
    t.prop("Object", playing, "SourceComponent", 0, ak_component);
    t.prop("Int", playing, "AkPlayingId", 0, 0);
    t.prop("Struct", psc, "LoopingAkPlayingInfo", 0, playing);
    toy_array_of(&mut t, psc, "EmitterInstances", "Struct", pointer);
    toy_array_of(&mut t, psc, "SMComponents", "Object", object);
    toy_array_of(&mut t, psc, "SMMaterialInterfaces", "Object", object);
    let param = toy_struct(&mut t, psc, "ParticleSysParam", None);
    t.prop("Name", param, "Name", 0, 0);
    t.prop("Byte", param, "ParamType", 0, 0);
    t.prop("Struct", param, "Vector", 0, vector);
    toy_array_of(&mut t, psc, "InstanceParameters", "Struct", param);
    let other = toy_struct(&mut t, psc, "ToyOtherParam", None);
    t.prop("Name", other, "Name", 0, 0);
    t.prop("Byte", other, "ParamType", 0, 0);
    t.prop("Struct", other, "Vector", 0, vector);
    t.class("WillowInventory", actor);
    let definition = t.class("WillowInventoryDefinition", object);
    t.prop("Struct", definition, "PickupFlagOffset", 0, vector);
    t.prop("Struct", definition, "LootBeamColorOverride", 0, color);
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Target", 0, system);
    // Links `ToyOtherParam` (review r2 of #617's wrong-type entry).
    t.prop("Struct", holder, "ToyOther", 0, other);
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
    globals: ObjRef,
    definition: ObjRef,
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

/// The fixture's world options.
#[derive(Clone, Copy)]
struct Opts {
    mode: ParityMode,
    preload: bool,
    scene: bool,
    flags: Option<ParticleEngineFlags>,
    clock: bool,
}

const OPTS: Opts = Opts { mode: ParityMode::Off, preload: true, scene: true, flags: Some(ParticleEngineFlags::SLICE), clock: true };

fn fx_with(mode: ParityMode, preload: bool, scene: bool) -> Fx {
    fx_opts(Opts { mode, preload, scene, ..OPTS })
}

fn fx_opts(o: Opts) -> Fx {
    let mode = o.mode;
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
    if o.scene {
        add_empty_scene(&mut vm).unwrap();
    }
    if let Some(flags) = o.flags {
        add_particle_engine_flags(&mut vm, flags).unwrap();
    }
    if o.clock {
        // An explicitly absent world: the time is 0 (ow_vm's world_time_seconds).
        vm.add_service(WorldTimeQuery(|_| Ok(None))).unwrap();
    }
    let class = vm.find_class(COMPONENT_CLASS).unwrap();
    let defaults = vm.defs.class_default(&mut vm.space, COMPONENT_CLASS).unwrap();
    if o.preload {
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
    let mut x = Fx { vm, dir, pickup, template, unloaded, defaults, gd, globals, definition };
    x.bands(&[(0, 9, 2), (10, 19, 1)]);
    x
}

fn fx() -> Fx {
    fx_opts(OPTS)
}

/// The slice's shape (the note's section 5): an auto-activating, audio-enabled default; a template with one LOD
/// distance, one `LODSettings` entry and `LODMethod` 1.
fn slice() -> Fx {
    slice_with(OPTS)
}

fn slice_with(o: Opts) -> Fx {
    let mut x = fx_opts(o);
    let d = x.defaults;
    x.set(d, "bAutoActivate", Value::Bool(true));
    x.set(d, "AudioEnabled", Value::Bool(true));
    let t = x.template;
    x.set(t, "LODMethod", Value::Byte(1));
    x.set(t, "LODDistances", Value::array(vec![Value::Float(0.0)]));
    let lod = x.lod(false);
    x.set(t, "LODSettings", Value::array(vec![lod]));
    x
}

impl Fx {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        set(&mut self.vm, o, name, v);
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        get(&self.vm, o, name)
    }

    /// `o.name := v` past the heap's write checks (a fixture of malformed data).
    fn put(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        *self.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }

    /// The zero element of `o`'s array field `name`, with fields replaced.
    fn element(&self, o: ObjRef, name: &str, fields: &[(&str, Value)]) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        let PropKind::Array(inner) = self.vm.space.classes.decl(p).unwrap().kind.clone() else { panic!() };
        with(&self.vm, &self.vm.space.classes.zero_value(&inner), fields)
    }

    fn lod(&self, lit: bool) -> Value {
        self.element(self.template, "LODSettings", &[("bLit", Value::Bool(lit))])
    }

    /// The globals definition's bands: (`MinLevel`, `MaxLevel`, `RarityRating`), colour (10 × rating, 20, 30, 255).
    fn bands(&mut self, bands: &[(i32, i32, u8)]) {
        let gd = self.gd;
        let color = |x: &Fx, r: u8| {
            let zero = x.element(gd, "RarityLevelColors", &[]);
            let c = ow_attr::eval::field(&x.vm.space, &zero, "Color").unwrap().clone();
            with(&x.vm, &c, &[("R", Value::Byte(10 * r)), ("G", Value::Byte(20)), ("B", Value::Byte(30)), ("A", Value::Byte(255))])
        };
        let entries: Vec<Value> = bands
            .iter()
            .map(|&(min, max, r)| {
                let c = color(self, r);
                self.element(
                    gd,
                    "RarityLevelColors",
                    &[("MinLevel", Value::Int(min)), ("MaxLevel", Value::Int(max)), ("RarityRating", Value::Byte(r)), ("Color", c)],
                )
            })
            .collect();
        self.set(gd, "RarityLevelColors", Value::array(entries));
    }

    fn call(&mut self) -> Result<Value, VmError> {
        let f = self.vm.find_function(PARTICLES_FN)?;
        let p = self.pickup;
        Ok(self.vm.call(f, Some(p), vec![])?.ret)
    }

    /// A held component of the particle class, made from the defaults' values and set on the pickup.
    fn held(&mut self) -> ObjRef {
        let c = make(&mut self.vm, COMPONENT_CLASS);
        let p = self.pickup;
        self.set(p, "PickupParticleComponent", Value::Object(Some(c)));
        c
    }

    /// The component on the pickup after a call.
    fn component(&self) -> ObjRef {
        let Value::Object(Some(c)) = self.get(self.pickup, "PickupParticleComponent") else { panic!("no component") };
        c
    }

    /// The pickup's and the defaults' values, the heap size and #518's three counters.
    fn state(&self) -> (Vec<Value>, Vec<Value>, usize, [u64; 3]) {
        let all = |o: ObjRef| -> Vec<Value> { self.vm.space.heap.get(o).unwrap().slots.to_vec() };
        let counters = [self.vm.space.heap.committed_writes(), self.vm.rng.draws(), self.vm.defs.load_transitions()];
        (all(self.pickup), all(self.defaults), self.vm.space.heap.handles().len(), counters)
    }

    /// The names of the planned writes, in order (the preflight, nothing applied).
    fn plan(&mut self) -> Vec<&'static str> {
        let p = self.pickup;
        let plan = preflight(&mut self.vm, p).unwrap();
        let mut names: Vec<&'static str> = plan.before.iter().map(|w| w.1).collect();
        names.push("InstanceParameters");
        names.extend(plan.after.iter().map(|w| w.1));
        names
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

/// The call is a data error naming `needle`, and changes nothing.
fn data_error(x: &mut Fx, needle: &str) {
    let before = x.state();
    match x.call() {
        Err(e @ VmError::Script { .. }) => assert!(e.to_string().contains(needle), "{needle}: {e}"),
        other => panic!("{needle}: expected a data error, got {other:?}"),
    }
    assert_eq!(x.state(), before, "{needle}: nothing written");
}

const PARTICLE: Missing = Missing::LayerAbsent(PARTICLE_LAYER);
const AUDIO: Missing = Missing::LayerAbsent(AUDIO_LAYER);

/// Step 1: no particle system is #520's removal refusal.
#[test]
fn no_particle_system_is_the_removal_refusal() {
    let mut x = fx();
    let p = x.pickup;
    x.set(p, "PickupParticles", Value::Object(None));
    refused(&mut x, Missing::NotRead, PARTICLES_REMOVAL, RECORD);
}

/// The note's test 1, the slice's case: a new component, made with the pickup as outer and attached by part 1, is
/// activated; every write lands in the native's order; `EmitterInstances` stays empty; no draw and no load.
#[test]
fn the_slice_case_activates_a_new_component_in_order() {
    let mut x = slice();
    let names = x.plan();
    let want = [
        // The new component and part 1's attach.
        "PickupParticleComponent", "Translation", "bNeedsReattach", "bNeedsUpdateTransform", "Owner", "Components",
        // SetTemplate, its reset (the stop-audio step on the old template, None) and the template.
        "bIsViewRelevanceDirty", "bWasCompleted", "SMComponents", "SMMaterialInterfaces", "bIsActive",
        "bStartEventPlayed", "LoopingAkPlayingInfo", "Template", "WarmupTime",
        // The activation: delays, the stop-audio step on T, the four flags, the transform request.
        "EmitterDelay", "AudioEventDelay", "bStartEventPlayed", "LoopingAkPlayingInfo", "bWasCompleted",
        "bSuppressSpawning", "bWasDeactivated", "bIsActive", "bNeedsUpdateTransform",
        // The initialisation inside it (attached by part 1, so AccumTickTime).
        "WarmupTime", "bSkipSpawnCountCheck", "SMComponents", "SMMaterialInterfaces", "bWasCompleted", "AccumTickTime",
        // bHasBeenActivated, the owner part's transform branch, the end of the activation.
        "bHasBeenActivated", "bNeedsUpdateTransform", "AccumTickTime", "LastRenderTime",
        // bAcceptsLights and the reattach request (T changed), then #581's steps 6 to 10.
        "bAcceptsLights", "bNeedsReattach", "AbsoluteTranslation", "AbsoluteRotation", "AbsoluteScale", "Rotation",
        "InstanceParameters", "bNeedsUpdateTransform",
    ];
    assert_eq!(names, want);
    let [_, draws, loads] = x.state().3;
    x.call().unwrap();
    let c = x.component();
    assert_ne!(c, x.defaults, "a new object");
    assert_eq!(x.vm.space.heap.get(c).unwrap().outer, Some(x.pickup), "the pickup is the outer");
    let Value::Array(list) = x.get(x.pickup, "Components") else { panic!() };
    assert_eq!(list.as_ref(), &vec![Value::Object(Some(c))], "attached by part 1, no default left in the list");
    for (name, v) in [
        ("Owner", Value::Object(Some(x.pickup))),
        ("Template", Value::Object(Some(x.template))),
        ("bIsActive", Value::Bool(true)),
        ("bHasBeenActivated", Value::Bool(true)),
        ("bIsViewRelevanceDirty", Value::Bool(true)),
        ("bNeedsReattach", Value::Bool(true)),
        ("bNeedsUpdateTransform", Value::Bool(true)),
        ("AbsoluteRotation", Value::Bool(true)),
        ("bAcceptsLights", Value::Bool(false)),
        ("LastRenderTime", Value::Float(0.0)),
        ("bAttached", Value::Bool(false)),
    ] {
        assert_eq!(x.get(c, name), v, "{name}");
    }
    assert_eq!(x.get(c, "EmitterInstances"), Value::array(Vec::new()), "ruling 4: left empty");
    // The colour parameter: the common band's colour (R 20, G 20, B 30), appended once.
    let Value::Array(params) = x.get(c, "InstanceParameters") else { panic!() };
    assert_eq!(params.len(), 1);
    let name = ow_attr::eval::field(&x.vm.space, &params[0], "Name").unwrap().clone();
    assert_eq!(name, Value::Name(x.vm.space.names.find(COLOR_PARAMETER).unwrap()));
    assert_eq!(ow_attr::eval::field(&x.vm.space, &params[0], "ParamType").unwrap(), &Value::Byte(PARAM_VECTOR));
    let v = ow_attr::eval::field(&x.vm.space, &params[0], "Vector").unwrap().clone();
    let axis = |a: &str| ow_attr::eval::field(&x.vm.space, &v, a).unwrap().clone();
    // byte × 1/255 (#581 step 7), not byte / 255: the two can differ in the last bit.
    let unit = |b: f32| Value::Float(b * (1.0 / 255.0));
    assert_eq!((axis("X"), axis("Y"), axis("Z")), (unit(20.0), unit(20.0), unit(30.0)));
    let [_, draws_after, loads_after] = x.state().3;
    assert_eq!((draws_after, loads_after), (draws, loads), "#518: only writes moved");
    // A second call finds the reattach the first requested still pending (the port runs no end-of-frame reattach):
    // part 1's refusal, before any write. With it cleared, as the engine's reattach would, the component is reused and
    // the parameter updated in place.
    refused(&mut x, Missing::NotRead, REATTACH_OWNER, RECORD);
    x.set(c, "bNeedsReattach", Value::Bool(false));
    x.call().unwrap();
    assert_eq!(x.component(), c);
    let Value::Array(params) = x.get(c, "InstanceParameters") else { panic!() };
    assert_eq!(params.len(), 1, "updated, not appended");
}

/// The note's test 2: `bAutoActivate` clear is the initialisation: its two copies and `bWasCompleted`, and with C
/// neither `bAttached` nor in its owner's `Components`, no `AccumTickTime` and no activation.
#[test]
fn the_initialisation_branch_copies_and_does_not_activate_unattached() {
    let mut x = fx();
    let c = x.held();
    let t = x.template;
    x.set(t, "WarmupTime", Value::Float(0.0));
    x.set(t, "bSkipSpawnCountCheck", Value::Bool(true));
    x.set(c, "AccumTickTime", Value::Float(5.0));
    x.set(c, "bWasCompleted", Value::Bool(true));
    x.call().unwrap();
    assert_eq!(x.get(c, "bSkipSpawnCountCheck"), Value::Bool(true));
    assert_eq!(x.get(c, "bWasCompleted"), Value::Bool(false));
    assert_eq!(x.get(c, "AccumTickTime"), Value::Float(5.0), "not attached: no AccumTickTime");
    assert_eq!(x.get(c, "bIsActive"), Value::Bool(false));
    assert_eq!(x.get(c, "bHasBeenActivated"), Value::Bool(false));
    // A new component attached by part 1 does get AccumTickTime.
    let mut y = fx();
    let d = y.defaults;
    y.set(d, "AccumTickTime", Value::Float(5.0));
    y.call().unwrap();
    let c = y.component();
    assert_eq!(y.get(c, "AccumTickTime"), Value::Float(0.0));
    assert_eq!(y.get(c, "bIsActive"), Value::Bool(false));
}

/// The note's test 3: the initialisation's auto-activation for an attached component that is not active. SetTemplate
/// never reaches it (its choice already activates such a component), so the planner is called directly.
#[test]
fn the_initialisations_auto_activation_runs_when_attached() {
    let mut x = slice();
    let c = make(&mut x.vm, COMPONENT_CLASS);
    let p = x.pickup;
    x.set(c, "bAutoActivate", Value::Bool(true));
    x.set(c, "Owner", Value::Object(Some(p)));
    x.set(p, "Components", Value::array(vec![Value::Object(Some(c))]));
    let t = x.template;
    let mut s = Setup::new(&x.vm, c, false, ParticleEngineFlags::SLICE);
    s.initialise(t).unwrap();
    let names: Vec<&str> = s.writes.iter().map(|w| w.1).collect();
    assert!(names.contains(&"bHasBeenActivated") && names.contains(&"LastRenderTime"), "{names:?}");
    // Not attached: no activation.
    let lone = make(&mut x.vm, COMPONENT_CLASS);
    x.set(lone, "bAutoActivate", Value::Bool(true));
    let mut s = Setup::new(&x.vm, lone, false, ParticleEngineFlags::SLICE);
    s.initialise(t).unwrap();
    assert!(!s.writes.iter().any(|w| w.1 == "bHasBeenActivated"));
}

/// The note's test 4, part one: the template's refusals, each before any write.
#[test]
fn the_templates_refusals_come_before_any_write() {
    // The PhysX swap.
    let mut x = slice();
    let (t, other) = (x.template, make(&mut x.vm, "Engine.ParticleSystem"));
    x.set(t, "PhysxParticleSystemRef", Value::Object(Some(other)));
    x.set(t, "bLoadIfPhysXLevel1", Value::Bool(true));
    refused(&mut x, Missing::NotRead, PHYSX, RECORD_ACTIVATION);
    // A reference with no level flag is no swap.
    x.set(t, "bLoadIfPhysXLevel1", Value::Bool(false));
    x.call().unwrap();
    // The LOD for location.
    let mut x = slice();
    let t = x.template;
    x.set(t, "LODDistances", Value::array(vec![Value::Float(0.0), Value::Float(100.0)]));
    refused(&mut x, Missing::LayerAbsent(CAMERA_LAYER), LOD_LOCATION, RECORD_ACTIVATION);
    // The delay range (hub ruling 2).
    let mut x = slice();
    let t = x.template;
    x.set(t, "bUseDelayRange", Value::Bool(true));
    refused(&mut x, Missing::NotPorted, DELAY_RANGE, RECORD_ACTIVATION);
    // T's stop audio events, in the activation.
    let mut x = slice();
    let (t, event) = (x.template, make(&mut x.vm, "Engine.AkEvent"));
    x.set(t, "StopLoopingAudioEvent", Value::Object(Some(event)));
    refused(&mut x, AUDIO, TEMPLATE_AUDIO, RECORD_ACTIVATION);
    // With AudioEnabled clear the step does not run.
    let d = x.defaults;
    x.set(d, "AudioEnabled", Value::Bool(false));
    x.call().unwrap();
    // The warm-up.
    let mut x = slice();
    let t = x.template;
    x.set(t, "WarmupTime", Value::Float(0.5));
    refused(&mut x, PARTICLE, WARM_UP, RECORD_ACTIVATION);
}

/// The note's test 4, part two: the held component's refusals: existing instances, the old template's audio, an LOD
/// level that would change (the implementer's guard), and a looping sound recorded.
#[test]
fn the_held_components_refusals_come_before_any_write() {
    let mut x = slice();
    let c = x.held();
    let ptr = x.element(c, "EmitterInstances", &[]);
    x.set(c, "EmitterInstances", Value::array(vec![ptr]));
    refused(&mut x, PARTICLE, INSTANCES_RESET, RECORD_ACTIVATION);
    // The old template's stop audio events, in the reset.
    let mut x = slice();
    let c = x.held();
    let (old, event) = (make(&mut x.vm, "Engine.ParticleSystem"), make(&mut x.vm, "Engine.AkEvent"));
    x.set(old, "StopAudioEvent", Value::Object(Some(event)));
    x.set(c, "Template", Value::Object(Some(old)));
    x.set(c, "AudioEnabled", Value::Bool(true));
    refused(&mut x, AUDIO, OLD_AUDIO, RECORD_ACTIVATION);
    // An old template the store has not loaded, read by the stop-audio step (AudioEnabled set): its own refusal, and
    // it is not loaded (review r1 of #609, item a).
    let u = x.unloaded;
    x.set(c, "Template", Value::Object(Some(u)));
    refused(&mut x, Missing::NotRead, OLD_NOT_LOADED, RECORD_ACTIVATION);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, u));
    // A looping sound recorded is the audio layer too.
    let mut x = slice();
    let c = x.held();
    x.set(c, "AudioEnabled", Value::Bool(true));
    let info = with(&x.vm, &x.get(c, "LoopingAkPlayingInfo"), &[("AkPlayingId", Value::Int(4))]);
    x.set(c, "LoopingAkPlayingInfo", info);
    refused(&mut x, AUDIO, OLD_AUDIO, RECORD_ACTIVATION);
    // An LOD level that would change.
    let mut x = slice();
    let c = x.held();
    x.set(c, "bAutoActivate", Value::Bool(true));
    x.set(c, "LODLevel", Value::Int(1));
    refused(&mut x, Missing::NotPorted, LOD_CHANGE, RECORD_ACTIVATION);
}

/// The note's test 4, part three: the owner and attach refusals: C2, D, #520's ATTACH for a never-attached component
/// with no scene, part 1's REATTACH_OWNER with `bNeedsReattach`, and #581's C1 and REATTACH_SCENE.
#[test]
fn the_owner_and_attach_refusals_come_before_any_write() {
    // C2: the pickup static.
    let mut x = slice();
    let p = x.pickup;
    x.set(p, "bStatic", Value::Bool(true));
    refused(&mut x, Missing::NotRead, C2, RECORD_PARTICLES);
    // ... unless it is being deleted: then part 1 attaches nothing, and the component has no owner.
    x.set(p, "bDeleteMe", Value::Bool(true));
    x.call().unwrap();
    // D: no owner and bAttached.
    let mut x = slice();
    let c = x.held();
    x.set(c, "bAutoActivate", Value::Bool(true));
    x.set(c, "bAttached", Value::Bool(true));
    refused(&mut x, Missing::NotRead, D, RECORD_PARTICLES);
    // ATTACH: an owner whose Components lack the component, not bAttached, and no scene layer.
    let mut x = slice_with(Opts { scene: false, ..OPTS });
    let c = x.held();
    let other = make(&mut x.vm, "WillowGame.WillowPickup");
    x.set(c, "Owner", Value::Object(Some(other)));
    refused(&mut x, Missing::LayerAbsent(SCENE_LAYER), ATTACH, RECORD);
    // REATTACH_OWNER: attached (in the owner's Components) with bNeedsReattach.
    let mut x = slice();
    let c = x.held();
    let p = x.pickup;
    x.set(c, "Owner", Value::Object(Some(p)));
    x.set(p, "Components", Value::array(vec![Value::Object(Some(c))]));
    x.set(c, "bNeedsReattach", Value::Bool(true));
    refused(&mut x, Missing::NotRead, REATTACH_OWNER, RECORD);
    // C1: the initialisation branch, then SetTemplate's reattach request with a static owner.
    let mut x = fx();
    let c = x.held();
    let p = x.pickup;
    x.set(c, "Owner", Value::Object(Some(p)));
    x.set(p, "Components", Value::array(vec![Value::Object(Some(c))]));
    x.set(p, "bTickIsDisabled", Value::Bool(true));
    refused(&mut x, Missing::NotRead, REATTACH_OWNER, RECORD);
    // REATTACH_SCENE: no owner, a scene.
    let mut x = fx();
    let c = x.held();
    let scene = with(&x.vm, &x.get(c, "Scene"), &[("Dummy", Value::Int(1))]);
    x.set(c, "Scene", scene);
    refused(&mut x, Missing::NotRead, REATTACH_SCENE, RECORD);
}

/// The note's test 4, part four: the engine flags (hub ruling 1): none supplied is NotRead, a non-game session
/// NotPorted (the implementer's guard).
#[test]
fn the_engine_flags_are_a_harness_input() {
    let mut x = slice_with(Opts { flags: None, ..OPTS });
    refused(&mut x, Missing::NotRead, FLAGS, RECORD_ACTIVATION);
    let outside = ParticleEngineFlags { is_game: false, ..ParticleEngineFlags::SLICE };
    let mut x = slice_with(Opts { flags: Some(outside), ..OPTS });
    refused(&mut x, Missing::NotPorted, OUTSIDE_SESSION, RECORD_ACTIVATION);
    let editor = ParticleEngineFlags { is_editor: true, ..ParticleEngineFlags::SLICE };
    let mut x = slice_with(Opts { flags: Some(editor), ..OPTS });
    refused(&mut x, Missing::NotPorted, OUTSIDE_SESSION, RECORD_ACTIVATION);
}

/// Review r1 of #609, item a: on a held component with `AudioEnabled` clear the stop-audio step does not run, so an
/// old template that is not loaded is never read: the call plans and applies, and the old template stays unloaded.
#[test]
fn an_unloaded_old_template_is_not_read_when_audio_is_off() {
    let mut x = slice();
    let c = x.held();
    let u = x.unloaded;
    x.set(c, "Template", Value::Object(Some(u)));
    x.set(c, "AudioEnabled", Value::Bool(false));
    x.call().unwrap();
    assert!(x.vm.defs.is_unloaded(&x.vm.space, u), "never read, never loaded");
    assert_eq!(x.get(c, "Template"), Value::Object(Some(x.template)));
}

/// Review r1 of #609, item b: the LOD bias and the forced lowest LOD are ported (the note's section 3, steps 11 and 16).
/// With one LOD distance a bias of 1 clamps to level 0, so nothing changes and the call passes. The forced lowest LOD
/// writes `bOverrideLODMethod` and `LODMethod` 1 and asks for level count − 1 = 0, which also passes. With `LODLevel` 1 it
/// is a change of level, the implementer's guard.
#[test]
fn the_lod_bias_and_the_forced_lowest_lod_are_ported() {
    let bias = ParticleEngineFlags { lod_bias: 1, ..ParticleEngineFlags::SLICE };
    let mut x = slice_with(Opts { flags: Some(bias), ..OPTS });
    x.call().unwrap();
    let lowest = ParticleEngineFlags { force_lowest: true, ..ParticleEngineFlags::SLICE };
    let mut x = slice_with(Opts { flags: Some(lowest), ..OPTS });
    x.call().unwrap();
    let c = x.component();
    assert_eq!(x.get(c, "bOverrideLODMethod"), Value::Bool(true));
    assert_eq!(x.get(c, "LODMethod"), Value::Byte(1));
    let mut x = slice_with(Opts { flags: Some(lowest), ..OPTS });
    let c = x.held();
    x.set(c, "bAutoActivate", Value::Bool(true));
    x.set(c, "LODLevel", Value::Int(1));
    refused(&mut x, Missing::NotPorted, LOD_CHANGE, RECORD_ACTIVATION);
}

/// Review r1 of #617, item 2, and review r2, item 2: step 11 itself, with `game_thread_lod` clear (so the level for
/// location before the delay does not run) and two LOD distances. The forced lowest LOD asks for level 1: `LODLevel` 1
/// plans, `LODLevel` 0 is `LOD_CHANGE`. With a bias of −1 step 16 moves that level 1 to 0: `LODLevel` 0 plans and 1 is
/// `LOD_CHANGE`. With the forced lowest LOD clear, a non-automatic `LODMethod` and no bias, no level is checked.
#[test]
fn step_11_sets_the_forced_lowest_level() {
    let lowest = ParticleEngineFlags { force_lowest: true, game_thread_lod: false, ..ParticleEngineFlags::SLICE };
    let fixed = ParticleEngineFlags { game_thread_lod: false, ..ParticleEngineFlags::SLICE };
    let biased = ParticleEngineFlags { lod_bias: -1, ..lowest };
    let cases = [(lowest, 1, true), (lowest, 0, false), (biased, 0, true), (biased, 1, false), (fixed, 0, true), (fixed, 1, true)];
    for (flags, level, ok) in cases {
        let mut x = slice_with(Opts { flags: Some(flags), ..OPTS });
        let t = x.template;
        x.set(t, "LODDistances", Value::array(vec![Value::Float(0.0), Value::Float(100.0)]));
        let c = x.held();
        x.set(c, "bAutoActivate", Value::Bool(true));
        x.set(c, "LODLevel", Value::Int(level));
        if ok {
            x.call().unwrap_or_else(|e| panic!("force_lowest {} bias {} level {level}: {e}", flags.force_lowest, flags.lod_bias));
            assert_eq!(x.get(c, "LODLevel"), Value::Int(level), "no level written");
        } else {
            refused(&mut x, Missing::NotPorted, LOD_CHANGE, RECORD_ACTIVATION);
        }
    }
}

/// Review r1 of #609, item c, and review r1 of #617, item 1: a non-zero `AkPlayingId` counts as a recorded looping
/// sound (the audio layer is absent); a `SourceComponent` with a zero playing id is the test the note did not read,
/// `NotYet { NotRead }`, before any write; and that refusal interns no parameter name (item 3).
#[test]
fn a_looping_sound_is_recorded_by_its_playing_id_only() {
    let mut x = slice();
    let c = x.held();
    x.set(c, "AudioEnabled", Value::Bool(true));
    let source = make(&mut x.vm, "Engine.AkComponent");
    let info = with(&x.vm, &x.get(c, "LoopingAkPlayingInfo"), &[("SourceComponent", Value::Object(Some(source)))]);
    x.set(c, "LoopingAkPlayingInfo", info);
    assert_eq!(x.vm.space.names.find(COLOR_PARAMETER), None, "the fixture has no RColor name");
    refused(&mut x, Missing::NotRead, LOOPING_UNREAD, RECORD_ACTIVATION);
    assert_eq!(x.vm.space.names.find(COLOR_PARAMETER), None, "not interned by a refusal");
    let mut x = slice();
    let c = x.held();
    x.set(c, "AudioEnabled", Value::Bool(true));
    let info = with(&x.vm, &x.get(c, "LoopingAkPlayingInfo"), &[("AkPlayingId", Value::Int(7))]);
    x.set(c, "LoopingAkPlayingInfo", info);
    refused(&mut x, Missing::LayerAbsent(AUDIO_LAYER), OLD_AUDIO, RECORD_ACTIVATION);
}

/// Review r1 of #609, item d: the plan holds the whole `InstanceParameters` value (the "RColor" entry built), and the
/// apply phase writes exactly that value.
#[test]
fn the_parameter_value_is_built_in_the_plan() {
    let mut x = slice();
    let p = x.pickup;
    let plan = preflight(&mut x.vm, p).unwrap();
    let (planned, patch) = plan.parameter.clone().expect("a parameter value");
    assert_eq!(patch, Some(0), "the appended entry waits for its name");
    let items = planned.elements().expect("an array").to_vec();
    // The preflight interns nothing (review r2 of #617): the appended entry holds the placeholder name.
    assert_eq!(x.vm.space.names.find(COLOR_PARAMETER), None, "not interned by the plan");
    assert!(items.iter().any(|e| matches!(field(&x.vm, e, "Name"), Ok(Value::Name(n)) if n == ow_object::ids::NameId::NONE)), "{items:?}");
    let mut y = slice();
    y.call().unwrap();
    let c = y.component();
    assert_eq!(y.get(c, "InstanceParameters").elements().map(<[Value]>::len), Some(items.len()));
    // The apply phase interned the real name and set it on the appended entry.
    let name = y.vm.space.names.find(COLOR_PARAMETER).expect("interned by the write");
    let written = y.get(c, "InstanceParameters").elements().unwrap().to_vec();
    assert!(written.iter().any(|e| matches!(field(&y.vm, e, "Name"), Ok(Value::Name(n)) if n == name)), "{written:?}");
}

/// Review r2 of #617, item 1: a held component whose `InstanceParameters` holds an entry with the expected fields but
/// another struct type passes the field reads and fails the late `fits`. The call is a data error, and the name table
/// and #518's counters are unchanged: no "RColor" was interned.
#[test]
fn a_late_fits_failure_interns_no_parameter_name() {
    let mut x = slice();
    let c = x.held();
    let ty = x.vm.space.classes.struct_by_path("Engine.ParticleSystemComponent.ToyOtherParam").expect("the toy struct");
    let wrong = x.vm.space.classes.zero_value(&PropKind::Struct(ty));
    x.put(c, "InstanceParameters", Value::array(vec![wrong]));
    let names = x.vm.space.names.len();
    assert_eq!(x.vm.space.names.find(COLOR_PARAMETER), None);
    data_error(&mut x, "InstanceParameters cannot take the value");
    assert_eq!(x.vm.space.names.find(COLOR_PARAMETER), None, "not interned");
    assert_eq!(x.vm.space.names.len(), names, "the name table is unchanged");
}

/// The note's test 5: a template object (the class default itself, held): SetTemplate's own writes only, no
/// activation even with `bAutoActivate` set.
#[test]
fn a_template_object_gets_set_templates_own_writes_only() {
    let mut x = slice();
    let (p, d) = (x.pickup, x.defaults);
    x.set(p, "PickupParticleComponent", Value::Object(Some(d)));
    let mut s = Setup::new(&x.vm, d, false, ParticleEngineFlags::SLICE);
    s.set_template(x.template).unwrap();
    let names: Vec<&str> = s.writes.iter().map(|w| w.1).collect();
    assert_eq!(names, ["bIsViewRelevanceDirty", "bWasCompleted", "Template", "WarmupTime", "bAcceptsLights"]);
    x.call().unwrap();
    assert_eq!(x.get(d, "bIsActive"), Value::Bool(false));
    assert_eq!(x.get(d, "Template"), Value::Object(Some(x.template)));
}

/// The note's test 6: particles not allowed is `Template` := None in SetTemplate, then #581's later steps.
#[test]
fn particles_not_allowed_sets_no_template() {
    let off = ParticleEngineFlags { particles_allowed: false, ..ParticleEngineFlags::SLICE };
    let mut x = slice_with(Opts { flags: Some(off), ..OPTS });
    x.call().unwrap();
    let c = x.component();
    assert_eq!(x.get(c, "Template"), Value::Object(None));
    assert_eq!(x.get(c, "bIsActive"), Value::Bool(false));
    assert_eq!(x.get(c, "bIsViewRelevanceDirty"), Value::Bool(false), "nothing else of SetTemplate");
    assert_eq!(x.get(c, "AbsoluteRotation"), Value::Bool(true), "#581's step 6 still runs");
}

/// The note's test 7: `DetailMode` above the system's skips the activation's steps; `LastRenderTime` is still written.
#[test]
fn a_detail_mode_above_the_systems_skips_the_activation() {
    let mut x = slice();
    let d = x.defaults;
    x.set(d, "DetailMode", Value::Byte(2));
    x.set(d, "LastRenderTime", Value::Float(7.0));
    x.call().unwrap();
    let c = x.component();
    assert_eq!(x.get(c, "LastRenderTime"), Value::Float(0.0));
    assert_eq!(x.get(c, "bIsActive"), Value::Bool(false));
    assert_eq!(x.get(c, "bHasBeenActivated"), Value::Bool(false));
}

/// The note's test 8: a world with no clock refuses `LastRenderTime` before any write.
#[test]
fn a_world_without_a_clock_refuses_last_render_time() {
    let mut x = slice_with(Opts { clock: false, ..OPTS });
    refused(&mut x, Missing::LayerAbsent(CLOCK_LAYER), WORLD_TIME, RECORD_ACTIVATION);
}

/// Refusal G: a new component with no preload in the world.
#[test]
fn a_new_component_without_the_preload_is_refusal_g() {
    let mut x = fx_with(ParityMode::Off, false, true);
    refused(&mut x, Missing::NotRead, G, RECORD_PARTICLES);
}

/// The attach plan from the default's values: no scene layer is #520's `ATTACH`; the default `bAttached` is
/// `ATTACH_DETACH`; a pickup being destroyed attaches nothing, so neither applies.
#[test]
fn the_attach_plan_uses_part_1s_guards_and_refusals() {
    let mut x = fx_with(ParityMode::Off, true, false);
    refused(&mut x, Missing::LayerAbsent(SCENE_LAYER), ATTACH, RECORD);
    let p = x.pickup;
    x.set(p, "bDeleteMe", Value::Bool(true));
    x.call().unwrap();
    let mut x = fx();
    let d = x.defaults;
    x.set(d, "bAttached", Value::Bool(true));
    refused(&mut x, Missing::NotRead, ATTACH_DETACH, RECORD);
}

/// Refusal A: the band holding the level rates `RARITY_Rainbow` (by name; 1 in this toy's enum). It comes after G and
/// the attach plan, and before B. With no globals definition held, nothing is read (review r1 of #593).
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
    // A level no band holds is RARITY_Unknown: not Rainbow, and opaque white.
    let mut z = fx();
    let p = z.pickup;
    z.set(p, "InventoryRarityLevel", Value::Int(40));
    z.call().unwrap();
    // No globals definition: no rarity read, white.
    let mut w = fx();
    let (p, g) = (w.pickup, w.globals);
    w.set(p, "InventoryRarityLevel", Value::Int(12));
    w.set(g, "TheGlobalsDefinition", Value::Object(None));
    w.call().unwrap();
    let c = w.component();
    let Value::Array(params) = w.get(c, "InstanceParameters") else { panic!() };
    let v = ow_attr::eval::field(&w.vm.space, &params[0], "Vector").unwrap().clone();
    assert_eq!(ow_attr::eval::field(&w.vm.space, &v, "X").unwrap(), &Value::Float(1.0));
}

/// The definition's non-zero `LootBeamColorOverride` replaces the band's colour.
#[test]
fn a_loot_beam_override_replaces_the_colour() {
    let mut x = slice();
    let def = x.definition;
    let over = with(&x.vm, &x.get(def, "LootBeamColorOverride"), &[("R", Value::Byte(255))]);
    x.set(def, "LootBeamColorOverride", over);
    x.call().unwrap();
    let c = x.component();
    let Value::Array(params) = x.get(c, "InstanceParameters") else { panic!() };
    let v = ow_attr::eval::field(&x.vm.space, &params[0], "Vector").unwrap().clone();
    let axis = |a: &str| ow_attr::eval::field(&x.vm.space, &v, a).unwrap().clone();
    assert_eq!((axis("X"), axis("Y")), (Value::Float(1.0), Value::Float(0.0)));
}

/// Refusal B: a template the store has not loaded, refused without loading it.
#[test]
fn an_unloaded_template_is_refusal_b_and_stays_unloaded() {
    let mut x = slice();
    let (p, u) = (x.pickup, x.unloaded);
    x.set(p, "PickupParticles", Value::Object(Some(u)));
    refused(&mut x, Missing::NotRead, B, RECORD_PARTICLES);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, u), "not loaded");
}

/// Review r1 of #593: data errors, each naming its field and writing nothing.
#[test]
fn malformed_inputs_are_data_errors_naming_the_field() {
    // A held component of the wrong class.
    let mut x = fx();
    let (p, t) = (x.pickup, x.template);
    x.put(p, "PickupParticleComponent", Value::Object(Some(t)));
    data_error(&mut x, "not a ParticleSystemComponent");
    // InventoryRarityLevel of the wrong kind.
    let mut x = fx();
    let p = x.pickup;
    x.put(p, "InventoryRarityLevel", Value::Bool(true));
    data_error(&mut x, "InventoryRarityLevel");
    // A rating outside the enum, and one of the wrong kind, in a band after the matching one (every entry checked).
    for bad_rating in [Value::Byte(9), Value::Int(1)] {
        let mut x = fx();
        x.bands(&[(0, 9, 2), (10, 19, 2)]);
        let gd = x.gd;
        let Value::Array(entries) = x.get(gd, "RarityLevelColors") else { panic!() };
        let mut entries = entries.as_ref().clone();
        entries[1] = with(&x.vm, &entries[1], &[("RarityRating", bad_rating)]);
        x.put(gd, "RarityLevelColors", Value::array(entries));
        data_error(&mut x, "RarityRating");
    }
    // The template's WarmupTime, LODSettings and an entry's bLit.
    let mut x = slice();
    let t = x.template;
    x.put(t, "WarmupTime", Value::Int(1));
    data_error(&mut x, "WarmupTime");
    let mut x = slice();
    let t = x.template;
    x.put(t, "LODSettings", Value::Int(1));
    data_error(&mut x, "LODSettings");
    let mut x = slice();
    let t = x.template;
    let lod = with(&x.vm, &x.lod(false), &[("bLit", Value::Int(1))]);
    x.put(t, "LODSettings", Value::array(vec![lod]));
    data_error(&mut x, "bLit");
}

/// Reference binds nothing.
#[test]
fn reference_binds_nothing() {
    let x = fx_with(ParityMode::Reference, true, true);
    assert!(!x.vm.has_native(SPAWN_PICKUP_PARTICLES));
}
