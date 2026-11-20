//! The spawn's component update seen from its readers (lane IMPL-SPAWN-COMPONENT-ATTACH, review r1 of #610): a toy
//! actor spawned by the world's own spawn, whose template holds a component, and what the Off rules that read the
//! component's `Owner` then answer. Here because `ow-combat` reaches the spawn (`ow-world`), the deferred reattach
//! (`ow-inventory`) and the hold kinds (this crate). Invented packages with the game's class and field names
//! (AGENTS.md rule 9). AI-assisted (Claude Code).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;
use ow_world::actors::{Actors, SpawnRequest, component_attach, spawn};
use ow_world::world::{BalanceState, World};

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE_FLAGS: u32 = 0x400 | 0x20000;

/// `Core.upk`: `Object` and `Component` (the instancing copies `Core.Component` descendants).
fn core() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let object = t.class("Object", 0);
    t.class("Component", object);
    t.p.build()
}

/// The six fields of the spawn's component update on a component class; `actor` types `Owner`.
fn attach_fields(t: &mut ScriptToy, class: i32, owner: i32, scene: i32) {
    t.prop("Object", class, "Owner", 0, owner);
    t.prop("Struct", class, "Scene", 0, scene);
    for f in ["bAttached", "bNeedsReattach", "bNeedsUpdateTransform", "bSkipChildComponentUpdate"] {
        t.prop("Bool", class, f, 0, 0);
    }
}

/// `Engine.upk`: `Actor` with the fields the spawn reads, `WorldInfo`, a `SkeletalMeshComponent` (the attach fields and
/// `bForceRefpose`), and `ToyHolder`, an actor with a `Comp` field and a native `PreBeginPlay` the tests bind.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ScriptStruct",
        "IntProperty",
        "FloatProperty",
        "BoolProperty",
        "ByteProperty",
        "ObjectProperty",
        "ArrayProperty",
        "StructProperty",
    ]);
    let core = t.p.add_import("Core", "Package", 0, "Core");
    let component = t.p.add_import("Core", "Class", core, "Component");
    let object = t.class("Object", 0);
    let vector = toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    let rotator = toy_struct(&mut t, object, "Rotator", None);
    for f in ["Pitch", "Yaw", "Roll"] {
        t.prop("Int", rotator, f, 0, 0);
    }
    let scene = toy_struct(&mut t, object, "ScenePointer", None);
    t.prop("Int", scene, "Dummy", 0, 0);
    let actor = t.class("Actor", object);
    for b in [
        "bStatic",
        "bNoDelete",
        "bDeleteMe",
        "bPendingDelete",
        "bCollideWorld",
        "bCollideWhenPlacing",
        "bCollideActors",
        "bNetTemporary",
        "bShouldBaseAtStartup",
    ] {
        t.prop("Bool", actor, b, 0, 0);
    }
    for b in ["Role", "RemoteRole", "TickGroup", "Physics"] {
        t.prop("Byte", actor, b, 0, 0);
    }
    for f in ["LifeSpan", "CreationTime"] {
        t.prop("Float", actor, f, 0, 0);
    }
    for o in ["Owner", "Base", "Instigator", "WorldInfo"] {
        t.prop("Object", actor, o, 0, actor);
    }
    t.prop("Struct", actor, "Location", 0, vector);
    t.prop("Struct", actor, "Rotation", 0, rotator);
    for a in ["Attached", "Touching", "GeneratedEvents"] {
        toy_array_of(&mut t, actor, a, "Object", object);
    }
    // The component arrays hold components, as the game's do (the step appends its AI copies to `AllComponents`).
    for a in ["Components", "AllComponents"] {
        toy_array_of(&mut t, actor, a, "Object", component);
    }
    let world_info = t.class("WorldInfo", actor);
    for f in ["TimeSeconds", "RealTimeSeconds", "TimeDilation"] {
        t.prop("Float", world_info, f, 0, 0);
    }
    t.prop("Byte", world_info, "NetMode", 0, 0);
    let mesh = t.class("SkeletalMeshComponent", component);
    attach_fields(&mut t, mesh, actor, scene);
    t.prop("Int", mesh, "bForceRefpose", 0, 0);
    let holder = t.class("ToyHolder", actor);
    t.prop("Object", holder, "Comp", 0, component);
    let spec = FunctionSpec::new(NATIVE_FLAGS, "PreBeginPlay");
    t.function(holder, "PreBeginPlay", &[], None, &[], &spec, |_| None);
    t.p.build()
}

/// `GearboxFramework.upk`: `AIComponent` (the attach fields, `MovementHolds` of `AIHoldData`, the native
/// `HoldMovement`) and `ToyAIComponent`, an invented subclass whose own attach entry was not read.
fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ScriptStruct",
        "IntProperty",
        "BoolProperty",
        "NameProperty",
        "ObjectProperty",
        "ArrayProperty",
        "StructProperty",
    ]);
    let core = t.p.add_import("Core", "Package", 0, "Core");
    let component = t.p.add_import("Core", "Class", core, "Component");
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let ai = t.class("AIComponent", component);
    let scene = toy_struct(&mut t, ai, "ScenePointer", None);
    t.prop("Int", scene, "Dummy", 0, 0);
    attach_fields(&mut t, ai, actor, scene);
    let hold = toy_struct(&mut t, ai, "AIHoldData", None);
    t.prop("Name", hold, "HoldName", 0, 0);
    t.prop("Object", hold, "Tag", 0, object);
    toy_array_of(&mut t, ai, "MovementHolds", "Struct", hold);
    let spec = FunctionSpec::new(NATIVE_FLAGS, "HoldMovement");
    let params = [Param::new("Name", "HoldName"), Param { type_ref: object, ..Param::new("Object", "Tag") }];
    t.function(ai, "HoldMovement", &params, None, &[], &spec, |_| None);
    t.class("ToyAIComponent", ai);
    t.p.build()
}

struct Fx {
    vm: Vm,
    wi: ObjRef,
    /// What each spawned `ToyHolder`'s `PreBeginPlay` saw: the actor and its component copy's `Owner`.
    seen: Rc<RefCell<Vec<(ObjRef, Value)>>>,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn snapshot() -> BalanceSnapshot {
    BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::new(0).unwrap(),
        PlayerCount::SINGLE,
        ExpLevel::new(8),
        GameStage(8),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .unwrap()
}

/// An Off world: the standard one (`World::new`, which installs the empty scene layer) when `world`, else a bare VM
/// with only the actor list (no scene layer). The world info is placed; play has begun when `begin`.
fn fx(world: bool, begin: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-combat-attach-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (c, e, g) = (core(), engine(), gearbox());
    let files: [(&str, &[u8]); 3] = [("Core.upk", &c), ("Engine.upk", &e), ("GearboxFramework.upk", &g)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let mut vm = if world {
        World::new(vm, BalanceState::new(snapshot()), &[]).unwrap().vm
    } else {
        let mut vm = vm;
        let actor = vm.find_class("Engine.Actor").unwrap();
        vm.add_service(Actors::new(actor)).unwrap();
        vm
    };
    for class in ["Engine.ToyHolder", "Engine.SkeletalMeshComponent", "GearboxFramework.AIComponent", "GearboxFramework.ToyAIComponent"] {
        vm.find_class(class).unwrap();
    }
    register(&mut vm).unwrap();
    // `ToyHolder.PreBeginPlay`, a native here: it records the component copy's `Owner` as the event sees it.
    let seen: Rc<RefCell<Vec<(ObjRef, Value)>>> = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let f = vm.find_function("Engine.ToyHolder.PreBeginPlay").unwrap();
    let key = vm.function(f).unwrap().native_key.clone().unwrap();
    vm.register_native(
        &key,
        Rc::new(move |c: &mut NativeCall<'_>| {
            let me = c.this.unwrap();
            let class = c.vm.space.heap.get(me)?.class;
            let comp_prop = c.vm.space.prop(class, "Comp").unwrap();
            if let Value::Object(Some(copy)) = c.vm.space.read(me, PropAddr::scalar(comp_prop))?.clone() {
                let cc = c.vm.space.heap.get(copy)?.class;
                let owner = c.vm.space.prop(cc, "Owner").unwrap();
                let v = c.vm.space.read(copy, PropAddr::scalar(owner))?.clone();
                s.borrow_mut().push((me, v));
            }
            Ok(Value::None)
        }),
    )
    .unwrap();
    let c = vm.find_class("Engine.WorldInfo").unwrap();
    let wi = ow_vm::interp::instantiate_from_defaults(&mut vm, c).unwrap();
    let actors = vm.service::<Actors>().unwrap();
    actors.borrow_mut().place_world_info(wi).unwrap();
    if begin {
        actors.borrow_mut().begin_play();
    }
    Fx { vm, wi, seen, dir }
}

impl Fx {
    fn class(&mut self, path: &str) -> ClassId {
        self.vm.find_class(path).unwrap()
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    /// A `ToyHolder` archetype whose `Comp` and `Components` name one component of `component` it owns (none when
    /// `component` is None), as the class default's slots plus that.
    fn archetype(&mut self, component: Option<&str>) -> (ObjRef, Option<ObjRef>) {
        let holder = self.class("Engine.ToyHolder");
        let cdo = self.vm.defs.class_default(&mut self.vm.space, "Engine.ToyHolder").unwrap();
        let archetype = self.vm.space.alloc(holder, "ToyHolder_Archetype", None, Origin::Runtime).unwrap();
        let slots = self.vm.space.heap.get(cdo).unwrap().slots.clone();
        self.vm.space.heap.get_mut(archetype).unwrap().slots = slots;
        let Some(component) = component else {
            return (archetype, None);
        };
        let class = self.class(component);
        let comp = self.vm.space.alloc(class, "ToyComponent0", Some(archetype), Origin::Runtime).unwrap();
        // `Comp` and `Components` are filled as the packages fill them, through tags, unchecked: `Components` is typed
        // with the package's own root, which a `Core.Component` is not under.
        let comp_prop = self.vm.space.prop(holder, "Comp").unwrap();
        *self.vm.space.slot_mut(archetype, PropAddr::scalar(comp_prop)).unwrap() = Value::Object(Some(comp));
        let p = self.vm.space.prop(holder, "Components").unwrap();
        *self.vm.space.slot_mut(archetype, PropAddr::scalar(p)).unwrap() = Value::array(vec![Value::Object(Some(comp))]);
        (archetype, Some(comp))
    }

    fn spawn(&mut self, archetype: ObjRef) -> Result<Option<ObjRef>, VmError> {
        let holder = self.class("Engine.ToyHolder");
        let req = SpawnRequest::new(Some(holder)).with_template(Some(archetype));
        let wi = self.wi;
        spawn(&mut self.vm, wi, &req)
    }

    /// The spawned actor's copy of its component (its `Comp`, re-pointed by the instancing).
    fn copy(&self, actor: ObjRef) -> ObjRef {
        match self.get(actor, "Comp") {
            Value::Object(Some(c)) => c,
            other => panic!("no component copy: {other:?}"),
        }
    }

    fn counts(&self) -> (usize, usize) {
        let listed = self.vm.service::<Actors>().unwrap().borrow().list().len();
        (listed, self.vm.space.heap.handles().len())
    }
}

/// The deferred reattach (`ow-inventory` `attach_mesh`, review r1 of #610 item 1): its preflight refuses whenever the
/// component's `Owner` is set, whatever `bAttached` holds. A world-spawned primitive component's copy now has the actor
/// as `Owner` (and `bAttached` false), so `SetForceRefPose` on it refuses with `REATTACH_OWNER`, before any write; the
/// template component, with no `Owner`, takes the write.
#[test]
fn set_force_ref_pose_on_a_spawned_copy_meets_the_owner_refusal() {
    let mut x = fx(true, true);
    let (archetype, template) = x.archetype(Some("Engine.SkeletalMeshComponent"));
    let actor = x.spawn(archetype).unwrap().unwrap();
    let copy = x.copy(actor);
    assert_eq!(x.get(copy, "Owner"), Value::Object(Some(actor)));
    assert_eq!(x.get(copy, "bAttached"), Value::Bool(false));
    let before = x.get(copy, "bForceRefpose");
    match ow_inventory::attach_mesh::set_force_ref_pose(&mut x.vm, copy, true) {
        Err(VmError::NotYet(n)) => {
            assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, ow_inventory::attach_mesh::REATTACH_OWNER))
        }
        other => panic!("expected REATTACH_OWNER, got {other:?}"),
    }
    assert_eq!(x.get(copy, "bForceRefpose"), before, "nothing written");
    ow_inventory::attach_mesh::set_force_ref_pose(&mut x.vm, template.unwrap(), true).unwrap();
}

/// The write order (review r1 of #610 item 2): after play began, the actor's `PreBeginPlay` (a native the test binds)
/// already sees its component copy's `Owner` set to the actor.
#[test]
fn pre_begin_play_sees_the_copy_s_owner() {
    let mut x = fx(true, true);
    let (archetype, _) = x.archetype(Some("Engine.SkeletalMeshComponent"));
    let actor = x.spawn(archetype).unwrap().unwrap();
    assert_eq!(*x.seen.borrow(), vec![(actor, Value::Object(Some(actor)))]);
}

/// `HoldMovement` on an AI component the world spawned (review r1 of #610 item 2): its copy is attached (`bAttached`
/// true, in the actor's `AllComponents`) and owned, so a new movement hold appends the pair and then stops with the
/// terminal `NotYet { NotRead }`.
#[test]
fn hold_movement_on_a_spawned_ai_component_is_terminal() {
    let mut x = fx(true, true);
    let (archetype, _) = x.archetype(Some("GearboxFramework.AIComponent"));
    let actor = x.spawn(archetype).unwrap().unwrap();
    let copy = x.copy(actor);
    assert_eq!(x.get(copy, "bAttached"), Value::Bool(true));
    assert_eq!(x.get(actor, "AllComponents"), Value::array(vec![Value::Object(Some(copy))]));
    let tag = actor;
    let f = x.vm.find_function("GearboxFramework.AIComponent.HoldMovement").unwrap();
    let name = Value::Name(x.vm.space.names.intern("Safe"));
    let err = x.vm.call(f, Some(copy), vec![name, Value::Object(Some(tag))]).unwrap_err();
    assert!(matches!(&err, VmError::NotYet(n) if n.missing == Missing::NotRead), "{err}");
    assert!(err.to_string().contains(PARTIAL_WRITES), "terminal: {err}");
    // Review r2 of #610: the hold's pair was appended before the stop.
    let holds = crate::hit::access::array(&mut x.vm, copy, "MovementHolds").unwrap();
    assert_eq!(holds.len(), 1, "the new hold is appended before the terminal stop");
}

/// Refusals through the world's spawn (review r1 of #610 item 2), each leaving no actor (the level list and the heap as
/// before): an `AIComponent` subclass other than the two read; and, on a bare VM without the scene layer, a component
/// to attach (`LayerAbsent`). The unloaded-handle refusal is not reachable through the spawn: the instancing plan loads
/// each template component before this step, and refuses a stand-in with no slots itself (the unit tests cover it).
#[test]
fn spawn_refusals_leave_no_actor() {
    let mut x = fx(true, true);
    let (archetype, _) = x.archetype(Some("GearboxFramework.ToyAIComponent"));
    let before = x.counts();
    match x.spawn(archetype) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.starts_with(component_attach::OTHER_AI_CLASS), "{n}");
        }
        other => panic!("expected the other-subclass refusal, got {other:?}"),
    }
    assert_eq!(x.counts(), before, "no actor made");

    let mut x = fx(false, false);
    assert!(x.vm.service::<component_attach::SceneLayer>().is_none());
    let (archetype, _) = x.archetype(Some("GearboxFramework.AIComponent"));
    let before = x.counts();
    match x.spawn(archetype) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::LayerAbsent(component_attach::SCENE_LAYER));
            assert_eq!(n.what, component_attach::ATTACH);
        }
        other => panic!("expected LayerAbsent, got {other:?}"),
    }
    assert_eq!(x.counts(), before, "no actor made");
}

/// An actor with no components spawns on a bare VM without the scene layer: the layer is not asked for.
#[test]
fn no_components_spawns_without_the_layer() {
    let mut x = fx(false, false);
    let (archetype, _) = x.archetype(None);
    let actor = x.spawn(archetype).unwrap().unwrap();
    assert_eq!(x.get(actor, "Components"), Value::array(vec![]));
}
