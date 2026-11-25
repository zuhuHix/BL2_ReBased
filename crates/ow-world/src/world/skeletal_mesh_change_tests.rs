//! Tests of `SkeletalMeshComponent.SetSkeletalMesh`'s mesh change in Off (lane IMPL-SKELMESH-MESH-CHANGE,
//! NATIVE_SKELMESH_MESH_CHANGE.md section 4 and its test list) on invented packages (AGENTS.md rule 9): `Core.upk`
//! declares `Object` with `Vector` and `Rotator`; `Engine.upk` declares `SkeletalMesh` (with `Origin` and `RotOrigin`,
//! and same-named lookalike structs of its own for FIX-645-PREFLIGHT), `SkeletalMeshComponent` with the fields the rule reads and
//! writes, and invented variants for the declaration cases; `GD_ToyMesh.upk` holds meshes, a holder whose fields name
//! unloaded meshes, and a mesh whose payload is cut short (its load fails). The LOD count comes from an in-memory
//! [`SkeletonSource`](crate::world::skeletal::SkeletonSource). Every value is invented. AI-assisted (Claude Code).

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
use ow_package::testkit::ints;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::world::skeletal::{ReferenceSkeleton, SkeletalMeshes, SkeletonSource};

static NEXT: AtomicU32 = AtomicU32::new(0);
const PATH: &str = "Engine.SkeletalMeshComponent.SetSkeletalMesh";
const COMPONENT: &str = "Engine.SkeletalMeshComponent";
const LOD_INFO: &str = "Engine.SkeletalMesh.SkelMeshComponentLODInfo";

/// The component's fields, except `bHasValidBodies` (declared per class below).
/// `lod` is `LODInfo`'s element (kind, type reference).
fn component_fields(t: &mut ScriptToy, c: i32, object: i32, mesh: i32, lod: (&str, i32)) {
    t.prop("Object", c, "SkeletalMesh", 0, mesh);
    toy_array_of(t, c, "SpaceBases", "Int", 0);
    for (n, k) in [("PhysicsAssetInstance", object), ("Animations", object), ("ParentAnimComponent", object), ("PhysicsAsset", object)] {
        t.prop("Object", c, n, 0, k);
    }
    t.prop("Bool", c, "bAttached", 0, 0);
    t.prop("Bool", c, "bAnimTreeInitialised", 0, 0);
    toy_array_of(t, c, "MorphSets", "Object", object);
    t.prop_tail("Map", c, "MorphTargetIndexMap", 0, &ints(&[0, 0]));
    toy_array_of(t, c, "LODInfo", lod.0, lod.1);
}

fn packages() -> Vec<(&'static str, Vec<u8>)> {
    let core = {
        let mut t = ScriptToy::with_imports(&["Class", "ScriptStruct", "IntProperty", "FloatProperty"]);
        let object = t.class("Object", 0);
        let vector = toy_struct(&mut t, object, "Vector", None);
        for n in ["X", "Y", "Z"] {
            t.prop("Float", vector, n, 0, 0);
        }
        let rotator = toy_struct(&mut t, object, "Rotator", None);
        for n in ["Pitch", "Yaw", "Roll"] {
            t.prop("Int", rotator, n, 0, 0);
        }
        t.build()
    };
    let engine = {
        let mut t = ScriptToy::with_imports(&[
            "Class",
            "ScriptStruct",
            "ObjectProperty",
            "ArrayProperty",
            "IntProperty",
            "FloatProperty",
            "BoolProperty",
            "StructProperty",
            "MapProperty",
            "ByteProperty",
        ]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let vector = t.p.add_import("Core", "ScriptStruct", object, "Vector");
        let rotator = t.p.add_import("Core", "ScriptStruct", object, "Rotator");
        let mesh = t.class("SkeletalMesh", object);
        // FIX-645-PREFLIGHT: structs with the members and names of Vector and Rotator that are not Core's.
        let look_vector = toy_struct(&mut t, mesh, "Vector", None);
        for n in ["X", "Y", "Z"] {
            t.prop("Float", look_vector, n, 0, 0);
        }
        let look_rotator = toy_struct(&mut t, mesh, "Rotator", None);
        for n in ["Pitch", "Yaw", "Roll"] {
            t.prop("Int", look_rotator, n, 0, 0);
        }
        t.prop("Struct", mesh, "Origin", 0, vector);
        t.prop("Struct", mesh, "RotOrigin", 0, rotator);
        let look_origin = t.class("ToyLookOriginMesh", object);
        t.prop("Struct", look_origin, "Origin", 0, look_vector);
        t.prop("Struct", look_origin, "RotOrigin", 0, rotator);
        let look_rot = t.class("ToyLookRotMesh", object);
        t.prop("Struct", look_rot, "Origin", 0, vector);
        t.prop("Struct", look_rot, "RotOrigin", 0, look_rotator);
        // Row 11's declaration cases: Origin an int; no RotOrigin.
        let int_origin = t.class("ToyIntOriginMesh", object);
        t.prop("Int", int_origin, "Origin", 0, 0);
        t.prop("Struct", int_origin, "RotOrigin", 0, rotator);
        let no_rot = t.class("ToyNoRotMesh", object);
        t.prop("Struct", no_rot, "Origin", 0, vector);
        // IMPL-ROW15: LODInfo's entry struct (its InstanceWeightIdx default -1 is set in `fx_with`), and one of the
        // same shape whose default the fixture leaves at 0.
        // The five members of the installed struct, by the runner's read (declared here in another order, so the
        // check goes by name); `ToyBadLODInfo` declares `InstanceWeightIdx` a byte, and `ToyRenamedLODInfo` has the
        // same kinds under other names (review r1 of #678, item 2).
        let lod_struct = |t: &mut ScriptToy, name: &str, usage: &str, idx: (&str, &str)| {
            let s = toy_struct(t, mesh, name, None);
            t.prop(idx.1, s, idx.0, 0, 0);
            toy_array_of(t, s, "HiddenMaterials", "Bool", 0);
            t.prop("Bool", s, "bNeedsInstanceWeightUpdate", 0, 0);
            t.prop("Byte", s, usage, 0, 0);
            t.prop("Bool", s, "bAlwaysUseInstanceWeights", 0, 0);
            s
        };
        let lod = lod_struct(&mut t, "SkelMeshComponentLODInfo", "InstanceWeightUsage", ("InstanceWeightIdx", "Int"));
        let bad_lod = lod_struct(&mut t, "ToyBadLODInfo", "InstanceWeightUsage", ("InstanceWeightIdx", "Byte"));
        let renamed_lod = lod_struct(&mut t, "ToyRenamedLODInfo", "ToyUsage", ("ToyIdx", "Int"));
        let good = ("Struct", lod);
        let component = t.class("SkeletalMeshComponent", object);
        component_fields(&mut t, component, object, object, good);
        t.prop("Bool", component, "bHasValidBodies", 0, 0);
        // Row D's cases: bHasValidBodies an int; no bHasValidBodies.
        let int_bodies = t.class("ToyIntBodiesComponent", object);
        component_fields(&mut t, int_bodies, object, object, good);
        t.prop("Int", int_bodies, "bHasValidBodies", 0, 0);
        let no_bodies = t.class("ToyNoBodiesComponent", object);
        component_fields(&mut t, no_bodies, object, object, good);
        // FIX-645-PREFLIGHT: bHasValidBodies declared first, so it takes the last slot (the toy's slots run in reverse
        // declaration order) and a cut there leaves every slot the rows read.
        let late_bodies = t.class("ToyLateBodiesComponent", object);
        t.prop("Bool", late_bodies, "bHasValidBodies", 0, 0);
        component_fields(&mut t, late_bodies, object, object, good);
        // IMPL-ROW15: an entry struct whose default is not the note's, and LODInfo declared as ints.
        let bad_default = t.class("ToyBadLodComponent", object);
        component_fields(&mut t, bad_default, object, object, ("Struct", bad_lod));
        t.prop("Bool", bad_default, "bHasValidBodies", 0, 0);
        let renamed = t.class("ToyRenamedLodComponent", object);
        component_fields(&mut t, renamed, object, object, ("Struct", renamed_lod));
        t.prop("Bool", renamed, "bHasValidBodies", 0, 0);
        let int_lod = t.class("ToyIntLodComponent", object);
        component_fields(&mut t, int_lod, object, object, ("Int", 0));
        t.prop("Bool", int_lod, "bHasValidBodies", 0, 0);
        let holder = t.class("ToyHolder", object);
        for n in ["Lazy", "Lazy2", "Broken"] {
            t.prop("Object", holder, n, 0, object);
        }
        t.build()
    };
    let gd = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let mesh = t.p.add_import("Core", "Class", engine, "SkeletalMesh");
        let int_origin = t.p.add_import("Core", "Class", engine, "ToyIntOriginMesh");
        let no_rot = t.p.add_import("Core", "Class", engine, "ToyNoRotMesh");
        let look_origin = t.p.add_import("Core", "Class", engine, "ToyLookOriginMesh");
        let look_rot = t.p.add_import("Core", "Class", engine, "ToyLookRotMesh");
        let holder = t.p.add_import("Core", "Class", engine, "ToyHolder");
        let mut empty = ints(&[0]);
        empty.extend_from_slice(&t.none);
        for n in ["Mesh_A", "Mesh_B", "Mesh_Zero"] {
            t.p.add_export(mesh, n, empty.clone());
        }
        t.p.add_export(int_origin, "Mesh_IntOrigin", empty.clone());
        t.p.add_export(no_rot, "Mesh_NoRot", empty.clone());
        t.p.add_export(look_origin, "Mesh_LookOrigin", empty.clone());
        t.p.add_export(look_rot, "Mesh_LookRot", empty.clone());
        let first = t.p.next_export();
        t.p.add_export(mesh, "Mesh_Lazy", empty.clone());
        t.p.add_export(mesh, "Mesh_Lazy2", empty.clone());
        t.p.add_export(mesh, "Mesh_Broken", vec![1, 2]);
        let mut h = ints(&[0]);
        for (i, n) in ["Lazy", "Lazy2", "Broken"].iter().enumerate() {
            h.extend(t.p.object_tag(n, first + i32::try_from(i).unwrap()));
        }
        h.extend_from_slice(&t.none);
        t.p.add_export(holder, "Holder_0", h);
        t.build()
    };
    vec![("Core.upk", core), ("Engine.upk", engine), ("GD_ToyMesh.upk", gd)]
}

/// The LOD counts by mesh leaf (any other leaf has no answer), and the trailing counts by leaf: a leaf listed in the
/// second list answers it (`None` for no list); any other leaf with a count answers that many zeros.
struct Lods(Vec<(&'static str, usize)>, Vec<(&'static str, Option<Vec<usize>>)>);

impl SkeletonSource for Lods {
    fn skeleton(&self, _: &str, _: &str, _: &str) -> Result<Option<ReferenceSkeleton>, String> {
        Ok(None)
    }
    fn lod_count(&self, _: &str, _: &str, leaf: &str) -> Result<Option<usize>, String> {
        Ok(self.0.iter().find(|(l, _)| *l == leaf).map(|(_, n)| *n))
    }
    fn lod_trailing_in(&self, package: &ow_package::package::Package, index: i32) -> Result<Option<Vec<usize>>, String> {
        let leaf = package.object(index).map_err(|e| e.to_string())?.name.clone();
        if let Some((_, t)) = self.1.iter().find(|(l, _)| *l == leaf) {
            return Ok(t.clone());
        }
        Ok(self.0.iter().find(|(l, _)| *l == leaf).map(|(_, n)| vec![0; *n]))
    }
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

/// The fixture, with the LOD layer answering `lods` (or not installed with `None`), zero trailing counts.
fn fx_with(lods: Option<Vec<(&'static str, usize)>>) -> Fx {
    fx_trailing(lods, Vec::new())
}

/// [`fx_with`], with the trailing counts of `trailing`'s leaves.
fn fx_trailing(lods: Option<Vec<(&'static str, usize)>>, trailing: Vec<(&'static str, Option<Vec<usize>>)>) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-world-smmc-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let files = packages();
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    let defs = DefStore::from_synthetic(&dir, &refs, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    for c in [
        COMPONENT,
        "Engine.SkeletalMesh",
        "Engine.ToyIntBodiesComponent",
        "Engine.ToyNoBodiesComponent",
        "Engine.ToyLateBodiesComponent",
        "Engine.ToyBadLodComponent",
        "Engine.ToyIntLodComponent",
        "Engine.ToyRenamedLodComponent",
    ] {
        vm.find_class(c).unwrap();
    }
    let component = vm.space.classes.by_path(COMPONENT).unwrap();
    let mesh = vm.space.classes.by_path("Engine.SkeletalMesh").unwrap();
    let d = |vm: &mut Vm, n: &str, k: PropKind| PropertyDecl::new(vm.space.names.intern(n), k);
    let params = vec![
        d(&mut vm, "NewMesh", PropKind::Object { class: mesh }).with_flags(PropertyDecl::PARAM),
        d(&mut vm, "bKeepSpaceBases", PropKind::Bool).with_flags(PropertyDecl::PARAM | PropertyDecl::OPTIONAL),
    ];
    let f = FunctionDecl::native(ObjectPath::parse(PATH).unwrap(), SET_SKELETAL_MESH).with_owner(component).with_params(params);
    vm.declare_function(f).unwrap();
    register(&mut vm).unwrap();
    if let Some(l) = lods {
        SkeletalMeshes::install(&mut vm, Box::new(Lods(l, trailing))).unwrap();
    }
    let mut x = Fx { vm, dir };
    for n in ["Mesh_A", "Mesh_B", "Mesh_Zero", "Mesh_IntOrigin", "Mesh_NoRot", "Mesh_LookOrigin", "Mesh_LookRot", "Holder_0"] {
        x.export(n);
    }
    x
}

/// The default: every resident mesh of the package answers a known LOD count of zero.
fn fx() -> Fx {
    fx_with(Some(vec![
        ("Mesh_A", 0),
        ("Mesh_B", 0),
        ("Mesh_Zero", 0),
        ("Mesh_IntOrigin", 0),
        ("Mesh_NoRot", 0),
        ("Mesh_LookOrigin", 0),
        ("Mesh_LookRot", 0),
    ]))
}

/// #518's counters, and the name table's length.
fn marks(vm: &Vm) -> (u64, u64, u64, usize) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
}

impl Fx {
    fn export(&mut self, leaf: &str) -> ObjRef {
        let path = ObjectPath::parse(&format!("GD_ToyMesh.{leaf}")).unwrap();
        self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap()
    }

    /// The holder's field `n`: an unloaded handle (the `Broken` one fails when loaded).
    fn held(&mut self, n: &str) -> ObjRef {
        let holder = self.export("Holder_0");
        match self.get(holder, n) {
            Value::Object(Some(o)) => o,
            other => panic!("{n}: {other:?}"),
        }
    }

    /// A component of `class`, holding `mesh`, every other field at its default.
    fn component_of(&mut self, class: &str, mesh: Option<ObjRef>) -> ObjRef {
        let c = self.vm.space.classes.by_path(class).unwrap();
        let o = self.vm.space.alloc(c, "Comp", None, Origin::Runtime).unwrap();
        self.put(o, "SkeletalMesh", Value::Object(mesh));
        o
    }

    fn component(&mut self, mesh: Option<ObjRef>) -> ObjRef {
        self.component_of(COMPONENT, mesh)
    }

    fn get(&self, o: ObjRef, n: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, n).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    /// `o.n := v`, past the heap's write checks (a fixture).
    fn put(&mut self, o: ObjRef, n: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, n).unwrap();
        *self.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }

    /// Member `m` of the struct field `n` of `o` set to `v`.
    fn put_member(&mut self, o: ObjRef, n: &str, m: &str, v: Value) {
        let mut s = self.get(o, n);
        let ty = s.as_struct().unwrap().ty;
        let id = self.vm.space.names.find(m).unwrap();
        let (i, _) = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap();
        s.struct_mut().unwrap().fields[usize::try_from(i).unwrap()] = v;
        self.put(o, n, s);
    }

    fn call(&mut self, c: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH).unwrap();
        self.vm.call(f, Some(c), args).map(|r| r.ret)
    }

    /// One refused call: the refusal, with #518's counters, the name table's length and every slot of the component
    /// unchanged.
    fn refused(&mut self, c: ObjRef, args: Vec<Value>) -> VmError {
        let (before, slots) = (marks(&self.vm), self.vm.space.heap.get(c).unwrap().slots.clone());
        let e = self.call(c, args).unwrap_err();
        assert_eq!(marks(&self.vm), before, "no write, draw, load or name: {e}");
        assert_eq!(self.vm.space.heap.get(c).unwrap().slots, slots, "the component unchanged: {e}");
        e
    }
}

fn some(o: ObjRef) -> Value {
    Value::Object(Some(o))
}

/// The refusal's kind and text.
fn kind(e: VmError) -> (Missing, String) {
    match e {
        VmError::NotYet(n) => {
            assert_eq!(n.record, CHANGE_RECORD, "{n}");
            (n.missing, n.what.to_string())
        }
        other => panic!("expected a NotYet, got {other:?}"),
    }
}

fn script_text(e: VmError) -> String {
    match e {
        VmError::Script { message, .. } => message,
        other => panic!("expected a script error, got {other:?}"),
    }
}

/// Test 1's addition: a malformed argument is reported ahead of a malformed `SkeletalMesh` declaration (row 2 before
/// row 3); the receiver's residency before both.
#[test]
fn a_malformed_argument_comes_before_the_mesh_declaration() {
    let mut x = fx();
    let c = x.component_of("Engine.ToyIntBodiesComponent", None);
    x.put(c, "SkeletalMesh", Value::Int(3));
    assert!(script_text(x.refused(c, vec![Value::Int(1)])).contains("NewMesh holds"));
}

/// Test 2: rows 5 and 6, an unloaded or failed old mesh (a resident new one), an unloaded or failed new mesh (no old
/// one), and with both unloaded, the old one's refusal. Nothing is loaded.
#[test]
fn unloaded_and_failed_meshes_are_not_read() {
    let mut x = fx();
    let (lazy, lazy2, broken, a) = (x.held("Lazy"), x.held("Lazy2"), x.held("Broken"), x.export("Mesh_A"));
    assert!(x.vm.defs.ensure_loaded(&mut x.vm.space, broken).is_err(), "the cut payload fails to load");
    for old in [lazy, broken] {
        let c = x.component(Some(old));
        assert_eq!(kind(x.refused(c, vec![some(a)])), (Missing::NotRead, OLD_UNLOADED.to_string()));
    }
    for new in [lazy, broken] {
        let c = x.component(None);
        assert_eq!(kind(x.refused(c, vec![some(new)])), (Missing::NotRead, NEW_UNLOADED.to_string()));
    }
    let c = x.component(Some(lazy));
    assert_eq!(kind(x.refused(c, vec![some(lazy2)])).1, OLD_UNLOADED, "both unloaded: the old mesh's");
    for o in [lazy, lazy2] {
        assert!(x.vm.defs.is_unloaded(&x.vm.space, o), "never loaded");
    }
}

/// One fixture change: the component and a second mesh.
type Setter = Box<dyn Fn(&mut Fx, ObjRef, ObjRef)>;

/// Test 3: each row of C on its own, before any write.
#[test]
fn each_unported_path_refuses_before_any_write() {
    let cases: Vec<(&str, Setter, bool, Missing, &str)> = vec![
        ("7 keep", Box::new(|x, c, _| x.put(c, "SpaceBases", Value::array(vec![Value::Int(1)]))), true, Missing::LayerAbsent(BONES_LAYER), KEEP),
        ("8 physics instance", Box::new(|x, c, o| x.put(c, "PhysicsAssetInstance", some(o))), false, Missing::LayerAbsent(PHYSICS_INSTANCE_LAYER), PHYSICS_INSTANCE),
        ("9 attached", Box::new(|x, c, _| x.put(c, "bAttached", Value::Bool(true))), false, Missing::NotRead, ATTACHED),
        ("10 tree", Box::new(|x, c, o| x.put(c, "Animations", some(o))), false, Missing::NotPorted, TREE),
        ("12 parent", Box::new(|x, c, o| x.put(c, "ParentAnimComponent", some(o))), false, Missing::LayerAbsent(BONE_NAMES_LAYER), PARENT_BONES),
        ("13 physics asset", Box::new(|x, c, o| x.put(c, "PhysicsAsset", some(o))), false, Missing::LayerAbsent(BONE_NAMES_LAYER), VALID_BODIES),
        ("14 morph sets", Box::new(|x, c, o| x.put(c, "MorphSets", Value::array(vec![some(o)]))), false, Missing::NotPorted, MORPH),
    ];
    for (row, set, keep, missing, text) in cases {
        let mut x = fx();
        let (a, b) = (x.export("Mesh_A"), x.export("Mesh_B"));
        let c = x.component(Some(a));
        set(&mut x, c, b);
        let got = kind(x.refused(c, vec![some(b), Value::Bool(keep)]));
        assert_eq!(got, (missing, text.to_string()), "row {row}");
    }
    // Row 11: a non-zero Origin, then a non-zero RotOrigin, on the new mesh and then on the old.
    for (field, member, v) in [("Origin", "X", Value::Float(1.0)), ("RotOrigin", "Yaw", Value::Int(16384))] {
        for on_new in [true, false] {
            let mut x = fx();
            let (a, b) = (x.export("Mesh_A"), x.export("Mesh_B"));
            x.put_member(if on_new { b } else { a }, field, member, v.clone());
            let c = x.component(Some(a));
            assert_eq!(kind(x.refused(c, vec![some(b)])), (Missing::LayerAbsent(TRANSFORM_LAYER), TRANSFORM.to_string()), "{field} new {on_new}");
        }
    }
    // Row 14: a MorphTargetIndexMap that is not None.
    let mut x = fx();
    let (a, b) = (x.export("Mesh_A"), x.export("Mesh_B"));
    let c = x.component(Some(a));
    x.put(c, "MorphTargetIndexMap", Value::Int(1));
    assert_eq!(kind(x.refused(c, vec![some(b)])), (Missing::NotPorted, MORPH.to_string()));
}

/// Test 4: mixed cases: a non-zero origin with a LOD blocker reports the transform (row 11 before 15); attached with
/// a tree reports row 9; morph sets with a LOD blocker report row 14.
#[test]
fn mixed_cases_report_the_earlier_row() {
    let mut x = fx_with(None);
    let (a, b) = (x.export("Mesh_A"), x.export("Mesh_B"));
    x.put_member(b, "Origin", "Z", Value::Float(2.0));
    let c = x.component(Some(a));
    assert_eq!(kind(x.refused(c, vec![some(b)])).0, Missing::LayerAbsent(TRANSFORM_LAYER));
    let c = x.component(Some(a));
    x.put(c, "bAttached", Value::Bool(true));
    x.put(c, "Animations", some(a));
    assert_eq!(kind(x.refused(c, vec![some(b)])).1, ATTACHED);
    let zero = x.export("Mesh_Zero");
    let c = x.component(Some(a));
    x.put(c, "MorphSets", Value::array(vec![some(a)]));
    assert_eq!(kind(x.refused(c, vec![some(zero)])).1, MORPH);
}

/// Tests 5 and 8: keep with a current mesh and an empty buffer is no refusal; the minimal change (no old mesh) makes
/// the four writes, and `LODInfo` and `MorphTargetIndexMap` stay as they were.
#[test]
fn the_minimal_change_makes_the_four_writes() {
    let mut x = fx();
    let (a, b) = (x.export("Mesh_A"), x.export("Mesh_B"));
    for (old, keep) in [(None, false), (Some(a), true)] {
        let c = x.component(old);
        x.put(c, "bAnimTreeInitialised", Value::Bool(true));
        x.put(c, "bHasValidBodies", Value::Bool(true));
        let (lod, morph) = (x.get(c, "LODInfo"), x.get(c, "MorphTargetIndexMap"));
        let (w, d, l, n) = marks(&x.vm);
        x.call(c, vec![some(b), Value::Bool(keep)]).unwrap();
        assert_eq!(marks(&x.vm), (w + 4, d, l, n), "four writes, no draw, no load, no name");
        assert_eq!(x.get(c, "bAnimTreeInitialised"), Value::Bool(false));
        assert_eq!(x.get(c, "SkeletalMesh"), some(b));
        assert_eq!(x.get(c, "SpaceBases"), Value::array(Vec::new()));
        assert_eq!(x.get(c, "bHasValidBodies"), Value::Bool(false));
        assert_eq!((x.get(c, "LODInfo"), x.get(c, "MorphTargetIndexMap")), (lod, morph), "not written");
    }
}

/// Test 6: row 11's declarations: `Origin` of another kind, or no `RotOrigin`, is a script error naming it, before
/// any write; `-0.0` members count as zero.
#[test]
fn origin_declarations_and_negative_zero() {
    let mut x = fx();
    let a = x.export("Mesh_A");
    for (leaf, field) in [("Mesh_IntOrigin", "Origin"), ("Mesh_NoRot", "RotOrigin")] {
        let bad = x.export(leaf);
        let c = x.component(Some(a));
        let e = script_text(x.refused(c, vec![some(bad)]));
        assert!(e.contains(field), "{leaf}: {e}");
    }
    let b = x.export("Mesh_B");
    x.put_member(b, "Origin", "Y", Value::Float(-0.0));
    let c = x.component(Some(a));
    x.call(c, vec![some(b)]).unwrap();
    assert_eq!(x.get(c, "SkeletalMesh"), some(b), "-0.0 is zero");
}

/// IMPL-ROW15: `n` entries of `LODInfo`'s struct, each its script default with the usage byte set to `usage` (an
/// invented value that tells a kept entry from a refilled one).
fn entries(x: &Fx, n: usize, usage: u8) -> Value {
    let ty = x.vm.space.classes.struct_by_path(LOD_INFO).unwrap();
    let mut e = x.vm.space.classes.struct_defaults(ty).unwrap();
    let id = x.vm.space.names.find("InstanceWeightUsage").unwrap();
    let (i, _) = x.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap();
    e.struct_mut().unwrap().fields[usize::try_from(i).unwrap()] = Value::Byte(usage);
    Value::array(vec![e; n])
}

/// Test 7, row 15 on the hub's rulings (IMPL-ROW15, ruling 2 as corrected): with all trailing counts zero, a
/// `LODInfo` length equal to the LOD count writes nothing more (the four writes, in order); a length that differs needs
/// a refill, whose natively constructed entry's usage byte is unread: `NotRead` (`LOD_USAGE`), write-free. N=1 against
/// an empty `LODInfo` is link 8's shape.
#[test]
fn the_lod_row_refill_refuses_on_the_unread_usage_byte() {
    let mut x = fx_with(Some(vec![("Mesh_A", 1), ("Mesh_B", 2), ("Mesh_Zero", 0)]));
    let (a, b, zero) = (x.export("Mesh_A"), x.export("Mesh_B"), x.export("Mesh_Zero"));
    for (mesh, before) in [(b, entries(&x, 2, 7)), (zero, Value::array(Vec::new()))] {
        let c = x.component(None);
        x.put(c, "LODInfo", before.clone());
        let plan = answer(&x.vm, c, Some(mesh), false).unwrap().unwrap();
        let names: Vec<String> = plan
            .writes
            .iter()
            .map(|(p, _)| x.vm.space.names.text(x.vm.space.classes.decl(*p).unwrap().name).to_string())
            .collect();
        assert_eq!(names, ["bAnimTreeInitialised", "SkeletalMesh", "SpaceBases", "bHasValidBodies"], "in order");
        let (w, d, l, n) = marks(&x.vm);
        x.call(c, vec![some(mesh)]).unwrap();
        assert_eq!(marks(&x.vm), (w + 4, d, l, n), "four writes, no draw, no load, no name");
        assert_eq!((x.get(c, "LODInfo"), x.get(c, "SkeletalMesh")), (before, some(mesh)), "LODInfo kept");
    }
    for (mesh, before) in [(a, Value::array(Vec::new())), (b, entries(&x, 1, 7)), (zero, entries(&x, 1, 7))] {
        let c = x.component(None);
        x.put(c, "LODInfo", before);
        assert_eq!(kind(x.refused(c, vec![some(mesh)])), (Missing::NotRead, LOD_USAGE.to_string()));
    }
}

/// Test 7's refusals, unchanged and write-free: no layer, a count with no trailing list, a non-zero trailing count,
/// and a trailing list of another length are row 15's `LayerAbsent`.
#[test]
fn the_lod_row_refuses_without_zero_trailing_counts() {
    let mut y = fx_with(None);
    let a = y.export("Mesh_A");
    let c = y.component(None);
    assert_eq!(kind(y.refused(c, vec![some(a)])), (Missing::LayerAbsent(LOD_LAYER), LOD.to_string()), "no layer");
    let mut x = fx_trailing(
        Some(vec![("Mesh_A", 1), ("Mesh_B", 2), ("Mesh_Zero", 1)]),
        vec![("Mesh_A", None), ("Mesh_B", Some(vec![0, 1])), ("Mesh_Zero", Some(vec![0, 0]))],
    );
    for leaf in ["Mesh_A", "Mesh_B", "Mesh_Zero"] {
        let mesh = x.export(leaf);
        let c = x.component(None);
        assert_eq!(kind(x.refused(c, vec![some(mesh)])), (Missing::LayerAbsent(LOD_LAYER), LOD.to_string()), "{leaf}");
    }
    // A mesh the source has no count for.
    let mut z = fx_with(Some(vec![("Mesh_A", 1)]));
    let b = z.export("Mesh_B");
    let c = z.component(None);
    assert_eq!(kind(z.refused(c, vec![some(b)])).0, Missing::LayerAbsent(LOD_LAYER), "no count");
}

/// Review r1 of #678, item 1 (kept after r2): a source that answers "not known" to both in-package questions (and
/// nothing by name) gives row 15's `LayerAbsent`, write-free. `ow-cli`'s source does not answer so for a non-zero
/// trailing count: after review r2 the reader's error surfaces, write-free (decision 2026-11-25-13).
#[test]
fn a_cli_like_source_s_unsupported_trailing_count_is_layer_absent() {
    struct CliLike;
    impl SkeletonSource for CliLike {
        fn skeleton(&self, _: &str, _: &str, _: &str) -> Result<Option<ReferenceSkeleton>, String> {
            Ok(None)
        }
        fn lod_count_in(&self, _: &ow_package::package::Package, _: i32) -> Result<Option<usize>, String> {
            Ok(None)
        }
        fn lod_trailing_in(&self, _: &ow_package::package::Package, _: i32) -> Result<Option<Vec<usize>>, String> {
            Ok(None)
        }
    }
    let mut x = fx_with(None);
    SkeletalMeshes::install(&mut x.vm, Box::new(CliLike)).unwrap();
    let b = x.export("Mesh_B");
    let c = x.component(None);
    assert_eq!(kind(x.refused(c, vec![some(b)])), (Missing::LayerAbsent(LOD_LAYER), LOD.to_string()));
}

/// D's `LODInfo` checks (IMPL-ROW15): an entry struct that does not declare the five members by name and kind is
/// `NotRead` (`LOD_ENTRY`) when a refill is needed, before the usage byte's refusal, and not asked when none is;
/// `LODInfo` declared as ints is a script error naming it with a new mesh, and not read on a clear. Each refusal
/// writes nothing.
#[test]
fn the_lod_info_declaration_and_default_are_checked_before_any_write() {
    let mut x = fx_with(Some(vec![("Mesh_A", 1), ("Mesh_Zero", 0)]));
    let (a, zero) = (x.export("Mesh_A"), x.export("Mesh_Zero"));
    let c = x.component_of("Engine.ToyBadLodComponent", None);
    assert_eq!(kind(x.refused(c, vec![some(a)])), (Missing::NotRead, LOD_ENTRY.to_string()), "a byte index");
    let c = x.component_of("Engine.ToyBadLodComponent", None);
    x.call(c, vec![some(zero)]).unwrap();
    assert_eq!(x.get(c, "SkeletalMesh"), some(zero), "no refill needed: the entry is not asked");
    // Review r1 of #678, item 2: the same kinds and values under other names.
    let c = x.component_of("Engine.ToyRenamedLodComponent", None);
    assert_eq!(kind(x.refused(c, vec![some(a)])), (Missing::NotRead, LOD_ENTRY.to_string()), "renamed members");
    let c = x.component_of("Engine.ToyIntLodComponent", None);
    assert!(script_text(x.refused(c, vec![some(zero)])).contains("LODInfo"));
    let c = x.component_of("Engine.ToyIntLodComponent", Some(zero));
    let (w, ..) = marks(&x.vm);
    x.call(c, vec![Value::Object(None)]).unwrap();
    assert_eq!(marks(&x.vm).0, w + 4, "a clear does not read LODInfo");
}

/// Test 9: the clear (a set mesh to None): an unloaded old mesh is row 5; a resident old mesh at zero origin gives the
/// four writes with `SkeletalMesh` None and `LODInfo` unchanged, with no layer answer at all; a non-zero origin is the
/// transform's LayerAbsent.
#[test]
fn the_clear() {
    let mut x = fx_with(None);
    let (lazy, a) = (x.held("Lazy"), x.export("Mesh_A"));
    let c = x.component(Some(lazy));
    assert_eq!(kind(x.refused(c, vec![Value::Object(None)])).1, OLD_UNLOADED);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy));
    let c = x.component(Some(a));
    let kept = entries(&x, 1, 5);
    x.put(c, "LODInfo", kept.clone());
    let (w, ..) = marks(&x.vm);
    x.call(c, vec![Value::Object(None)]).unwrap();
    assert_eq!(marks(&x.vm).0, w + 4);
    assert_eq!(x.get(c, "SkeletalMesh"), Value::Object(None));
    assert_eq!(x.get(c, "LODInfo"), kept, "a clear leaves LODInfo");
    x.put_member(a, "RotOrigin", "Roll", Value::Int(1));
    let c = x.component(Some(a));
    assert_eq!(kind(x.refused(c, vec![Value::Object(None)])).0, Missing::LayerAbsent(TRANSFORM_LAYER));
}

/// Test 10: row D: a late destination that cannot take its write (`bHasValidBodies` an int, or not declared) is a
/// script error naming it, with nothing written.
#[test]
fn an_unwritable_destination_writes_nothing() {
    let mut x = fx();
    let b = x.export("Mesh_B");
    for class in ["Engine.ToyIntBodiesComponent", "Engine.ToyNoBodiesComponent"] {
        let c = x.component_of(class, None);
        let e = script_text(x.refused(c, vec![some(b)]));
        assert!(e.contains("bHasValidBodies"), "{class}: {e}");
        assert_eq!(x.get(c, "SkeletalMesh"), Value::Object(None), "{class}: SkeletalMesh unchanged");
    }
}

/// FIX-645-PREFLIGHT, finding 1: a receiver missing the slot of any field the rule reads or writes (its slots cut at
/// that field) is row 1's `NotRead`, with nothing written and no "stopped after partial writes" prefix. On
/// `ToyLateBodiesComponent` only `bHasValidBodies`'s slot is missing; before the fix that call committed the first
/// three writes and then failed with an untyped heap error.
#[test]
fn a_receiver_missing_a_slot_writes_nothing() {
    let fields = [
        "SkeletalMesh",
        "SpaceBases",
        "PhysicsAssetInstance",
        "Animations",
        "ParentAnimComponent",
        "PhysicsAsset",
        "bAttached",
        "bAnimTreeInitialised",
        "MorphSets",
        "MorphTargetIndexMap",
        "LODInfo",
        "bHasValidBodies",
    ];
    let mut x = fx();
    let (a, b) = (x.export("Mesh_A"), x.export("Mesh_B"));
    let cases = fields.iter().map(|f| (COMPONENT, *f)).chain([("Engine.ToyLateBodiesComponent", "bHasValidBodies")]);
    for (class_path, field) in cases {
        let class = x.vm.space.classes.by_path(class_path).unwrap();
        let p = x.vm.space.prop(class, field).unwrap();
        let slot = x.vm.space.classes.resolve(class, PropAddr::scalar(p)).unwrap();
        let c = x.component_of(class_path, Some(a));
        x.vm.space.heap.get_mut(c).unwrap().slots.truncate(slot);
        match x.refused(c, vec![some(b)]) {
            VmError::NotYet(n) => {
                assert_eq!((n.missing, n.what.as_ref(), n.record), (Missing::NotRead, CUT, RECORD), "{class_path}.{field}");
            }
            other => panic!("{class_path}.{field}: expected a NotYet, got {other:?}"),
        }
    }
    // The late class whole: the four writes, as on the stock component.
    let c = x.component_of("Engine.ToyLateBodiesComponent", Some(a));
    x.call(c, vec![some(b)]).unwrap();
    assert_eq!(x.get(c, "SkeletalMesh"), some(b));
}

/// FIX-645-PREFLIGHT, finding 1 on the meshes: a resident mesh with its slots cut is row 5's or row 6's `NotRead`
/// with the cut text (not the unloaded-handle one, review r1 of #664, item 3), before row 11 reads its origin.
#[test]
fn a_cut_mesh_is_not_read() {
    let mut x = fx();
    let (a, b) = (x.export("Mesh_A"), x.export("Mesh_B"));
    x.vm.space.heap.get_mut(b).unwrap().slots.truncate(1);
    let c = x.component(None);
    assert_eq!(kind(x.refused(c, vec![some(b)])), (Missing::NotRead, NEW_CUT.to_string()));
    let c = x.component(Some(b));
    assert_eq!(kind(x.refused(c, vec![some(a)])), (Missing::NotRead, OLD_CUT.to_string()));
}

/// Review r1 of #664, item 1: a correctly declared `Origin` or `RotOrigin` whose slot holds a value of the
/// same-named lookalike struct (zero members, put past the heap's checks) is a script error naming the field, with
/// nothing written; checking the declaration alone let all four writes through.
#[test]
fn a_lookalike_value_in_a_declared_origin_is_refused() {
    for (field, look, expected) in [
        ("Origin", "Engine.SkeletalMesh.Vector", VECTOR),
        ("RotOrigin", "Engine.SkeletalMesh.Rotator", ROTATOR),
    ] {
        let mut x = fx();
        let (a, b) = (x.export("Mesh_A"), x.export("Mesh_B"));
        let ty = x.vm.space.classes.struct_by_path(look).unwrap();
        let zero = x.vm.space.classes.struct_layout(ty).unwrap().zero().clone();
        x.put(b, field, zero);
        let c = x.component(Some(a));
        let e = script_text(x.refused(c, vec![some(b)]));
        assert!(e.contains(&format!("{field} holds a struct of another type, not a {expected}")), "{field}: {e}");
    }
}

/// FIX-645-PREFLIGHT, finding 2: an `Origin` or a `RotOrigin` declared as a struct with the right name and members
/// that is not `Core.Object.Vector` / `Core.Object.Rotator` is a script error naming the field, on the new mesh and on
/// the old, with nothing written.
#[test]
fn a_lookalike_origin_struct_is_refused() {
    let mut x = fx();
    let a = x.export("Mesh_A");
    for (leaf, field, expected) in [("Mesh_LookOrigin", "Origin", VECTOR), ("Mesh_LookRot", "RotOrigin", ROTATOR)] {
        let look = x.export(leaf);
        for (old, new) in [(a, look), (look, a)] {
            let c = x.component(Some(old));
            let e = script_text(x.refused(c, vec![some(new)]));
            assert!(e.contains(&format!("{field} is a struct of another type, not a {expected}")), "{leaf}: {e}");
        }
    }
}

/// Test 11: Reference binds nothing; the world's pack binds the key in Off only.
#[test]
fn reference_binds_nothing() {
    let dir = std::env::temp_dir().join(format!("ow-world-smmc-ref-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let files = packages();
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    let defs = DefStore::from_synthetic(&dir, &refs, IdentityPolicy::FirstInScanOrder, ParityMode::Reference).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Reference);
    ow_vm::api::NativePack::register(&crate::world::Pack, &mut vm).unwrap();
    assert!(!vm.native_keys().any(|k| k == SET_SKELETAL_MESH));
    let _ = std::fs::remove_dir_all(&dir);
}
