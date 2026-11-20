//! Tests of `SkeletalMeshComponent.SetSkeletalMesh` in Off (lane IMPL-SET-SKELETAL-MESH) on invented classes, defined
//! by hand with the note's class and field names, and a small invented package for an unloaded handle (AGENTS.md
//! rule 9). AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const PATH: &str = "Engine.SkeletalMeshComponent.SetSkeletalMesh";

/// `ToyMesh.upk`: a holder whose `Held` names `Thing_0`, an export the store keeps unloaded until it is loaded.
fn toy_package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let thing = t.class("Thing", object);
    let holder = t.class("Holder", object);
    t.prop("Object", holder, "Held", 0, object);
    let held = t.p.next_export() + 1;
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("Held", held));
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(holder, "Holder_0", payload));
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(thing, "Thing_0", payload));
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

fn fx() -> Fx {
    let mut s = ObjectSpace::new();
    let object = s.classes.define("Core.Object", None, vec![]).unwrap();
    let d = |s: &mut ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(s.names.intern(n), k);
    let mesh = s.classes.define("Engine.SkeletalMesh", Some(object), vec![]).unwrap();
    let decls = vec![d(&mut s, "SkeletalMesh", PropKind::Object { class: mesh })];
    let component = s.classes.define("Engine.SkeletalMeshComponent", Some(object), decls).unwrap();
    s.classes.define("Toy.NoMeshComponent", Some(object), vec![]).unwrap();
    let decls = vec![d(&mut s, "SkeletalMesh", PropKind::Int)];
    s.classes.define("Toy.IntMeshComponent", Some(object), decls).unwrap();
    let params = vec![
        d(&mut s, "NewMesh", PropKind::Object { class: mesh }).with_flags(PropertyDecl::PARAM),
        d(&mut s, "bKeepSpaceBases", PropKind::Bool).with_flags(PropertyDecl::PARAM | PropertyDecl::OPTIONAL),
    ];
    let dir = std::env::temp_dir().join(format!("ow-world-ssm-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let package = toy_package();
    let defs = DefStore::from_synthetic(&dir, &[("ToyMesh.upk", package.as_slice())], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let mut vm = Vm::new(s, defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let f = FunctionDecl::native(ObjectPath::parse(PATH).unwrap(), SET_SKELETAL_MESH).with_owner(component).with_params(params);
    vm.declare_function(f).unwrap();
    register(&mut vm).unwrap();
    Fx { vm, dir }
}

/// #518's counters: heap committed writes, generator draws and definition-store load transitions (review r1 of #599,
/// item 1). Every call, refusals included, must leave all three as they were.
fn counters(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

impl Fx {
    fn obj(&mut self, path: &str, name: &str) -> ObjRef {
        let c = self.vm.space.classes.by_path(path).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn component(&mut self, mesh: Option<ObjRef>) -> ObjRef {
        let c = self.obj("Engine.SkeletalMeshComponent", "Mesh0");
        let class = self.vm.space.heap.get(c).unwrap().class;
        let p = self.vm.space.prop(class, "SkeletalMesh").unwrap();
        *self.vm.space.slot_mut(c, PropAddr::scalar(p)).unwrap() = Value::Object(mesh);
        c
    }

    fn mesh_of(&self, c: ObjRef) -> Value {
        let class = self.vm.space.heap.get(c).unwrap().class;
        let p = self.vm.space.prop(class, "SkeletalMesh").unwrap();
        self.vm.space.read(c, PropAddr::scalar(p)).unwrap().clone()
    }

    /// The toy package's unloaded `Thing_0` handle.
    fn unloaded(&mut self) -> ObjRef {
        let path = ObjectPath::parse("ToyMesh.Holder_0").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        let class = self.vm.space.heap.get(holder).unwrap().class;
        let p = self.vm.space.prop(class, "Held").unwrap();
        let Value::Object(Some(o)) = self.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else {
            panic!("a held object")
        };
        assert!(self.vm.defs.is_unloaded(&self.vm.space, o));
        o
    }

    /// The native on `c`, with `args` as given (an omitted `bKeepSpaceBases` is not supplied), checked write-free.
    fn call(&mut self, c: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH).unwrap();
        let before = counters(&self.vm);
        let r = self.vm.call(f, Some(c), args).map(|r| r.ret);
        assert_eq!(counters(&self.vm), before, "no write, draw or load: {r:?}");
        r
    }
}

fn not_yet(r: Result<Value, VmError>, missing: Missing, words: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, missing, "{n}");
            assert!(n.what.contains(words), "{n}");
            assert!(!n.what.starts_with("stopped after partial writes: "), "{n}");
        }
        other => panic!("expected NotYet {missing:?}, got {other:?}"),
    }
}

fn script_error(r: Result<Value, VmError>, words: &str) {
    match r {
        Err(VmError::Script { message, .. }) => assert!(message.contains(words), "{message}"),
        other => panic!("expected Script, got {other:?}"),
    }
}

/// Equal handles are the native's own "nothing to do", with the flag omitted or supplied, even when the mesh is an
/// unloaded handle (identity comes before residency).
#[test]
fn the_same_mesh_does_nothing() {
    let mut x = fx();
    let none = x.component(None);
    assert_eq!(x.call(none, vec![Value::Object(None)]).unwrap(), Value::None, "None to None, the flag omitted");
    x.call(none, vec![Value::Object(None), Value::Bool(true)]).unwrap();
    assert_eq!(x.mesh_of(none), Value::Object(None));
    let mesh = x.obj("Engine.SkeletalMesh", "Mesh");
    let set = x.component(Some(mesh));
    x.call(set, vec![Value::Object(Some(mesh)), Value::Bool(false)]).unwrap();
    let lazy = x.unloaded();
    let lazy_set = x.component(Some(lazy));
    x.call(lazy_set, vec![Value::Object(Some(lazy))]).unwrap();
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "the unloaded mesh stays unloaded");
}

/// A replacement and a clear are the mesh change's rows now (lane IMPL-SKELMESH-MESH-CHANGE); their tests are in
/// `skeletal_mesh_change_tests.rs`. On this fixture's bare component (no row field declared), a change to a resident
/// mesh is the first row's field read, a script error, with nothing written.
#[test]
fn a_mesh_change_on_a_bare_component_is_a_script_error() {
    let mut x = fx();
    let mesh = x.obj("Engine.SkeletalMesh", "Mesh");
    let none = x.component(None);
    script_error(x.call(none, vec![Value::Object(Some(mesh))]), "PhysicsAssetInstance");
    assert_eq!(x.mesh_of(none), Value::Object(None), "unchanged");
}

/// The receiver's residency comes first; then the field and the argument shapes are `VmError::Script`.
#[test]
fn the_receiver_and_the_shapes() {
    let mut x = fx();
    let lazy = x.unloaded();
    not_yet(x.call(lazy, vec![Value::Object(None)]), Missing::NotRead, "the component is an unloaded handle");
    // Review r2 of #599: the receiver is checked before a badly shaped argument.
    not_yet(x.call(lazy, vec![Value::Int(3)]), Missing::NotRead, "the component is an unloaded handle");
    let bare = x.obj("Toy.NoMeshComponent", "Bare");
    script_error(x.call(bare, vec![Value::Object(None)]), "declares no SkeletalMesh");
    let int = x.obj("Toy.IntMeshComponent", "Int");
    script_error(x.call(int, vec![Value::Object(None)]), "is not an object field");
    let c = x.component(None);
    script_error(x.call(c, vec![Value::Int(3)]), "NewMesh holds");
    script_error(x.call(c, vec![Value::Object(None), Value::Int(1)]), "bKeepSpaceBases holds");
}
