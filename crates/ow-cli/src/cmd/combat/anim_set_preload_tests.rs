//! The anim-set preload of the `maya` step (lane PRELOAD-MAYA-ANIMSETS) on an invented package (AGENTS.md rule 9).
//! An archetype has a `Mesh` component listing two anim sets, and a `BodyClass` with weapon holds. The holds name sets
//! through `AnimSetList`, one through its `ParentDefinition`, one through the parent body class
//! (`WeaponActionParentClass`), and one repeats a set of the component's list. One hold is not resident, and one set
//! is named by nothing. AI-assisted (Claude Code).

use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::toy_array_of;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;

use ow_object::heap::Origin;

use super::super::{anim_set_preload_line, hold_anim_sets, preload_mesh_anim_sets};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "ArrayProperty"]);
    let object = t.class("Object", 0);
    let pawn = t.class("ToyPawn", object);
    t.prop("Object", pawn, "Mesh", 0, object);
    t.prop("Object", pawn, "BodyClass", 0, object);
    let component = t.class("ToyMeshComponent", object);
    toy_array_of(&mut t, component, "AnimSets", "Object", object);
    let body = t.class("ToyBodyClass", object);
    toy_array_of(&mut t, body, "WeaponHoldDefs", "Object", object);
    t.prop("Object", body, "DefaultWeaponHoldDef", 0, object);
    t.prop("Object", body, "WeaponActionParentClass", 0, object);
    let hold = t.class("ToyHold", object);
    toy_array_of(&mut t, hold, "AnimSetList", "Object", object);
    t.prop("Object", hold, "ParentDefinition", 0, object);
    let set = t.class("ToyAnimSet", object);
    toy_array_of(&mut t, set, "Sequences", "Object", object);
    let sequence = t.class("ToyAnimSequence", object);
    let list = |t: &mut ScriptToy, name: &str, items: &[i32]| -> Vec<u8> {
        let mut value = i32::try_from(items.len()).unwrap().to_le_bytes().to_vec();
        for i in items {
            value.extend_from_slice(&i.to_le_bytes());
        }
        t.p.tag(name, "ArrayProperty", &value)
    };
    // An export of `class` named `name` with the tags `tags` writes.
    let export = |t: &mut ScriptToy, class: i32, name: &str, tags: &dyn Fn(&mut ScriptToy) -> Vec<u8>| {
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(tags(t));
        payload.extend_from_slice(&t.none);
        t.p.add_export(class, name, payload)
    };
    let seq = |t: &mut ScriptToy, name: &str| export(t, sequence, name, &|_| Vec::new());
    let anim_set = |t: &mut ScriptToy, name: &str, items: Vec<i32>| {
        export(t, set, name, &move |t| list(t, "Sequences", &items))
    };
    let (a1, a2, b1) = (seq(&mut t, "Seq_A1"), seq(&mut t, "Seq_A2"), seq(&mut t, "Seq_B1"));
    let (c1, d1, e1, f1, g1) =
        (seq(&mut t, "Seq_C1"), seq(&mut t, "Seq_D1"), seq(&mut t, "Seq_E1"), seq(&mut t, "Seq_F1"), seq(&mut t, "Seq_G1"));
    let set_a = anim_set(&mut t, "Set_A", vec![a1, a2]);
    let set_b = anim_set(&mut t, "Set_B", vec![b1]);
    anim_set(&mut t, "Set_C", vec![c1]);
    let set_d = anim_set(&mut t, "Set_D", vec![d1]);
    let set_e = anim_set(&mut t, "Set_E", vec![e1]);
    let set_f = anim_set(&mut t, "Set_F", vec![f1]);
    let set_g = anim_set(&mut t, "Set_G", vec![g1]);
    let hold_export = |t: &mut ScriptToy, name: &str, sets: Vec<i32>, parent: i32| {
        export(t, hold, name, &move |t| {
            let mut tags = list(t, "AnimSetList", &sets);
            if parent != 0 {
                tags.extend(t.p.object_tag("ParentDefinition", parent));
            }
            tags
        })
    };
    let hold0 = hold_export(&mut t, "Hold_Parent", vec![set_e], 0);
    let hold1 = hold_export(&mut t, "Hold_Run", vec![set_d], hold0);
    let hold2 = hold_export(&mut t, "Hold_Default", vec![set_a], 0);
    let hold3 = hold_export(&mut t, "Hold_ParentClass", vec![set_f], 0);
    let hold4 = hold_export(&mut t, "Hold_Lazy", vec![set_g], 0);
    let parent_body = export(&mut t, body, "Body_Parent", &move |t| list(t, "WeaponHoldDefs", &[hold3]));
    let body_class = export(&mut t, body, "Body_Toy", &move |t| {
        let mut tags = list(t, "WeaponHoldDefs", &[hold1, 0, hold4]);
        tags.extend(t.p.object_tag("DefaultWeaponHoldDef", hold2));
        tags.extend(t.p.object_tag("WeaponActionParentClass", parent_body));
        tags
    });
    let mesh_ref = export(&mut t, component, "ToyMesh_0", &move |t| list(t, "AnimSets", &[set_a, 0, set_b]));
    export(&mut t, pawn, "ToyArchetype", &move |t| {
        let mut tags = t.p.object_tag("Mesh", mesh_ref);
        tags.extend(t.p.object_tag("BodyClass", body_class));
        tags
    });
    t.build()
}

fn find(vm: &mut Vm, path: &str) -> ObjRef {
    let path = ObjectPath::parse(path).unwrap();
    vm.defs.find(&mut vm.space, &path).unwrap().expect("found")
}

fn slot(vm: &Vm, o: ObjRef, name: &str) -> Value {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
}

fn object(v: &Value) -> ObjRef {
    match v {
        Value::Object(Some(o)) => *o,
        other => panic!("{other:?}"),
    }
}

fn objects(v: &Value) -> Vec<ObjRef> {
    v.elements().unwrap().iter().filter_map(|e| if let Value::Object(Some(o)) = e { Some(*o) } else { None }).collect()
}

/// The component's sets, then the holds' sets (each once), and their sequences are loaded, each logged, in that
/// order; the hold that is not resident is skipped, so its set stays unloaded; the set nothing names is never loaded.
#[test]
fn exactly_the_mesh_s_and_the_holds_anim_sets_and_their_sequences_are_loaded() {
    let dir = std::env::temp_dir().join(format!("ow-cli-animsets-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("GD_ToyPawn.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let archetype = find(&mut vm, "GD_ToyPawn.ToyArchetype");
    // What the set-up has made resident before this step: the component, the body classes and four holds. The lazy
    // hold stays a handle.
    let mesh = object(&slot(&vm, archetype, "Mesh"));
    let body = object(&slot(&vm, archetype, "BodyClass"));
    for o in [mesh, body] {
        vm.defs.ensure_loaded(&mut vm.space, o).unwrap();
    }
    let parent_body = object(&slot(&vm, body, "WeaponActionParentClass"));
    vm.defs.ensure_loaded(&mut vm.space, parent_body).unwrap();
    let holds = objects(&slot(&vm, body, "WeaponHoldDefs"));
    let (run, lazy_hold) = (holds[0], holds[1]);
    let default = object(&slot(&vm, body, "DefaultWeaponHoldDef"));
    let parent_class_hold = objects(&slot(&vm, parent_body, "WeaponHoldDefs"))[0];
    for o in [run, default, parent_class_hold] {
        vm.defs.ensure_loaded(&mut vm.space, o).unwrap();
    }
    let parent_hold = object(&slot(&vm, run, "ParentDefinition"));
    vm.defs.ensure_loaded(&mut vm.space, parent_hold).unwrap();
    assert!(vm.defs.is_unloaded(&vm.space, lazy_hold));
    vm.log.clear();
    let before = vm.defs.load_transitions();
    preload_mesh_anim_sets(&mut vm, archetype).unwrap();
    let loaded = [
        "Set_A", "Seq_A1", "Seq_A2", "Set_B", "Seq_B1", // the component's list
        "Set_E", "Seq_E1", "Set_D", "Seq_D1", // Hold_Run: its parent definition's set first
        "Set_F", "Seq_F1", // the parent body class's hold (Hold_Default's Set_A is already loaded)
    ];
    assert_eq!(vm.defs.load_transitions() - before, loaded.len() as u64, "those sets and sequences, nothing else");
    let mut expected: Vec<String> = loaded.iter().map(|n| anim_set_preload_line(&format!("GD_ToyPawn.{n}"))).collect();
    // The lazy hold's skip is logged where the walk meets it, before any load.
    expected.insert(0, "harness input: set-up preload: the weapon hold GD_ToyPawn.Hold_Lazy is not resident; skipped".to_string());
    let lines: Vec<String> = vm.log.iter().filter(|l| l.starts_with("harness input:")).cloned().collect();
    assert_eq!(lines, expected);
    assert!(vm.defs.is_unloaded(&vm.space, lazy_hold), "a hold is never preloaded");
    assert!(!vm.log.iter().any(|l| l.contains("Set_C") || l.contains("Set_G")), "{:?}", vm.log);
    let loads = vm.defs.load_transitions();
    preload_mesh_anim_sets(&mut vm, archetype).unwrap();
    assert_eq!(vm.defs.load_transitions(), loads, "a second run loads nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A VM over the test package, nothing loaded yet.
fn toy_vm(dir: &std::path::Path) -> Vm {
    let bytes = package();
    let defs = DefStore::from_synthetic(dir, &[("GD_ToyPawn.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
}

/// A run-time object of `class` (a class of the test package) whose slots are all set. It has no store path, so the
/// log names it by its reference.
fn runtime(vm: &mut Vm, class: &str, name: &str) -> ObjRef {
    let c = vm.find_class(&format!("GD_ToyPawn.{class}")).unwrap();
    ow_vm::interp::instantiate_from_defaults(vm, c).unwrap();
    let o = vm.space.alloc(c, name, None, Origin::Runtime).unwrap();
    let defaults = vm.defs.class_default(&mut vm.space, &format!("GD_ToyPawn.{class}")).unwrap();
    let slots = vm.space.heap.get(defaults).unwrap().slots.clone();
    vm.space.heap.get_mut(o).unwrap().slots = slots;
    o
}

fn put(vm: &mut Vm, o: ObjRef, name: &str, v: Value) {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    *vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
}

fn list(items: &[ObjRef]) -> Value {
    Value::array(items.iter().map(|o| Value::Object(Some(*o))).collect())
}

/// Review of #754, LOW 1: a body class, a hold or an anim set that is a stand-in without its slots is logged and
/// skipped, never read (no `SlotMissing`).
#[test]
fn stand_ins_are_logged_and_skipped() {
    let dir = std::env::temp_dir().join(format!("ow-cli-animsets-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let mut vm = toy_vm(&dir);
    // A stand-in body class: no slots.
    let stand_in_body = runtime(&mut vm, "ToyBodyClass", "StandInBody");
    vm.space.heap.get_mut(stand_in_body).unwrap().slots.clear();
    assert_eq!(hold_anim_sets(&mut vm, stand_in_body).unwrap(), Vec::<ObjRef>::new());
    assert!(vm.log.iter().any(|l| l.contains("the body class") && l.ends_with("skipped")), "{:?}", vm.log);
    // A resident body class whose only hold is a stand-in.
    let body = runtime(&mut vm, "ToyBodyClass", "Body");
    let hold = runtime(&mut vm, "ToyHold", "StandInHold");
    vm.space.heap.get_mut(hold).unwrap().slots.clear();
    put(&mut vm, body, "WeaponHoldDefs", list(&[hold]));
    assert_eq!(hold_anim_sets(&mut vm, body).unwrap(), Vec::<ObjRef>::new());
    assert!(vm.log.iter().any(|l| l.contains("the weapon hold") && l.ends_with("skipped")), "{:?}", vm.log);
    // A hold naming a stand-in set: the set is skipped at the preload, its Sequences never read.
    let archetype = runtime(&mut vm, "ToyPawn", "Archetype");
    let body = runtime(&mut vm, "ToyBodyClass", "Body2");
    let hold = runtime(&mut vm, "ToyHold", "Hold");
    let set = runtime(&mut vm, "ToyAnimSet", "StandInSet");
    vm.space.heap.get_mut(set).unwrap().slots.clear();
    put(&mut vm, hold, "AnimSetList", list(&[set]));
    put(&mut vm, body, "WeaponHoldDefs", list(&[hold]));
    put(&mut vm, archetype, "BodyClass", Value::Object(Some(body)));
    preload_mesh_anim_sets(&mut vm, archetype).unwrap();
    assert!(vm.log.iter().any(|l| l.contains("the anim set") && l.ends_with("skipped")), "{:?}", vm.log);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review of #754, LOW 2: a body-class chain and a hold chain that come back to themselves stop, logged.
#[test]
fn chain_loops_stop_with_a_harness_line() {
    let dir = std::env::temp_dir().join(format!("ow-cli-animsets-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let mut vm = toy_vm(&dir);
    let body = runtime(&mut vm, "ToyBodyClass", "LoopBody");
    let hold = runtime(&mut vm, "ToyHold", "LoopHold");
    put(&mut vm, body, "WeaponActionParentClass", Value::Object(Some(body)));
    put(&mut vm, hold, "ParentDefinition", Value::Object(Some(hold)));
    put(&mut vm, body, "WeaponHoldDefs", list(&[hold]));
    hold_anim_sets(&mut vm, body).unwrap();
    assert!(vm.log.iter().any(|l| l.contains("the body-class chain comes back to")), "{:?}", vm.log);
    assert!(vm.log.iter().any(|l| l.contains("the weapon-hold chain comes back to")), "{:?}", vm.log);
    let _ = std::fs::remove_dir_all(&dir);
}
