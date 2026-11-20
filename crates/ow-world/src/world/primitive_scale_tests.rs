//! Tests of `PrimitiveComponent.SetScale` in Off (lane IMPL-SETSCALE) on invented classes, defined by hand with the
//! note's class and field names, and a small invented package for an unloaded handle (AGENTS.md rule 9).
//! AI-assisted (Claude Code, Opus 5.5).

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
const PATH: &str = "Engine.PrimitiveComponent.SetScale";

/// `ToyScale.upk`: a holder whose `Held` names `Thing_0`, an export the store keeps unloaded until it is loaded.
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
    let flags = |s: &mut ObjectSpace| {
        vec![d(s, "bStatic", PropKind::Bool), d(s, "bTickIsDisabled", PropKind::Bool), d(s, "bDeleteMe", PropKind::Bool)]
    };
    let decls = flags(&mut s);
    let actor = s.classes.define("Engine.Actor", Some(object), decls).unwrap();
    // An owner class that declares bDeleteMe as an int.
    let decls = vec![d(&mut s, "bStatic", PropKind::Bool), d(&mut s, "bTickIsDisabled", PropKind::Bool), d(&mut s, "bDeleteMe", PropKind::Int)];
    s.classes.define("Toy.IntDeleteActor", Some(object), decls).unwrap();
    s.classes.define("Toy.BareActor", Some(object), vec![]).unwrap();
    let component = |s: &mut ObjectSpace, scale: PropKind| {
        vec![
            d(s, "Owner", PropKind::Object { class: actor }),
            d(s, "bAttached", PropKind::Bool),
            d(s, "bNeedsUpdateTransform", PropKind::Bool),
            d(s, "Scale", scale),
        ]
    };
    let decls = component(&mut s, PropKind::Float);
    let primitive = s.classes.define("Engine.PrimitiveComponent", Some(object), decls).unwrap();
    let decls = component(&mut s, PropKind::Int);
    s.classes.define("Toy.IntScaleComponent", Some(object), decls).unwrap();
    s.classes.define("Toy.BareComponent", Some(object), vec![]).unwrap();
    let params = vec![d(&mut s, "NewScale", PropKind::Float).with_flags(PropertyDecl::PARAM)];
    let dir = std::env::temp_dir().join(format!("ow-world-scale-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let package = toy_package();
    let defs = DefStore::from_synthetic(&dir, &[("ToyScale.upk", package.as_slice())], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let mut vm = Vm::new(s, defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let f = FunctionDecl::native(ObjectPath::parse(PATH).unwrap(), SET_SCALE).with_owner(primitive).with_params(params);
    vm.declare_function(f).unwrap();
    register(&mut vm).unwrap();
    Fx { vm, dir }
}

/// #518's counters and the name table's size: heap committed writes, generator draws, definition-store load
/// transitions and interned names.
fn marks(vm: &Vm) -> (u64, u64, u64, usize) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
}

impl Fx {
    fn obj(&mut self, path: &str, name: &str) -> ObjRef {
        let c = self.vm.space.classes.by_path(path).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, field: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, field).unwrap();
        *self.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }

    fn get(&self, o: ObjRef, field: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, field).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    /// A primitive component with `Scale` 1, the given owner and `bAttached`.
    fn component(&mut self, owner: Option<ObjRef>, attached: bool) -> ObjRef {
        let c = self.obj("Engine.PrimitiveComponent", "Prim");
        self.set(c, "Scale", Value::Float(1.0));
        self.set(c, "Owner", Value::Object(owner));
        self.set(c, "bAttached", Value::Bool(attached));
        c
    }

    /// An actor with the given `bStatic`, `bTickIsDisabled` and `bDeleteMe`.
    fn actor(&mut self, stat: bool, tick: bool, delete: bool) -> ObjRef {
        let a = self.obj("Engine.Actor", "Act");
        self.set(a, "bStatic", Value::Bool(stat));
        self.set(a, "bTickIsDisabled", Value::Bool(tick));
        self.set(a, "bDeleteMe", Value::Bool(delete));
        a
    }

    /// The toy package's unloaded `Thing_0` handle.
    fn unloaded(&mut self) -> ObjRef {
        let path = ObjectPath::parse("ToyScale.Holder_0").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        let Value::Object(Some(o)) = self.get(holder, "Held") else { panic!("a held object") };
        assert!(self.vm.defs.is_unloaded(&self.vm.space, o));
        o
    }

    fn call_raw(&mut self, c: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH).unwrap();
        self.vm.call(f, c, args).map(|r| r.ret)
    }

    /// The native on `c`. A refusal or a no-change call moves none of the marks.
    fn call(&mut self, c: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let before = marks(&self.vm);
        let r = self.call_raw(Some(c), args);
        assert_eq!(marks(&self.vm), before, "no write, draw, load or name: {r:?}");
        r
    }

    /// The native on `c` when it writes: exactly the two writes, and nothing else moves.
    fn call_writing(&mut self, c: ObjRef, scale: f32) {
        let (w, d, l, n) = marks(&self.vm);
        assert_eq!(self.call_raw(Some(c), vec![Value::Float(scale)]).unwrap(), Value::None);
        assert_eq!(marks(&self.vm), (w + 2, d, l, n), "two writes, nothing drawn, loaded or named");
    }
}

fn not_yet(r: Result<Value, VmError>, words: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert!(n.what.contains(words), "{n}");
            assert_eq!(n.record, RECORD, "{n}");
            assert!(!n.what.starts_with("stopped after partial writes: "), "{n}");
        }
        other => panic!("expected NotYet NotRead, got {other:?}"),
    }
}

fn script_error(r: Result<Value, VmError>, words: &str) {
    match r {
        Err(VmError::Script { message, .. }) => assert!(message.contains(words), "{message}"),
        other => panic!("expected Script, got {other:?}"),
    }
}

/// The same scale does nothing, whatever the transform update would have needed.
#[test]
fn the_same_scale_does_nothing() {
    let mut x = fx();
    let attached = x.component(None, true);
    assert_eq!(x.call(attached, vec![Value::Float(1.0)]).unwrap(), Value::None);
    let fixed = x.actor(true, false, false);
    let owned = x.component(Some(fixed), false);
    x.call(owned, vec![Value::Float(1.0)]).unwrap();
    for c in [attached, owned] {
        assert_eq!(x.get(c, "bNeedsUpdateTransform"), Value::Bool(false), "the flag stays clear");
    }
}

/// A change with no update to ask for writes `Scale`, then sets `bNeedsUpdateTransform`.
#[test]
fn a_change_writes_the_scale_and_the_flag() {
    let mut x = fx();
    let loose = x.component(None, false);
    x.call_writing(loose, 2.5);
    assert_eq!(x.get(loose, "Scale"), Value::Float(2.5));
    assert_eq!(x.get(loose, "bNeedsUpdateTransform"), Value::Bool(true));
    // Owners that ask nothing: a ticking dynamic actor, and fixed actors being deleted.
    for (stat, tick, delete) in [(false, false, false), (true, false, true), (false, true, true)] {
        let a = x.actor(stat, tick, delete);
        let c = x.component(Some(a), true);
        x.call_writing(c, 0.5);
        assert_eq!(x.get(c, "Scale"), Value::Float(0.5), "owner {stat} {tick} {delete}");
        assert_eq!(x.get(c, "bNeedsUpdateTransform"), Value::Bool(true));
    }
    // A NaN is a change (an ordinary float inequality), even against a NaN.
    let nan = x.component(None, false);
    x.set(nan, "Scale", Value::Float(f32::NAN));
    x.call_writing(nan, f32::NAN);
}

/// The transform update's unread virtuals and an unloaded owner refuse before either write.
#[test]
fn an_unread_update_refuses_before_any_write() {
    let mut x = fx();
    let attached = x.component(None, true);
    not_yet(x.call(attached, vec![Value::Float(2.0)]), OWN_UPDATE);
    let mut refused = vec![attached];
    for (stat, tick) in [(true, false), (false, true), (true, true)] {
        let a = x.actor(stat, tick, false);
        let c = x.component(Some(a), false);
        not_yet(x.call(c, vec![Value::Float(2.0)]), OWNER_UPDATE);
        refused.push(c);
    }
    let lazy = x.unloaded();
    let c = x.component(Some(lazy), false);
    not_yet(x.call(c, vec![Value::Float(2.0)]), UNLOADED_OWNER);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "the owner is not loaded");
    // The same scale with an unloaded owner reads no flag of it, and does nothing.
    x.call(c, vec![Value::Float(1.0)]).unwrap();
    refused.push(c);
    for c in refused {
        assert_eq!(x.get(c, "Scale"), Value::Float(1.0), "unchanged");
        assert_eq!(x.get(c, "bNeedsUpdateTransform"), Value::Bool(false), "unchanged");
    }
}

/// The receiver, its residency, the argument and the declarations.
#[test]
fn the_receiver_and_the_shapes() {
    let mut x = fx();
    let before = marks(&x.vm);
    script_error(x.call_raw(None, vec![Value::Float(2.0)]), "no component");
    assert_eq!(marks(&x.vm), before);
    let lazy = x.unloaded();
    not_yet(x.call(lazy, vec![Value::Float(2.0)]), "the component is an unloaded handle");
    let c = x.component(None, false);
    script_error(x.call(c, vec![Value::Int(2)]), "NewScale holds");
    let bare = x.obj("Toy.BareComponent", "Bare");
    script_error(x.call(bare, vec![Value::Float(2.0)]), "declares no Scale");
    let int = x.obj("Toy.IntScaleComponent", "Int");
    script_error(x.call(int, vec![Value::Float(2.0)]), "Scale is not declared as the native reads it");
    // The owner's declarations are checked even when the scale does not change.
    for (class, words) in [("Toy.BareActor", "declares no bStatic"), ("Toy.IntDeleteActor", "bDeleteMe is not declared")] {
        let a = x.obj(class, "Odd");
        let c = x.component(Some(a), false);
        script_error(x.call(c, vec![Value::Float(1.0)]), words);
        script_error(x.call(c, vec![Value::Float(2.0)]), words);
        assert_eq!(x.get(c, "Scale"), Value::Float(1.0), "unchanged");
    }
}
