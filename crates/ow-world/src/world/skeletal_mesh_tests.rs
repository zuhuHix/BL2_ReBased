//! Tests of `SkeletalMeshComponent.SetSkeletalMesh` in Off (lanes IMPL-SET-SKELETAL-MESH and
//! IMPL-SKELMESH-MESH-CHANGE, NATIVE_SKELMESH_MESH_CHANGE.md's tests) on invented classes, defined by hand with the
//! notes' class and field names, and a small invented package for an unloaded and a failed handle (AGENTS.md rule 9).
//! Every value is invented. AI-assisted (Claude Code).

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
    // `Holder_1` names `Broken_0`, an export cut short (no terminator), whose load fails.
    let broken = t.p.next_export() + 1;
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("Held", broken));
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(holder, "Holder_1", payload));
    t.p.add_export_with(Export::new(thing, "Broken_0", ints(&[0])));
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
    fx_with(ParityMode::Off, true)
}

fn fx_with(mode: ParityMode, bind: bool) -> Fx {
    let mut s = ObjectSpace::new();
    let object = s.classes.define("Core.Object", None, vec![]).unwrap();
    let d = |s: &mut ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(s.names.intern(n), k);
    let fields = vec![d(&mut s, "X", PropKind::Float), d(&mut s, "Y", PropKind::Float), d(&mut s, "Z", PropKind::Float)];
    let vector = s.classes.define_struct("Core.Object.Vector", None, fields).unwrap();
    let fields = vec![d(&mut s, "Pitch", PropKind::Int), d(&mut s, "Yaw", PropKind::Int), d(&mut s, "Roll", PropKind::Int)];
    let rotator = s.classes.define_struct("Core.Object.Rotator", None, fields).unwrap();
    let decls = vec![
        d(&mut s, "Origin", PropKind::Struct(vector)),
        d(&mut s, "RotOrigin", PropKind::Struct(rotator)),
        d(&mut s, "LODInfo", PropKind::Array(Box::new(PropKind::Int))),
    ];
    let mesh = s.classes.define("Engine.SkeletalMesh", Some(object), decls).unwrap();
    let decls = vec![
        d(&mut s, "SkeletalMesh", PropKind::Object { class: mesh }),
        d(&mut s, "SpaceBases", PropKind::Array(Box::new(PropKind::Int))),
        d(&mut s, "PhysicsAssetInstance", PropKind::Object { class: object }),
        d(&mut s, "Animations", PropKind::Object { class: object }),
        d(&mut s, "bAttached", PropKind::Bool),
        d(&mut s, "ParentAnimComponent", PropKind::Object { class: object }),
        d(&mut s, "PhysicsAsset", PropKind::Object { class: object }),
        d(&mut s, "MorphSets", PropKind::Array(Box::new(PropKind::Object { class: object }))),
        d(&mut s, "LODInfo", PropKind::Array(Box::new(PropKind::Int))),
        d(&mut s, "bAnimTreeInitialised", PropKind::Bool),
        d(&mut s, "bHasValidBodies", PropKind::Bool),
        d(&mut s, "MorphTargetIndexMap", PropKind::Map { key: None, value: None }),
    ];
    let component = s.classes.define("Engine.SkeletalMeshComponent", Some(object), decls).unwrap();
    // Row D: `bHasValidBodies` declared as an int (a child redeclaring it), or not declared at all.
    let decls = vec![d(&mut s, "bHasValidBodies", PropKind::Int)];
    s.classes.define("Toy.IntValidBodiesComponent", Some(component), decls).unwrap();
    let no_bodies: Vec<PropertyDecl> = s
        .classes
        .props(component)
        .unwrap()
        .into_iter()
        .map(|(_, p)| p.clone())
        .filter(|p| s.names.text(p.name) != "bHasValidBodies")
        .collect();
    s.classes.define("Toy.NoValidBodiesComponent", Some(object), no_bodies).unwrap();
    // Row 11: `Origin` declared as another kind; `RotOrigin` not declared.
    let decls = vec![d(&mut s, "Origin", PropKind::Int)];
    s.classes.define("Toy.IntOriginMesh", Some(mesh), decls).unwrap();
    let decls = vec![d(&mut s, "Origin", PropKind::Struct(vector)), d(&mut s, "LODInfo", PropKind::Array(Box::new(PropKind::Int)))];
    s.classes.define("Toy.NoRotOriginMesh", Some(object), decls).unwrap();
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
    let mut vm = Vm::new(s, defs, Rc::new(AttributeSystem::new_empty()), mode);
    let f = FunctionDecl::native(ObjectPath::parse(PATH).unwrap(), SET_SKELETAL_MESH).with_owner(component).with_params(params);
    vm.declare_function(f).unwrap();
    if bind {
        register(&mut vm).unwrap();
    }
    Fx { vm, dir }
}

/// #518's counters: heap committed writes, generator draws and definition-store load transitions (review r1 of #599,
/// item 1), and the name table's length (no name interned). Every refusal must leave all four as they were.
fn counters(vm: &Vm) -> (u64, u64, u64, usize) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
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
        assert_eq!(counters(&self.vm), before, "no write, draw, load or name: {r:?}");
        r
    }

    /// The native on `c` where it may write: the counters are not compared.
    fn call_writing(&mut self, c: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH).unwrap();
        self.vm.call(f, Some(c), args).map(|r| r.ret)
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        *self.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    /// A resident mesh with a zero origin, its identity the toy package's export `export` (1-based), so #569's layer can
    /// name it to a source.
    fn mesh_at(&mut self, path: &str, name: &str, export: i32) -> ObjRef {
        // The toy package, opened through one of its handles (setup, before any counter is read).
        let lazy = self.unloaded();
        let (Origin::Export { package, .. } | Origin::Resource { package, .. }) = self.vm.space.heap.get(lazy).unwrap().origin else {
            panic!("an export handle")
        };
        let c = self.vm.space.classes.by_path(path).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Export { package, index: export }).unwrap()
    }

    /// A resident `Engine.SkeletalMesh` at zero origin, named by the export `Thing_0` (whose leaf `Thing` the LOD source knows as 0).
    fn mesh(&mut self, name: &str) -> ObjRef {
        self.mesh_at("Engine.SkeletalMesh", name, 2)
    }

    /// Installs #569's layer with a source that knows the LOD count of each export named in `counts`.
    fn lods(&mut self, counts: &[(&str, usize)]) {
        let counts = counts.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        crate::world::skeletal::SkeletalMeshes::install(&mut self.vm, Box::new(Lods(counts))).unwrap();
    }

    /// A component of `class` on `mesh`, with the minimal shape: every flag clear, every list empty (link 8's case).
    fn minimal(&mut self, class: &str, mesh: Option<ObjRef>) -> ObjRef {
        let c = self.obj(class, "Comp");
        self.set(c, "SkeletalMesh", Value::Object(mesh));
        c
    }

    /// The toy package's `Broken_0` handle, after one failed load (done here, before any counter is read).
    fn failed(&mut self) -> ObjRef {
        let path = ObjectPath::parse("ToyMesh.Holder_1").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        let class = self.vm.space.heap.get(holder).unwrap().class;
        let p = self.vm.space.prop(class, "Held").unwrap();
        let Value::Object(Some(o)) = self.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else {
            panic!("a held object")
        };
        assert!(self.vm.defs.ensure_loaded(&mut self.vm.space, o).is_err(), "the cut-short export fails to load");
        assert!(self.vm.defs.is_failed(&self.vm.space, o));
        o
    }
}

/// A stand-in source for #569's layer: no skeletons, and a known LOD count for the exports it names.
struct Lods(ow_core::DetMap<String, usize>);

impl crate::world::skeletal::SkeletonSource for Lods {
    fn skeleton(&self, _: &str, _: &str, _: &str) -> Result<Option<crate::world::skeletal::ReferenceSkeleton>, String> {
        Ok(None)
    }
    fn lod_count(&self, _: &str, _: &str, leaf: &str) -> Result<Option<usize>, String> {
        Ok(self.0.get(leaf).copied())
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
    // Test 1 (#635 r1): a malformed argument is reported ahead of a malformed `SkeletalMesh` declaration.
    script_error(x.call(int, vec![Value::Int(3)]), "NewMesh holds");
}

/// Test 2, rows 5 and 6: an unloaded or failed old mesh (with a resident new one), and an unloaded or failed new mesh
/// (with no old one), each `NotRead` with nothing loaded; with both unloaded, the old mesh is reported.
#[test]
fn unresident_meshes_are_not_read_old_first() {
    let mut x = fx();
    let lazy = x.unloaded();
    let failed = x.failed();
    let new = x.mesh("New");
    for bad in [lazy, failed] {
        let c = x.minimal("Engine.SkeletalMeshComponent", Some(bad));
        not_yet(x.call(c, vec![Value::Object(Some(new))]), Missing::NotRead, OLD_MESH_UNLOADED);
        let c = x.minimal("Engine.SkeletalMeshComponent", None);
        not_yet(x.call(c, vec![Value::Object(Some(bad))]), Missing::NotRead, NEW_MESH_UNLOADED);
        assert_eq!(x.get(c, "SkeletalMesh"), Value::Object(None), "unchanged");
    }
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "still unloaded");
    let c = x.minimal("Engine.SkeletalMeshComponent", Some(lazy));
    not_yet(x.call(c, vec![Value::Object(Some(failed))]), Missing::NotRead, OLD_MESH_UNLOADED);
    let class = x.vm.space.classes.by_path("Engine.SkeletalMesh").unwrap();
    let stand_in = x.vm.space.alloc_stand_in(class, "ToyStandIn", Origin::Resource { package: 0, index: -1 }).unwrap();
    let c = x.minimal("Engine.SkeletalMeshComponent", None);
    not_yet(x.call(c, vec![Value::Object(Some(stand_in))]), Missing::NotRead, NEW_MESH_STAND_IN);
}

fn vector(x: &Fx, v: [f32; 3]) -> Value {
    let id = x.vm.space.classes.struct_by_path("Core.Object.Vector").unwrap();
    Value::structure(id, v.iter().map(|f| Value::Float(*f)).collect())
}

fn rotator(x: &Fx, v: [i32; 3]) -> Value {
    let id = x.vm.space.classes.struct_by_path("Core.Object.Rotator").unwrap();
    Value::structure(id, v.iter().map(|i| Value::Int(*i)).collect())
}

/// Test 3, each row of C on its own, before any write: the component's settings, then the mesh ones (row 11).
#[test]
fn each_row_of_c_refuses_before_any_write() {
    let mut x = fx();
    x.lods(&[("Thing", 0)]);
    let thing = x.obj("Engine.SkeletalMesh", "Thing");
    let some = Value::Object(Some(thing));
    let rows: Vec<(&str, Value, Missing, &str)> = vec![
        ("SpaceBases", Value::array(vec![Value::Int(1)]), Missing::LayerAbsent(BONES_LAYER), KEEP_BONES),
        ("PhysicsAssetInstance", some.clone(), Missing::LayerAbsent(PHYSICS_INSTANCE_LAYER), PHYSICS_INSTANCE),
        ("bAttached", Value::Bool(true), Missing::NotRead, REATTACH),
        ("Animations", some.clone(), Missing::NotPorted, TREE_STATE),
        ("ParentAnimComponent", some.clone(), Missing::LayerAbsent(BONE_NAMES_LAYER), PARENT_BONE_MAP),
        ("PhysicsAsset", some.clone(), Missing::LayerAbsent(BONE_NAMES_LAYER), VALID_BODIES),
        ("MorphSets", Value::array(vec![some]), Missing::NotPorted, MORPH_MAP),
        ("MorphTargetIndexMap", Value::Int(1), Missing::NotPorted, MORPH_MAP),
    ];
    for (field, value, missing, what) in rows {
        let old = x.mesh("Old");
        let new = x.mesh("New");
        let c = x.minimal("Engine.SkeletalMeshComponent", Some(old));
        x.set(c, field, value);
        not_yet(x.call(c, vec![Value::Object(Some(new)), Value::Bool(true)]), missing, what);
        assert_eq!(x.get(c, "SkeletalMesh"), Value::Object(Some(old)), "{field}: unchanged");
    }
    // Row 11: a non-zero `Origin`, and separately a non-zero `RotOrigin`, on the new mesh and then on the old one.
    for on_old in [false, true] {
        for field in ["Origin", "RotOrigin"] {
            let old = x.mesh("Old");
            let new = x.mesh("New");
            let value = if field == "Origin" { vector(&x, [0.0, 2.5, 0.0]) } else { rotator(&x, [0, 0, 90]) };
            x.set(if on_old { old } else { new }, field, value);
            let c = x.minimal("Engine.SkeletalMeshComponent", Some(old));
            not_yet(x.call(c, vec![Value::Object(Some(new))]), Missing::LayerAbsent(TRANSFORM_LAYER), TRANSFORM);
        }
    }
}

/// Test 4, mixed cases: a non-zero origin with a LOD blocker reports the transform (row 11 before 15); attached with
/// a tree reports row 9; morph sets with a LOD blocker report row 14.
#[test]
fn mixed_cases_report_the_earlier_row() {
    let mut x = fx();
    let new = x.mesh("New");
    let thing = x.obj("Engine.SkeletalMesh", "Thing");
    x.set(new, "Origin", vector(&x, [1.0, 0.0, 0.0]));
    let c = x.minimal("Engine.SkeletalMeshComponent", None);
    not_yet(x.call(c, vec![Value::Object(Some(new))]), Missing::LayerAbsent(TRANSFORM_LAYER), TRANSFORM);
    let new = x.mesh("Zero");
    let c = x.minimal("Engine.SkeletalMeshComponent", None);
    x.set(c, "bAttached", Value::Bool(true));
    x.set(c, "Animations", Value::Object(Some(thing)));
    not_yet(x.call(c, vec![Value::Object(Some(new))]), Missing::NotRead, REATTACH);
    let c = x.minimal("Engine.SkeletalMeshComponent", None);
    x.set(c, "MorphSets", Value::array(vec![Value::Object(Some(thing))]));
    not_yet(x.call(c, vec![Value::Object(Some(new))]), Missing::NotPorted, MORPH_MAP);
}

/// Test 5: keeping with a current mesh and an **empty** buffer is not row 7; the change goes through.
#[test]
fn keep_with_an_empty_buffer_passes() {
    let mut x = fx();
    x.lods(&[("Thing", 0)]);
    let old = x.mesh("Old");
    let new = x.mesh("New");
    let c = x.minimal("Engine.SkeletalMeshComponent", Some(old));
    x.call_writing(c, vec![Value::Object(Some(new)), Value::Bool(true)]).unwrap();
    assert_eq!(x.get(c, "SkeletalMesh"), Value::Object(Some(new)));
}

/// Test 6, row 11's declarations: `Origin` declared as another kind, or `RotOrigin` undeclared, is a `VmError::Script`
/// naming it, before any write; `-0.0` members count as zero.
#[test]
fn row_11_declarations_and_negative_zero() {
    let mut x = fx();
    for (class, field) in [("Toy.IntOriginMesh", "Origin"), ("Toy.NoRotOriginMesh", "RotOrigin")] {
        let new = x.mesh_at(class, "Bad", 2);
        let c = x.minimal("Engine.SkeletalMeshComponent", None);
        script_error(x.call(c, vec![Value::Object(Some(new))]), field);
        assert_eq!(x.get(c, "SkeletalMesh"), Value::Object(None), "{class}: unchanged");
    }
    x.lods(&[("Thing", 0)]);
    let new = x.mesh("New");
    x.set(new, "Origin", vector(&x, [-0.0, -0.0, 0.0]));
    let c = x.minimal("Engine.SkeletalMeshComponent", None);
    x.call_writing(c, vec![Value::Object(Some(new))]).unwrap();
    assert_eq!(x.get(c, "SkeletalMesh"), Value::Object(Some(new)), "-0.0 is zero");
}

/// Test 7, row 15: no layer answer is `LayerAbsent`; a known count of zero with an empty `LODInfo` is nothing; a known
/// count of 2 is `LayerAbsent` both with 2 entries and with none.
#[test]
fn row_15_lod_info() {
    let mut x = fx();
    let new = x.mesh("New");
    let c = x.minimal("Engine.SkeletalMeshComponent", None);
    not_yet(x.call(c, vec![Value::Object(Some(new))]), Missing::LayerAbsent(LOD_LAYER), LOD_INFO);
    x.lods(&[("Thing", 0), ("Broken", 2)]);
    x.call_writing(c, vec![Value::Object(Some(new))]).unwrap();
    let two = x.mesh_at("Engine.SkeletalMesh", "Two", 4);
    for entries in [2, 0] {
        let c = x.minimal("Engine.SkeletalMeshComponent", None);
        x.set(c, "LODInfo", Value::array(vec![Value::Int(0); entries]));
        not_yet(x.call(c, vec![Value::Object(Some(two))]), Missing::LayerAbsent(LOD_LAYER), LOD_INFO);
    }
    // A known count of zero against a non-empty `LODInfo` is refused too.
    let c = x.minimal("Engine.SkeletalMeshComponent", None);
    x.set(c, "LODInfo", Value::array(vec![Value::Int(0)]));
    not_yet(x.call(c, vec![Value::Object(Some(new))]), Missing::LayerAbsent(LOD_LAYER), LOD_INFO);
}

/// Test 8, the minimal change (link 8's shape) with a layer stand-in answering a known LOD count of zero: the four
/// writes in order, `LODInfo` and `MorphTargetIndexMap` as they were. Four heap writes, no draw, no load, no name.
#[test]
fn the_minimal_change_writes_in_order() {
    let mut x = fx();
    x.lods(&[("Thing", 0)]);
    let new = x.mesh("New");
    let c = x.minimal("Engine.SkeletalMeshComponent", None);
    x.set(c, "bAnimTreeInitialised", Value::Bool(true));
    x.set(c, "bHasValidBodies", Value::Bool(true));
    let before = counters(&x.vm);
    x.call_writing(c, vec![Value::Object(Some(new))]).unwrap();
    let after = counters(&x.vm);
    assert_eq!(after.0 - before.0, 4, "four writes");
    assert_eq!((after.1, after.2, after.3), (before.1, before.2, before.3), "no draw, load or name");
    assert_eq!(x.get(c, "bAnimTreeInitialised"), Value::Bool(false));
    assert_eq!(x.get(c, "SkeletalMesh"), Value::Object(Some(new)));
    assert_eq!(x.get(c, "SpaceBases"), Value::array(Vec::new()));
    assert_eq!(x.get(c, "bHasValidBodies"), Value::Bool(false));
    assert_eq!(x.get(c, "LODInfo"), Value::array(Vec::new()));
    assert_eq!(x.get(c, "MorphTargetIndexMap"), Value::None);
}

/// Test 9, the clear (a set old mesh, `NewMesh` None) on a minimal component, with no layer installed at all.
#[test]
fn the_clear() {
    let mut x = fx();
    let lazy = x.unloaded();
    let c = x.minimal("Engine.SkeletalMeshComponent", Some(lazy));
    not_yet(x.call(c, vec![Value::Object(None)]), Missing::NotRead, OLD_MESH_UNLOADED);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "nothing loaded");
    let old = x.mesh("Old");
    let c = x.minimal("Engine.SkeletalMeshComponent", Some(old));
    x.set(c, "bAnimTreeInitialised", Value::Bool(true));
    x.set(c, "LODInfo", Value::array(vec![Value::Int(5)]));
    let before = counters(&x.vm);
    x.call_writing(c, vec![Value::Object(None)]).unwrap();
    assert_eq!(counters(&x.vm).0 - before.0, 4, "the four writes");
    assert_eq!(x.get(c, "SkeletalMesh"), Value::Object(None));
    assert_eq!(x.get(c, "bAnimTreeInitialised"), Value::Bool(false));
    assert_eq!(x.get(c, "SpaceBases"), Value::array(Vec::new()));
    assert_eq!(x.get(c, "LODInfo"), Value::array(vec![Value::Int(5)]), "unchanged");
    let old = x.mesh("Turned");
    x.set(old, "RotOrigin", rotator(&x, [0, 16384, 0]));
    let c = x.minimal("Engine.SkeletalMeshComponent", Some(old));
    not_yet(x.call(c, vec![Value::Object(None)]), Missing::LayerAbsent(TRANSFORM_LAYER), TRANSFORM);
}

/// Test 10, row D: a late destination that cannot be written (`bHasValidBodies` an int, or undeclared) is the
/// `VmError::Script` naming it; the earlier destinations stay unchanged and no counter moves.
#[test]
fn row_d_destinations() {
    let mut x = fx();
    x.lods(&[("Thing", 0)]);
    let new = x.mesh("New");
    for class in ["Toy.IntValidBodiesComponent", "Toy.NoValidBodiesComponent"] {
        let c = x.minimal(class, None);
        x.set(c, "bAnimTreeInitialised", Value::Bool(true));
        x.set(c, "SpaceBases", Value::array(vec![Value::Int(3)]));
        script_error(x.call(c, vec![Value::Object(Some(new))]), "bHasValidBodies");
        assert_eq!(x.get(c, "bAnimTreeInitialised"), Value::Bool(true), "{class}");
        assert_eq!(x.get(c, "SkeletalMesh"), Value::Object(None), "{class}");
        assert_eq!(x.get(c, "SpaceBases"), Value::array(vec![Value::Int(3)]), "{class}");
    }
}

/// Test 11: under Reference the world pack binds nothing (the stub policy answers); in Off a key bound first keeps its
/// binding (keep-existing).
#[test]
fn reference_is_unchanged_and_the_binding_keeps_an_existing_one() {
    use ow_vm::api::NativePack;
    let mut r = fx_with(ParityMode::Reference, false);
    crate::world::Pack.register(&mut r.vm).unwrap();
    assert!(!r.vm.has_native(SET_SKELETAL_MESH), "Reference binds nothing");
    let mut x = fx_with(ParityMode::Off, false);
    x.vm.register_native(SET_SKELETAL_MESH, Rc::new(|_c: &mut ow_vm::api::NativeCall<'_>| Ok(Value::Int(7)))).unwrap();
    crate::world::Pack.register(&mut x.vm).unwrap();
    let c = x.minimal("Engine.SkeletalMeshComponent", None);
    let new = x.mesh("New");
    assert_eq!(x.call_writing(c, vec![Value::Object(Some(new))]).unwrap(), Value::Int(7), "the earlier binding stays");
    let mut y = fx_with(ParityMode::Off, false);
    crate::world::Pack.register(&mut y.vm).unwrap();
    assert!(y.vm.has_native(SET_SKELETAL_MESH), "Off binds it");
}
