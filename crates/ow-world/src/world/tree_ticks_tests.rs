//! `UpdateAnimations`' per-node step in Off (NATIVE_UPDATEANIMATIONS_TREE_TICKS.md section 6, tests 1 to 11) on
//! invented packages (AGENTS.md rule 9): `Core` (`Object`); `Engine` (the anim-node classes the tests name on their
//! exact paths, a component with the fields the step reads, `SkeletalMesh`, `AnimSet`, `AnimSequence`, a toy unknown
//! node class, a tree class whose `NodeInitTag` is a bool, lazy exports behind a holder, and the native
//! `UpdateAnimations`); `WillowGame` (the feather blend, a malformed `WillowAnimNodeSequence` and the additive node).
//! Every value is invented; every call goes through the VM. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{NameId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::ints;
use ow_package::testkit::reflect::toy_array_of;
use ow_package::testkit::script::{FunctionSpec, ScriptToy};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;
use crate::world::bone_transform::{UPDATE_ANIMATIONS, register};

static NEXT: AtomicU32 = AtomicU32::new(0);
const IMPORTS: [&str; 8] = [
    "Class",
    "Function",
    "ObjectProperty",
    "ArrayProperty",
    "NameProperty",
    "IntProperty",
    "BoolProperty",
    "ByteProperty",
];
const NATIVE: u32 = 0x400 | 0x20000;
const UPDATE: &str = "Engine.SkeletalMeshComponent.UpdateAnimations";
const COMPONENT: &str = "Engine.SkeletalMeshComponent";

fn core() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    t.class("Object", 0);
    t.build()
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let core = t.p.add_import("Core", "Package", 0, "Core");
    let object = t.p.add_import("Core", "Class", core, "Object");
    let node = t.class("AnimNode", object);
    t.prop("Int", node, "NodeInitTag", 0, 0);
    t.prop("Object", node, "SkelComponent", 0, object);
    t.prop("Byte", node, "CachedNumDesiredBones", 0, 0);
    let blend = t.class("AnimNodeBlendBase", node);
    t.class("AnimNodeBlendList", blend);
    t.class("AnimTree", blend);
    let sequence = t.class("AnimNodeSequence", node);
    t.prop("Name", sequence, "AnimSeqName", 0, 0);
    t.prop("Object", sequence, "AnimSeq", 0, object);
    t.prop("Int", sequence, "AnimLinkupIndex", 0, 0);
    t.prop("Bool", sequence, "bIsIssuingNotifies", 0, 0);
    t.prop("Bool", sequence, "bReverseAnimSeqSearchOrder", 0, 0);
    t.class("ToyUnknownNode", node);
    let bad_tree = t.class("ToyBoolTagTree", object);
    t.prop("Bool", bad_tree, "NodeInitTag", 0, 0);
    t.class("SkeletalMesh", object);
    let component = t.class("SkeletalMeshComponent", object);
    t.prop("Object", component, "Animations", 0, object);
    toy_array_of(&mut t, component, "AnimTickArray", "Object", object);
    t.prop("Int", component, "InitTag", 0, 0);
    t.prop("Object", component, "SkeletalMesh", 0, object);
    toy_array_of(&mut t, component, "AnimSets", "Object", object);
    let spec = FunctionSpec::new(NATIVE, "UpdateAnimations");
    t.function(component, "UpdateAnimations", &[], None, &[], &spec, |_| None);
    let set = t.class("AnimSet", object);
    toy_array_of(&mut t, set, "Sequences", "Object", object);
    let anim = t.class("AnimSequence", object);
    t.prop("Name", anim, "SequenceName", 0, 0);
    // Lazy exports behind a holder: a sequence node (a tick entry) and an anim set.
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Node", 0, object);
    t.prop("Object", holder, "Set", 0, object);
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    let lazy_node = t.p.add_export(sequence, "Seq_Lazy", empty.clone());
    let lazy_set = t.p.add_export(set, "Set_Lazy", empty);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Node", lazy_node));
    h.extend(t.p.object_tag("Set", lazy_set));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let core = t.p.add_import("Core", "Package", 0, "Core");
    let object = t.p.add_import("Core", "Class", core, "Object");
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let node = t.p.add_import("Core", "Class", engine, "AnimNode");
    let blend = t.p.add_import("Core", "Class", engine, "AnimNodeBlendBase");
    let sequence = t.p.add_import("Core", "Class", engine, "AnimNodeSequence");
    t.class("WillowAnimNodeFeatherBlend", blend);
    // A plain-family class with AnimSeqName declared as an int (test 10).
    let bad = t.class("WillowAnimNodeSequence", node);
    t.prop("Int", bad, "AnimSeqName", 0, 0);
    let additive = t.class("WillowAnimNodeSequenceAdditiveBlend", sequence);
    t.prop("Bool", additive, "bAutoGenerateAdditiveSeqName", 0, 0);
    t.prop("Name", additive, "AddAnimSeqName", 0, 0);
    t.prop("Object", additive, "AddAnimSeq", 0, object);
    t.prop("Int", additive, "AddAnimLinkupIndex", 0, 0);
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

fn fx_in(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-world-ticks-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (c, e, w) = (core(), engine(), willow());
    let files: Vec<(&str, &[u8])> = vec![("Core.upk", &c), ("Engine.upk", &e), ("WillowGame.upk", &w)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    for class in [
        "Engine.AnimNode",
        "Engine.AnimNodeBlendList",
        "Engine.AnimTree",
        "Engine.AnimNodeSequence",
        "Engine.ToyUnknownNode",
        "Engine.ToyBoolTagTree",
        "Engine.SkeletalMesh",
        COMPONENT,
        "Engine.AnimSet",
        "Engine.AnimSequence",
        "WillowGame.WillowAnimNodeFeatherBlend",
        "WillowGame.WillowAnimNodeSequence",
        "WillowGame.WillowAnimNodeSequenceAdditiveBlend",
    ] {
        vm.find_class(class).unwrap();
    }
    register(&mut vm).unwrap();
    Fx { vm, dir }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
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

    fn name(&mut self, text: &str) -> Value {
        Value::Name(self.vm.space.names.intern(text))
    }

    fn list(items: &[ObjRef]) -> Value {
        Value::array(items.iter().map(|o| Value::Object(Some(*o))).collect())
    }

    /// A component whose tree root (`NodeInitTag` 7) ticks `nodes`.
    fn component(&mut self, nodes: &[ObjRef]) -> ObjRef {
        let root = self.obj("Engine.AnimTree");
        self.set(root, "NodeInitTag", Value::Int(7));
        let c = self.obj(COMPONENT);
        self.set(c, "Animations", Value::Object(Some(root)));
        self.set(c, "AnimTickArray", Self::list(nodes));
        c
    }

    /// A sequence node of `class` named `name`, `AnimLinkupIndex` 5 and `CachedNumDesiredBones` 9.
    fn sequence(&mut self, class: &str, name: &str) -> ObjRef {
        let n = self.obj(class);
        let name = self.name(name);
        self.set(n, "AnimSeqName", name);
        self.set(n, "AnimLinkupIndex", Value::Int(5));
        self.set(n, "CachedNumDesiredBones", Value::Byte(9));
        n
    }

    /// An additive node named `name`, its auto-name flag `auto` and its `AddAnimSeqName` `add` (None for `None`).
    fn additive(&mut self, auto: bool, add: Option<&str>) -> ObjRef {
        let n = self.sequence("WillowGame.WillowAnimNodeSequenceAdditiveBlend", "ToyIdle");
        self.set(n, "bAutoGenerateAdditiveSeqName", Value::Bool(auto));
        let add = match add {
            Some(text) => self.name(text),
            None => Value::Name(NameId::NONE),
        };
        self.set(n, "AddAnimSeqName", add);
        self.set(n, "AddAnimLinkupIndex", Value::Int(4));
        n
    }

    /// An anim set holding sequences named `names`.
    fn anim_set(&mut self, names: &[&str]) -> ObjRef {
        let set = self.obj("Engine.AnimSet");
        let mut sequences = Vec::new();
        for text in names {
            let s = self.obj("Engine.AnimSequence");
            let name = self.name(text);
            self.set(s, "SequenceName", name);
            sequences.push(s);
        }
        self.set(set, "Sequences", Self::list(&sequences));
        set
    }

    /// Gives `node` the component `c` as its `SkelComponent`, with a mesh and the anim sets `sets`.
    fn link(&mut self, node: ObjRef, c: ObjRef, sets: &[ObjRef]) {
        let mesh = self.obj("Engine.SkeletalMesh");
        self.set(c, "SkeletalMesh", Value::Object(Some(mesh)));
        self.set(c, "AnimSets", Self::list(sets));
        self.set(node, "SkelComponent", Value::Object(Some(c)));
    }

    fn update(&mut self, c: ObjRef) -> Result<Value, VmError> {
        let f = self.vm.find_function(UPDATE).unwrap();
        Ok(self.vm.call(f, Some(c), vec![])?.ret)
    }

    fn holder(&mut self, field: &str) -> ObjRef {
        let path = ObjectPath::parse("Engine.ToyHolderObject").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        match self.get(holder, field) {
            Value::Object(Some(o)) => {
                assert!(self.vm.defs.is_unloaded(&self.vm.space, o));
                o
            }
            other => panic!("{other:?}"),
        }
    }

    /// #518's counters, the class-code loads and the name table.
    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }

    fn text(&self, v: &Value) -> String {
        match v {
            Value::Name(n) => self.vm.space.names.text(*n).into_owned(),
            other => panic!("{other:?}"),
        }
    }
}

/// A `NotRead` with `what`, nothing moved.
fn refused(x: &mut Fx, c: ObjRef, what: &str) {
    let before = x.marks();
    match x.update(c) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert!(n.what == what || (what == NOT_LOADED && n.what.starts_with(NOT_LOADED)), "{n}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before, "nothing written, drawn, loaded or interned: {what}");
    assert_eq!(x.get(c, "InitTag"), Value::Int(0), "the tag included");
}

fn script(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::Script { message, .. }) => message,
        other => panic!("{other:?}"),
    }
}

/// Test 1: no tree, nothing; an empty tick list, only the tag.
#[test]
fn no_tree_is_nothing_and_an_empty_tick_list_only_the_tag() {
    let mut x = fx();
    let c = x.obj(COMPONENT);
    let before = x.marks();
    assert_eq!(x.update(c).unwrap(), Value::None);
    assert_eq!(x.marks(), before);
    let c = x.component(&[]);
    let before = x.marks();
    x.update(c).unwrap();
    assert_eq!(x.get(c, "InitTag"), Value::Int(8), "NodeInitTag + 1");
    assert_eq!(x.marks().0, before.0 + 1, "one write");
}

/// Test 2: empty-step classes, only the tag.
#[test]
fn empty_step_classes_write_only_the_tag() {
    let mut x = fx();
    let a = x.obj("Engine.AnimNodeBlendList");
    let b = x.obj("WillowGame.WillowAnimNodeFeatherBlend");
    x.set(a, "CachedNumDesiredBones", Value::Byte(3));
    let c = x.component(&[a, b]);
    let before = x.marks();
    x.update(c).unwrap();
    assert_eq!(x.get(c, "InitTag"), Value::Int(8));
    assert_eq!(x.marks().0, before.0 + 1, "one write");
    assert_eq!(x.get(a, "CachedNumDesiredBones"), Value::Byte(3));
}

/// Test 3: a plain sequence node with no sequence: its name kept, no sequence, link-up -1, no desired bones.
#[test]
fn a_plain_sequence_node_drops_its_sequence_and_keeps_its_name() {
    let mut x = fx();
    let n = x.sequence("Engine.AnimNodeSequence", "ToyRun");
    let c = x.component(&[n]);
    x.update(c).unwrap();
    assert_eq!(x.text(&x.get(n, "AnimSeqName")), "ToyRun");
    assert_eq!(x.get(n, "AnimSeq"), Value::Object(None));
    assert_eq!(x.get(n, "AnimLinkupIndex"), Value::Int(-1));
    assert_eq!(x.get(n, "CachedNumDesiredBones"), Value::Byte(0));
}

/// Test 4: issuing notifies under a name: no reset (a set sequence stays, the link-up too), the bones 0; issuing with
/// the name None: the reset of test 3.
#[test]
fn issuing_notifies_skips_the_reset_only_under_a_name() {
    let mut x = fx();
    let n = x.sequence("Engine.AnimNodeSequence", "ToyRun");
    x.set(n, "bIsIssuingNotifies", Value::Bool(true));
    let anim = x.obj("Engine.AnimSequence");
    x.set(n, "AnimSeq", Value::Object(Some(anim)));
    let c = x.component(&[n]);
    x.update(c).unwrap();
    assert_eq!(x.get(n, "AnimSeq"), Value::Object(Some(anim)));
    assert_eq!(x.get(n, "AnimLinkupIndex"), Value::Int(5));
    assert_eq!(x.get(n, "CachedNumDesiredBones"), Value::Byte(0));
    assert_eq!(x.text(&x.get(n, "AnimSeqName")), "ToyRun");
    let m = x.sequence("Engine.AnimNodeSequence", "None");
    x.set(m, "bIsIssuingNotifies", Value::Bool(true));
    let c = x.component(&[m]);
    x.update(c).unwrap();
    assert_eq!(x.get(m, "AnimLinkupIndex"), Value::Int(-1), "from 5 to -1");
    assert_eq!(x.get(m, "AnimSeq"), Value::Object(None));
    assert_eq!(x.get(m, "CachedNumDesiredBones"), Value::Byte(0));
}

/// Test 5: a set sequence with the reset to run: the meta-data `NotRead`, nothing written.
#[test]
fn a_set_sequence_is_the_meta_data_refusal() {
    let mut x = fx();
    let n = x.sequence("Engine.AnimNodeSequence", "ToyRun");
    let anim = x.obj("Engine.AnimSequence");
    x.set(n, "AnimSeq", Value::Object(Some(anim)));
    let c = x.component(&[n]);
    refused(&mut x, c, SEQUENCE_CHANGES);
}

/// Test 6: the additive override with no `SkelComponent`: flag clear keeps its name and interns nothing; flag set
/// writes `Add_None`, interned once, at the write.
#[test]
fn the_additive_override_re_sets_its_sequence() {
    let mut x = fx();
    let n = x.additive(false, Some("ToyAdd"));
    let c = x.component(&[n]);
    let before = x.marks();
    x.update(c).unwrap();
    assert_eq!(x.marks().4, before.4, "nothing interned");
    assert_eq!(x.text(&x.get(n, "AddAnimSeqName")), "ToyAdd");
    assert_eq!(x.get(n, "AddAnimSeq"), Value::Object(None));
    assert_eq!(x.get(n, "AddAnimLinkupIndex"), Value::Int(-1));
    assert_eq!(x.get(n, "AnimLinkupIndex"), Value::Int(-1), "the plain step first");
    assert_eq!(x.text(&x.get(n, "AnimSeqName")), "ToyIdle");
    let m = x.additive(true, Some("ToyAdd"));
    let c = x.component(&[m]);
    assert!(x.vm.space.names.find(AUTO_NAME).is_none(), "the fixture's table lacks it");
    let before = x.marks();
    x.update(c).unwrap();
    assert_eq!(x.marks().4, before.4 + 1, "interned once");
    assert_eq!(x.text(&x.get(m, "AddAnimSeqName")), AUTO_NAME);
    assert_eq!(x.get(m, "AddAnimSeq"), Value::Object(None));
    assert_eq!(x.get(m, "AddAnimLinkupIndex"), Value::Int(-1));
}

/// Test 7: the lookup, last set first (first set first with `bReverseAnimSeqSearchOrder`); a match is the link-up
/// `NotRead`, no match is None; `AddAnimSeqName` None looks nothing up; a lazy set is otherwise the not-loaded
/// `NotRead`.
#[test]
fn the_lookup_and_its_refusals() {
    // A flag-clear node whose name is in the second set (searched first), and an auto-name node the same way.
    for (auto, add, looked) in [(false, Some("ToyAdd"), "ToyAdd"), (true, None, AUTO_NAME)] {
        let mut x = fx();
        let n = x.additive(auto, add);
        let c = x.component(&[n]);
        let first = x.anim_set(&["ToyOther"]);
        let second = x.anim_set(&["ToyMore", looked]);
        x.link(n, c, &[first, second]);
        refused(&mut x, c, LINKUP);
        // The match only in the first set, searched first with the flag.
        let mut y = fx();
        let n = y.additive(auto, add);
        y.set(n, "bReverseAnimSeqSearchOrder", Value::Bool(true));
        let c = y.component(&[n]);
        let first = y.anim_set(&[looked]);
        let second = y.anim_set(&["ToyOther"]);
        y.link(n, c, &[first, second]);
        refused(&mut y, c, LINKUP);
    }
    // No match anywhere: None.
    let mut x = fx();
    let n = x.additive(false, Some("ToyAdd"));
    let c = x.component(&[n]);
    let set = x.anim_set(&["ToyOther"]);
    x.link(n, c, &[set]);
    x.update(c).unwrap();
    assert_eq!(x.get(n, "AddAnimSeq"), Value::Object(None));
    // AddAnimSeqName None, flag clear: no lookup, so a lazy set is not read.
    let mut x = fx();
    let n = x.additive(false, None);
    let c = x.component(&[n]);
    let lazy = x.holder("Set");
    x.link(n, c, &[lazy]);
    x.update(c).unwrap();
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "not read");
    // A name to look up: the lazy set is the not-loaded refusal.
    let mut x = fx();
    let n = x.additive(false, Some("ToyAdd"));
    let c = x.component(&[n]);
    let lazy = x.holder("Set");
    x.link(n, c, &[lazy]);
    refused(&mut x, c, NOT_LOADED);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "still unloaded");
    // The refusal names the role and the handle's path.
    match x.update(c) {
        Err(VmError::NotYet(n)) => assert_eq!(&*n.what, not_loaded("the anim set", "Engine.Set_Lazy")),
        other => panic!("{other:?}"),
    }
}

/// Test 8: an unknown node class: its `NotRead`, nothing written.
#[test]
fn an_unknown_node_class_is_not_read() {
    let mut x = fx();
    let n = x.obj("Engine.ToyUnknownNode");
    let c = x.component(&[n]);
    refused(&mut x, c, &per_node_step("Engine.ToyUnknownNode"));
}

/// Test 9: a lazy tick-list entry: the not-loaded `NotRead`.
#[test]
fn a_lazy_tick_entry_is_not_read() {
    let mut x = fx();
    let lazy = x.holder("Node");
    let c = x.component(&[lazy]);
    refused(&mut x, c, TICK_NODE_NOT_LOADED);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy));
}

/// Test 10: `AnimSeqName` not a name, and `NodeInitTag` not an int: script errors, nothing written.
#[test]
fn malformed_declarations_are_script_errors() {
    let mut x = fx();
    let n = x.obj("WillowGame.WillowAnimNodeSequence");
    let c = x.component(&[n]);
    let before = x.marks();
    assert!(script(x.update(c)).contains("AnimSeqName is not declared as Name"));
    assert_eq!(x.marks(), before);
    let root = x.obj("Engine.ToyBoolTagTree");
    let c = x.obj(COMPONENT);
    x.set(c, "Animations", Value::Object(Some(root)));
    let before = x.marks();
    assert!(script(x.update(c)).contains("NodeInitTag is not declared as Int"));
    assert_eq!(x.marks(), before);
}

/// Test 11: Reference binds nothing, so the step never runs there.
#[test]
fn reference_is_unchanged() {
    let x = fx_in(ParityMode::Reference);
    assert!(!x.vm.has_native(UPDATE_ANIMATIONS));
}
