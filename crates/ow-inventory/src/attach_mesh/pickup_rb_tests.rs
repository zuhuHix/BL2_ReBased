//! Tests of `InitializePickupForRBPhysics` in Off (lane IMPL-PICKUP-RB) on three invented packages built with
//! `ow_package::testkit` (AGENTS.md rule 9). `Engine` has the actor, the components, the two enums (in orders unlike
//! the game's, so a rule that used a constant would fail), the inventory, its definition and a holder whose load makes
//! an unloaded definition handle; `GearboxFramework` the globals definition's base; `WillowGame` the weapon, the
//! pickup with its native, the globals and the globals definition. Every value is invented. AI-assisted (Claude Code).

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
use ow_package::testkit::script::{FunctionSpec, ScriptToy};
use ow_package::testkit::{Export, ints};
use ow_package::testkit::reflect::toy_struct;
use ow_vm::api::StubPolicy;

use super::*;
use crate::attach_mesh::{INIT_RB_PHYSICS, REATTACH_OWNER, REATTACH_SCENE, RECORD, register};

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);
const RB_FN: &str = "WillowGame.WillowPickup.InitializePickupForRBPhysics";

const IMPORTS: [&str; 10] = [
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
];

/// The container's bits: the nine the native sets or clears, and `Pawn`, which it keeps.
const BITS: [&str; 10] = [
    "Default",
    "Pawn",
    "WillowPickup",
    "DeadPawn",
    "GameplayPhysics",
    "EffectPhysics",
    "TossedItems",
    "TossedItemsPlayerVehicle",
    "TossedItemsEnemyVehicle",
    "TossedItemsPlayerVehicleEnemyVehicle",
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
    let pointer = toy_struct(&mut t, object, "Pointer", None);
    t.prop("Int", pointer, "Dummy", 0, 0);
    let actor = t.class("Actor", object);
    let physics = toy_enum(&mut t, actor, "EPhysics", &["PHYS_None", "PHYS_RigidBody", "PHYS_Falling"]);
    t.prop("Byte", actor, "Physics", 0, physics);
    for b in ["bDeleteMe", "bStatic", "bTickIsDisabled"] {
        t.prop("Bool", actor, b, 0, 0);
    }
    t.prop("Object", actor, "CollisionComponent", 0, object);
    let component = t.class("ActorComponent", object);
    t.prop("Struct", component, "Scene", 0, pointer);
    t.prop("Object", component, "Owner", 0, actor);
    t.prop("Bool", component, "bNeedsReattach", 0, 0);
    let primitive = t.class("PrimitiveComponent", component);
    let channel = toy_enum(&mut t, primitive, "ERBCollisionChannel", &["RBCC_Default", "RBCC_Pawn", "RBCC_WillowPickup"]);
    let container = toy_struct(&mut t, primitive, "RBCollisionChannelContainer", None);
    for b in BITS {
        t.prop("Bool", container, b, 0, 0);
    }
    t.prop("Struct", primitive, "Scale3D", 0, vector);
    for b in ["BlockRigidBody", "CollideActors", "BlockActors", "bNotifyRigidBodyCollision"] {
        t.prop("Bool", primitive, b, 0, 0);
    }
    t.prop("Byte", primitive, "ScriptRigidBodyCollisionThresholdCompiled", 0, 0);
    t.prop("Byte", primitive, "RBChannel", 0, channel);
    t.prop("Struct", primitive, "RBCollideWithChannels", 0, container);
    let asset = t.class("PhysicsAsset", object);
    let mesh = t.class("SkeletalMeshComponent", primitive);
    t.prop("Float", mesh, "PhysicsWeight", 0, 0);
    t.prop("Object", mesh, "PhysicsAsset", 0, asset);
    t.prop("Bool", mesh, "bHasPhysicsAssetInstance", 0, 0);
    t.class("WillowInventory", actor);
    let definition = t.class("WillowInventoryDefinition", object);
    t.prop("Object", definition, "PhysicsAsset", 0, asset);
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Target", 0, definition);
    let target = t.p.add_export(definition, "ToyDefinition", {
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
    let vector = t.p.add_import("Core", "ScriptStruct", object, "Vector");
    let rotator = t.p.add_import("Core", "ScriptStruct", object, "Rotator");
    let gearbox = t.p.add_import("Core", "Package", 0, "GearboxFramework");
    let gbx_definition = t.p.add_import("Core", "Class", gearbox, "GearboxGlobalsDefinition");
    let weapon = t.class("WillowWeapon", inventory);
    let data = toy_struct(&mut t, weapon, "WeaponDefinitionData", None);
    t.prop("Object", data, "WeaponTypeDefinition", 0, object);
    t.prop("Struct", weapon, "DefinitionData", 0, data);
    t.class("WeaponTypeDefinition", definition);
    let pickup = t.class("WillowPickup", actor);
    for b in ["bUseRBPhysics", "bForceRBToSleep", "bTorque", "bTorqueTemporary"] {
        t.prop("Bool", pickup, b, 0, 0);
    }
    t.prop("Float", pickup, "MaxRBAwakeTime", 0, 0);
    t.prop("Struct", pickup, "RotationRate", 0, rotator);
    t.prop("Struct", pickup, "Torque", 0, vector);
    t.prop("Object", pickup, "Inventory", 0, inventory);
    t.prop("Int", pickup, "MaxImpactEffectPlayCount", 0, 0);
    t.prop("Float", pickup, "ImpactEffectDelay", 0, 0);
    t.prop("Object", pickup, "ImpactVelocityAkRtpc", 0, object);
    let spec = FunctionSpec::new(NATIVE, "InitializePickupForRBPhysics");
    t.function(pickup, "InitializePickupForRBPhysics", &[], None, &[], &spec, |_| None);
    let globals = t.class("WillowGlobals", object);
    t.prop("Object", globals, "TheGlobalsDefinition", 0, gbx_definition);
    let gd = t.class("GlobalsDefinition", gbx_definition);
    for f in ["DropImpactEffectMax", "DropImpactEffectDelay", "DropImpactEffectThreshold"] {
        t.prop("Float", gd, f, 0, 0);
    }
    t.prop("Object", gd, "DropImpactEffectVelocityAkRtpc", 0, object);
    for f in SPIN_FIELDS {
        t.prop("Float", gd, f, 0, 0);
    }
    t.build()
}

/// A world with the pickup (`bUseRBPhysics` set, `MaxRBAwakeTime` 15), a skeletal mesh as its `CollisionComponent`
/// (owned by the pickup, `CollideActors` clear), a weapon whose definition holds a physics asset, the globals and
/// their definition (invented impact and spin values), the timers and the world query.
struct Fx {
    vm: Vm,
    dir: PathBuf,
    pickup: ObjRef,
    mesh: ObjRef,
    weapon: ObjRef,
    definition: ObjRef,
    asset: ObjRef,
    unloaded: ObjRef,
    rtpc: ObjRef,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx_mode(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-pickup-rb-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g, w) = (engine(), gearbox(), willow_game());
    let files = [("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice()), ("WillowGame.upk", w.as_slice())];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    for c in ["Engine.SkeletalMeshComponent", "Engine.ActorComponent", "WillowGame.WillowPickup", "WillowGame.GlobalsDefinition"] {
        vm.find_class(c).unwrap();
    }
    register(&mut vm).unwrap();
    vm.add_service(ow_world::timers::Timers::new()).unwrap();
    let holder = vm.defs.find(&mut vm.space, &ObjectPath::parse("Engine.ToyHolderObject").unwrap()).unwrap().unwrap();
    let Value::Object(Some(unloaded)) = get(&vm, holder, "Target") else { panic!("the holder's target") };
    assert!(vm.defs.is_unloaded(&vm.space, unloaded), "an unloaded definition handle");
    let pickup = make(&mut vm, "WillowGame.WillowPickup");
    set(&mut vm, pickup, "bUseRBPhysics", Value::Bool(true));
    set(&mut vm, pickup, "MaxRBAwakeTime", Value::Float(15.0));
    let mesh = make(&mut vm, "Engine.SkeletalMeshComponent");
    set(&mut vm, mesh, "Owner", Value::Object(Some(pickup)));
    set(&mut vm, pickup, "CollisionComponent", Value::Object(Some(mesh)));
    let asset = make(&mut vm, "Engine.PhysicsAsset");
    let definition = make(&mut vm, "WillowGame.WeaponTypeDefinition");
    set(&mut vm, definition, "PhysicsAsset", Value::Object(Some(asset)));
    let weapon = make(&mut vm, "WillowGame.WillowWeapon");
    set(&mut vm, pickup, "Inventory", Value::Object(Some(weapon)));
    let globals = make(&mut vm, "WillowGame.WillowGlobals");
    let gd = make(&mut vm, "WillowGame.GlobalsDefinition");
    set(&mut vm, globals, "TheGlobalsDefinition", Value::Object(Some(gd)));
    let rtpc = make(&mut vm, "Engine.Object");
    for (f, v) in [
        ("DropImpactEffectMax", 3.7),
        ("DropImpactEffectDelay", 0.5),
        ("DropImpactEffectThreshold", 41.0),
        ("PickupSpinVMin", 1.0),
        ("PickupSpinVMax", 2.0),
        ("PickupSpinHMin", 3.0),
        ("PickupSpinHMax", 4.0),
    ] {
        set(&mut vm, gd, f, Value::Float(v));
    }
    set(&mut vm, gd, "DropImpactEffectVelocityAkRtpc", Value::Object(Some(rtpc)));
    let q = ow_world::query::install(&mut vm).unwrap();
    q.borrow_mut().set_globals(globals);
    let mut x = Fx { vm, dir, pickup, mesh, weapon, definition, asset, unloaded, rtpc };
    x.set_definition(Some(definition));
    x
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

/// Field `name` of struct value `v`, by the struct's layout.
fn field(vm: &Vm, v: &Value, name: &str) -> Value {
    let Value::Struct(s) = v else { panic!("{name}: not a struct") };
    let layout = vm.space.classes.struct_layout(s.ty).unwrap();
    let (i, _) = layout.field(vm.space.names.find(name).unwrap()).unwrap();
    s.fields[i as usize].clone()
}

/// `v`, a struct, with every field `value`.
fn filled(v: &Value, value: Value) -> Value {
    let Value::Struct(s) = v else { panic!("not a struct") };
    Value::Struct(Rc::new(StructValue { ty: s.ty, fields: vec![value; s.fields.len()] }))
}

fn fx() -> Fx {
    fx_mode(ParityMode::Off)
}

impl Fx {
    fn obj(&mut self, class: &str) -> ObjRef {
        make(&mut self.vm, class)
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        set(&mut self.vm, o, name, v);
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        get(&self.vm, o, name)
    }

    fn set_definition(&mut self, d: Option<ObjRef>) {
        let data = filled(&self.get(self.weapon, "DefinitionData"), Value::Object(d));
        let w = self.weapon;
        self.set(w, "DefinitionData", data);
    }

    fn call(&mut self) -> Result<Value, VmError> {
        let f = self.vm.find_function(RB_FN)?;
        let p = self.pickup;
        Ok(self.vm.call(f, Some(p), vec![])?.ret)
    }

    /// Every value of the pickup and the mesh, and the pickup's timers: what a refusal must leave unchanged.
    fn state(&self) -> (Vec<Value>, Vec<Value>, usize, u64) {
        let all = |o: ObjRef| -> Vec<Value> { self.vm.space.heap.get(o).unwrap().slots.to_vec() };
        let timers = ow_world::timers::timers_of(&self.vm).unwrap().borrow().of(self.pickup).len();
        (all(self.pickup), all(self.mesh), timers, self.vm.rng.draws())
    }

    fn timer(&self, func: &str) -> Option<(f32, bool)> {
        let name = self.vm.space.names.find(func)?;
        let timers = ow_world::timers::timers_of(&self.vm).unwrap();
        let t = timers.borrow();
        t.of(self.pickup).iter().find(|t| t.func == name).map(|t| (t.rate, t.looping))
    }

    fn bits(&self) -> Vec<(&'static str, bool)> {
        let v = self.get(self.mesh, "RBCollideWithChannels");
        BITS.iter()
            .map(|&b| match field(&self.vm, &v, b) {
                Value::Bool(x) => (b, x),
                other => panic!("{b}: {other:?}"),
            })
            .collect()
    }

    fn vector(&self, o: ObjRef, name: &str) -> [f32; 3] {
        let v = self.get(o, name);
        ["X", "Y", "Z"].map(|f| match field(&self.vm, &v, f) {
            Value::Float(x) => x,
            other => panic!("{f}: {other:?}"),
        })
    }
}

fn refused(x: &mut Fx, missing: Missing, what: &str, record: &str) {
    let before = x.state();
    let n = match x.call() {
        Err(VmError::NotYet(n)) => n,
        other => panic!("{what}: expected NotYet, got {other:?}"),
    };
    assert_eq!((n.missing.clone(), n.what.as_ref(), n.record), (missing, what, record));
    assert!(!ow_core::notyet::is_partial_writes(&n.what), "a preflight refusal is unmarked");
    assert_eq!(x.state(), before, "{what}: nothing written, no timer, no draw");
}

/// `bUseRBPhysics` clear: nothing at all, not even the declarations past it.
#[test]
fn a_pickup_that_skips_the_rigid_body_set_up_is_left_alone() {
    let mut x = fx();
    x.set(x.pickup, "bUseRBPhysics", Value::Bool(false));
    let before = x.state();
    assert_eq!(x.call().unwrap(), Value::None);
    assert_eq!(x.state(), before);
}

/// Without a `PrimitiveComponent`: steps 1, 2, 5 and 7 only, and no globals asked for (the world's query is not
/// consulted: the pickup world here has none).
#[test]
fn without_a_primitive_component_only_the_pickups_own_steps_run() {
    for none in [true, false] {
        let mut x = fx();
        let mesh_before = x.state().1;
        if none {
            x.set(x.pickup, "CollisionComponent", Value::Object(None));
        } else {
            let plain = x.obj("Engine.ActorComponent");
            x.set(x.pickup, "CollisionComponent", Value::Object(Some(plain)));
        }
        // Globals without a definition: consulting them would refuse.
        let bare = x.obj("WillowGame.WillowGlobals");
        x.vm.service::<ow_world::query::WorldQuery>().unwrap().borrow_mut().set_globals(bare);
        x.set(x.pickup, "bForceRBToSleep", Value::Bool(true));
        let rate = filled(&x.get(x.pickup, "RotationRate"), Value::Int(5));
        x.set(x.pickup, "RotationRate", rate);
        let draws = x.vm.rng.draws();
        assert_eq!(x.call().unwrap(), Value::None, "none: {none}");
        assert_eq!(x.get(x.pickup, "bForceRBToSleep"), Value::Bool(false));
        let rate = x.get(x.pickup, "RotationRate");
        assert_eq!(rate, filled(&rate, Value::Int(0)), "RotationRate zero");
        assert_eq!(x.get(x.pickup, "Physics"), Value::Byte(1), "PHYS_RigidBody by name");
        assert_eq!(x.timer("MaxRBAwakeTimer"), Some((15.0, false)));
        assert_eq!(x.timer("CheckForRigidBodySleepState"), Some((1.0, true)));
        assert_eq!(x.timer("EnableRagdollCollision"), Some((0.3, false)));
        assert_eq!(x.vm.rng.draws(), draws, "no spin without a component");
        assert_eq!(x.state().1, mesh_before, "the mesh is not touched");
        assert_eq!(x.get(x.pickup, "MaxImpactEffectPlayCount"), Value::Int(0), "no impact fields without a component");
    }
}

/// The slice's case: a skeletal mesh owned by the pickup, `CollideActors` clear, a definition with a physics asset.
/// Every step 4 write lands, the asset branch at rest is logged, and the spin draws three times on the engine-seeded
/// stand-in only.
#[test]
fn the_asset_branch_runs_at_rest_with_every_write() {
    let mut x = fx();
    // Every bit set first, so the two clears show; `Pawn` stays set.
    let all = filled(&x.get(x.mesh, "RBCollideWithChannels"), Value::Bool(true));
    x.set(x.mesh, "RBCollideWithChannels", all);
    let draws = x.vm.rng.draws();
    assert_eq!(x.call().unwrap(), Value::None);
    assert_eq!(x.vector(x.mesh, "Scale3D"), [1.0; 3]);
    for (f, v) in [
        ("BlockRigidBody", true),
        ("CollideActors", true),
        ("bNeedsReattach", true),
        ("BlockActors", false),
        ("bHasPhysicsAssetInstance", true),
        ("bNotifyRigidBodyCollision", true),
    ] {
        assert_eq!(x.get(x.mesh, f), Value::Bool(v), "{f}");
    }
    assert_eq!(x.get(x.mesh, "PhysicsWeight"), Value::Float(1.0));
    assert_eq!(x.get(x.mesh, "PhysicsAsset"), Value::Object(Some(x.asset)));
    assert_eq!(x.get(x.pickup, "MaxImpactEffectPlayCount"), Value::Int(3), "3.7 truncated");
    assert_eq!(x.get(x.pickup, "ImpactEffectDelay"), Value::Float(0.5));
    assert_eq!(x.get(x.pickup, "ImpactVelocityAkRtpc"), Value::Object(Some(x.rtpc)));
    assert_eq!(x.get(x.mesh, "ScriptRigidBodyCollisionThresholdCompiled"), Value::Byte(10), "41 x 0.25 truncated");
    assert_eq!(x.get(x.mesh, "RBChannel"), Value::Byte(2), "RBCC_WillowPickup by name");
    let want: Vec<(&str, bool)> = BITS.iter().map(|&b| (b, !matches!(b, "WillowPickup" | "DeadPawn"))).collect();
    assert_eq!(x.bits(), want, "two cleared, seven set, Pawn kept");
    assert_eq!(x.get(x.pickup, "Physics"), Value::Byte(1));
    assert_eq!(x.timer("EnableRagdollCollision"), Some((0.3, false)));
    assert_eq!(x.vm.rng.draws() - draws, 3, "the spin's three draws");
    assert_eq!(x.vm.rng.engine_seeded().draws(), 3, "all three on the engine-seeded stand-in, so no other stream moved");
    assert!(x.vm.log.iter().any(|l| l.contains("answered at rest") && l.contains("UNVERIFIED")), "{:?}", x.vm.log);
}

/// `CollideActors` already set: no reattach. `bTorque` set: no draw, and `bTorqueTemporary` clears it.
#[test]
fn the_torque_path_makes_no_draw_and_a_set_collide_flag_no_reattach() {
    let mut x = fx();
    x.set(x.mesh, "CollideActors", Value::Bool(true));
    x.set(x.pickup, "bTorque", Value::Bool(true));
    x.set(x.pickup, "bTorqueTemporary", Value::Bool(true));
    let draws = x.vm.rng.draws();
    x.call().unwrap();
    assert_eq!(x.get(x.mesh, "bNeedsReattach"), Value::Bool(false), "no reattach");
    assert_eq!(x.get(x.pickup, "bTorque"), Value::Bool(false), "the temporary torque is cleared");
    assert_eq!(x.vm.rng.draws(), draws, "no draw");
    // A torque kept: bTorque stays.
    let mut x = fx();
    x.set(x.pickup, "bTorque", Value::Bool(true));
    x.call().unwrap();
    assert_eq!(x.get(x.pickup, "bTorque"), Value::Bool(true));
}

/// Refusal 2 with no inventory, no definition or no physics asset, unmarked, nothing written (no timer, no draw).
#[test]
fn the_branch_without_a_physics_asset_is_not_read() {
    let record = RECORD_RB;
    let mut x = fx();
    x.set(x.pickup, "Inventory", Value::Object(None));
    refused(&mut x, Missing::NotRead, NO_ASSET, record);
    let mut x = fx();
    x.set_definition(None);
    refused(&mut x, Missing::NotRead, NO_ASSET, record);
    let mut x = fx();
    let d = x.definition;
    x.set(d, "PhysicsAsset", Value::Object(None));
    refused(&mut x, Missing::NotRead, NO_ASSET, record);
}

/// Refusal 2 for a `PrimitiveComponent` that is not a `SkeletalMeshComponent`.
#[test]
fn a_primitive_that_is_not_a_skeletal_mesh_is_not_read() {
    let mut x = fx();
    let primitive = x.obj("Engine.PrimitiveComponent");
    x.set(primitive, "Owner", Value::Object(Some(x.pickup)));
    x.set(x.pickup, "CollisionComponent", Value::Object(Some(primitive)));
    x.mesh = primitive;
    refused(&mut x, Missing::NotRead, NO_ASSET, RECORD_RB);
}

/// Refusal 5: a definition handle the store has not loaded, refused without loading it.
#[test]
fn an_unloaded_definition_is_not_read_and_not_loaded() {
    let mut x = fx();
    let u = x.unloaded;
    x.set_definition(Some(u));
    let loads = x.vm.defs.load_transitions();
    refused(&mut x, Missing::NotRead, UNLOADED_DEFINITION, RECORD_RB);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, u) && x.vm.defs.load_transitions() == loads, "not loaded");
}

/// Refusals 3 and 4 (the deferred reattach, `CollideActors` clear), and the owner cases that get the flag only.
#[test]
fn the_reattach_refuses_a_static_or_tick_disabled_owner_and_an_ownerless_scene() {
    for flag in ["bStatic", "bTickIsDisabled"] {
        let mut x = fx();
        let owner = x.obj("Engine.Actor");
        x.set(owner, flag, Value::Bool(true));
        x.set(x.mesh, "Owner", Value::Object(Some(owner)));
        refused(&mut x, Missing::NotRead, REATTACH_OWNER, RECORD);
        // The same owner being destroyed: the flag only.
        x.set(owner, "bDeleteMe", Value::Bool(true));
        x.call().unwrap();
        assert_eq!(x.get(x.mesh, "bNeedsReattach"), Value::Bool(true), "{flag} with bDeleteMe");
    }
    let mut x = fx();
    x.set(x.mesh, "Owner", Value::Object(None));
    let scene = filled(&x.get(x.mesh, "Scene"), Value::Int(1));
    x.set(x.mesh, "Scene", scene);
    refused(&mut x, Missing::NotRead, REATTACH_SCENE, RECORD);
    // No owner and an empty scene: the flag only.
    let scene = filled(&x.get(x.mesh, "Scene"), Value::Int(0));
    x.set(x.mesh, "Scene", scene);
    x.call().unwrap();
    assert_eq!(x.get(x.mesh, "bNeedsReattach"), Value::Bool(true));
}

/// The globals definition missing from the world (its preflight's own refusal) comes before any write.
#[test]
fn the_globals_refusal_comes_before_any_write() {
    let mut x = fx();
    let globals = x.obj("WillowGame.WillowGlobals");
    x.vm.service::<ow_world::query::WorldQuery>().unwrap().borrow_mut().set_globals(globals);
    let before = x.state();
    match x.call() {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(!ow_core::notyet::is_partial_writes(&n.what), "{n}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(x.state(), before);
}

/// Reference binds nothing (the stub policy refuses the call).
#[test]
fn reference_binds_nothing() {
    let x = fx_mode(ParityMode::Reference);
    assert!(!x.vm.has_native(INIT_RB_PHYSICS));
}
