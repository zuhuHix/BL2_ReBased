//! Tests of the bone-transform layer in Off (lane IMPL-BONE-TRANSFORM-LAYER, NATIVE_BONE_TRANSFORM_LAYER.md section
//! 6.5, #646 at 94ab4339) on invented packages (AGENTS.md rule 9): `Core.upk` declares `Object` and its `BoneAtom`
//! struct; `Engine.upk` declares `SkeletalMesh`, `SkeletalMeshComponent` with the fields the layer and the natives
//! read, and one variant class per wrong field kind; `GD_ToyMesh.upk` holds meshes and a holder whose field names an
//! unloaded one. The skeleton comes from an in-memory source counting its reads. Every value is invented. AI-assisted
//! (Claude Code).

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropertyDecl;
use ow_object::value::Value;
use ow_package::testkit::ints;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::{FunctionDecl, NativeCall, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::world::skeletal::{
    self, BoneBind, PARENT_COPY, PARENT_COPY_TEXT, PoseUpdate, ROOT_MOTION, ReferenceSkeleton,
    SkeletalMeshes, SkeletonBone, SkeletonSource, TREE_POSE, TREE_POSE_TEXT, UNLOADED_MESH,
};

static NEXT: AtomicU32 = AtomicU32::new(0);
const COMPONENT: &str = "Engine.SkeletalMeshComponent";
/// cos 45° = sin 45°, a quarter-turn's half angle.
const H: f32 = 0.707_106_77;
/// A quarter turn about Z.
const RZ90: [f32; 4] = [0.0, 0.0, H, H];

/// One wrong field kind: the variant class and the field declared with another kind in it.
const WRONG: [(&str, &str); 8] = [
    ("ToyIntMeshComponent", "SkeletalMesh"),
    ("ToyBoolRefposeComponent", "bForceRefpose"),
    ("ToyIntSkipComponent", "bSkipUpdateSkelPose"),
    ("ToyIntAnimationsComponent", "Animations"),
    ("ToyIntParentComponent", "ParentAnimComponent"),
    ("ToyIntDiscardComponent", "bForceDiscardRootMotion"),
    ("ToyIntAttachedComponent", "bAttached"),
    ("ToyIntBasesComponent", "SpaceBases"),
];

fn packages() -> Vec<(&'static str, Vec<u8>)> {
    let core = {
        let mut t = ScriptToy::with_imports(&["Class", "ScriptStruct", "FloatProperty"]);
        let object = t.class("Object", 0);
        let atom = toy_struct(&mut t, object, "BoneAtom", None);
        t.prop("Float", atom, "Scale", 0, 0);
        // Another struct, for a SpaceBases of the wrong element (review r1 of #656, item 3).
        let other = toy_struct(&mut t, object, "ToyOtherAtom", None);
        t.prop("Float", other, "Scale", 0, 0);
        t.build()
    };
    let engine = {
        let mut t = ScriptToy::with_imports(&[
            "Class",
            "ScriptStruct",
            "ObjectProperty",
            "ArrayProperty",
            "IntProperty",
            "BoolProperty",
            "StructProperty",
        ]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let atom = t.p.add_import("Core", "ScriptStruct", object, "BoneAtom");
        let mesh = t.class("SkeletalMesh", object);
        // One invented field, so a slotless stand-in of the class is told from a full object (main's stand-in rule:
        // no slots where the class lays some out).
        t.prop("Int", mesh, "ToyLodCount", 0, 0);
        // The component's fields: each in `wrong` declared as another kind, `missing` not declared at all.
        let fields = |t: &mut ScriptToy, c: i32, wrong: &[&str], missing: Option<&str>| {
            let is = |n: &str| wrong.contains(&n);
            let t = &mut Gate { t, missing };
            if is("SkeletalMesh") {
                t.prop("Int", c, "SkeletalMesh", 0, 0);
            } else {
                t.prop("Object", c, "SkeletalMesh", 0, mesh);
            }
            for n in ["ParentAnimComponent", "Animations"] {
                if is(n) {
                    t.prop("Int", c, n, 0, 0);
                } else {
                    t.prop("Object", c, n, 0, object);
                }
            }
            for n in [
                "bSkipUpdateSkelPose",
                "bForceDiscardRootMotion",
                "bAttached",
            ] {
                if is(n) {
                    t.prop("Int", c, n, 0, 0);
                } else {
                    t.prop("Bool", c, n, 0, 0);
                }
            }
            if is("bForceRefpose") {
                t.prop("Bool", c, "bForceRefpose", 0, 0);
            } else {
                t.prop("Int", c, "bForceRefpose", 0, 0);
            }
            if is("SpaceBases") {
                t.array(c, "SpaceBases", "Int", 0);
            } else {
                t.array(c, "SpaceBases", "Struct", atom);
            }
        };
        let component = t.class("SkeletalMeshComponent", object);
        fields(&mut t, component, &[], None);
        for (class, field) in WRONG {
            let c = t.class(class, object);
            fields(&mut t, c, &[field], None);
        }
        for field in SEVEN {
            let c = t.class(&format!("ToyNo{field}Component"), object);
            fields(&mut t, c, &[], Some(field));
        }
        // Two wrong: the first in 6.3's list (Animations) is reported, not SpaceBases.
        let other =
            t.p.add_import("Core", "ScriptStruct", object, "ToyOtherAtom");
        let struct_bases = t.class("ToyStructBasesComponent", object);
        t.prop("Object", struct_bases, "SkeletalMesh", 0, mesh);
        toy_array_of(&mut t, struct_bases, "SpaceBases", "Struct", other);
        let two = t.class("ToyTwoWrongComponent", object);
        fields(&mut t, two, &["SpaceBases", "Animations"], None);
        let holder = t.class("ToyHolder", object);
        t.prop("Object", holder, "Lazy", 0, object);
        t.build()
    };
    let gd = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let mesh = t.p.add_import("Core", "Class", engine, "SkeletalMesh");
        let holder = t.p.add_import("Core", "Class", engine, "ToyHolder");
        let mut empty = ints(&[0]);
        empty.extend_from_slice(&t.none);
        for n in ["Mesh_Chain", "Mesh_NoBind", "Mesh_Chain2"] {
            t.p.add_export(mesh, n, empty.clone());
        }
        let lazy = t.p.add_export(mesh, "Mesh_Lazy", empty.clone());
        let mut h = ints(&[0]);
        h.extend(t.p.object_tag("Lazy", lazy));
        h.extend_from_slice(&t.none);
        t.p.add_export(holder, "Holder_0", h);
        t.build()
    };
    vec![
        ("Core.upk", core),
        ("Engine.upk", engine),
        ("GD_ToyMesh.upk", gd),
    ]
}

/// 6.3's seven fields, in its list's order.
const SEVEN: [&str; 7] = [
    "SkeletalMesh",
    "Animations",
    "ParentAnimComponent",
    "bSkipUpdateSkelPose",
    "bForceDiscardRootMotion",
    "bForceRefpose",
    "SpaceBases",
];

/// The package builder with one field left undeclared.
struct Gate<'a> {
    t: &'a mut ScriptToy,
    missing: Option<&'a str>,
}

impl Gate<'_> {
    fn prop(&mut self, kind: &str, c: i32, name: &str, flags: u64, type_ref: i32) {
        if self.missing != Some(name) {
            self.t.prop(kind, c, name, flags, type_ref);
        }
    }
    fn array(&mut self, c: i32, name: &str, inner: &str, type_ref: i32) {
        if self.missing != Some(name) {
            toy_array_of(self.t, c, name, inner, type_ref);
        }
    }
}

fn bone(name: &str, parent: Option<usize>, bind: Option<([f32; 4], [f32; 3])>) -> SkeletonBone {
    SkeletonBone {
        name: name.into(),
        parent,
        bind: bind.map(|(rotation, translation)| BoneBind {
            rotation,
            translation,
        }),
    }
}

/// `Mesh_Chain` and `Mesh_Chain2`: three bones, root → B1 → B2, with invented bind poses; `Mesh_NoBind`: the same
/// chain with B1's bind pose unknown.
struct Source {
    reads: Rc<Cell<u32>>,
    /// Whether `Mesh_NoBind` has its bind data after all (test 2's fresh source).
    no_bind_known: bool,
}

impl SkeletonSource for Source {
    fn skeleton(&self, _: &str, _: &str, leaf: &str) -> Result<Option<ReferenceSkeleton>, String> {
        self.reads.set(self.reads.get() + 1);
        if !matches!(leaf, "Mesh_Chain" | "Mesh_Chain2" | "Mesh_NoBind") {
            return Ok(None);
        }
        let b1 = if leaf == "Mesh_NoBind" && !self.no_bind_known {
            None
        } else {
            Some(([0.0, 0.0, 0.0, 1.0], [0.0, 2.0, 0.0]))
        };
        Ok(Some(ReferenceSkeleton {
            bones: vec![
                bone("Root", None, Some((RZ90, [1.0, 0.0, 0.0]))),
                bone("B1", Some(0), b1),
                bone("B2", Some(1), Some((RZ90, [3.0, 0.0, 0.0]))),
            ],
        }))
    }
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    reads: Rc<Cell<u32>>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The fixture in `mode`, with the natives bound (`bind`) or not.
fn fx_with(mode: ParityMode, bind: bool) -> Fx {
    fx_source(mode, bind, false)
}

/// [`fx_with`], its source knowing `Mesh_NoBind`'s bind pose when `no_bind_known`.
fn fx_source(mode: ParityMode, bind: bool, no_bind_known: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-world-btl-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let files = packages();
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    let defs =
        DefStore::from_synthetic(&dir, &refs, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    );
    vm.find_class(COMPONENT).unwrap();
    vm.find_class("Engine.SkeletalMesh").unwrap();
    for (class, _) in WRONG {
        vm.find_class(&format!("Engine.{class}")).unwrap();
    }
    for field in SEVEN {
        vm.find_class(&format!("Engine.ToyNo{field}Component"))
            .unwrap();
    }
    vm.find_class("Engine.ToyTwoWrongComponent").unwrap();
    vm.find_class("Engine.ToyStructBasesComponent").unwrap();
    let component = vm.space.classes.by_path(COMPONENT).unwrap();
    for (path, key) in [(FORCE, FORCE_SKEL_UPDATE), (UPDATE, UPDATE_ANIMATIONS)] {
        let f = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key)
            .with_owner(component)
            .with_params(Vec::<PropertyDecl>::new());
        vm.declare_function(f).unwrap();
    }
    if bind {
        register(&mut vm).unwrap();
    }
    let reads = Rc::new(Cell::new(0));
    SkeletalMeshes::install(
        &mut vm,
        Box::new(Source {
            reads: Rc::clone(&reads),
            no_bind_known,
        }),
    )
    .unwrap();
    let mut x = Fx { vm, dir, reads };
    for n in ["Mesh_Chain", "Mesh_NoBind", "Mesh_Chain2", "Holder_0"] {
        x.export(n);
    }
    x
}

fn fx() -> Fx {
    fx_with(ParityMode::Off, true)
}

/// #518's counters and the name table's length.
fn marks(vm: &Vm) -> (u64, u64, u64, usize) {
    (
        vm.space.heap.committed_writes(),
        vm.rng.draws(),
        vm.defs.load_transitions(),
        vm.space.names.len(),
    )
}

impl Fx {
    fn export(&mut self, leaf: &str) -> ObjRef {
        let path = ObjectPath::parse(&format!("GD_ToyMesh.{leaf}")).unwrap();
        self.vm
            .defs
            .find(&mut self.vm.space, &path)
            .unwrap()
            .unwrap()
    }

    fn lazy(&mut self) -> ObjRef {
        let holder = self.export("Holder_0");
        match self.get(holder, "Lazy") {
            Value::Object(Some(o)) => o,
            other => panic!("{other:?}"),
        }
    }

    fn component_of(&mut self, class: &str, mesh: Option<ObjRef>) -> ObjRef {
        let c = self.vm.space.classes.by_path(class).unwrap();
        let o = self
            .vm
            .space
            .alloc(c, "Comp", None, Origin::Runtime)
            .unwrap();
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

    fn put(&mut self, o: ObjRef, n: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, n).unwrap();
        *self.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }

    /// `pose_update`, its counters checked unchanged whatever it answers (test 9).
    fn pose(&mut self, c: ObjRef) -> Result<PoseUpdate, VmError> {
        let before = marks(&self.vm);
        let r = skeletal::pose_update(&self.vm, c);
        assert_eq!(
            marks(&self.vm),
            before,
            "pose_update writes, draws, loads and interns nothing: {r:?}"
        );
        r
    }

    fn call(&mut self, c: ObjRef, path: &str) -> Result<Value, VmError> {
        let f = self.vm.find_function(path).unwrap();
        let before = marks(&self.vm);
        let r = self.vm.call(f, Some(c), vec![]).map(|r| r.ret);
        assert_eq!(
            marks(&self.vm),
            before,
            "{path} writes, draws, loads and interns nothing: {r:?}"
        );
        r
    }
}

fn refusal<T: std::fmt::Debug>(r: Result<T, VmError>) -> (Missing, String) {
    match r {
        Err(VmError::NotYet(n)) => (n.missing, n.what.to_string()),
        other => panic!("expected a NotYet, got {other:?}"),
    }
}

/// A refusal's text and record.
fn record<T: std::fmt::Debug>(r: Result<T, VmError>) -> (String, &'static str) {
    match r {
        Err(VmError::NotYet(n)) => (n.what.to_string(), n.record),
        other => panic!("expected a NotYet, got {other:?}"),
    }
}

fn script<T: std::fmt::Debug>(r: Result<T, VmError>) -> String {
    match r {
        Err(VmError::Script { message, .. }) => message,
        other => panic!("expected a script error, got {other:?}"),
    }
}

fn close(a: &[f32], b: &[f32]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
}

const FORCE: &str = "Engine.SkeletalMeshComponent.ForceSkelUpdate";
const UPDATE: &str = "Engine.SkeletalMeshComponent.UpdateAnimations";

/// Test 1: the chain follows 6.1's formula: the root is its bind pose, every scale is 1, and a 90-degree parent
/// rotation carries the child's translation.
#[test]
fn the_reference_pose_composes_root_first() {
    let mut x = fx();
    let mesh = x.export("Mesh_Chain");
    let pose = skeletal::reference_space_bases(&x.vm, mesh).unwrap();
    assert_eq!(pose.len(), 3);
    assert!(
        close(&pose[0].rotation, &RZ90) && close(&pose[0].translation, &[1.0, 0.0, 0.0]),
        "{:?}",
        pose[0]
    );
    // B1: the root's quarter turn carries (0, 2, 0) to (-2, 0, 0), plus the root's (1, 0, 0).
    assert!(
        close(&pose[1].rotation, &RZ90) && close(&pose[1].translation, &[-1.0, 0.0, 0.0]),
        "{:?}",
        pose[1]
    );
    // B2: two quarter turns; the parent's turn carries (3, 0, 0) to (0, 3, 0), plus (-1, 0, 0).
    assert!(
        close(&pose[2].rotation, &[0.0, 0.0, 1.0, 0.0])
            && close(&pose[2].translation, &[-1.0, 3.0, 0.0]),
        "{:?}",
        pose[2]
    );
    assert!(pose.iter().all(|a| a.scale == 1.0));
}

/// Test 2: a bone with `bind: None` is #569's layer's LayerAbsent. A second ask of the same mesh refuses again with the
/// same answer, and the composed-pose cache holds no entry for that mesh. A fresh `SkeletalMeshes` service (both caches
/// empty: a new fixture's) whose source has the bind data then computes the pose. Nothing is written, loaded or drawn.
#[test]
fn an_unknown_bind_pose_is_layer_absent_and_caches_nothing() {
    let mut x = fx();
    let no_bind = x.export("Mesh_NoBind");
    let before = marks(&x.vm);
    let first = refusal(skeletal::reference_space_bases(&x.vm, no_bind));
    assert_eq!(first.0, Missing::LayerAbsent(skeletal::LAYER));
    // Review r1 of #656, item 1: this note's record and text.
    assert_eq!(
        record(skeletal::reference_space_bases(&x.vm, no_bind)),
        (skeletal::NO_BIND_POSE.to_string(), skeletal::BONE_RECORD)
    );
    assert_eq!(
        refusal(skeletal::reference_space_bases(&x.vm, no_bind)),
        first,
        "the same answer again"
    );
    assert!(
        !skeletal::has_cached_pose(&x.vm, no_bind),
        "no entry for that mesh"
    );
    assert_eq!(marks(&x.vm), before);
    let mut y = fx_source(ParityMode::Off, true, true);
    let no_bind = y.export("Mesh_NoBind");
    assert_eq!(
        skeletal::reference_space_bases(&y.vm, no_bind)
            .unwrap()
            .len(),
        3,
        "the fresh service computes"
    );
    assert!(skeletal::has_cached_pose(&y.vm, no_bind));
}

/// Test 3: the composed-pose cache. A second ask computes nothing: the source is asked once in total.
#[test]
fn a_second_ask_computes_nothing() {
    let mut x = fx();
    let mesh = x.export("Mesh_Chain");
    let first = skeletal::reference_space_bases(&x.vm, mesh).unwrap();
    assert_eq!(x.reads.get(), 1);
    let second = skeletal::reference_space_bases(&x.vm, mesh).unwrap();
    assert_eq!(x.reads.get(), 1, "the source is asked once in total");
    assert!(Rc::ptr_eq(&first, &second));
}

/// Test 4: the read gives the buffer as stored: empty is empty (no pose is made), filled gives its entries; a
/// `SpaceBases` of another kind is a script error.
#[test]
fn the_read_is_the_buffer_as_stored() {
    let mut x = fx();
    let mesh = x.export("Mesh_Chain");
    let c = x.component(Some(mesh));
    // Each read alone leaves the counters and the name table as they were.
    let read = |x: &Fx, c: ObjRef| {
        let before = marks(&x.vm);
        let r = skeletal::stored_space_bases(&x.vm, c);
        assert_eq!(marks(&x.vm), before, "{r:?}");
        r
    };
    assert_eq!(read(&x, c).unwrap(), Vec::<Value>::new());
    assert_eq!(x.reads.get(), 0, "no pose is made");
    let ty =
        x.vm.space
            .classes
            .struct_by_path(skeletal::BONE_ATOM)
            .unwrap();
    let atom =
        x.vm.space
            .classes
            .zero_value(&ow_object::layout::PropKind::Struct(ty));
    x.put(
        c,
        "SpaceBases",
        Value::array(vec![atom.clone(), atom.clone()]),
    );
    assert_eq!(read(&x, c).unwrap(), vec![atom.clone(), atom]);
    let ints = x.component_of("Engine.ToyIntBasesComponent", Some(mesh));
    assert!(
        script(read(&x, ints))
            .contains(".SpaceBases is not declared as an array of Core.Object.BoneAtom")
    );
    // Review r1 of #656, item 3: an array of another struct, and no SpaceBases at all.
    let structs = x.component_of("Engine.ToyStructBasesComponent", Some(mesh));
    assert!(
        script(read(&x, structs))
            .contains(".SpaceBases is not declared as an array of Core.Object.BoneAtom")
    );
    let undeclared = x.component_of("Engine.ToyNoSpaceBasesComponent", Some(mesh));
    assert!(
        script(read(&x, undeclared))
            .contains(".SpaceBases is not declared as an array of Core.Object.BoneAtom")
    );
}

/// Test 5: `pose_update`, each rule on its own.
#[test]
fn the_pose_update_rule_by_rule() {
    let mut x = fx();
    let (mesh, lazy) = (x.export("Mesh_Chain"), x.lazy());
    let none = x.component(None);
    assert_eq!(x.pose(none).unwrap(), PoseUpdate::Skip);
    let skip = x.component(Some(lazy));
    x.put(skip, "bSkipUpdateSkelPose", Value::Bool(true));
    assert_eq!(
        x.pose(skip).unwrap(),
        PoseUpdate::Skip,
        "the skip comes before the residency"
    );
    let c = x.component(Some(lazy));
    assert_eq!(
        refusal(x.pose(c)),
        (Missing::NotRead, UNLOADED_MESH.to_string())
    );
    assert_eq!(record(x.pose(c)).1, skeletal::BONE_RECORD);
    // A slotless stand-in mesh is not resident either (review r1 of #656, item 3).
    let mesh_class = x.vm.space.classes.by_path("Engine.SkeletalMesh").unwrap();
    let ghost =
        x.vm.space
            .alloc_stand_in(
                mesh_class,
                "ToyGhostMesh",
                ow_object::heap::Origin::Resource {
                    package: 0,
                    index: 1,
                },
            )
            .unwrap();
    let c = x.component(Some(ghost));
    assert_eq!(
        refusal(x.pose(c)),
        (Missing::NotRead, UNLOADED_MESH.to_string())
    );
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy));
    let c = x.component(Some(mesh));
    x.put(c, "ParentAnimComponent", Value::Object(Some(mesh)));
    assert_eq!(
        refusal(x.pose(c)),
        (
            Missing::LayerAbsent(PARENT_COPY),
            PARENT_COPY_TEXT.to_string()
        )
    );
    let c = x.component(Some(mesh));
    x.put(c, "Animations", Value::Object(Some(mesh)));
    assert_eq!(
        refusal(x.pose(c)),
        (Missing::LayerAbsent(TREE_POSE), TREE_POSE_TEXT.to_string())
    );
    x.put(c, "bForceRefpose", Value::Int(1));
    assert!(matches!(x.pose(c).unwrap(), PoseUpdate::Write(p) if p.len() == 3));
    let c = x.component(Some(mesh));
    x.put(c, "bForceDiscardRootMotion", Value::Bool(true));
    assert_eq!(
        refusal(x.pose(c)),
        (Missing::NotPorted, ROOT_MOTION.to_string())
    );
    let c = x.component(Some(mesh));
    let expected = skeletal::reference_space_bases(&x.vm, mesh).unwrap();
    assert_eq!(
        x.pose(c).unwrap(),
        PoseUpdate::Write(expected),
        "a tree-less component gets 6.1's entries"
    );
}

/// Test 6: the field kinds, checked up front. On a component with `SkeletalMesh` None (rule 1 would give `Skip`), each
/// of the seven fields in turn declared with a wrong kind, or undeclared, is still a script error naming it; with two
/// wrong, the first in 6.3's list is reported. `bAttached` of another kind is `ForceSkelUpdate`'s script error. A
/// `bForceRefpose` of 2 counts as set. No counter moves (`Fx::pose`, test 9).
#[test]
fn the_field_kinds() {
    let mut x = fx();
    let mesh = x.export("Mesh_Chain");
    let bare = |x: &mut Fx, class: &str| {
        let c = x.vm.space.classes.by_path(class).unwrap();
        x.vm.space.alloc(c, "Comp", None, Origin::Runtime).unwrap()
    };
    for (class, field) in WRONG {
        let class = format!("Engine.{class}");
        if field == "bAttached" {
            let c = x.component_of(&class, Some(mesh));
            let e = script(x.call(c, FORCE));
            assert!(
                e.contains(&format!(".{field} is not declared")),
                "{class}: {e}"
            );
            continue;
        }
        let c = bare(&mut x, &class);
        let e = script(x.pose(c));
        let want = if field == "SpaceBases" {
            format!(".{field} is not declared as an array")
        } else {
            format!(".{field} is not declared")
        };
        assert!(e.contains(&want), "{class}: {e}");
    }
    for field in SEVEN {
        let c = bare(&mut x, &format!("Engine.ToyNo{field}Component"));
        let e = script(x.pose(c));
        let want = if field == "SpaceBases" {
            format!(".{field} is not declared as an array")
        } else {
            format!(".{field} is not declared")
        };
        assert!(e.contains(&want), "undeclared {field}: {e}");
    }
    let c = bare(&mut x, "Engine.ToyTwoWrongComponent");
    let e = script(x.pose(c));
    assert!(
        e.contains(".Animations is not declared") && !e.contains(".SpaceBases is not declared"),
        "the first in the list: {e}"
    );
    let c = x.component(Some(mesh));
    x.put(c, "Animations", Value::Object(Some(mesh)));
    x.put(c, "bForceRefpose", Value::Int(2));
    assert!(
        matches!(x.pose(c).unwrap(), PoseUpdate::Write(_)),
        "2 counts as set"
    );
}

/// Tests 7 and 8: `ForceSkelUpdate`: unattached is nothing, attached is NotRead (`bAttached` of another kind: test
/// 6). `UpdateAnimations`: no tree is nothing; a tree is NotPorted, the tag unwritten; `Animations` of another kind is
/// a script error. An unloaded receiver of either is NotRead. No counter moves (`Fx::call`).
#[test]
fn the_two_natives() {
    let mut x = fx();
    let mesh = x.export("Mesh_Chain");
    let c = x.component(Some(mesh));
    assert_eq!(x.call(c, FORCE).unwrap(), Value::None);
    assert_eq!(x.call(c, UPDATE).unwrap(), Value::None);
    x.put(c, "bAttached", Value::Bool(true));
    x.put(c, "Animations", Value::Object(Some(mesh)));
    let slots = x.vm.space.heap.get(c).unwrap().slots.clone();
    assert_eq!(
        refusal(x.call(c, FORCE)),
        (Missing::NotRead, FORCE_ATTACHED.to_string())
    );
    assert_eq!(
        refusal(x.call(c, UPDATE)),
        (Missing::NotPorted, TREE_TICKS.to_string())
    );
    assert_eq!(
        x.vm.space.heap.get(c).unwrap().slots,
        slots,
        "nothing written, the tag included"
    );
    let ints = x.component_of("Engine.ToyIntAnimationsComponent", Some(mesh));
    let e = script(x.call(ints, UPDATE));
    assert!(
        e.contains(".Animations is not declared as an object"),
        "{e}"
    );
    let lazy = x.lazy();
    for path in [FORCE, UPDATE] {
        assert_eq!(refusal(x.call(lazy, path)).0, Missing::NotRead, "{path}");
    }
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy));
}

/// Test 10: keep-existing per key in Off (an earlier handler kept, the other key still bound); nothing under
/// Reference.
#[test]
fn keep_existing_in_off_and_nothing_in_reference() {
    for (pre, path) in [(FORCE_SKEL_UPDATE, FORCE), (UPDATE_ANIMATIONS, UPDATE)] {
        let mut x = fx_with(ParityMode::Off, false);
        x.vm.register_native(pre, Rc::new(|_c: &mut NativeCall<'_>| Ok(Value::Int(7))))
            .unwrap();
        register(&mut x.vm).unwrap();
        assert!(
            x.vm.has_native(FORCE_SKEL_UPDATE) && x.vm.has_native(UPDATE_ANIMATIONS),
            "{pre}"
        );
        let mesh = x.export("Mesh_Chain");
        let c = x.component(Some(mesh));
        assert_eq!(
            x.call(c, path).unwrap(),
            Value::Int(7),
            "the earlier handler kept"
        );
    }
    let r = fx_with(ParityMode::Reference, true);
    assert!(!r.vm.has_native(FORCE_SKEL_UPDATE) && !r.vm.has_native(UPDATE_ANIMATIONS));
}
