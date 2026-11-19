//! The skeletal mesh data layer on invented data (AGENTS.md rule 9): small invented packages for a mesh export and its
//! socket subobject (identity, loading state), hand-defined classes for the socket values, and an in-memory
//! [`SkeletonSource`] for the skeleton. AI-assisted (Claude Code).

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::ints;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// An in-memory source: the skeleton it answers for one leaf name, counting its reads.
struct Fixed {
    leaf: &'static str,
    answer: Result<ReferenceSkeleton, String>,
    reads: Rc<Cell<u32>>,
}

impl SkeletonSource for Fixed {
    fn skeleton(&self, package: &str, path: &str, leaf: &str) -> Result<Option<ReferenceSkeleton>, String> {
        self.reads.set(self.reads.get() + 1);
        assert_eq!(package, "GD_ToyMesh", "the file package");
        assert!(path.ends_with(leaf), "{path}");
        if leaf != self.leaf {
            return Ok(None);
        }
        self.answer.clone().map(Some)
    }
}

fn bones(parents: &[Option<usize>]) -> ReferenceSkeleton {
    ReferenceSkeleton {
        bones: parents.iter().enumerate().map(|(i, &p)| SkeletonBone { name: format!("Bone{i}"), parent: p }).collect(),
    }
}

/// `Core.upk`; `Engine.upk` with `SkeletalMesh` (`Sockets`, an array of objects) and `SkeletalMeshSocket`; and
/// `GD_ToyMesh.upk` with `Mesh_A` (one socket subobject, `Socket_0`), `Mesh_B` (no sockets) and `NotAMesh`.
fn packages() -> Vec<(&'static str, Vec<u8>)> {
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let engine = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "ArrayProperty", "NameProperty", "ScriptStruct"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let mesh = t.class("SkeletalMesh", object);
        toy_array_of(&mut t, mesh, "Sockets", "Object", object);
        let socket = t.class("SkeletalMeshSocket", object);
        t.prop("Name", socket, "SocketName", 0, 0);
        t.prop("Name", socket, "BoneName", 0, 0);
        let _ = toy_struct(&mut t, socket, "ToyUnused", None);
        t.class("ToyOther", object);
        t.build()
    };
    let gd = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let package_class = t.p.add_import("Core", "Class", core, "Package");
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let mesh = t.p.add_import("Core", "Class", engine, "SkeletalMesh");
        let socket = t.p.add_import("Core", "Class", engine, "SkeletalMeshSocket");
        let other = t.p.add_import("Core", "Class", engine, "ToyOther");
        let mut empty = ints(&[0]);
        empty.extend_from_slice(&t.none);
        let a = t.p.next_export();
        let mut payload = ints(&[0]);
        payload.extend(t.p.array_tag("Sockets", 1, &ints(&[a + 1])));
        payload.extend_from_slice(&t.none);
        t.p.add_export(mesh, "Mesh_A", payload);
        let s = t.p.add_export(socket, "Socket_0", empty.clone());
        t.p.set_outer(s, a);
        t.p.add_export(mesh, "Mesh_B", empty.clone());
        t.p.add_export(other, "NotAMesh", empty.clone());
        // Two same-named meshes in two groups (review r1 of #569, item 2), and a mesh whose spelling the test interns
        // differently first (item 4).
        let left = t.p.add_export(package_class, "Left", empty.clone());
        let right = t.p.add_export(package_class, "Right", empty.clone());
        let l = t.p.add_export(mesh, "Twin", empty.clone());
        t.p.set_outer(l, left);
        let r = t.p.add_export(mesh, "Twin", empty.clone());
        t.p.set_outer(r, right);
        t.p.add_export(mesh, "Mesh_C", empty);
        t.build()
    };
    vec![("Core.upk", core), ("Engine.upk", engine), ("GD_ToyMesh.upk", gd)]
}

struct Env {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn env() -> Env {
    let dir = std::env::temp_dir().join(format!("ow-world-skel-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let files = packages();
    let borrowed: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    let defs = DefStore::from_synthetic(&dir, &borrowed, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    Env { vm, dir }
}

fn find(e: &mut Env, path: &str) -> ObjRef {
    let p = ow_core::ObjectPath::parse(path).unwrap();
    e.vm.defs.find(&mut e.vm.space, &p).unwrap().expect(path)
}

fn not_yet(r: Result<impl std::fmt::Debug, VmError>) -> ow_core::NotYet {
    match r {
        Err(VmError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// With no source installed, the layer is `LayerAbsent` with the text #529 names.
#[test]
fn no_source_is_the_named_layer() {
    let mut e = env();
    let mesh = find(&mut e, "GD_ToyMesh.Mesh_A");
    let n = not_yet(reference_skeleton(&e.vm, mesh));
    assert!(matches!(&n.missing, Missing::LayerAbsent(l) if *l == LAYER), "{n}");
}

/// The source's skeleton, cached by identity (a second ask reads nothing), with nothing written; a mesh the source has
/// no data for is the named `LayerAbsent`.
#[test]
fn the_skeleton_comes_from_the_source_and_is_cached() {
    let mut e = env();
    let (a, b) = (find(&mut e, "GD_ToyMesh.Mesh_A"), find(&mut e, "GD_ToyMesh.Mesh_B"));
    let reads = Rc::new(Cell::new(0));
    let source = Fixed { leaf: "Mesh_A", answer: Ok(bones(&[None, Some(0), Some(1), Some(0)])), reads: reads.clone() };
    SkeletalMeshes::install(&mut e.vm, Box::new(source)).unwrap();
    let heap = format!("{:?}", e.vm.space.heap);
    let s = reference_skeleton(&e.vm, a).unwrap();
    assert_eq!(s.len(), 4);
    assert_eq!(s.bones[2].parent, Some(1));
    assert_eq!(s.bone_index("bone3"), Some(3), "names compare as names do");
    let again = reference_skeleton(&e.vm, a).unwrap();
    assert!(Rc::ptr_eq(&s, &again));
    assert_eq!(reads.get(), 1, "the cache answers the second ask");
    let n = not_yet(reference_skeleton(&e.vm, b));
    assert!(matches!(&n.missing, Missing::LayerAbsent(l) if *l == LAYER), "{n}");
    assert_eq!(format!("{:?}", e.vm.space.heap), heap, "nothing written");
}

/// A parse failure and a skeleton out of parent order are script errors naming the mesh; a receiver that is not a
/// skeletal mesh is a script error.
#[test]
fn bad_data_is_a_typed_error() {
    let mut twins = bones(&[None, Some(0)]);
    twins.bones[1].name = "BONE0".to_string();
    for answer in [Err("a truncated chunk".to_string()), Ok(bones(&[None, Some(2), Some(0)])), Ok(bones(&[Some(0)])), Ok(twins)] {
        let mut e = env();
        let a = find(&mut e, "GD_ToyMesh.Mesh_A");
        SkeletalMeshes::install(&mut e.vm, Box::new(Fixed { leaf: "Mesh_A", answer, reads: Rc::new(Cell::new(0)) })).unwrap();
        match reference_skeleton(&e.vm, a) {
            Err(VmError::Script { message, .. }) => assert!(message.contains("GD_ToyMesh.Mesh_A"), "{message}"),
            other => panic!("{other:?}"),
        }
    }
    let mut e = env();
    let other = find(&mut e, "GD_ToyMesh.NotAMesh");
    assert!(matches!(reference_skeleton(&e.vm, other), Err(VmError::Script { .. })));
}

/// The mesh's socket subobject is a handle the store has not loaded: `LayerAbsent` (the store's load), nothing
/// loaded. Once the harness has loaded it, the socket is read.
#[test]
fn an_unloaded_socket_is_refused_and_loads_nothing() {
    let mut e = env();
    let a = find(&mut e, "GD_ToyMesh.Mesh_A");
    let class = e.vm.space.heap.get(a).unwrap().class;
    let p = e.vm.space.prop(class, "Sockets").unwrap();
    let Value::Object(Some(socket)) = e.vm.space.read(a, PropAddr::scalar(p)).unwrap().elements().unwrap()[0] else {
        panic!("a socket")
    };
    // Review r1 of #569, item 5: the fixture must give an unloaded handle, or the refusal is not tested.
    assert!(e.vm.defs.is_unloaded(&e.vm.space, socket), "the fixture's socket arrives unloaded");
    let heap = format!("{:?}", e.vm.space.heap);
    let n = not_yet(sockets(&e.vm, a));
    assert!(matches!(&n.missing, Missing::LayerAbsent(_)), "{n}");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, socket), "nothing loaded");
    assert_eq!(format!("{:?}", e.vm.space.heap), heap);
    e.vm.defs.ensure_loaded(&mut e.vm.space, socket).unwrap();
    // The toy socket class declares no location fields: the read after the load is a script error, not a NotYet.
    assert!(matches!(sockets(&e.vm, a), Err(VmError::Script { .. })));
    let b = find(&mut e, "GD_ToyMesh.Mesh_B");
    assert_eq!(sockets(&e.vm, b).unwrap(), Vec::new());
}

// ---------------------------------------------------------------------------------------------------------------
// The socket values, on hand-defined classes
// ---------------------------------------------------------------------------------------------------------------

struct Hand {
    vm: Vm,
    mesh: ClassId,
    socket: ClassId,
    vector: StructId,
    rotator: StructId,
}

fn hand() -> Hand {
    let mut s = ObjectSpace::new();
    let object = s.classes.define("Core.Object", None, vec![]).unwrap();
    let d = |s: &mut ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(s.names.intern(n), k);
    let decls = vec![d(&mut s, "X", PropKind::Float), d(&mut s, "Y", PropKind::Float), d(&mut s, "Z", PropKind::Float)];
    let vector = s.classes.define_struct("Core.Object.Vector", None, decls).unwrap();
    let decls = vec![d(&mut s, "Pitch", PropKind::Int), d(&mut s, "Yaw", PropKind::Int), d(&mut s, "Roll", PropKind::Int)];
    let rotator = s.classes.define_struct("Core.Object.Rotator", None, decls).unwrap();
    let decls = vec![d(&mut s, "Sockets", PropKind::Array(Box::new(PropKind::Object { class: object })))];
    let mesh = s.classes.define("Engine.SkeletalMesh", Some(object), decls).unwrap();
    let decls = vec![
        d(&mut s, "SocketName", PropKind::Name),
        d(&mut s, "BoneName", PropKind::Name),
        d(&mut s, "RelativeLocation", PropKind::Struct(vector)),
        d(&mut s, "RelativeRotation", PropKind::Struct(rotator)),
        d(&mut s, "RelativeScale", PropKind::Struct(vector)),
    ];
    let socket = s.classes.define("Engine.SkeletalMeshSocket", Some(object), decls).unwrap();
    let vm = Vm::new(s, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    Hand { vm, mesh, socket, vector, rotator }
}

impl Hand {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn socket(&mut self, name: &str, bone: &str, at: [f32; 3], rot: [i32; 3]) -> ObjRef {
        let s = self.vm.space.alloc(self.socket, name, None, Origin::Runtime).unwrap();
        let (n, b) = (self.vm.space.names.intern(name), self.vm.space.names.intern(bone));
        self.set(s, "SocketName", Value::Name(n));
        self.set(s, "BoneName", Value::Name(b));
        self.set(s, "RelativeLocation", Value::structure(self.vector, at.iter().map(|&f| Value::Float(f)).collect()));
        self.set(s, "RelativeRotation", Value::structure(self.rotator, rot.iter().map(|&i| Value::Int(i)).collect()));
        self.set(s, "RelativeScale", Value::structure(self.vector, vec![Value::Float(1.0); 3]));
        s
    }
}

/// The sockets in `Sockets` order with their bone, offset, rotation and scale; a None entry is `NotRead`; a stand-in
/// with no slots is `LayerAbsent`.
#[test]
fn the_sockets_are_read_in_order() {
    let mut h = hand();
    let mesh = h.vm.space.alloc(h.mesh, "Mesh", None, Origin::Runtime).unwrap();
    let (s0, s1) = (h.socket("Spine", "Bone2", [1.0, 2.0, 3.0], [0, 16384, 0]), h.socket("Root", "Bone0", [0.0; 3], [0; 3]));
    h.set(mesh, "Sockets", Value::array(vec![Value::Object(Some(s0)), Value::Object(Some(s1))]));
    let heap = format!("{:?}", h.vm.space.heap);
    let out = sockets(&h.vm, mesh).unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!((out[0].name.as_str(), out[0].bone.as_str()), ("Spine", "Bone2"));
    assert_eq!((out[0].location, out[0].rotation, out[0].scale), ([1.0, 2.0, 3.0], [0, 16384, 0], [1.0; 3]));
    assert_eq!(out[1].name, "Root");
    assert_eq!(format!("{:?}", h.vm.space.heap), heap, "nothing written");
    h.set(mesh, "Sockets", Value::array(vec![Value::Object(Some(s0)), Value::Object(None)]));
    assert_eq!(not_yet(sockets(&h.vm, mesh)).missing, Missing::NotRead);
    let socket_class = h.socket;
    let stand_in = h.vm.space.alloc_stand_in(socket_class, "Socket_StandIn", Origin::Default { class: socket_class }).unwrap();
    h.set(mesh, "Sockets", Value::array(vec![Value::Object(Some(stand_in))]));
    assert!(matches!(not_yet(sockets(&h.vm, mesh)).missing, Missing::LayerAbsent(_)));
    // No source is installed here: the skeleton lookup names the layer.
    assert!(matches!(not_yet(reference_skeleton(&h.vm, mesh)).missing, Missing::LayerAbsent(_)));
}

/// Review r1 of #569, item 2: two meshes of one package with the same name (in two groups) are refused, whatever the
/// source would answer, so no two cache entries can hold one file's bones.
#[test]
fn same_named_meshes_in_one_package_are_refused() {
    let mut e = env();
    let left = find(&mut e, "GD_ToyMesh.Left.Twin");
    let right = find(&mut e, "GD_ToyMesh.Right.Twin");
    assert_ne!(left, right);
    let reads = Rc::new(Cell::new(0));
    SkeletalMeshes::install(&mut e.vm, Box::new(Fixed { leaf: "Twin", answer: Ok(bones(&[None])), reads: reads.clone() })).unwrap();
    for mesh in [left, right] {
        let n = not_yet(reference_skeleton(&e.vm, mesh));
        assert!(matches!(&n.missing, Missing::LayerAbsent(l) if *l == LAYER), "{n}");
        assert!(n.what.contains("ambiguous"), "{n}");
    }
    assert_eq!(reads.get(), 0, "the source is not asked");
}

/// Review r1 of #569, item 4: the file lookup uses the export entry's own spelling, not the name table's first one.
#[test]
fn the_leaf_is_the_exports_own_spelling() {
    let mut e = env();
    e.vm.space.names.intern("MESH_C");
    let c = find(&mut e, "GD_ToyMesh.Mesh_C");
    let reads = Rc::new(Cell::new(0));
    SkeletalMeshes::install(&mut e.vm, Box::new(Fixed { leaf: "Mesh_C", answer: Ok(bones(&[None])), reads })).unwrap();
    assert_eq!(reference_skeleton(&e.vm, c).unwrap().len(), 1, "the source saw Mesh_C, not MESH_C");
}
