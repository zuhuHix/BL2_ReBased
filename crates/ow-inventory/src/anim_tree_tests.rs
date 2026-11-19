//! Tests of `SkeletalMeshComponent.SetAnimTreeTemplate` in Off (NATIVE_SET_ANIM_TREE_TEMPLATE.md; lane
//! IMPL-SETANIMTREETEMPLATE) on one invented `Engine` package built with `ow_package::testkit` (AGENTS.md rule 9):
//! `Actor`, `ActorComponent` (`Owner`), `SkeletalMeshComponent` with the fields the native touches and the native, and
//! `ToyOwner`, an actor whose `AnimTreeUpdated` event calls the invented native `ToyRecord(SkelMesh)` that the tests
//! bind. Every call goes through the VM.

use std::cell::RefCell;
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
use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
const EVENT_FLAGS: u32 = 0x1 | 0x2 | 0x800 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 7] =
    ["Class", "Function", "ObjectProperty", "BoolProperty", "ArrayProperty", "MapProperty", "IntProperty"];

const RECORD_KEY: &str = "ToyOwner.ToyRecord(object)";
const PATH: &str = "Engine.SkeletalMeshComponent.SetAnimTreeTemplate";

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
    let owner = t.class("ToyOwner", actor);
    let skel = || Param { type_ref: component, ..Param::new("Object", "SkelMesh") };
    let record = t.function(owner, "ToyRecord", &[skel()], None, &[], &FunctionSpec::new(NATIVE, "ToyRecord"), |_| None);
    t.function(owner, "AnimTreeUpdated", &[skel()], None, &[], &FunctionSpec::new(EVENT_FLAGS, "AnimTreeUpdated"), |f| {
        let mut a = Asm::new();
        a.stmt().final_function(record.function, |a| {
            a.local(f.get("SkelMesh"));
        });
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.build()
}

/// What `ToyRecord` saw, and whether it refuses.
#[derive(Default)]
struct Seen {
    calls: Vec<(Option<ObjRef>, Value)>,
    refuse: bool,
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    seen: Rc<RefCell<Seen>>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A VM in `mode`. `registry` None binds the module's own [`register`]; Some binds [`native_with`] in its place.
fn fx(mode: ParityMode, registry: Option<Registry>) -> Fx {
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
    for c in ["Engine.SkeletalMeshComponent", "Engine.ToyOwner"] {
        vm.find_class(c).unwrap();
    }
    match registry {
        None => register(&mut vm).unwrap(),
        Some(r) => vm.register_native(SET_ANIM_TREE_TEMPLATE, native_with(r)).unwrap(),
    }
    let seen = Rc::new(RefCell::new(Seen::default()));
    let s = seen.clone();
    vm.register_native(
        RECORD_KEY,
        Rc::new(move |c: &mut NativeCall<'_>| {
            let arg = c.arg(0)?.clone();
            s.borrow_mut().calls.push((c.this, arg));
            if s.borrow().refuse {
                return Err(NotYet::not_read("an invented owner refusal", "toy").into());
            }
            Ok(Value::None)
        }),
    )
    .unwrap();
    Fx { vm, dir, seen }
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

    fn call(&mut self, me: ObjRef, template: Option<ObjRef>) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH)?;
        Ok(self.vm.call(f, Some(me), vec![Value::Object(template)])?.ret)
    }

    /// A component with every field the native writes set to something other than what it writes.
    fn dirty_mesh(&mut self) -> ObjRef {
        let mesh = self.obj("Engine.SkeletalMeshComponent");
        let thing = self.obj("Engine.Actor");
        self.set(mesh, "AnimTreeTemplate", Value::Object(Some(thing)));
        self.set(mesh, "bAnimTreeInitialised", Value::Bool(true));
        for name in ["AnimAlwaysTickArray", "SkelControlTickArray", "ActiveMorphs", "ActiveCurveMorphs"] {
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

fn len(v: &Value) -> usize {
    match v {
        Value::Array(a) => a.len(),
        other => panic!("expected an array, got {other:?}"),
    }
}

/// The bound native's default (the note's section 4, step 1.4): the registry walk is `NotYet { LayerAbsent }`, before
/// any write, so the component is unchanged and the owner hears nothing.
#[test]
fn the_bound_native_stops_at_the_registry_before_any_write() {
    let mut x = fx(ParityMode::Off, None);
    let mesh = x.dirty_mesh();
    let owner = x.obj("Engine.ToyOwner");
    x.set(mesh, "Owner", Value::Object(Some(owner)));
    let before: Vec<Value> = ["AnimTreeTemplate", "bAnimTreeInitialised", "ActiveMorphs"].map(|f| x.get(mesh, f)).into();
    let n = not_yet(x.call(mesh, None));
    assert_eq!((n.what.as_ref(), n.record), (REGISTRY, RECORD));
    assert_eq!(n.missing, Missing::LayerAbsent(REGISTRY_LAYER));
    let after: Vec<Value> = ["AnimTreeTemplate", "bAnimTreeInitialised", "ActiveMorphs"].map(|f| x.get(mesh, f)).into();
    assert_eq!(before, after);
    assert!(x.seen.borrow().calls.is_empty());
}

/// The preflight's three `NotRead`s, in the note's order, each before any write: a template first (whatever the
/// component holds), then a non-empty tick list, then a set `Animations`.
#[test]
fn the_preflight_refuses_in_the_notes_order() {
    let mut x = fx(ParityMode::Off, Some(Registry::Nothing));
    let mesh = x.dirty_mesh();
    let node = x.obj("Engine.Actor");
    x.set(mesh, "AnimTickArray", Value::array(vec![Value::Object(Some(node))]));
    x.set(mesh, "Animations", Value::Object(Some(node)));
    let template = x.obj("Engine.Actor");
    let n = not_yet(x.call(mesh, Some(template)));
    assert_eq!((n.what.as_ref(), n.missing, n.record), (WITH_TEMPLATE, Missing::NotRead, RECORD));
    let n = not_yet(x.call(mesh, None));
    assert_eq!((n.what.as_ref(), n.missing), (TICK_NODES, Missing::NotRead));
    x.set(mesh, "AnimTickArray", Value::array(Vec::new()));
    let n = not_yet(x.call(mesh, None));
    assert_eq!((n.what.as_ref(), n.missing), (CURRENT_TREE, Missing::NotRead));
    assert_eq!(x.get(mesh, "bAnimTreeInitialised"), Value::Bool(true), "nothing written");
    assert_eq!(x.get(mesh, "Animations"), Value::Object(Some(node)));
}

/// With the registry ruled empty (the note's second option, not bound in Off): every write in order, then the owner's
/// `AnimTreeUpdated(component)` runs as script on the owner.
#[test]
fn with_an_empty_registry_it_clears_the_tree_and_tells_the_owner() {
    let mut x = fx(ParityMode::Off, Some(Registry::Nothing));
    let mesh = x.dirty_mesh();
    let owner = x.obj("Engine.ToyOwner");
    x.set(mesh, "Owner", Value::Object(Some(owner)));
    assert_eq!(x.call(mesh, None).unwrap(), Value::None);
    assert_eq!(x.get(mesh, "AnimTreeTemplate"), Value::Object(None));
    assert_eq!(x.get(mesh, "Animations"), Value::Object(None));
    assert_eq!(x.get(mesh, "bAnimTreeInitialised"), Value::Bool(false));
    for name in ["AnimTickArray", "AnimAlwaysTickArray", "SkelControlTickArray", "ActiveMorphs", "ActiveCurveMorphs"] {
        assert_eq!(len(&x.get(mesh, name)), 0, "{name}");
    }
    assert_eq!(x.get(mesh, "MorphTargetIndexMap"), Value::None);
    assert_eq!(x.seen.borrow().calls, vec![(Some(owner), Value::Object(Some(mesh)))]);
}

/// No owner: the writes, and no event.
#[test]
fn without_an_owner_no_event_is_sent() {
    let mut x = fx(ParityMode::Off, Some(Registry::Nothing));
    let mesh = x.dirty_mesh();
    assert_eq!(x.call(mesh, None).unwrap(), Value::None);
    assert_eq!(x.get(mesh, "AnimTreeTemplate"), Value::Object(None));
    assert!(x.seen.borrow().calls.is_empty());
}

/// A `NotYet` from the owner's event, and an owner class without the event, come after the writes: terminal, marked
/// once as after partial writes, kind and record kept, the writes not rolled back.
#[test]
fn an_owner_refusal_after_the_writes_is_marked() {
    let mut x = fx(ParityMode::Off, Some(Registry::Nothing));
    let mesh = x.dirty_mesh();
    let owner = x.obj("Engine.ToyOwner");
    x.set(mesh, "Owner", Value::Object(Some(owner)));
    x.seen.borrow_mut().refuse = true;
    let n = not_yet(x.call(mesh, None));
    assert_eq!(n.what.as_ref(), "stopped after partial writes: an invented owner refusal");
    assert_eq!((n.missing, n.record), (Missing::NotRead, "toy"));
    assert_eq!(x.get(mesh, "bAnimTreeInitialised"), Value::Bool(false), "kept");

    let mesh = x.dirty_mesh();
    let plain = x.obj("Engine.Actor");
    x.set(mesh, "Owner", Value::Object(Some(plain)));
    let n = not_yet(x.call(mesh, None));
    assert_eq!(n.what, format!("stopped after partial writes: {NO_EVENT}"));
    assert_eq!((n.missing, n.record), (Missing::NotRead, RECORD));
    assert_eq!(x.get(mesh, "AnimTreeTemplate"), Value::Object(None), "kept");
}

/// A component class without the declarations is a data error before any write.
#[test]
fn a_component_without_the_fields_is_a_data_error() {
    let mut x = fx(ParityMode::Off, Some(Registry::Nothing));
    let bare = x.obj("Engine.ActorComponent");
    match set_anim_tree_template(&mut x.vm, bare, None, Registry::Nothing) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("AnimTreeTemplate"), "{message}"),
        other => panic!("expected a data error, got {other:?}"),
    }
}

/// Reference binds nothing: the stub policy answers.
#[test]
fn reference_binds_nothing() {
    let mut x = fx(ParityMode::Reference, None);
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    assert!(matches!(x.call(mesh, None), Err(VmError::UnboundNative(_))));
}
