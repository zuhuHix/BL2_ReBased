//! The anim-set preload of the `maya` step (lane PRELOAD-MAYA-ANIMSETS) on an invented package (AGENTS.md rule 9):
//! an archetype whose `Mesh` component lists two anim sets, each with its sequences, and a third set, with a sequence of
//! its own, that the list does not name. AI-assisted (Claude Code).

use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;

use super::super::{anim_set_preload_line, preload_mesh_anim_sets};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "ArrayProperty"]);
    let object = t.class("Object", 0);
    let pawn = t.class("ToyPawn", object);
    t.prop("Object", pawn, "Mesh", 0, object);
    let component = t.class("ToyMeshComponent", object);
    ow_package::testkit::reflect::toy_array_of(&mut t, component, "AnimSets", "Object", object);
    let set = t.class("ToyAnimSet", object);
    ow_package::testkit::reflect::toy_array_of(&mut t, set, "Sequences", "Object", object);
    let sequence = t.class("ToyAnimSequence", object);
    let plain = |t: &mut ScriptToy, class: i32, name: &str| {
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend_from_slice(&t.none);
        t.p.add_export(class, name, payload)
    };
    let list = |t: &mut ScriptToy, name: &str, items: &[i32]| -> Vec<u8> {
        let mut value = i32::try_from(items.len()).unwrap().to_le_bytes().to_vec();
        for i in items {
            value.extend_from_slice(&i.to_le_bytes());
        }
        t.p.tag(name, "ArrayProperty", &value)
    };
    let a1 = plain(&mut t, sequence, "Seq_A1");
    let a2 = plain(&mut t, sequence, "Seq_A2");
    let b1 = plain(&mut t, sequence, "Seq_B1");
    let c1 = plain(&mut t, sequence, "Seq_C1");
    let set_export = |t: &mut ScriptToy, name: &str, items: &[i32]| {
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(list(t, "Sequences", items));
        payload.extend_from_slice(&t.none);
        t.p.add_export(set, name, payload)
    };
    let set_a = set_export(&mut t, "Set_A", &[a1, a2]);
    let set_b = set_export(&mut t, "Set_B", &[b1]);
    set_export(&mut t, "Set_C", &[c1]);
    let mut mesh = 0i32.to_le_bytes().to_vec();
    mesh.extend(list(&mut t, "AnimSets", &[set_a, 0, set_b]));
    mesh.extend_from_slice(&t.none);
    let mesh_ref = t.p.add_export(component, "ToyMesh_0", mesh);
    let mut archetype = 0i32.to_le_bytes().to_vec();
    archetype.extend(t.p.object_tag("Mesh", mesh_ref));
    archetype.extend_from_slice(&t.none);
    t.p.add_export(pawn, "ToyArchetype", archetype);
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

fn objects(v: &Value) -> Vec<ObjRef> {
    v.elements().unwrap().iter().filter_map(|e| if let Value::Object(Some(o)) = e { Some(*o) } else { None }).collect()
}

/// The listed sets and their sequences are loaded, each logged; the unlisted set and its sequence stay unloaded.
#[test]
fn exactly_the_mesh_s_anim_sets_and_their_sequences_are_loaded() {
    let dir = std::env::temp_dir().join(format!("ow-cli-animsets-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("GD_ToyPawn.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let archetype = find(&mut vm, "GD_ToyPawn.ToyArchetype");
    // The component, as the same-package preload leaves it: resident.
    let Value::Object(Some(mesh)) = slot(&vm, archetype, "Mesh") else { panic!("a Mesh") };
    vm.defs.ensure_loaded(&mut vm.space, mesh).unwrap();
    let sets = objects(&slot(&vm, mesh, "AnimSets"));
    assert_eq!(sets.len(), 2, "two listed, one None entry");
    assert!(sets.iter().all(|&s| vm.defs.is_unloaded(&vm.space, s)));
    // The unlisted set: its handle exists only once something names it; find it after the preload.
    vm.log.clear();
    let before = vm.defs.load_transitions();
    preload_mesh_anim_sets(&mut vm, archetype).unwrap();
    assert_eq!(vm.defs.load_transitions() - before, 5, "two sets and three sequences, nothing else");
    for &s in &sets {
        assert!(!vm.defs.is_unloaded(&vm.space, s), "a listed set is loaded");
        for q in objects(&slot(&vm, s, "Sequences")) {
            assert!(!vm.defs.is_unloaded(&vm.space, q), "its sequences are loaded");
        }
    }
    let lines: Vec<&String> = vm.log.iter().filter(|l| l.starts_with("harness input:")).collect();
    let expected: Vec<String> = ["GD_ToyPawn.Set_A", "GD_ToyPawn.Seq_A1", "GD_ToyPawn.Seq_A2", "GD_ToyPawn.Set_B", "GD_ToyPawn.Seq_B1"]
        .iter()
        .map(|p| anim_set_preload_line(p))
        .collect();
    assert_eq!(lines.iter().map(|l| l.as_str()).collect::<Vec<_>>(), expected.iter().map(String::as_str).collect::<Vec<_>>());
    // Nothing else: the unlisted set and its sequence were never loaded by the preload.
    assert!(!vm.log.iter().any(|l| l.contains("Set_C") || l.contains("Seq_C1")), "{:?}", vm.log);
    let loads = vm.defs.load_transitions();
    preload_mesh_anim_sets(&mut vm, archetype).unwrap();
    assert_eq!(vm.defs.load_transitions(), loads, "a second run loads nothing");
    let _ = std::fs::remove_dir_all(&dir);
}
