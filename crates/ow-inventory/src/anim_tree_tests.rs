//! Tests of `SkeletalMeshComponent.SetAnimTreeTemplate` in Off (NATIVE_SET_ANIM_TREE_TEMPLATE.md; lane
//! IMPL-SETANIMTREETEMPLATE) on one invented `Engine` package built with `ow_package::testkit` (AGENTS.md rule 9):
//! `Actor`, `ActorComponent` (`Owner`), and `SkeletalMeshComponent` with the fields of the note and the native. Every
//! call goes through the VM, and every refusal is graded by its `NotYet` kind.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, NotYet, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 6] = ["Class", "Function", "ObjectProperty", "BoolProperty", "ArrayProperty", "MapProperty"];

const PATH: &str = "Engine.SkeletalMeshComponent.SetAnimTreeTemplate";
/// Fields the note's teardown would write. The native must leave them unchanged.
const UNTOUCHED: [&str; 4] = ["AnimTreeTemplate", "bAnimTreeInitialised", "ActiveMorphs", "SkelControlTickArray"];

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let component = t.class("ActorComponent", object);
    t.prop("Object", component, "Owner", 0, actor);
    let mesh = t.class("SkeletalMeshComponent", component);
    t.prop("Object", mesh, "AnimTreeTemplate", 0, object);
    t.prop("Object", mesh, "Animations", 0, object);
    t.prop("Bool", mesh, "bAnimTreeInitialised", 0, 0);
    for name in ["AnimTickArray", "AnimAlwaysTickArray", "SkelControlTickArray", "ActiveMorphs", "ActiveCurveMorphs"] {
        let array = t.p.next_export();
        t.prop("Array", mesh, name, 0, array + 1);
        t.prop("Object", array, name, 0, object);
    }
    t.prop_tail("Map", mesh, "MorphTargetIndexMap", 0, &ow_package::testkit::ints(&[0, 0]));
    let template = Param { type_ref: object, ..Param::new("Object", "NewTemplate") };
    t.function(mesh, "SetAnimTreeTemplate", &[template], None, &[], &FunctionSpec::new(NATIVE, "SetAnimTreeTemplate"), |_| None);
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

fn fx(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-anim-tree-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let e = engine();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", e.as_slice())], IdentityPolicy::FirstInScanOrder, mode)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    vm.find_class("Engine.SkeletalMeshComponent").unwrap();
    register(&mut vm).unwrap();
    Fx { vm, dir }
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

    fn fields(&self, o: ObjRef) -> Vec<Value> {
        UNTOUCHED.iter().map(|f| self.get(o, f)).collect()
    }

    fn call(&mut self, me: ObjRef, template: Option<ObjRef>) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH)?;
        Ok(self.vm.call(f, Some(me), vec![Value::Object(template)])?.ret)
    }

    /// A component whose to-be-written fields hold something other than what the teardown would write.
    fn dirty_mesh(&mut self) -> ObjRef {
        let mesh = self.obj("Engine.SkeletalMeshComponent");
        let thing = self.obj("Engine.Actor");
        self.set(mesh, "AnimTreeTemplate", Value::Object(Some(thing)));
        self.set(mesh, "bAnimTreeInitialised", Value::Bool(true));
        for name in ["SkelControlTickArray", "ActiveMorphs"] {
            self.set(mesh, name, Value::array(vec![Value::Object(Some(thing))]));
        }
        mesh
    }
}

fn not_yet(r: Result<Value, VmError>) -> NotYet {
    match r {
        Err(VmError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// The slice's call: None on a component with class defaults. The native ends at the registry walk with
/// `NotYet { LayerAbsent }`. The result is unmarked (no partial-writes prefix) and nothing is written.
#[test]
fn the_slice_call_ends_at_the_registry_unmarked_and_writes_nothing() {
    let mut x = fx(ParityMode::Off);
    let fresh = x.obj("Engine.SkeletalMeshComponent");
    let n = not_yet(x.call(fresh, None));
    assert_eq!(n.missing, Missing::LayerAbsent(REGISTRY_LAYER));
    assert_eq!((n.what.as_ref(), n.record), (REGISTRY, RECORD));
    assert!(!ow_core::notyet::is_partial_writes(&n.what));

    let mesh = x.dirty_mesh();
    let before = x.fields(mesh);
    assert_eq!(not_yet(x.call(mesh, None)).missing, Missing::LayerAbsent(REGISTRY_LAYER));
    assert_eq!(x.fields(mesh), before, "nothing written");
}

/// Items 1-3 come before the registry, in the note's order: a template first (whatever the component holds), then a
/// non-empty tick list, then a set `Animations`. Each is a `NotRead` and writes nothing.
#[test]
fn items_one_to_three_are_not_read_in_the_notes_order() {
    let mut x = fx(ParityMode::Off);
    let mesh = x.dirty_mesh();
    let node = x.obj("Engine.Actor");
    x.set(mesh, "AnimTickArray", Value::array(vec![Value::Object(Some(node))]));
    x.set(mesh, "Animations", Value::Object(Some(node)));
    let before = x.fields(mesh);
    let template = x.obj("Engine.Actor");
    let n = not_yet(x.call(mesh, Some(template)));
    assert_eq!((n.missing, n.what.as_ref(), n.record), (Missing::NotRead, WITH_TEMPLATE, RECORD));
    let n = not_yet(x.call(mesh, None));
    assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, TICK_NODES));
    x.set(mesh, "AnimTickArray", Value::array(Vec::new()));
    let n = not_yet(x.call(mesh, None));
    assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, CURRENT_TREE));
    assert_eq!(x.fields(mesh), before, "nothing written");
    assert_eq!(x.get(mesh, "Animations"), Value::Object(Some(node)));
}

/// A component class without the declarations is a data error, before the registry.
#[test]
fn a_component_without_the_fields_is_a_data_error() {
    let mut x = fx(ParityMode::Off);
    let bare = x.obj("Engine.ActorComponent");
    match set_anim_tree_template(&x.vm, bare, None) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("AnimTickArray"), "{message}"),
        other => panic!("expected a data error, got {other:?}"),
    }
}

/// Reference binds nothing: the stub policy answers.
#[test]
fn reference_binds_nothing() {
    let mut x = fx(ParityMode::Reference);
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    assert!(matches!(x.call(mesh, None), Err(VmError::UnboundNative(_))));
}
