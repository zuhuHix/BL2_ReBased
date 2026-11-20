//! Tests of the `AttachMeshToPickup` part 1 natives in Off (NATIVE_ATTACH_MESH_TO_PICKUP.md; lane
//! IMPL-ATTACH-MESH-TO-PICKUP) on one invented `Engine` package built with `ow_package::testkit` (AGENTS.md rule 9):
//! `Object` with a `Pointer` struct, `Actor` (`Components`, `bDeleteMe`, the native `AttachComponent`),
//! `ActorComponent` (`Scene`, `Owner` and the three flags), `PrimitiveComponent` (`HiddenGame`, `bOwnerNoSee`, the two
//! natives) and `SkeletalMeshComponent` (`bForceRefpose`, `bOldForceRefPose`, `SkeletalMesh`, the two natives). Every
//! call goes through the VM, and every refusal is graded by its `NotYet` kind.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, NotYet, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{NameId, ObjRef, PropAddr};
use ow_object::value::{StructValue, Value};
use ow_package::testkit::reflect::toy_struct;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 10] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "BoolProperty",
    "IntProperty",
    "ArrayProperty",
    "StructProperty",
    "NameProperty",
    "ByteProperty",
];

const FORCE: &str = "Engine.SkeletalMeshComponent.SetForceRefPose";
const HIDDEN: &str = "Engine.PrimitiveComponent.SetHidden";
const NO_SEE: &str = "Engine.PrimitiveComponent.SetOwnerNoSee";
const HIDE_BONE_FN: &str = "Engine.SkeletalMeshComponent.HideBoneByName";
const ATTACH_FN: &str = "Engine.Actor.AttachComponent";
const LIGHT_FN: &str = "Engine.PrimitiveComponent.SetLightEnvironment";
const RB_FN: &str = "Engine.WillowPickup.InitializePickupForRBPhysics";
const PARTICLES_FN: &str = "Engine.WillowPickup.SpawnPickupParticles";

/// Every component field a native here may write, checked unchanged after a refusal.
const WRITTEN: [&str; 8] = [
    "Owner",
    "bAttached",
    "bNeedsReattach",
    "bNeedsUpdateTransform",
    "HiddenGame",
    "bOwnerNoSee",
    "bForceRefpose",
    "bOldForceRefPose",
];

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let pointer = toy_struct(&mut t, object, "Pointer", None);
    t.prop("Int", pointer, "Dummy", 0, 0);
    let actor = t.class("Actor", object);
    let component = t.class("ActorComponent", object);
    let array = t.p.next_export();
    t.prop("Array", actor, "Components", 0, array + 1);
    t.prop("Object", array, "Components", 0, component);
    t.prop("Bool", actor, "bDeleteMe", 0, 0);
    let new_component = Param { type_ref: component, ..Param::new("Object", "NewComponent") };
    t.function(actor, "AttachComponent", &[new_component], None, &[], &FunctionSpec::new(NATIVE, "AttachComponent"), |_| None);
    t.prop("Struct", component, "Scene", 0, pointer);
    t.prop("Object", component, "Owner", 0, actor);
    for flag in ["bAttached", "bNeedsReattach", "bNeedsUpdateTransform"] {
        t.prop("Bool", component, flag, 0, 0);
    }
    let primitive = t.class("PrimitiveComponent", component);
    t.prop("Bool", primitive, "HiddenGame", 0, 0);
    t.prop("Bool", primitive, "bOwnerNoSee", 0, 0);
    let light = t.class("LightEnvironmentComponent", component);
    t.prop("Object", primitive, "LightEnvironment", 0, light);
    t.prop("Object", primitive, "PreviousLightEnvironment", 0, light);
    let env = Param { type_ref: light, ..Param::new("Object", "NewLightEnvironment") };
    t.function(primitive, "SetLightEnvironment", &[env], None, &[], &FunctionSpec::new(NATIVE, "SetLightEnvironment"), |_| None);
    // Two components with broken declarations past the light environment's checks (review r1 of #520).
    let odd_flag = t.class("OddFlagLight", object);
    t.prop("Object", odd_flag, "LightEnvironment", 0, light);
    t.prop("Int", odd_flag, "bAttached", 0, 0);
    let odd_tail = t.class("OddTailLight", object);
    t.prop("Object", odd_tail, "LightEnvironment", 0, light);
    t.prop("Bool", odd_tail, "bAttached", 0, 0);
    t.prop("Object", odd_tail, "Owner", 0, object);
    t.prop("Int", odd_tail, "Scene", 0, 0);
    t.prop("Int", odd_tail, "PreviousLightEnvironment", 0, 0);
    t.prop("Bool", odd_tail, "bNeedsReattach", 0, 0);
    // Review r2 of #520: a light component whose `bNeedsReattach` is its last slot (declared first: the toy's slots run
    // in reverse declaration order), so a slot vector one short loses only it; and one with a struct `Scene` but a
    // `PreviousLightEnvironment` that takes no object.
    let tail_slot = t.class("TailSlotLight", object);
    t.prop("Bool", tail_slot, "bNeedsReattach", 0, 0);
    t.prop("Object", tail_slot, "LightEnvironment", 0, light);
    t.prop("Bool", tail_slot, "bAttached", 0, 0);
    t.prop("Object", tail_slot, "Owner", 0, object);
    t.prop("Struct", tail_slot, "Scene", 0, pointer);
    t.prop("Object", tail_slot, "PreviousLightEnvironment", 0, light);
    let odd_prev = t.class("OddPrevLight", object);
    t.prop("Object", odd_prev, "LightEnvironment", 0, light);
    t.prop("Bool", odd_prev, "bAttached", 0, 0);
    t.prop("Object", odd_prev, "Owner", 0, object);
    t.prop("Struct", odd_prev, "Scene", 0, pointer);
    t.prop("Int", odd_prev, "PreviousLightEnvironment", 0, 0);
    t.prop("Bool", odd_prev, "bNeedsReattach", 0, 0);
    let pickup = t.class("WillowPickup", actor);
    t.prop("Bool", pickup, "bUseRBPhysics", 0, 0);
    t.prop("Object", pickup, "PickupParticles", 0, object);
    t.function(pickup, "InitializePickupForRBPhysics", &[], None, &[], &FunctionSpec::new(NATIVE, "InitializePickupForRBPhysics"), |_| None);
    t.function(pickup, "SpawnPickupParticles", &[], None, &[], &FunctionSpec::new(NATIVE, "SpawnPickupParticles"), |_| None);
    t.function(primitive, "SetHidden", &[Param::new("Bool", "NewHidden")], None, &[], &FunctionSpec::new(NATIVE, "SetHidden"), |_| None);
    t.function(primitive, "SetOwnerNoSee", &[Param::new("Bool", "bNewOwnerNoSee")], None, &[], &FunctionSpec::new(NATIVE, "SetOwnerNoSee"), |_| None);
    let bad_actor = t.class("BadActor", object);
    t.prop("Int", bad_actor, "Components", 0, 0);
    t.prop("Bool", bad_actor, "bDeleteMe", 0, 0);
    let ints_actor = t.class("IntsActor", object);
    let ints = t.p.next_export();
    t.prop("Array", ints_actor, "Components", 0, ints + 1);
    t.prop("Int", ints, "Components", 0, 0);
    t.prop("Bool", ints_actor, "bDeleteMe", 0, 0);
    let loose = t.class("LooseComponent", object);
    t.prop("Struct", loose, "Scene", 0, pointer);
    t.prop("Object", loose, "Owner", 0, object);
    for flag in ["bAttached", "bNeedsReattach", "bNeedsUpdateTransform"] {
        t.prop("Bool", loose, flag, 0, 0);
    }
    let bad_component = t.class("BadComponent", object);
    t.prop("Struct", bad_component, "Scene", 0, pointer);
    t.prop("Int", bad_component, "Owner", 0, 0);
    for flag in ["bAttached", "bNeedsReattach", "bNeedsUpdateTransform"] {
        t.prop("Bool", bad_component, flag, 0, 0);
    }
    let mesh = t.class("SkeletalMeshComponent", primitive);
    t.prop("Int", mesh, "bForceRefpose", 0, 0);
    t.prop("Int", mesh, "bOldForceRefPose", 0, 0);
    t.prop("Object", mesh, "SkeletalMesh", 0, object);
    t.function(mesh, "SetForceRefPose", &[Param::new("Bool", "bNewForceRefPose")], None, &[], &FunctionSpec::new(NATIVE, "SetForceRefPose"), |_| None);
    let bone = [Param::new("Name", "BoneName"), Param::new("Byte", "PhysBodyOption")];
    t.function(mesh, "HideBoneByName", &bone, None, &[], &FunctionSpec::new(NATIVE, "HideBoneByName"), |_| None);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A VM in `mode`, with the empty scene layer when `scene` is set.
fn fx(mode: ParityMode, scene: bool) -> Fx {
    fx_with(mode, scene, |_| {})
}

/// [`fx`], with `before` run on the VM before [`register`] (to prebind a key).
fn fx_with(mode: ParityMode, scene: bool, before: impl FnOnce(&mut Vm)) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-attach-mesh-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let e = engine();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", e.as_slice())], IdentityPolicy::FirstInScanOrder, mode)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    for c in ["Engine.SkeletalMeshComponent", "Engine.Actor"] {
        vm.find_class(c).unwrap();
    }
    before(&mut vm);
    register(&mut vm).unwrap();
    if scene {
        add_empty_scene(&mut vm).unwrap();
        add_empty_scene(&mut vm).unwrap(); // a second add keeps the one layer
    }
    Fx { vm, dir }
}

impl Fx {
    fn obj(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn written(&self, o: ObjRef) -> Vec<Value> {
        WRITTEN.iter().map(|f| self.get(o, f)).collect()
    }

    fn call(&mut self, path: &str, me: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        Ok(self.vm.call(f, Some(me), args)?.ret)
    }

    /// A component with a non-zero `Scene`.
    fn with_scene(&mut self, o: ObjRef) {
        let Value::Struct(s) = self.get(o, "Scene") else { panic!("Scene is a struct") };
        let scene = Value::Struct(Rc::new(StructValue { ty: s.ty, fields: vec![Value::Int(1)] }));
        self.set(o, "Scene", scene);
    }
}

fn not_yet(r: Result<Value, VmError>) -> NotYet {
    match r {
        Err(VmError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn unmarked(n: &NotYet, missing: Missing, what: &str) {
    assert_eq!((n.missing.clone(), n.what.as_ref(), n.record), (missing, what, RECORD));
    assert!(!ow_core::notyet::is_partial_writes(&n.what), "a preflight refusal is unmarked");
}

/// The slice's case: no `Owner`, an empty `Scene`. `SetForceRefPose` writes the fitted int, then `bNeedsReattach`,
/// and nothing else. It always writes: false gives 0, with no "unchanged" short cut.
#[test]
fn set_force_ref_pose_writes_the_int_then_the_reattach_flag() {
    let mut x = fx(ParityMode::Off, true);
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    let before = x.written(mesh);
    assert_eq!(x.call(FORCE, mesh, vec![Value::Bool(true)]).unwrap(), Value::None);
    let mut want = before.clone();
    want[2] = Value::Bool(true);
    want[6] = Value::Int(1);
    assert_eq!(x.written(mesh), want, "bForceRefpose = 1 and bNeedsReattach, nothing else");
    x.set(mesh, "bNeedsReattach", Value::Bool(false));
    x.call(FORCE, mesh, vec![Value::Bool(false)]).unwrap();
    assert_eq!(x.get(mesh, "bForceRefpose"), Value::Int(0));
    assert_eq!(x.get(mesh, "bNeedsReattach"), Value::Bool(true), "written again: no unchanged short cut");
}

/// The deferred reattach's preflight, for each of its three natives: an `Owner`, then a non-zero `Scene`, each a
/// `NotRead` before any write.
#[test]
fn the_deferred_reattach_refuses_an_owner_or_a_scene_before_any_write() {
    let mut x = fx(ParityMode::Off, true);
    let calls: [(&str, Value); 3] =
        [(FORCE, Value::Bool(true)), (HIDDEN, Value::Bool(true)), (NO_SEE, Value::Bool(true))];
    for (path, arg) in calls {
        let mesh = x.obj("Engine.SkeletalMeshComponent");
        let owner = x.obj("Engine.Actor");
        x.set(mesh, "Owner", Value::Object(Some(owner)));
        let before = x.written(mesh);
        unmarked(&not_yet(x.call(path, mesh, vec![arg.clone()])), Missing::NotRead, REATTACH_OWNER);
        assert_eq!(x.written(mesh), before, "{path}: nothing written");

        let mesh = x.obj("Engine.SkeletalMeshComponent");
        x.with_scene(mesh);
        let before = x.written(mesh);
        unmarked(&not_yet(x.call(path, mesh, vec![arg])), Missing::NotRead, REATTACH_SCENE);
        assert_eq!(x.written(mesh), before, "{path}: nothing written");
    }
}

/// `SetHidden` and `SetOwnerNoSee`: a flag already equal to the argument is no write, even with an `Owner` (no
/// preflight runs). A changed flag is written, then `bNeedsReattach`.
#[test]
fn set_hidden_and_owner_no_see_write_only_a_changed_flag() {
    let mut x = fx(ParityMode::Off, true);
    for (path, flag) in [(HIDDEN, "HiddenGame"), (NO_SEE, "bOwnerNoSee")] {
        let mesh = x.obj("Engine.SkeletalMeshComponent");
        let owner = x.obj("Engine.Actor");
        x.set(mesh, "Owner", Value::Object(Some(owner)));
        let before = x.written(mesh);
        assert_eq!(x.call(path, mesh, vec![Value::Bool(false)]).unwrap(), Value::None);
        assert_eq!(x.written(mesh), before, "{path}: unchanged, nothing written");

        let mesh = x.obj("Engine.SkeletalMeshComponent");
        x.set(mesh, flag, Value::Bool(true));
        x.call(path, mesh, vec![Value::Bool(false)]).unwrap();
        assert_eq!(x.get(mesh, flag), Value::Bool(false), "{path}");
        assert_eq!(x.get(mesh, "bNeedsReattach"), Value::Bool(true), "{path}");
    }
}

/// `HideBoneByName`: a None name or no `SkeletalMesh` is "no bone", nothing written; a name on a component with a
/// mesh is `NotRead` before any write.
#[test]
fn hide_bone_by_name_does_nothing_without_a_mesh_and_is_not_read_with_one() {
    let mut x = fx(ParityMode::Off, true);
    let bone = x.vm.space.names.intern("ToyBone");
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    let before = x.written(mesh);
    for name in [NameId::NONE, bone] {
        assert_eq!(x.call(HIDE_BONE_FN, mesh, vec![Value::Name(name), Value::Byte(0)]).unwrap(), Value::None);
    }
    let asset = x.obj("Engine.Actor");
    x.set(mesh, "SkeletalMesh", Value::Object(Some(asset)));
    assert_eq!(x.call(HIDE_BONE_FN, mesh, vec![Value::Name(NameId::NONE), Value::Byte(0)]).unwrap(), Value::None);
    unmarked(&not_yet(x.call(HIDE_BONE_FN, mesh, vec![Value::Name(bone), Value::Byte(0)])), Missing::NotRead, HIDE_BONE);
    assert_eq!(x.written(mesh), before);
}

/// A world without the scene layer (review r1 of #511): the guards still answer a plain return (no component, a
/// `bDeleteMe` actor, a destroyed actor), and only after them is the attach `NotYet { LayerAbsent }`, unmarked,
/// nothing written. An attached component gets the layer refusal, not the detach one.
#[test]
fn without_the_scene_layer_the_guards_come_first_then_layer_absent() {
    let mut x = fx(ParityMode::Off, false);
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    let before = x.written(mesh);
    let actor = x.obj("Engine.Actor");
    assert_eq!(x.call(ATTACH_FN, actor, vec![Value::Object(None)]).unwrap(), Value::None);
    let gone = x.obj("Engine.Actor");
    x.set(gone, "bDeleteMe", Value::Bool(true));
    let dead = x.obj("Engine.Actor");
    x.vm.space.heap.destroy(dead).unwrap();
    for guarded in [gone, dead] {
        assert_eq!(x.call(ATTACH_FN, guarded, vec![Value::Object(Some(mesh))]).unwrap(), Value::None, "a plain return");
        assert_eq!(x.get(guarded, "Components"), Value::array(Vec::new()));
    }
    let n = not_yet(x.call(ATTACH_FN, actor, vec![Value::Object(Some(mesh))]));
    unmarked(&n, Missing::LayerAbsent(SCENE_LAYER), ATTACH);
    x.set(mesh, "bAttached", Value::Bool(true));
    let n = not_yet(x.call(ATTACH_FN, actor, vec![Value::Object(Some(mesh))]));
    unmarked(&n, Missing::LayerAbsent(SCENE_LAYER), ATTACH);
    x.set(mesh, "bAttached", Value::Bool(false));
    assert_eq!(x.written(mesh), before);
    assert_eq!(x.get(actor, "Components"), Value::array(Vec::new()));
}

/// With the empty scene layer (the hub's ruling): the guards do nothing; an attached component is `NotRead` before
/// any write (both flags seeded true, so a clear moved ahead of the preflight shows); otherwise the plan is the two
/// flag clears, `Owner`, then the `Components` append, in that order, with no duplicate check, and `Scene` and
/// `bAttached` are left as they are.
#[test]
fn with_the_empty_scene_the_attach_makes_only_the_script_visible_writes() {
    let mut x = fx(ParityMode::Off, true);
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    x.set(mesh, "bNeedsReattach", Value::Bool(true));
    x.set(mesh, "bNeedsUpdateTransform", Value::Bool(true));
    let scene = x.get(mesh, "Scene");

    let gone = x.obj("Engine.Actor");
    x.set(gone, "bDeleteMe", Value::Bool(true));
    let dead = x.obj("Engine.Actor");
    x.vm.space.heap.destroy(dead).unwrap();
    let before = x.written(mesh);
    for actor in [gone, dead] {
        assert_eq!(x.call(ATTACH_FN, actor, vec![Value::Object(Some(mesh))]).unwrap(), Value::None);
        assert_eq!(x.get(actor, "Components"), Value::array(Vec::new()));
    }
    assert_eq!(x.written(mesh), before, "the guards write nothing");

    let actor = x.obj("Engine.Actor");
    let attached = x.obj("Engine.SkeletalMeshComponent");
    x.set(attached, "bAttached", Value::Bool(true));
    x.set(attached, "bNeedsReattach", Value::Bool(true));
    x.set(attached, "bNeedsUpdateTransform", Value::Bool(true));
    let held = x.written(attached);
    let n = not_yet(x.call(ATTACH_FN, actor, vec![Value::Object(Some(attached))]));
    unmarked(&n, Missing::NotRead, "Actor.AttachComponent on an attached component: the detach before the attach");
    assert_eq!(x.written(attached), held, "no clear ahead of the preflight");
    let recorded = |x: &Fx, c: ObjRef| x.vm.service::<SceneLayer>().unwrap().borrow().recorded_owner(c);
    assert_eq!(recorded(&x, attached), None, "a refused attach records nothing (lane IMPL-COMPONENT-BOUNDS)");
    assert_eq!(x.get(actor, "Components"), Value::array(Vec::new()));

    let plan = attach_plan(&x.vm, actor, Some(mesh)).unwrap().expect("the attach writes");
    let order: Vec<(ObjRef, &str)> = plan.iter().map(|w| (w.0, w.1)).collect();
    assert_eq!(
        order,
        vec![(mesh, "bNeedsReattach"), (mesh, "bNeedsUpdateTransform"), (mesh, "Owner"), (actor, "Components")],
        "the note's order: the flag clears, Owner, then the append"
    );
    assert_eq!(recorded(&x, mesh), None, "the guards record nothing");
    assert_eq!(x.call(ATTACH_FN, actor, vec![Value::Object(Some(mesh))]).unwrap(), Value::None);
    assert_eq!(recorded(&x, mesh), Some(actor), "the successful attach publishes the record");
    assert!(ow_world::actors::attached_to(&x.vm, mesh, actor).unwrap());
    assert_eq!(x.get(mesh, "bNeedsReattach"), Value::Bool(false));
    assert_eq!(x.get(mesh, "bNeedsUpdateTransform"), Value::Bool(false));
    assert_eq!(x.get(mesh, "Owner"), Value::Object(Some(actor)));
    assert_eq!(x.get(mesh, "bAttached"), Value::Bool(false), "bAttached is not read: left as it is");
    assert_eq!(x.get(mesh, "Scene"), scene, "no scene is set");
    assert_eq!(x.get(actor, "Components"), Value::array(vec![Value::Object(Some(mesh))]));
    x.call(ATTACH_FN, actor, vec![Value::Object(Some(mesh))]).unwrap();
    assert_eq!(x.get(actor, "Components"), Value::array(vec![Value::Object(Some(mesh)); 2]), "no duplicate check");
}

/// A broken declaration on a field a step does not read (review r1 of #511): the guards, the layer refusal and the
/// detach preflight still answer as they would. `Test.BadActor` declares `Components` as an int, and
/// `Test.BadComponent` declares `Owner` as an int.
#[test]
fn an_unrelated_declaration_error_masks_no_guard_or_refusal() {
    let mut x = fx(ParityMode::Off, false);
    let bad_actor = x.obj("Engine.BadActor");
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    x.set(bad_actor, "bDeleteMe", Value::Bool(true));
    assert_eq!(x.call(ATTACH_FN, bad_actor, vec![Value::Object(Some(mesh))]).unwrap(), Value::None, "bDeleteMe");
    let dead = x.obj("Engine.BadActor");
    x.vm.space.heap.destroy(dead).unwrap();
    assert_eq!(x.call(ATTACH_FN, dead, vec![Value::Object(Some(mesh))]).unwrap(), Value::None, "destroyed");
    let live = x.obj("Engine.BadActor");
    let n = not_yet(x.call(ATTACH_FN, live, vec![Value::Object(Some(mesh))]));
    unmarked(&n, Missing::LayerAbsent(SCENE_LAYER), ATTACH);

    let mut x = fx(ParityMode::Off, true);
    let actor = x.obj("Engine.Actor");
    let odd = x.obj("Engine.BadComponent");
    x.set(odd, "bAttached", Value::Bool(true));
    let n = not_yet(x.call(ATTACH_FN, actor, vec![Value::Object(Some(odd))]));
    unmarked(&n, Missing::NotRead, ATTACH_DETACH);
    x.set(odd, "bAttached", Value::Bool(false));
    x.set(odd, "bNeedsReattach", Value::Bool(true));
    x.set(odd, "bNeedsUpdateTransform", Value::Bool(true));
    let e = x.call(ATTACH_FN, actor, vec![Value::Object(Some(odd))]).unwrap_err();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("Owner")), "a data error naming Owner: {e:?}");
    assert_eq!(x.get(odd, "bNeedsReattach"), Value::Bool(true), "no early clear");
    assert_eq!(x.get(odd, "bNeedsUpdateTransform"), Value::Bool(true), "no early clear");
    assert_eq!(x.get(actor, "Components"), Value::array(Vec::new()), "nothing written");
}

/// A destination whose declaration passes the outer kind check but cannot take the value (review r2 of #511):
/// `Components` declared as an array of ints. The write rules are checked for every planned write before the first,
/// so the call is a data error naming `Components`, with both flags, `Owner` and `Components` unchanged.
#[test]
fn a_components_array_of_ints_refuses_before_any_write() {
    let mut x = fx(ParityMode::Off, true);
    let actor = x.obj("Engine.IntsActor");
    // A component whose `Owner` takes any object, so only `Components` cannot take its value.
    let loose = x.obj("Engine.LooseComponent");
    x.set(loose, "bNeedsReattach", Value::Bool(true));
    x.set(loose, "bNeedsUpdateTransform", Value::Bool(true));
    let fields = ["bNeedsReattach", "bNeedsUpdateTransform", "Owner"];
    let before: Vec<Value> = fields.iter().map(|f| x.get(loose, f)).collect();
    let e = x.call(ATTACH_FN, actor, vec![Value::Object(Some(loose))]).unwrap_err();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("Components")), "{e:?}");
    let after: Vec<Value> = fields.iter().map(|f| x.get(loose, f)).collect();
    assert_eq!(after, before, "both flags and Owner unchanged");
    let layer = x.vm.service::<SceneLayer>().unwrap();
    assert_eq!(layer.borrow().recorded_owner(loose), None, "a failed attach records nothing");
    assert_eq!(x.get(actor, "Components"), Value::array(Vec::new()));
}

/// Reference binds none of the five: the stub policy refuses them.
#[test]
fn reference_binds_nothing() {
    let mut x = fx(ParityMode::Reference, false);
    for key in [
        SET_FORCE_REF_POSE,
        SET_HIDDEN,
        SET_OWNER_NO_SEE,
        HIDE_BONE_BY_NAME,
        ATTACH_COMPONENT,
        SET_LIGHT_ENVIRONMENT,
        INIT_RB_PHYSICS,
        SPAWN_PICKUP_PARTICLES,
    ] {
        assert!(!x.vm.has_native(key), "{key} is bound in Reference");
    }
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    let actor = x.obj("Engine.Actor");
    let before = x.written(mesh);
    for (path, me, args) in [
        (FORCE, mesh, vec![Value::Bool(true)]),
        (HIDDEN, mesh, vec![Value::Bool(true)]),
        (NO_SEE, mesh, vec![Value::Bool(true)]),
        (HIDE_BONE_FN, mesh, vec![Value::Name(NameId::NONE), Value::Byte(0)]),
        (ATTACH_FN, actor, vec![Value::Object(Some(mesh))]),
        (LIGHT_FN, mesh, vec![Value::Object(None)]),
        (RB_FN, actor, vec![]),
        (PARTICLES_FN, actor, vec![]),
    ] {
        match x.call(path, me, args) {
            Err(VmError::NotYet(n)) => assert_ne!(n.record, RECORD, "{path}"),
            Err(_) => {}
            Ok(v) => panic!("{path}: bound in Reference ({v:?})"),
        }
    }
    assert_eq!(x.written(mesh), before);
}

/// An error after a native's first write is marked once, its kind and record kept; other errors pass unchanged.
#[test]
fn an_error_after_the_first_write_is_marked_once() {
    let n = NotYet::layer_absent("x", SCENE_LAYER, RECORD);
    let Err(VmError::NotYet(m)) = Err::<(), _>(after_writes(after_writes(n.into()))) else { panic!() };
    assert_eq!(m.what.as_ref(), "stopped after partial writes: x");
    assert_eq!((m.missing, m.record), (Missing::LayerAbsent(SCENE_LAYER), RECORD));
    assert!(matches!(after_writes(VmError::ServiceBorrowed("x")), VmError::ServiceBorrowed("x")));
}

// ---- part 2 (the note's section 6) ---------------------------------------------------------------------------------

/// The light environment fields of `o`: `LightEnvironment`, `PreviousLightEnvironment`, `bNeedsReattach`.
fn light_fields(x: &Fx, o: ObjRef) -> Vec<Value> {
    ["LightEnvironment", "PreviousLightEnvironment", "bNeedsReattach"].iter().map(|f| x.get(o, f)).collect()
}

/// `SetLightEnvironment` (6.2): the current value is nothing, even on an attached component with an `Owner`; a
/// component not attached gets the one write `LightEnvironment`, even with an `Owner` (the slice's case after the
/// attach); an attached one with no `Owner` and an empty `Scene` gets `PreviousLightEnvironment`, `LightEnvironment`
/// and `bNeedsReattach`; an attached one with an `Owner` is the deferred reattach's `NotRead`, unmarked, nothing
/// written.
#[test]
fn set_light_environment_follows_the_three_cases() {
    let mut x = fx(ParityMode::Off, true);
    let old = x.obj("Engine.LightEnvironmentComponent");
    let new = x.obj("Engine.LightEnvironmentComponent");
    let owner = x.obj("Engine.Actor");

    let same = x.obj("Engine.SkeletalMeshComponent");
    x.set(same, "LightEnvironment", Value::Object(Some(old)));
    x.set(same, "bAttached", Value::Bool(true));
    x.set(same, "Owner", Value::Object(Some(owner)));
    let before = light_fields(&x, same);
    assert_eq!(x.call(LIGHT_FN, same, vec![Value::Object(Some(old))]).unwrap(), Value::None);
    assert_eq!(light_fields(&x, same), before, "the current value: nothing");

    let loose = x.obj("Engine.SkeletalMeshComponent");
    x.set(loose, "LightEnvironment", Value::Object(Some(old)));
    x.set(loose, "Owner", Value::Object(Some(owner)));
    x.call(LIGHT_FN, loose, vec![Value::Object(Some(new))]).unwrap();
    assert_eq!(
        light_fields(&x, loose),
        vec![Value::Object(Some(new)), Value::Object(None), Value::Bool(false)],
        "not attached: LightEnvironment only"
    );

    let attached = x.obj("Engine.SkeletalMeshComponent");
    x.set(attached, "LightEnvironment", Value::Object(Some(old)));
    x.set(attached, "bAttached", Value::Bool(true));
    x.call(LIGHT_FN, attached, vec![Value::Object(Some(new))]).unwrap();
    assert_eq!(
        light_fields(&x, attached),
        vec![Value::Object(Some(new)), Value::Object(Some(old)), Value::Bool(true)],
        "attached: the previous value kept, the new one stored, the reattach flag"
    );

    let owned = x.obj("Engine.SkeletalMeshComponent");
    x.set(owned, "LightEnvironment", Value::Object(Some(old)));
    x.set(owned, "bAttached", Value::Bool(true));
    x.set(owned, "Owner", Value::Object(Some(owner)));
    let before = light_fields(&x, owned);
    unmarked(&not_yet(x.call(LIGHT_FN, owned, vec![Value::Object(Some(new))])), Missing::NotRead, REATTACH_OWNER);
    assert_eq!(light_fields(&x, owned), before, "nothing written");
}

/// `InitializePickupForRBPhysics` (6.3): `bUseRBPhysics` false is nothing. The set rule is lane IMPL-PICKUP-RB's
/// (`attach_mesh/pickup_rb_tests.rs`); on this toy, whose pickup declares none of its fields, it is a data error
/// before any write.
#[test]
fn the_rigid_body_set_up_is_nothing_when_the_pickup_skips_it() {
    let mut x = fx(ParityMode::Off, true);
    let pickup = x.obj("Engine.WillowPickup");
    assert_eq!(x.call(RB_FN, pickup, vec![]).unwrap(), Value::None, "bUseRBPhysics false");
    x.set(pickup, "bUseRBPhysics", Value::Bool(true));
    let before = (x.get(pickup, "bUseRBPhysics"), x.get(pickup, "PickupParticles"), x.get(pickup, "Components"));
    let e = x.call(RB_FN, pickup, vec![]).unwrap_err();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("CollisionComponent")), "{e:?}");
    assert_eq!((x.get(pickup, "bUseRBPhysics"), x.get(pickup, "PickupParticles"), x.get(pickup, "Components")), before);
}

/// `SpawnPickupParticles` (6.4): the rule is lane IMPL-SPAWN-PICKUP-PARTICLES's (`attach_mesh/pickup_particles_tests.rs`).
/// On this toy, whose pickup declares none of the fields the note's preflight checks first, it is a data error before
/// any write, with or without a particle system.
#[test]
fn spawn_pickup_particles_on_a_toy_without_its_fields_is_a_data_error() {
    let mut x = fx(ParityMode::Off, true);
    let pickup = x.obj("Engine.WillowPickup");
    let system = x.obj("Engine.Actor");
    for held in [None, Some(system)] {
        x.set(pickup, "PickupParticles", Value::Object(held));
        let e = x.call(PARTICLES_FN, pickup, vec![]).unwrap_err();
        assert!(matches!(&e, VmError::Script { .. }), "{e:?}");
        assert_eq!(x.get(pickup, "Components"), Value::array(Vec::new()), "no component made or attached");
    }
}

/// First wins for part 2's keys (review r1 of #520): a key bound before the pack keeps its handler, and the other two
/// are still registered.
#[test]
fn a_prebound_part_2_key_keeps_its_handler_and_the_others_register() {
    let mut x = fx_with(ParityMode::Off, true, |vm| {
        vm.register_native(SET_LIGHT_ENVIRONMENT, Rc::new(|_c: &mut ow_vm::api::NativeCall<'_>| Ok(Value::Int(7)))).unwrap();
    });
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    assert_eq!(x.call(LIGHT_FN, mesh, vec![Value::Object(None)]).unwrap(), Value::Int(7), "the earlier handler kept");
    assert!(x.vm.has_native(INIT_RB_PHYSICS) && x.vm.has_native(SPAWN_PICKUP_PARTICLES));
    let pickup = x.obj("Engine.WillowPickup");
    assert_eq!(x.call(RB_FN, pickup, vec![]).unwrap(), Value::None, "this pack's handler");
}

/// The light environment's checks on their own paths (review r1 of #520): an equal value is nothing even with a broken
/// `bAttached`; on an attached component with an `Owner`, the deferred reattach's `NotRead` comes before the broken
/// `Scene` and `PreviousLightEnvironment` declarations, and nothing is written.
#[test]
fn the_light_environment_checks_mask_neither_the_no_op_nor_the_refusal() {
    let mut x = fx(ParityMode::Off, true);
    let old = x.obj("Engine.LightEnvironmentComponent");
    let new = x.obj("Engine.LightEnvironmentComponent");
    let flag = x.obj("Engine.OddFlagLight");
    x.set(flag, "LightEnvironment", Value::Object(Some(old)));
    assert_eq!(x.call(LIGHT_FN, flag, vec![Value::Object(Some(old))]).unwrap(), Value::None, "the equal value");
    let tail = x.obj("Engine.OddTailLight");
    let owner = x.obj("Engine.Actor");
    x.set(tail, "LightEnvironment", Value::Object(Some(old)));
    x.set(tail, "bAttached", Value::Bool(true));
    x.set(tail, "Owner", Value::Object(Some(owner)));
    unmarked(&not_yet(x.call(LIGHT_FN, tail, vec![Value::Object(Some(new))])), Missing::NotRead, REATTACH_OWNER);
    assert_eq!(x.get(tail, "LightEnvironment"), Value::Object(Some(old)));
    assert_eq!(x.get(tail, "bNeedsReattach"), Value::Bool(false));
}

/// A destination slot missing from the object (review r2 of #520): an attached, ownerless component with an empty
/// `Scene` whose slot vector is one short (`bNeedsReattach`'s slot gone). The call fails before any write: both light
/// fields stay unchanged.
#[test]
fn a_missing_destination_slot_refuses_before_any_light_write() {
    let mut x = fx(ParityMode::Off, true);
    let old = x.obj("Engine.LightEnvironmentComponent");
    let new = x.obj("Engine.LightEnvironmentComponent");
    let mesh = x.obj("Engine.TailSlotLight");
    x.set(mesh, "LightEnvironment", Value::Object(Some(old)));
    x.set(mesh, "bAttached", Value::Bool(true));
    let lights = |x: &Fx| (x.get(mesh, "LightEnvironment"), x.get(mesh, "PreviousLightEnvironment"));
    let before = lights(&x);
    let class = x.vm.space.heap.get(mesh).unwrap().class;
    let flag = x.vm.space.prop(class, "bNeedsReattach").unwrap();
    let slot = x.vm.space.classes.resolve(class, PropAddr::scalar(flag)).unwrap();
    let slots = x.vm.space.heap.get(mesh).unwrap().slots.len();
    assert_eq!(slot, slots - 1, "bNeedsReattach is the toy's last slot");
    x.vm.space.heap.get_mut(mesh).unwrap().slots.truncate(slot);
    let e = x.call(LIGHT_FN, mesh, vec![Value::Object(Some(new))]).unwrap_err();
    assert!(!matches!(&e, VmError::NotYet(_)), "a heap error, not a NotYet: {e:?}");
    assert_eq!(lights(&x), before, "neither light field written");
}

/// The scene refusal's place (review r2 of #520): an attached, ownerless component with a non-zero `Scene` and a
/// `PreviousLightEnvironment` that takes no object is the scene's `NotRead`, unprefixed, with nothing written.
#[test]
fn the_scene_refusal_comes_before_the_destination_checks() {
    let mut x = fx(ParityMode::Off, true);
    let old = x.obj("Engine.LightEnvironmentComponent");
    let new = x.obj("Engine.LightEnvironmentComponent");
    let odd = x.obj("Engine.OddPrevLight");
    x.set(odd, "LightEnvironment", Value::Object(Some(old)));
    x.set(odd, "bAttached", Value::Bool(true));
    x.with_scene(odd);
    let before = light_fields(&x, odd);
    unmarked(&not_yet(x.call(LIGHT_FN, odd, vec![Value::Object(Some(new))])), Missing::NotRead, REATTACH_SCENE);
    assert_eq!(light_fields(&x, odd), before, "nothing written");
}
