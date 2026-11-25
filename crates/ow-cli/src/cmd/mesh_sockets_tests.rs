//! The socket preload (lane IMPL-BODYCOMP-SOCKET-BONE-TABLES) on invented packages (AGENTS.md rule 9): a mesh export
//! with two socket subobjects, one of them an export whose load fails (its archetype is itself), and a mesh with none.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::testkit::reflect::toy_array_of;
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Env {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `GD_ToyMesh.Mesh_A` with `Socket_0` and `Socket_1`; `GD_ToyMesh.Mesh_Bad` with `Socket_0` and the cyclic
/// `Socket_Cycle`.
fn env(mode: ParityMode) -> Env {
    let dir = std::env::temp_dir().join(format!("ow-cli-mesh-sockets-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let engine = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "ArrayProperty", "NameProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let mesh = t.class("SkeletalMesh", object);
        toy_array_of(&mut t, mesh, "Sockets", "Object", object);
        let socket = t.class("SkeletalMeshSocket", object);
        t.prop("Name", socket, "SocketName", 0, 0);
        t.build()
    };
    let gd = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let mesh = t.p.add_import("Core", "Class", engine, "SkeletalMesh");
        let socket = t.p.add_import("Core", "Class", engine, "SkeletalMeshSocket");
        let mut empty = ints(&[0]);
        empty.extend_from_slice(&t.none);
        // Mesh_A, then its two sockets.
        let a = t.p.next_export();
        let mut payload = ints(&[0]);
        payload.extend(t.p.array_tag("Sockets", 2, &ints(&[a + 1, a + 2])));
        payload.extend_from_slice(&t.none);
        t.p.add_export(mesh, "Mesh_A", payload);
        for n in ["Socket_0", "Socket_1"] {
            let s = t.p.add_export(socket, n, empty.clone());
            t.p.set_outer(s, a);
        }
        // Mesh_Bad: a good socket, then one whose archetype is itself (the store's load refuses it).
        let b = t.p.next_export();
        let mut payload = ints(&[0]);
        payload.extend(t.p.array_tag("Sockets", 2, &ints(&[b + 1, b + 2])));
        payload.extend_from_slice(&t.none);
        t.p.add_export(mesh, "Mesh_Bad", payload);
        let good = t.p.add_export(socket, "Socket_Good", empty.clone());
        t.p.set_outer(good, b);
        let cycle_ref = t.p.next_export();
        let mut cycle = Export::new(socket, "Socket_Cycle", empty.clone());
        cycle.archetype = cycle_ref;
        let c = t.p.add_export_with(cycle);
        t.p.set_outer(c, b);
        t.build()
    };
    let files: Vec<(&str, &[u8])> = vec![("Core.upk", core.as_slice()), ("Engine.upk", engine.as_slice()), ("GD_ToyMesh.upk", gd.as_slice())];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    vm.find_class("Engine.SkeletalMesh").unwrap();
    vm.find_class("Engine.SkeletalMeshSocket").unwrap();
    Env { vm, dir }
}

/// The mesh at `path`, loaded (#592's preload's job), and its socket handles.
fn loaded_mesh(e: &mut Env, path: &str) -> Vec<ow_object::ids::ObjRef> {
    let m = e.vm.defs.find(&mut e.vm.space, &ObjectPath::parse(path).unwrap()).unwrap().unwrap();
    e.vm.defs.ensure_loaded(&mut e.vm.space, m).unwrap();
    let class = e.vm.space.heap.get(m).unwrap().class;
    let p = e.vm.space.prop(class, "Sockets").unwrap();
    let list = e.vm.space.read(m, PropAddr::scalar(p)).unwrap().clone();
    list.elements()
        .unwrap()
        .iter()
        .map(|v| match v {
            Value::Object(Some(s)) => *s,
            other => panic!("{other:?}"),
        })
        .collect()
}

fn harness_lines(vm: &Vm) -> Vec<String> {
    vm.log.iter().filter(|l| l.starts_with(INPUT)).cloned().collect()
}

/// Off: each unloaded socket is loaded, and the two lines say what was done; a second run loads nothing more.
#[test]
fn off_loads_the_sockets_and_logs_before_and_after() {
    let mut e = env(ParityMode::Off);
    let sockets = loaded_mesh(&mut e, "GD_ToyMesh.Mesh_A");
    assert!(sockets.iter().all(|s| e.vm.defs.is_unloaded(&e.vm.space, *s)), "cold before");
    preload_mesh_sockets(&mut e.vm, "GD_ToyMesh.Mesh_A").unwrap();
    assert!(sockets.iter().all(|s| e.vm.defs.is_loaded(&e.vm.space, *s)), "loaded after");
    let lines = harness_lines(&e.vm);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("UNVERIFIED stand-in"), "{}", lines[0]);
    assert!(lines[1].ends_with(": 2 loaded, 0 already readable, 0 unresolved stand-ins"), "{}", lines[1]);
    preload_mesh_sockets(&mut e.vm, "GD_ToyMesh.Mesh_A").unwrap();
    assert!(harness_lines(&e.vm)[3].ends_with(": 0 loaded, 2 already readable, 0 unresolved stand-ins"));
}

/// Outside Off nothing is loaded or logged.
#[test]
fn reference_does_nothing() {
    let mut e = env(ParityMode::Reference);
    let sockets = loaded_mesh(&mut e, "GD_ToyMesh.Mesh_A");
    preload_mesh_sockets(&mut e.vm, "GD_ToyMesh.Mesh_A").unwrap();
    assert!(harness_lines(&e.vm).is_empty());
    assert!(sockets.iter().all(|s| e.vm.defs.is_unloaded(&e.vm.space, *s)));
}

/// A mesh no package holds is logged and skipped, with nothing loaded.
#[test]
fn a_missing_mesh_is_logged_and_skipped() {
    let mut e = env(ParityMode::Off);
    let loads = (e.vm.defs.load_transitions() + e.vm.class_code_loads());
    preload_mesh_sockets(&mut e.vm, "GD_ToyMesh.NoSuchMesh").unwrap();
    assert!(harness_lines(&e.vm).last().unwrap().ends_with(": found nothing"));
    assert_eq!((e.vm.defs.load_transitions() + e.vm.class_code_loads()), loads);
}

/// A socket whose load fails keeps its error: logged, returned, and the earlier socket stays loaded.
#[test]
fn a_failed_socket_load_keeps_its_error() {
    let mut e = env(ParityMode::Off);
    let sockets = loaded_mesh(&mut e, "GD_ToyMesh.Mesh_Bad");
    let err = preload_mesh_sockets(&mut e.vm, "GD_ToyMesh.Mesh_Bad").unwrap_err();
    assert!(matches!(err, VmError::Def(_)), "{err:?}");
    let last = harness_lines(&e.vm).last().unwrap().clone();
    assert!(last.contains("the socket Socket_Cycle failed"), "{last}");
    assert!(e.vm.defs.is_loaded(&e.vm.space, sockets[0]), "the good socket was loaded first");
}

/// Review r1 of #641, item 3: a slotless stand-in (not an unloaded handle to the store) is counted apart, as an
/// unresolved stand-in, and is not loaded or reported readable.
#[test]
fn a_slotless_stand_in_is_reported_apart() {
    let mut e = env(ParityMode::Off);
    let sockets = loaded_mesh(&mut e, "GD_ToyMesh.Mesh_A");
    e.vm.defs.ensure_loaded(&mut e.vm.space, sockets[0]).unwrap();
    let class = e.vm.space.heap.get(sockets[1]).unwrap().class;
    let stand_in = e.vm.space.alloc(class, "Socket_StandIn", None, ow_object::heap::Origin::Runtime).unwrap();
    e.vm.space.heap.get_mut(stand_in).unwrap().slots.clear();
    let m = e.vm.defs.find(&mut e.vm.space, &ObjectPath::parse("GD_ToyMesh.Mesh_A").unwrap()).unwrap().unwrap();
    let mesh_class = e.vm.space.heap.get(m).unwrap().class;
    let p = e.vm.space.prop(mesh_class, "Sockets").unwrap();
    let list = Value::array(vec![Value::Object(Some(sockets[0])), Value::Object(Some(stand_in)), Value::Object(Some(sockets[1]))]);
    e.vm.space.write(m, PropAddr::scalar(p), list).unwrap();
    preload_mesh_sockets(&mut e.vm, "GD_ToyMesh.Mesh_A").unwrap();
    let last = harness_lines(&e.vm).last().unwrap().clone();
    assert!(last.ends_with(": 1 loaded, 1 already readable, 1 unresolved stand-ins"), "{last}");
    assert!(e.vm.space.heap.get(stand_in).unwrap().slots.is_empty(), "the stand-in is left as it is");
}
