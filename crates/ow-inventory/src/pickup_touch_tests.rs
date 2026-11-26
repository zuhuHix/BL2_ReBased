//! Tests of `DroppedPickup.TouchPickupTrace` in Off (NATIVE_TOUCH_PICKUP_TRACE.md section 5; lane
//! IMPL-TOUCH-PICKUP-TRACE) on two invented packages built with `ow_package::testkit` (AGENTS.md rule 9). Every call
//! goes through the VM, and each one checks its answer, #518's three counters (`Heap::committed_writes`,
//! `GameRng::draws`, `DefStore::load_transitions`) and the name table's length, all unchanged.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::{StructValue, Value};
use ow_package::testkit::ints;
use ow_package::testkit::reflect::toy_struct;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_world::placement::{Mode, SpawnCollision, Sweep, Test, install_collision};
use ow_world::trace_filter::{HASH_ADMISSION, OWNER_CHAIN_LINK, PRIMITIVE_LINE_TEST, SOURCE_VOLUME_TEST};
use ow_world::touch_trace::{AdmissionStandIn, LEVEL_PART, TOUCH_TRACE, ZERO_LENGTH};

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
/// A script function that calls the native with `None` for both vectors (review r1 of #638, item 2).
const CALL_NONE_FN: &str = "Engine.DroppedPickup.ToyTouchWithNone";
static NEXT: AtomicU32 = AtomicU32::new(0);
const TRACE_FN: &str = "Engine.DroppedPickup.TouchPickupTrace";

const IMPORTS: [&str; 7] =
    ["Class", "Function", "ScriptStruct", "ObjectProperty", "BoolProperty", "FloatProperty", "StructProperty"];

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let vector = toy_struct(&mut t, object, "Vector", None);
    let not_vector = toy_struct(&mut t, object, "ToyNotVector", None);
    for s in [vector, not_vector] {
        for f in ["X", "Y", "Z"] {
            t.prop("Float", s, f, 0, 0);
        }
    }
    let actor = t.class("Actor", object);
    for b in ["bDeleteMe", "bCollideActors", "bWorldGeometry"] {
        t.prop("Bool", actor, b, 0, 0);
    }
    t.prop("Object", actor, "Base", 0, actor);
    // The Owner chains the touch's source exclusion walks (NATIVE_FLOOR_TRACE_FLAGS.md section 4; lane
    // IMPL-FLOOR-TRACE-RULINGS).
    t.prop("Object", actor, "Owner", 0, actor);
    t.prop("Object", actor, "CollisionComponent", 0, object);
    // Declared so the invented struct's layout is registered with the class.
    t.prop("Struct", actor, "ToyNotVectorField", 0, not_vector);
    // Another struct named Vector, with float X, Y and Z, not the declared one (review r1 of #638, item 2).
    let other_vector = toy_struct(&mut t, actor, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", other_vector, f, 0, 0);
    }
    t.prop("Struct", actor, "ToyOtherVectorField", 0, other_vector);
    let volume = t.class("Volume", actor);
    t.prop("Bool", volume, "bPawnsOnly", 0, 0);
    t.class("InterpActor", actor);
    let pawn = t.class("Pawn", actor);
    for b in ["bIgnoreAllTraces", "bStationary"] {
        t.prop("Bool", pawn, b, 0, 0);
    }
    let component = t.class("ActorComponent", object);
    let primitive = t.class("PrimitiveComponent", component);
    for b in ["BulletListener", "BlockZeroExtent", "CollideActors"] {
        t.prop("Bool", primitive, b, 0, 0);
    }
    // A primitive with its own Owner field (review r1 of #638, item 1).
    let owned = t.class("ToyOwnedPrimitive", primitive);
    t.prop("Object", owned, "Owner", 0, actor);
    let dropped = t.class("DroppedPickup", actor);
    let at = |name: &str| Param { type_ref: vector, ..Param::new("Struct", name) };
    let trace = t.function(dropped, "TouchPickupTrace", &[at("StartLoc"), at("EndLoc")], Some(("Bool", 0)), &[], &FunctionSpec::new(NATIVE, "TouchPickupTrace"), |_| None);
    t.function(dropped, "ToyTouchWithNone", &[], None, &[], &FunctionSpec::new(SCRIPT, "ToyTouchWithNone"), |_| {
        let mut a = ow_package::testkit::script::Asm::new();
        a.stmt().final_function(trace.function, |a| {
            a.raw(&[ow_script::ops::op::NO_OBJECT]);
            a.raw(&[ow_script::ops::op::NO_OBJECT]);
        });
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.build()
}

fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let dropped = t.p.add_import("Core", "Class", engine, "DroppedPickup");
    let pickup = t.class("WillowPickup", dropped);
    t.class("ToyPickupSubclass", pickup);
    // An invented game class: no read filter row.
    t.class("ToyGameActor", actor);
    // Two exports the store leaves unloaded, reached through a holder: a pickup and an actor.
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Pickup", 0, pickup);
    t.prop("Object", holder, "Actor", 0, actor);
    let empty = {
        let mut p = ints(&[0]);
        p.extend_from_slice(&t.none);
        p
    };
    let lazy_pickup = t.p.add_export(pickup, "ToyLazyPickup", empty.clone());
    let lazy_actor = t.p.add_export(actor, "ToyLazyActor", empty);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Pickup", lazy_pickup));
    h.extend(t.p.object_tag("Actor", lazy_actor));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

/// An invented collision service: one answer for every level part, and the point sweeps it was asked.
struct Scripted {
    sweep: Sweep,
    asked: RefCell<Vec<([f32; 3], [f32; 3])>>,
}

impl SpawnCollision for Scripted {
    fn box_test(&self, _extent: [f32; 3], _at: [f32; 3], _mode: Mode) -> Result<Test, VmError> {
        Ok(Test::Unsupported)
    }
    fn box_sweep(&self, _extent: [f32; 3], _start: [f32; 3], _end: [f32; 3]) -> Result<Sweep, VmError> {
        Ok(Sweep::Unsupported)
    }
    fn point_sweep(&self, start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError> {
        self.asked.borrow_mut().push((start, end));
        Ok(self.sweep)
    }
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    pickup: ObjRef,
    service: Option<Rc<Scripted>>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const START: [f32; 3] = [100.0, 0.0, 40.0];
const END: [f32; 3] = [0.0, 0.0, 10.0];

/// A world with the actor list (an invented WorldInfo and the pickup), and a collision service answering `sweep`
/// (none when `sweep` is None).
fn fx(sweep: Option<Sweep>) -> Fx {
    fx_mode(ParityMode::Off, sweep)
}

fn fx_mode(mode: ParityMode, sweep: Option<Sweep>) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-touch-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow_game());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    for c in [
        "WillowGame.WillowPickup",
        "WillowGame.ToyPickupSubclass",
        "WillowGame.ToyGameActor",
        "Engine.Volume",
        "Engine.InterpActor",
        "Engine.Pawn",
        "Engine.PrimitiveComponent",
        "Engine.ToyOwnedPrimitive",
    ] {
        vm.find_class(c).unwrap();
    }
    register(&mut vm).unwrap();
    let actor = vm.find_class("Engine.Actor").unwrap();
    let actors = vm.add_service(ow_world::actors::Actors::new(actor)).unwrap();
    let info = make(&mut vm, "Engine.Actor");
    actors.borrow_mut().place_world_info(info).unwrap();
    let pickup = make(&mut vm, "WillowGame.WillowPickup");
    actors.borrow_mut().place(pickup).unwrap();
    let service = sweep.map(|sweep| Rc::new(Scripted { sweep, asked: RefCell::new(Vec::new()) }));
    if let Some(s) = &service {
        install_collision(&mut vm, s.clone()).unwrap();
    }
    Fx { vm, dir, pickup, service }
}

fn make(vm: &mut Vm, class: &str) -> ObjRef {
    let c = vm.find_class(class).unwrap();
    ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
}

fn set(vm: &mut Vm, o: ObjRef, name: &str, v: Value) {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.write(o, PropAddr::scalar(p), v).unwrap();
}

fn get(vm: &Vm, o: ObjRef, name: &str) -> Value {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
}

/// What a call must leave unchanged: #518's three counters and the name table's length.
fn marks(vm: &Vm) -> (u64, u64, u64, usize) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), (vm.defs.load_transitions() + vm.class_code_loads()), vm.space.names.len())
}

impl Fx {
    /// A vector of the struct named `name` (`Vector` or the invented `ToyNotVector`).
    fn vector_of(&self, name: &str, v: [f32; 3]) -> Value {
        let classes = &self.vm.space.classes;
        let ty = classes.struct_by_path(&format!("Engine.Object.{name}")).expect("the struct");
        let layout = classes.struct_layout(ty).unwrap();
        let mut fields = vec![Value::Float(0.0); 3];
        for (axis, x) in ["X", "Y", "Z"].into_iter().zip(v) {
            let (i, _) = layout.field(self.vm.space.names.find(axis).unwrap()).unwrap();
            fields[i as usize] = Value::Float(x);
        }
        Value::Struct(Rc::new(StructValue { ty, fields }))
    }

    fn call_with(&mut self, this: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(TRACE_FN)?;
        let start = marks(&self.vm);
        let r = self.vm.call(f, this, args).map(|r| r.ret);
        assert_eq!(marks(&self.vm), start, "#518's counters and the name table moved: {r:?}");
        r
    }

    fn trace(&mut self, this: ObjRef, start: [f32; 3], end: [f32; 3]) -> Result<Value, VmError> {
        let args = vec![self.vector_of("Vector", start), self.vector_of("Vector", end)];
        self.call_with(Some(this), args)
    }

    fn call(&mut self) -> Result<Value, VmError> {
        let p = self.pickup;
        self.trace(p, START, END)
    }

    /// A live actor of `class` with a primitive whose read flags are all set (the actor's `bCollideActors` too).
    fn blocker(&mut self, class: &str) -> (ObjRef, ObjRef) {
        let a = make(&mut self.vm, class);
        let c = make(&mut self.vm, "Engine.PrimitiveComponent");
        for f in ["BlockZeroExtent", "CollideActors"] {
            set(&mut self.vm, c, f, Value::Bool(true));
        }
        set(&mut self.vm, a, "bCollideActors", Value::Bool(true));
        set(&mut self.vm, a, "CollisionComponent", Value::Object(Some(c)));
        ow_world::actors::actors_of(&self.vm).unwrap().borrow_mut().place(a).unwrap();
        (a, c)
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        set(&mut self.vm, o, name, v);
    }

    /// The holder's unloaded handle in `field`.
    fn lazy(&mut self, field: &str) -> ObjRef {
        let holder =
            self.vm.defs.find(&mut self.vm.space, &ObjectPath::parse("WillowGame.ToyHolderObject").unwrap()).unwrap().unwrap();
        let Value::Object(Some(o)) = get(&self.vm, holder, field) else { panic!("the holder's {field}") };
        assert!(self.vm.defs.is_unloaded(&self.vm.space, o), "an unloaded handle");
        o
    }
}

fn not_yet(r: Result<Value, VmError>) -> ow_core::NotYet {
    match r {
        Err(VmError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn not_read(r: Result<Value, VmError>) -> String {
    let n = not_yet(r);
    assert_eq!(n.missing, Missing::NotRead, "{n}");
    n.what.into_owned()
}

fn script(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::Script { message, .. }) => message,
        other => panic!("expected a data error, got {other:?}"),
    }
}

/// Test 1: no receiver and a non-vector argument are data errors; a receiver of another exact class, `Actor` or a
/// `WillowPickup` subclass, is `NotRead` naming the class.
#[test]
fn the_receiver_its_class_and_the_arguments() {
    let mut x = fx(Some(Sweep::Clear));
    let args = vec![x.vector_of("Vector", START), x.vector_of("Vector", END)];
    assert!(script(x.call_with(None, args)).contains("without a pickup"));
    for class in ["Engine.Actor", "WillowGame.ToyPickupSubclass"] {
        let other = make(&mut x.vm, class);
        let what = not_read(x.trace(other, START, END));
        assert_eq!(what, format!("{TOUCH_TRACE_OF} {class}"));
    }
    let p = x.pickup;
    let args = vec![x.vector_of("ToyNotVector", START), x.vector_of("Vector", END)];
    assert!(script(x.call_with(Some(p), args)).contains("not a Vector"));
    assert!(x.service.as_ref().unwrap().asked.borrow().is_empty(), "nothing asked of the service");
}

/// Test 2: an unloaded `WillowPickup` handle is `NotRead`, and stays unloaded.
#[test]
fn an_unloaded_pickup_is_not_read_and_stays_unloaded() {
    let mut x = fx(Some(Sweep::Clear));
    let lazy = x.lazy("Pickup");
    assert_eq!(not_read(x.trace(lazy, START, END)), TOUCH_UNLOADED);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "still unloaded");
}

/// Test 3: no collision service is the spawn placement's layer, never "clear".
#[test]
fn no_collision_service_is_layer_absent() {
    let mut x = fx(None);
    let n = not_yet(x.call());
    assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
    assert_eq!(n.what.as_ref(), TOUCH_TRACE);
}

/// Test 4: a zero-length segment is `NotRead`.
#[test]
fn a_zero_length_segment_is_not_read() {
    let mut x = fx(Some(Sweep::Clear));
    let p = x.pickup;
    assert_eq!(not_read(x.trace(p, END, END)), ZERO_LENGTH);
}

/// Test 5: a level hit answers false; the service was asked from `EndLoc` to `StartLoc`, and no actor was read (a
/// live primitive of a class with no row, which the walk would refuse, is there).
#[test]
fn a_level_hit_is_blocked_and_reads_no_actor() {
    let mut x = fx(Some(Sweep::Hit { location: [50.0, 0.0, 25.0], normal: [1.0, 0.0, 0.0], fraction: 0.5 }));
    x.blocker("WillowGame.ToyGameActor");
    assert_eq!(x.call().unwrap(), Value::Bool(false));
    assert_eq!(*x.service.as_ref().unwrap().asked.borrow(), vec![(END, START)], "from the touch point to the toucher");
}

/// Test 6: a level part the service cannot answer is `NotRead` naming it.
#[test]
fn an_unsupported_level_part_is_not_read() {
    let mut x = fx(Some(Sweep::Unsupported));
    assert_eq!(not_read(x.call()), LEVEL_PART);
}

/// Test 7: a clear level with no live primitive is a clear touch.
#[test]
fn a_clear_level_with_no_primitive_is_clear() {
    let mut x = fx(Some(Sweep::Clear));
    assert_eq!(x.call().unwrap(), Value::Bool(true));
}

/// Test 8: the toucher pawn's primitive, the pickup's own, that of an actor in the pickup's `Owner` chain and that of
/// an actor the pickup owns (invented classes with no row, so they would refuse if tested) are not tested (the hub's
/// ruling 3 on #809, NATIVE_FLOOR_TRACE_FLAGS.md section 8, row 7).
#[test]
fn the_toucher_the_pickup_and_its_owner_chains_never_block() {
    let mut x = fx(Some(Sweep::Clear));
    x.blocker("Engine.Pawn");
    let own = make(&mut x.vm, "Engine.PrimitiveComponent");
    for f in ["BlockZeroExtent", "CollideActors"] {
        x.set(own, f, Value::Bool(true));
    }
    let p = x.pickup;
    x.set(p, "bCollideActors", Value::Bool(true));
    x.set(p, "CollisionComponent", Value::Object(Some(own)));
    let (up, _) = x.blocker("WillowGame.ToyGameActor");
    let (up_of_up, _) = x.blocker("WillowGame.ToyGameActor");
    x.set(p, "Owner", Value::Object(Some(up)));
    x.set(up, "Owner", Value::Object(Some(up_of_up)));
    let (owned, _) = x.blocker("WillowGame.ToyGameActor");
    x.set(owned, "Owner", Value::Object(Some(p)));
    assert_eq!(x.call().unwrap(), Value::Bool(true));
}

/// Section 8, row 7: the pickup's `Base` alone is no longer passed over (the chain is `Owner`, not `Base`).
#[test]
fn the_pickup_s_base_alone_is_tested() {
    let mut x = fx(Some(Sweep::Clear));
    let (base, _) = x.blocker("WillowGame.ToyGameActor");
    let p = x.pickup;
    x.set(p, "Base", Value::Object(Some(base)));
    assert!(not_read(x.call()).contains("the trace filter of WillowGame.ToyGameActor"));
}

/// Test 9: a non-world-geometry `Actor`'s primitive and a `Volume` without `bWorldGeometry` are not tested (bits 16
/// and 8 are not in the touch's set).
#[test]
fn other_actors_and_plain_volumes_are_not_tested() {
    let mut x = fx(Some(Sweep::Clear));
    x.blocker("Engine.Actor");
    x.blocker("Engine.Volume");
    assert_eq!(x.call().unwrap(), Value::Bool(true));
}

/// Test 10: a world-geometry `Actor`'s primitive and a mover's reach the hash admission first; a `bPawnsOnly` volume
/// is the source actor's volume test.
#[test]
fn world_geometry_and_movers_reach_the_hash_admission() {
    let mut x = fx(Some(Sweep::Clear));
    let (a, _) = x.blocker("Engine.Actor");
    x.set(a, "bWorldGeometry", Value::Bool(true));
    assert!(not_read(x.call()).contains(HASH_ADMISSION));
    let mut x = fx(Some(Sweep::Clear));
    x.blocker("Engine.InterpActor");
    let what = not_read(x.call());
    assert!(what.starts_with(TOUCH_TRACE) && what.contains(HASH_ADMISSION), "{what}");
    let mut x = fx(Some(Sweep::Clear));
    let (v, _) = x.blocker("Engine.Volume");
    x.set(v, "bPawnsOnly", Value::Bool(true));
    assert!(not_read(x.call()).contains(SOURCE_VOLUME_TEST));
}

/// Test 10a: with the hash stand-in installed (admission established), the same world-geometry primitive names its
/// own line test.
#[test]
fn with_admission_established_the_primitives_line_test_is_named() {
    let mut x = fx(Some(Sweep::Clear));
    x.vm.add_service(AdmissionStandIn).unwrap();
    let (a, _) = x.blocker("Engine.Actor");
    x.set(a, "bWorldGeometry", Value::Bool(true));
    let what = not_read(x.call());
    assert!(what.contains(PRIMITIVE_LINE_TEST) && !what.contains(HASH_ADMISSION), "{what}");
}

/// Test 10b: an `Owner` the store has not loaded, needed to decide a candidate, is `NotRead` and stays unloaded; an
/// `Owner` cycle ends the walk, with no candidate (true) and with one outside the chain (its own refusal).
#[test]
fn the_owner_chain_is_read_load_free_and_bounded() {
    let mut x = fx(Some(Sweep::Clear));
    let lazy = x.lazy("Actor");
    let p = x.pickup;
    x.set(p, "Owner", Value::Object(Some(lazy)));
    x.blocker("WillowGame.ToyGameActor");
    assert!(not_read(x.call()).contains(OWNER_CHAIN_LINK));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "the link is still unloaded");
    // With no candidate the chain is never needed.
    let mut x = fx(Some(Sweep::Clear));
    let lazy = x.lazy("Actor");
    let p = x.pickup;
    x.set(p, "Owner", Value::Object(Some(lazy)));
    assert_eq!(x.call().unwrap(), Value::Bool(true));
    // A cycle: the pickup owned by A, A owned by the pickup.
    let mut x = fx(Some(Sweep::Clear));
    let a = make(&mut x.vm, "Engine.Actor");
    let p = x.pickup;
    x.set(p, "Owner", Value::Object(Some(a)));
    x.set(a, "Owner", Value::Object(Some(p)));
    assert_eq!(x.call().unwrap(), Value::Bool(true));
    x.blocker("WillowGame.ToyGameActor");
    assert!(not_read(x.call()).contains("the trace filter of WillowGame.ToyGameActor"), "the walk ended, then the row");
}

/// Test 11: a primitive of an invented game class with no row is `NotRead`, "the trace filter of <class>".
#[test]
fn a_game_class_with_no_row_is_not_read() {
    let mut x = fx(Some(Sweep::Clear));
    x.blocker("WillowGame.ToyGameActor");
    let n = not_yet(x.call());
    assert_eq!(n.missing, Missing::NotRead);
    assert!(n.what.contains("the trace filter of WillowGame.ToyGameActor"), "{n}");
    assert_eq!(n.record, "NATIVE_SPAWN_PLACEMENT.md");
}

/// Test 12: a primitive with `BlockZeroExtent` clear is not tested, whatever its owner's class.
#[test]
fn a_primitive_that_blocks_no_zero_extent_is_not_tested() {
    let mut x = fx(Some(Sweep::Clear));
    let (_, c) = x.blocker("WillowGame.ToyGameActor");
    x.set(c, "BlockZeroExtent", Value::Bool(false));
    assert_eq!(x.call().unwrap(), Value::Bool(true));
}

/// Test 13: Reference binds nothing (the stub policy).
#[test]
fn reference_binds_nothing() {
    let x = fx_mode(ParityMode::Reference, Some(Sweep::Clear));
    assert!(!x.vm.has_native(TOUCH_PICKUP_TRACE));
}

/// Review r1 of #638, item 1 (with the `Owner` chains since the hub's ruling 3 on #809): the chains are decided before
/// the owner's flags are read, and an unloaded link refuses only a primitive the read filters leave as a possible
/// blocker.
#[test]
fn an_unloaded_owner_link_refuses_only_a_possible_blocker() {
    // Only a pawn's primitive: the Pawn row rejects it, so the undecided chain does not refuse.
    let mut x = fx(Some(Sweep::Clear));
    let lazy = x.lazy("Actor");
    let p = x.pickup;
    x.set(p, "Owner", Value::Object(Some(lazy)));
    x.blocker("Engine.Pawn");
    assert_eq!(x.call().unwrap(), Value::Bool(true));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "the link is still unloaded");
    // A primitive whose own Owner is the pickup's unloaded Owner: recognised as the chain's link before its flags are
    // read.
    let mut x = fx(Some(Sweep::Clear));
    let lazy = x.lazy("Actor");
    let p = x.pickup;
    x.set(p, "Owner", Value::Object(Some(lazy)));
    let (walked, _) = x.blocker("WillowGame.ToyGameActor");
    let owned = make(&mut x.vm, "Engine.ToyOwnedPrimitive");
    for f in ["BlockZeroExtent", "CollideActors"] {
        x.set(owned, f, Value::Bool(true));
    }
    x.set(owned, "Owner", Value::Object(Some(lazy)));
    x.set(walked, "CollisionComponent", Value::Object(Some(owned)));
    assert_eq!(x.call().unwrap(), Value::Bool(true));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "the link is still unloaded");
    // Review r2 of #638, item 1: a primitive whose own CollideActors is clear is no candidate, even with an unloaded
    // owner and the pickup's Owner on a different unloaded handle; the call checks the counters and the name table.
    let mut x = fx(Some(Sweep::Clear));
    let base = x.lazy("Actor");
    let owner = x.lazy("Pickup");
    let p = x.pickup;
    x.set(p, "Owner", Value::Object(Some(base)));
    let (walked, _) = x.blocker("WillowGame.ToyGameActor");
    let owned = make(&mut x.vm, "Engine.ToyOwnedPrimitive");
    x.set(owned, "BlockZeroExtent", Value::Bool(true));
    x.set(owned, "CollideActors", Value::Bool(false));
    x.set(owned, "Owner", Value::Object(Some(owner)));
    x.set(walked, "CollisionComponent", Value::Object(Some(owned)));
    assert_eq!(x.call().unwrap(), Value::Bool(true));
    for o in [base, owner] {
        assert!(x.vm.defs.is_unloaded(&x.vm.space, o), "still unloaded");
    }
    // The same primitive with CollideActors set is a possible blocker behind an undecided chain: it refuses.
    x.set(owned, "CollideActors", Value::Bool(true));
    let n = not_yet(x.call());
    assert!(n.what.contains(OWNER_CHAIN_LINK), "{n}");
}

/// Review r1 of #638, item 2: an argument must be the declared `Vector` struct. Another struct named `Vector` is a
/// data error, and so is a script `None`, which the interpreter hands over unchanged for this native in Off.
#[test]
fn only_the_declared_vector_passes() {
    let mut x = fx(Some(Sweep::Clear));
    let p = x.pickup;
    let ty = x.vm.space.classes.struct_by_path("Engine.Actor.Vector").expect("the other Vector");
    let other = Value::Struct(Rc::new(StructValue { ty, fields: vec![Value::Float(1.0); 3] }));
    let args = vec![other, x.vector_of("Vector", END)];
    assert!(script(x.call_with(Some(p), args)).contains("argument 0 is not a Vector"));
    // From script: TouchPickupTrace(None, None).
    let f = x.vm.find_function(CALL_NONE_FN).unwrap();
    let start = marks(&x.vm);
    let r = x.vm.call(f, Some(p), vec![]).map(|r| r.ret);
    assert_eq!(marks(&x.vm), start, "nothing moved: {r:?}");
    assert!(script(r).contains("not a Vector"));
    assert!(x.service.as_ref().unwrap().asked.borrow().is_empty(), "nothing reached the collision service");
    // Review r2 of #638, item 2: the exemption holds in Off only, checked when it is applied (the interpreter's
    // conversion itself is tested in ow-vm, `a_none_given_to_a_natives_struct_parameter_is_the_zero_struct`).
    assert!(x.vm.keeps_struct_none(TOUCH_PICKUP_TRACE));
    x.vm.parity = ParityMode::Reference;
    assert!(!x.vm.keeps_struct_none(TOUCH_PICKUP_TRACE), "not after a switch to Reference");
}

// ---- NATIVE_TOUCH_PICKUP_TRACE.md section 3.1.2 (lane IMPL-PICKUP-TRACE-ROWS): the pickup classes' filter row ----------

/// Tests 0 and 4: a pickup class with no row still refuses, "the trace filter of <class>". That is the refusal a second
/// `WillowPickup` gave before the rows (main's slice gate recorded it for link 7). With the rows in place, that state
/// is no longer reachable, so a `WillowPickup` subclass, still unread, stands in for it.
#[test]
fn a_pickup_subclass_still_refuses_as_unread() {
    let mut x = fx(Some(Sweep::Clear));
    x.blocker("WillowGame.ToyPickupSubclass");
    let n = not_yet(x.call());
    assert_eq!(n.missing, Missing::NotRead);
    assert!(n.what.contains("the trace filter of WillowGame.ToyPickupSubclass"), "{n}");
}

/// Tests 1 and 3: a second `WillowPickup` or `DroppedPickup` on the segment, not the source, with `bWorldGeometry`
/// clear, is not tested: the touch answers true.
#[test]
fn a_second_pickup_without_world_geometry_is_not_tested() {
    for class in ["WillowGame.WillowPickup", "Engine.DroppedPickup"] {
        let mut x = fx(Some(Sweep::Clear));
        x.blocker(class);
        assert_eq!(x.call().unwrap(), Value::Bool(true), "{class}");
    }
}

/// Test 2: the second pickup with `bWorldGeometry` set is tested by the shortcut: the hash admission, and with the
/// admission stand-in, its primitive's line test.
#[test]
fn a_second_pickup_with_world_geometry_is_tested() {
    let mut x = fx(Some(Sweep::Clear));
    let (a, _) = x.blocker("WillowGame.WillowPickup");
    x.set(a, "bWorldGeometry", Value::Bool(true));
    assert!(not_read(x.call()).contains(HASH_ADMISSION));
    let mut x = fx(Some(Sweep::Clear));
    x.vm.add_service(AdmissionStandIn).unwrap();
    let (a, _) = x.blocker("WillowGame.WillowPickup");
    x.set(a, "bWorldGeometry", Value::Bool(true));
    let what = not_read(x.call());
    assert!(what.contains(PRIMITIVE_LINE_TEST) && !what.contains(HASH_ADMISSION), "{what}");
}

/// Test 6: the source pickup's own blocking cylinder on its own line is excluded before any row is read: no refusal,
/// even with its `bWorldGeometry` set.
#[test]
fn the_source_pickup_s_own_cylinder_is_excluded() {
    let mut x = fx(Some(Sweep::Clear));
    let c = make(&mut x.vm, "Engine.PrimitiveComponent");
    for f in ["BlockZeroExtent", "CollideActors"] {
        x.set(c, f, Value::Bool(true));
    }
    let p = x.pickup;
    x.set(p, "bCollideActors", Value::Bool(true));
    x.set(p, "bWorldGeometry", Value::Bool(true));
    x.set(p, "CollisionComponent", Value::Object(Some(c)));
    assert_eq!(x.call().unwrap(), Value::Bool(true));
}
