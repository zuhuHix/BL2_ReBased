//! Tests of `IsStaggered` in Off (lane IMPL-IS-STAGGERED, NATIVE_IS_STAGGERED.md section 3) on invented packages with
//! the game's class and field names (AGENTS.md rule 9). Every value is invented. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::toy_struct;
use ow_package::testkit::script::{FunctionSpec, ScriptToy};
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE_FLAGS: u32 = 0x400 | 0x20000;

/// The shape of `WillowAIPawn.StaggerState` in the fixture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    /// `StaggerStateData { bIsStaggered: bool, bForced: bool }`.
    Good,
    /// No `StaggerState` declared.
    Missing,
    /// `StaggerState` an int.
    NotAStruct,
    /// `StaggerStateData` whose `bIsStaggered` is an int.
    NotABool,
    /// `StaggerState` of another struct type, `ToyOtherStateData { bIsStaggered: bool }`.
    OtherStruct,
}

/// `WillowGame.upk` (invented): `WillowPawn` (native `IsStaggered`, an object field `ToyLink`), `WillowAIPawn` (its
/// own native `IsStaggered`, `StaggerState` as `shape` says), a script child `ToyAIPawnChild`, an invented child
/// `ToyNativeAIPawn` (listed as native by the test that needs it), `WillowPlayerPawn`, the script children
/// `ToyOtherPawn` and `ToyPawnChild` of `WillowPawn`, and `ToyThing`, not a pawn.
fn willow(shape: Shape) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "BoolProperty",
        "IntProperty",
        "ObjectProperty",
        "StructProperty",
        "ScriptStruct",
    ]);
    let object = t.class("Object", 0);
    let pawn = t.class("WillowPawn", object);
    t.prop("Object", pawn, "ToyLink", 0, object);
    // A stagger-like field on the plain WillowPawn (test 3): the plain class still answers false.
    t.prop("Bool", pawn, "bIsStaggered", 0, 0);
    let ai = t.class("WillowAIPawn", pawn);
    match shape {
        Shape::Missing => {}
        Shape::NotAStruct => {
            t.prop("Int", ai, "StaggerState", 0, 0);
        }
        Shape::OtherStruct => {
            let state = toy_struct(&mut t, ai, "ToyOtherStateData", None);
            t.prop("Bool", state, "bIsStaggered", 0, 0);
            t.prop("Struct", ai, "StaggerState", 0, state);
        }
        Shape::Good | Shape::NotABool => {
            let state = toy_struct(&mut t, ai, "StaggerStateData", None);
            t.prop(if shape == Shape::Good { "Bool" } else { "Int" }, state, "bIsStaggered", 0, 0);
            t.prop("Bool", state, "bForced", 0, 0);
            t.prop("Struct", ai, "StaggerState", 0, state);
        }
    }
    for class in [pawn, ai] {
        let spec = FunctionSpec::new(NATIVE_FLAGS, "IsStaggered");
        t.function(class, "IsStaggered", &[], Some(("Bool", 0)), &[], &spec, |_| None);
    }
    t.class("ToyAIPawnChild", ai);
    t.class("ToyNativeAIPawn", ai);
    t.class("WillowPlayerPawn", pawn);
    t.class("ToyOtherPawn", pawn);
    t.class("ToyPawnChild", pawn);
    t.class("ToyThing", object);
    t.p.build()
}

/// `GD_ToyPawns.upk` (invented): `Holder`, a `WillowPawn` whose `ToyLink` names `Peer`, a `WillowAIPawn` export, so
/// reading the loaded holder gives `Peer` as an unloaded store handle.
fn pawns() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
    let pawn = t.p.add_import("Core", "Class", willow, "WillowPawn");
    let ai = t.p.add_import("Core", "Class", willow, "WillowAIPawn");
    let mut body = ow_package::testkit::ints(&[0]);
    body.extend_from_slice(&t.none);
    let peer = t.p.add_export(ai, "Peer", body);
    let mut holder = ow_package::testkit::ints(&[0]);
    holder.extend(t.p.tag("ToyLink", "ObjectProperty", &ow_package::testkit::ints(&[peer])));
    holder.extend_from_slice(&t.none);
    t.p.add_export(pawn, "Holder", holder);
    t.p.build()
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

fn fx_with(mode: ParityMode, shape: Shape) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-combat-stagger-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (w, p) = (willow(shape), pawns());
    let files: [(&str, &[u8]); 2] = [("WillowGame.upk", &w), ("GD_ToyPawns.upk", &p)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    // The classes are loaded by the fixture, so no answer below is the first to load one.
    for c in [
        "WillowPawn",
        "WillowAIPawn",
        "ToyAIPawnChild",
        "ToyNativeAIPawn",
        "WillowPlayerPawn",
        "ToyOtherPawn",
        "ToyPawnChild",
        "ToyThing",
    ] {
        vm.find_class(&format!("WillowGame.{c}")).unwrap();
    }
    Fx { vm, dir }
}

fn fx() -> Fx {
    fx_with(ParityMode::Off, Shape::Good)
}

impl Fx {
    fn pawn(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(&format!("WillowGame.{class}")).unwrap();
        self.vm.space.alloc(c, "ToyPawn", None, Origin::Runtime).unwrap()
    }

    fn set_staggered(&mut self, pawn: ObjRef, staggered: bool) {
        let class = self.vm.space.heap.get(pawn).unwrap().class;
        let p = self.vm.space.prop(class, "StaggerState").unwrap();
        let mut v = self.vm.space.read(pawn, PropAddr::scalar(p)).unwrap().clone();
        assert!(super::super::access::set_field(&self.vm, &mut v, "bIsStaggered", Value::Bool(staggered)));
        self.vm.space.write(pawn, PropAddr::scalar(p), v).unwrap();
    }

    /// `IsStaggered` declared on `class`, called from the host on `pawn`.
    fn call(&mut self, class: &str, pawn: ObjRef) -> Result<Value, VmError> {
        let f = self.vm.find_function(&format!("WillowGame.{class}.IsStaggered")).unwrap();
        Ok(self.vm.call(f, Some(pawn), vec![])?.ret)
    }

    /// #518's three counters.
    fn counters(&self) -> (u64, u64, u64) {
        (self.vm.space.heap.committed_writes(), self.vm.defs.load_transitions(), self.vm.rng.draws())
    }

    /// The `Peer` export, an unloaded handle read from the loaded holder.
    fn unloaded_pawn(&mut self) -> ObjRef {
        let path = ObjectPath::parse("GD_ToyPawns.Holder").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        let class = self.vm.space.heap.get(holder).unwrap().class;
        let p = self.vm.space.prop(class, "ToyLink").unwrap();
        let Value::Object(Some(peer)) = self.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else {
            panic!("the holder names its peer")
        };
        assert!(self.vm.defs.is_unloaded(&self.vm.space, peer), "a genuinely unloaded handle");
        peer
    }

    /// The answer for `pawn` must be `NotYet { NotRead }` containing `what`, with the counters unchanged.
    fn refuses(&self, pawn: ObjRef, what: &str) {
        self.refuses_with(pawn, &NATIVE_AI_PAWN_DESCENDANTS, what);
    }

    fn refuses_with(&self, pawn: ObjRef, native: &[&str], what: &str) {
        let before = self.counters();
        match is_staggered_with(&self.vm, pawn, native) {
            Err(VmError::NotYet(n)) => {
                assert_eq!((n.missing, n.record), (Missing::NotRead, RECORD_STAGGER), "{n}");
                assert!(n.what.contains(what), "{n}");
            }
            other => panic!("expected the NotYet {what:?}, got {other:?}"),
        }
        assert_eq!(self.counters(), before, "a refusal writes, loads and draws nothing");
    }
}

/// A `WillowAIPawn` answers its `StaggerState.bIsStaggered`, clear and set, through both declarations; a script child
/// of it takes the same rule; a plain `WillowPawn` answers false.
#[test]
fn the_ai_rule_reads_the_stagger_bool_and_a_plain_willow_pawn_is_false() {
    let mut x = fx();
    register(&mut x.vm).unwrap();
    for class in ["WillowAIPawn", "ToyAIPawnChild"] {
        let pawn = x.pawn(class);
        let before = x.counters();
        assert_eq!(x.call("WillowAIPawn", pawn).unwrap(), Value::Bool(false), "{class}: clear");
        assert_eq!(x.counters(), before, "{class}: nothing written, loaded or drawn");
        x.set_staggered(pawn, true);
        assert_eq!(x.call("WillowAIPawn", pawn).unwrap(), Value::Bool(true), "{class}: set");
        assert_eq!(x.call("WillowPawn", pawn).unwrap(), Value::Bool(true), "{class}: through WillowPawn's key");
    }
    let plain = x.pawn("WillowPawn");
    let p = x.vm.space.prop(x.vm.space.heap.get(plain).unwrap().class, "bIsStaggered").unwrap();
    x.vm.space.write(plain, PropAddr::scalar(p), Value::Bool(true)).unwrap();
    assert_eq!(x.call("WillowPawn", plain).unwrap(), Value::Bool(false), "a stagger-like field changes nothing");
}

/// S2: `WillowPlayerPawn`, another `WillowPawn` subclass and a script-only child of `WillowPawn`, whose virtual was not
/// read (a script-only child of `WillowPawn` is not assumed to answer false).
#[test]
fn another_pawn_class_is_s2() {
    let mut x = fx();
    for class in ["WillowPlayerPawn", "ToyOtherPawn", "ToyPawnChild"] {
        let pawn = x.pawn(class);
        x.refuses(pawn, &format!("the stagger rule of WillowGame.{class}"));
    }
}

/// S2: a native descendant of `WillowAIPawn` (an invented class, listed as native for the test) holds its own slot,
/// which was not read; unlisted, the same class is a script-only child and takes the AI rule.
#[test]
fn a_native_descendant_of_willow_ai_pawn_is_s2() {
    let mut x = fx();
    let pawn = x.pawn("ToyNativeAIPawn");
    x.refuses_with(pawn, &["WillowGame.ToyNativeAIPawn"], "the stagger rule of WillowGame.ToyNativeAIPawn");
    assert!(!is_staggered_with(&x.vm, pawn, &[]).unwrap());
    assert_eq!(NATIVE_AI_PAWN_DESCENDANTS, ["WillowGame.WillowAICranePawn"], "the list carried with the binding");
}

/// A receiver outside `WillowPawn` is a `VmError::Script`; so is a call with no receiver.
#[test]
fn a_receiver_outside_willow_pawn_is_a_script_error() {
    let mut x = fx();
    let thing = x.pawn("ToyThing");
    match is_staggered(&x.vm, thing) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("is not a WillowPawn"), "{message}"),
        other => panic!("expected a script error, got {other:?}"),
    }
    register(&mut x.vm).unwrap();
    let f = x.vm.find_function("WillowGame.WillowAIPawn.IsStaggered").unwrap();
    assert!(matches!(x.vm.call(f, None, vec![]), Err(VmError::Script { .. })));
}

/// S1, two texts: an unloaded pawn handle and a slotless stand-in, refused with nothing loaded.
#[test]
fn an_unloaded_pawn_is_s1() {
    let mut x = fx();
    let peer = x.unloaded_pawn();
    x.refuses(peer, "the pawn WillowGame.WillowAIPawn Peer is an unloaded handle (a load is a write)");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, peer), "still unloaded");
    let class = x.vm.find_class("WillowGame.WillowAIPawn").unwrap();
    let stand_in = x.vm.space.alloc_stand_in(class, "ToyStandIn", Origin::Resource { package: 0, index: -1 }).unwrap();
    x.refuses(
        stand_in,
        "the pawn WillowGame.WillowAIPawn ToyStandIn is a stand-in with no properties (an object the store cannot load)",
    );
}

/// The wrong shape, three cases, each a `VmError::Script` naming the field.
#[test]
fn a_wrong_stagger_state_is_a_script_error() {
    for (shape, wanted) in [
        (Shape::Missing, "StaggerState is not declared"),
        (Shape::NotAStruct, "StaggerState is not a struct"),
        (Shape::NotABool, "not a bool"),
        (Shape::OtherStruct, "is not a WillowGame.WillowAIPawn.StaggerStateData"),
    ] {
        let x = fx_with(ParityMode::Off, shape);
        let mut x = x;
        let pawn = x.pawn("WillowAIPawn");
        match is_staggered(&x.vm, pawn) {
            Err(VmError::Script { message, .. }) => assert!(message.contains(wanted), "{shape:?}: {message}"),
            other => panic!("{shape:?}: expected a script error, got {other:?}"),
        }
    }
}

/// Both registration orders: a key bound before the pack keeps that binding, and a keep-existing bind after the pack
/// keeps the pack's. Under Reference the pack binds nothing.
#[test]
fn both_registration_orders_and_reference() {
    let stand_in = || Rc::new(|_c: &mut NativeCall<'_>| Ok(Value::Bool(true)));
    // Before: a stand-in on WillowPawn's key survives the pack; the AI key gets the rule.
    let mut x = fx();
    let f = x.vm.find_function("WillowGame.WillowPawn.IsStaggered").unwrap();
    let key = x.vm.function(f).unwrap().native_key.clone().unwrap();
    x.vm.register_native(&key, stand_in()).unwrap();
    register(&mut x.vm).unwrap();
    let plain = x.pawn("WillowPawn");
    assert_eq!(x.call("WillowPawn", plain).unwrap(), Value::Bool(true), "kept");
    let ai = x.pawn("WillowAIPawn");
    assert_eq!(x.call("WillowAIPawn", ai).unwrap(), Value::Bool(false), "the rule");
    // After: a keep-existing bind leaves the pack's rule in place.
    let mut x = fx();
    register(&mut x.vm).unwrap();
    super::super::bind_fn(&mut x.vm, "WillowGame.WillowAIPawn.IsStaggered", |_c| Ok(Value::Bool(true))).unwrap();
    let ai = x.pawn("WillowAIPawn");
    assert_eq!(x.call("WillowAIPawn", ai).unwrap(), Value::Bool(false), "the rule kept");
    // Reference: nothing bound.
    let mut x = fx_with(ParityMode::Reference, Shape::Good);
    register(&mut x.vm).unwrap();
    for path in IS_STAGGERED {
        let f = x.vm.find_function(path).unwrap();
        assert!(!x.vm.is_bound(f), "{path}: unbound under Reference");
    }
}
