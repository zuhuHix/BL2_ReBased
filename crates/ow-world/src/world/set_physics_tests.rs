//! Tests of `Actor.SetPhysics` in Off (lane IMPL-ACTOR-SETPHYSICS, NATIVE_ACTOR_SET_PHYSICS.md section 4) on three
//! invented packages built with `ow_package::testkit` (AGENTS.md rule 9):
//! - `Engine`: `Actor` (`Physics`, `Role` with an `ENetRole` enum, `Base`, `PhysicsVolume`, `CollisionComponent`, the
//!   AI flags, the native `SetPhysics(byte)`), `Pawn` (`bForceFloorCheck`), `PhysicsVolume` (a spy
//!   `PhysicsChangedFor(Actor)`) and two volume lookalikes;
//! - `GearboxFramework`: `GearboxPawn`, and `ToyNativePawn`, a native class below it whose physics change was not read;
//! - `WillowGame`: `WillowPawn` (the four notify flags and spy events), `WillowAIPawn`, `WillowPlayerPawn`, and three
//!   script-only subclasses with decodable, non-native class bodies (`ToyAIPawn`, `ToyWillowPawn`, `ToyPlainPawn`).
//!
//! The native classes' toy bodies do not decode, which the native walk counts as native. Every value is invented; every
//! call goes through the VM. The test with `ow-combat`'s hit pack is in `ow-combat` (this crate cannot depend on it).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::{Missing, NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::{ClassBodySpec, class_body};
use ow_package::testkit::script::{Asm, CPF_OPT, CPF_PARM, FunctionSpec, Param, ScriptToy, function_payload};
use ow_package::testkit::{Export, ints};
use ow_vm::api::{NativeCall, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::world::{BalanceState, World};

const NATIVE_FLAGS: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);
const IMPORTS: [&str; 11] = [
    "Class",
    "Function",
    "Enum",
    "State",
    "ObjectProperty",
    "BoolProperty",
    "ByteProperty",
    "NameProperty",
    "ScriptStruct",
    "StructProperty",
    "FloatProperty",
];
const SCRIPT_FLAGS: u32 = 0x1 | 0x2 | 0x20000;
const ROLES: [&str; 5] = ["ROLE_None", "ROLE_SimulatedProxy", "ROLE_AutonomousProxy", "ROLE_Authority", "ROLE_MAX"];
const AUTHORITY: u8 = 3;
const SIMULATED: u8 = 1;
const CUSTOM: u8 = 14;

/// How `WillowPawn`'s `StoppedFalling` is declared (the note's test 10).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Good,
    /// `WillowPawn` declares no `StoppedFalling`.
    NoStoppedFalling,
    /// `StoppedFalling(bool)`.
    WrongStoppedFalling,
    /// `BeganFalling(byte[2])`, a static array (review r1 of #696, item 2).
    ArrayBeganFalling,
}

fn enumeration(t: &mut ScriptToy, owner: i32, name: &str, values: &[&str]) -> i32 {
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    payload.extend(ints(&[0, i32::try_from(values.len()).unwrap()]));
    for v in values {
        payload.extend_from_slice(&t.p.fname(v, 0));
    }
    let mut e = Export::new(t.import("Enum"), name, payload);
    e.outer = owner;
    t.p.add_export_with(e)
}

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param]) {
    t.function(owner, name, params, None, &[], &FunctionSpec::new(NATIVE_FLAGS, name), |_| None);
}

/// A native `name(<kind>[dim] param)`: its one parameter a static array of `dim` (the testkit's `Param` has no
/// dimension), the property exported first and the function after it, as `ScriptToy::function` orders them.
fn native_array_param(t: &mut ScriptToy, owner: i32, name: &str, kind: &str, param: &str, dim: i32) {
    let f = t.p.next_export() + 1;
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    payload.extend_from_slice(&ints(&[0, dim]));
    payload.extend_from_slice(&CPF_PARM.to_le_bytes());
    payload.extend_from_slice(&t.none);
    payload.extend_from_slice(&ints(&[0, 0]));
    let mut prop = Export::new(t.import(&format!("{kind}Property")), param, payload);
    prop.outer = f;
    t.p.add_export_with(prop);
    let payload = function_payload(&mut t.p, None, &FunctionSpec::new(NATIVE_FLAGS, name));
    let mut e = Export::new(t.import("Function"), name, payload);
    e.outer = owner;
    assert_eq!(t.p.add_export_with(e), f);
}

/// A State export `name` of `owner` with parent `parent` (0: none) and no code (the state body of `ow_vm::states`'
/// own tests).
fn state(t: &mut ScriptToy, owner: i32, name: &str, parent: i32) -> i32 {
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    payload.extend_from_slice(&ints(&[0, parent, 0, 0, 0, 0, 0]));
    payload.extend_from_slice(&ints(&[0, 0]));
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&ow_vm::states::NO_LABEL_TABLE.to_le_bytes());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&ints(&[0]));
    let mut export = Export::new(t.import("State"), name, payload);
    export.outer = owner;
    export.super_ = parent;
    t.p.add_export_with(export)
}

/// A script-only class `name` below `super_ref`: a decodable class body with the non-native flags, and its default.
fn script_class(t: &mut ScriptToy, name: &str, super_ref: i32) -> i32 {
    let at = t.p.next_export();
    let body = class_body(&mut t.p, &ClassBodySpec { flags: 2, ..ClassBodySpec::new(super_ref, 0, at + 1) });
    let mut export = Export::new(0, name, body);
    export.super_ = super_ref;
    assert_eq!(t.p.add_export_with(export), at);
    let mut default = ints(&[0]);
    default.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(at, &format!("Default__{name}"), default));
    at
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    // Object.GotoState, the VM's native 113, to put a pawn in a state (review r1 of #696, item 1).
    let mut goto = FunctionSpec::new(NATIVE_FLAGS, "GotoState");
    goto.native = 113;
    let opt = |kind: &str, name: &str| Param { flags: CPF_OPT, ..Param::new(kind, name) };
    let params = [opt("Name", "NewState"), opt("Name", "Label"), opt("Bool", "bForceEvents"), opt("Bool", "bKeepStack")];
    t.function(object, "GotoState", &params, None, &[], &goto, |_| None);
    let actor = t.class("Actor", object);
    let roles = enumeration(&mut t, actor, "ENetRole", &ROLES);
    t.prop("Byte", actor, "Physics", 0, 0);
    t.prop("Byte", actor, "Role", 0, roles);
    t.prop("Object", actor, "Base", 0, actor);
    let volume = t.class("PhysicsVolume", actor);
    t.prop("Object", actor, "PhysicsVolume", 0, volume);
    t.prop("Object", actor, "CollisionComponent", 0, object);
    t.prop("Bool", actor, "bTearOff", 0, 0);
    t.prop("Bool", actor, "bHardAttach", 0, 0);
    // The vectors the transition into PHYS_None or PHYS_Rotating zeroes (lane IMPL-LINK34-AFTER-MIND).
    let vector = ow_package::testkit::reflect::toy_struct(&mut t, actor, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    for f in ["Velocity", "Acceleration"] {
        t.prop("Struct", actor, f, 0, vector);
    }
    native(&mut t, actor, "SetPhysics", &[Param::new("Byte", "NewPhysics")]);
    let other = Param { type_ref: actor, ..Param::new("Object", "Other") };
    native(&mut t, volume, "PhysicsChangedFor", std::slice::from_ref(&other));
    // A volume subclass whose code a test leaves unloaded, one class that declares no PhysicsChangedFor, and one that
    // declares it with an int.
    t.class("ToyColdVolume", volume);
    t.class("ToyNoEventVolume", actor);
    let wrong = t.class("ToyWrongVolume", actor);
    native(&mut t, wrong, "PhysicsChangedFor", &[Param::new("Byte", "Other")]);
    // An object parameter of a class that does not accept a pawn (review r1 of #696, item 2).
    let unrelated = t.class("ToyUnrelatedVolume", actor);
    let other_volume = Param { type_ref: volume, ..Param::new("Object", "Other") };
    native(&mut t, unrelated, "PhysicsChangedFor", std::slice::from_ref(&other_volume));
    let pawn = t.class("Pawn", actor);
    t.prop("Bool", pawn, "bForceFloorCheck", 0, 0);
    script_class(&mut t, "ToyPlainPawn", pawn);
    // A lazy volume and a lazy base, held by a holder export.
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Volume", 0, volume);
    t.prop("Object", holder, "Thing", 0, actor);
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    let stored_volume = t.p.add_export(volume, "ToyStoredVolume", empty.clone());
    let stored_thing = t.p.add_export(actor, "ToyStoredThing", empty);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Volume", stored_volume));
    h.extend(t.p.object_tag("Thing", stored_thing));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let gearbox_pawn = t.class("GearboxPawn", pawn);
    t.class("ToyNativePawn", gearbox_pawn);
    t.build()
}

fn willow(variant: Variant) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let gbx = t.p.add_import("Core", "Package", 0, "GearboxFramework");
    let gearbox_pawn = t.p.add_import("Core", "Class", gbx, "GearboxPawn");
    let willow_pawn = t.class("WillowPawn", gearbox_pawn);
    for f in ["bNotifyStopFalling", "bNotifyBeginFalling", "bNotifyBeginWalking", "bNotifyStopWalking"] {
        t.prop("Bool", willow_pawn, f, 0, 0);
    }
    match variant {
        Variant::Good | Variant::ArrayBeganFalling => native(&mut t, willow_pawn, "StoppedFalling", &[]),
        Variant::NoStoppedFalling => {}
        Variant::WrongStoppedFalling => native(&mut t, willow_pawn, "StoppedFalling", &[Param::new("Bool", "bOdd")]),
    }
    if variant == Variant::ArrayBeganFalling {
        native_array_param(&mut t, willow_pawn, "BeganFalling", "Byte", "PreviousPhysics", 2);
    } else {
        native(&mut t, willow_pawn, "BeganFalling", &[Param::new("Byte", "PreviousPhysics")]);
    }
    native(&mut t, willow_pawn, "BeganWalking", &[Param::new("Byte", "PreviousPhysics")]);
    native(&mut t, willow_pawn, "StoppedWalking", &[]);
    let ai = t.class("WillowAIPawn", willow_pawn);
    t.class("WillowPlayerPawn", willow_pawn);
    script_class(&mut t, "ToyAIPawn", ai);
    script_class(&mut t, "ToyWillowPawn", willow_pawn);
    script_class(&mut t, "ToyColdWillowPawn", willow_pawn);
    // A pawn with state A, which overrides StoppedFalling, and state B extends A (review r1 of #696, item 1).
    let stated = script_class(&mut t, "ToyStatePawn", willow_pawn);
    let a = state(&mut t, stated, "Falling", 0);
    t.function(a, "StoppedFalling", &[], None, &[], &FunctionSpec::new(SCRIPT_FLAGS, "StoppedFalling"), |_| {
        let mut asm = Asm::new();
        asm.stmt().return_nothing();
        asm.end();
        Some(asm)
    });
    state(&mut t, stated, "Landing", a);
    t.build()
}

/// The spy events, by function path: what they saw, in order.
const SPIES: [&str; 5] = [
    "Engine.PhysicsVolume.PhysicsChangedFor",
    "WillowGame.WillowPawn.StoppedFalling",
    "WillowGame.WillowPawn.BeganFalling",
    "WillowGame.WillowPawn.BeganWalking",
    "WillowGame.WillowPawn.StoppedWalking",
];

struct Fx {
    world: World,
    calls: Rc<RefCell<Vec<String>>>,
    /// The volume event answers a typed NotYet (the note's test 14).
    fail_volume: Rc<RefCell<bool>>,
    /// The volume event moves the actor to this state first (review r2 of #696).
    goto_on_volume: Rc<RefCell<Option<&'static str>>>,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx_variant(mode: ParityMode, variant: Variant) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-world-setphys-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g, w) = (engine(), gearbox(), willow(variant));
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .unwrap();
    let mut world = World::new(vm, BalanceState::new(snapshot), &[]).unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let fail_volume = Rc::new(RefCell::new(false));
    let goto_on_volume: Rc<RefCell<Option<&'static str>>> = Rc::new(RefCell::new(None));
    for path in SPIES {
        let Ok(f) = world.vm.find_function(path) else { continue };
        let key = world.vm.function(f).unwrap().native_key.clone().unwrap();
        let name = path.rsplit('.').next().unwrap();
        let (seen, fail, goto) = (calls.clone(), fail_volume.clone(), goto_on_volume.clone());
        world
            .vm
            .register_native(
                &key,
                Rc::new(move |c: &mut NativeCall<'_>| {
                    let args: Vec<String> = (0..c.arg_count())
                        .map(|i| match c.arg(i).unwrap() {
                            Value::Byte(b) => format!("{b}"),
                            Value::Object(Some(_)) => "actor".into(),
                            other => format!("{other:?}"),
                        })
                        .collect();
                    // What Physics holds when the event runs (the actor is the receiver, or the volume's argument).
                    let actor = match c.arg_count() {
                        1 if matches!(c.arg(0).unwrap(), Value::Object(Some(_))) => match c.arg(0).unwrap() {
                            Value::Object(Some(o)) => *o,
                            _ => unreachable!(),
                        },
                        _ => c.this.unwrap(),
                    };
                    let class = c.vm.space.heap.get(actor)?.class;
                    let p = c.vm.space.prop(class, "Physics").unwrap();
                    let physics = c.vm.space.read(actor, PropAddr::scalar(p))?.clone();
                    seen.borrow_mut().push(format!("{name}({}) physics={physics:?}", args.join(",")));
                    let target = *goto.borrow();
                    if name == "PhysicsChangedFor"
                        && let Some(state) = target
                    {
                        let state = Value::Name(c.vm.space.names.intern(state));
                        let f = c.vm.find_function("Engine.Object.GotoState")?;
                        c.call_script(f, Some(actor), vec![state])?;
                    }
                    if name == "PhysicsChangedFor" && *fail.borrow() {
                        return Err(NotYet::not_read("a toy native the volume event reaches", "test").into());
                    }
                    Ok(Value::None)
                }),
            )
            .unwrap();
    }
    Fx { world, calls, fail_volume, goto_on_volume, dir }
}

fn fx() -> Fx {
    fx_variant(ParityMode::Off, Variant::Good)
}

impl Fx {
    fn vm(&mut self) -> &mut Vm {
        &mut self.world.vm
    }

    fn obj(&mut self, class: &str) -> ObjRef {
        let vm = self.vm();
        let c = vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
    }

    /// An object of `class` with its layout defined and its code not loaded.
    fn cold(&mut self, class: &str) -> ObjRef {
        let vm = self.vm();
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, class).unwrap();
        let c = code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap();
        assert!(!vm.class_code_loaded(c), "{class} starts cold");
        vm.space.alloc(c, "Cold", None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let vm = self.vm();
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap();
        vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    /// `o.name := v`, past the heap's checks (a fixture value of another class).
    fn put(&mut self, o: ObjRef, name: &str, v: Value) {
        let vm = self.vm();
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap();
        *vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }

    fn get(&mut self, o: ObjRef, name: &str) -> Value {
        let vm = self.vm();
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap();
        vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn physics(&mut self, o: ObjRef, mode: u8) -> Result<Value, VmError> {
        let vm = self.vm();
        let f = vm.find_function("Engine.Actor.SetPhysics").unwrap();
        Ok(vm.call(f, Some(o), vec![Value::Byte(mode)])?.ret)
    }

    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }

    /// The holder export's field `n`: a store handle not yet loaded.
    fn held(&mut self, n: &str) -> ObjRef {
        let path = ObjectPath::parse("Engine.ToyHolderObject").unwrap();
        let vm = self.vm();
        let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
        match self.get(holder, n) {
            Value::Object(Some(o)) => {
                assert!(self.world.vm.defs.is_unloaded(&self.world.vm.space, o));
                o
            }
            other => panic!("{other:?}"),
        }
    }

    /// The code loads (where on main), #518's counters and the name table's length, and the heap.
    fn state(&self) -> (u64, u64, u64, usize, String) {
        let vm = &self.world.vm;
        (
            vm.space.heap.committed_writes(),
            vm.rng.draws(),
            vm.defs.load_transitions(),
            vm.space.names.len(),
            format!("{:?}", vm.space.heap),
        )
    }

    /// A preflight refusal, or an answer that changes nothing: the result, with the state and the events unchanged.
    fn same(&mut self, o: ObjRef, mode: u8) -> Result<Value, VmError> {
        let before = (self.state(), self.calls());
        let r = self.physics(o, mode);
        assert_eq!((self.state(), self.calls()), before, "nothing changed: {r:?}");
        r
    }
}

fn not_yet(r: Result<Value, VmError>) -> (Missing, String) {
    match r {
        Err(VmError::NotYet(n)) => (n.missing, n.what.to_string()),
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn script_error(r: Result<Value, VmError>, needle: &str) {
    match r {
        Err(VmError::Script { message, .. }) => assert!(message.contains(needle), "{message}"),
        other => panic!("expected a script error naming {needle}, got {other:?}"),
    }
}

/// The note's test 1: a script-only `WillowAIPawn` subclass with defaults, asked for `PHYS_Custom`: `Physics` 14,
/// exactly one write, no event.
#[test]
fn link_9_s_custom_mode_writes_only_physics() {
    let mut x = fx();
    let pawn = x.obj("WillowGame.ToyAIPawn");
    x.set(pawn, "Role", Value::Byte(AUTHORITY));
    let writes = x.world.vm.space.heap.committed_writes();
    assert_eq!(x.physics(pawn, CUSTOM).unwrap(), Value::None);
    assert_eq!(x.get(pawn, "Physics"), Value::Byte(CUSTOM));
    assert_eq!(x.world.vm.space.heap.committed_writes(), writes + 1, "only Physics");
    assert!(x.calls().is_empty());
}

/// The note's test 2: the same mode as now changes nothing, even with every notify flag set.
#[test]
fn the_same_mode_changes_nothing() {
    let mut x = fx();
    let pawn = x.obj("WillowGame.ToyAIPawn");
    x.set(pawn, "Physics", Value::Byte(2));
    for f in ["bNotifyStopFalling", "bNotifyBeginFalling", "bNotifyBeginWalking", "bNotifyStopWalking"] {
        x.set(pawn, f, Value::Bool(true));
    }
    assert_eq!(x.same(pawn, 2).unwrap(), Value::None);
}

/// The note's test 3: `bHardAttach` drops 14 with `Physics` unchanged; `PHYS_Interpolating` (7) goes through.
#[test]
fn hard_attach_drops_all_but_interpolating() {
    let mut x = fx();
    let pawn = x.obj("WillowGame.ToyAIPawn");
    x.set(pawn, "bHardAttach", Value::Bool(true));
    assert_eq!(x.same(pawn, CUSTOM).unwrap(), Value::None);
    assert_eq!(x.get(pawn, "Physics"), Value::Byte(0));
    x.physics(pawn, 7).unwrap();
    assert_eq!(x.get(pawn, "Physics"), Value::Byte(7));
}

/// The note's tests 4 and 5: the falling and walking events, after the write, each with its flag.
#[test]
fn the_falling_and_walking_events_follow_the_transition() {
    let mut x = fx();
    let pawn = x.obj("WillowGame.ToyAIPawn");
    x.set(pawn, "Physics", Value::Byte(2));
    x.set(pawn, "bNotifyStopFalling", Value::Bool(true));
    x.physics(pawn, CUSTOM).unwrap();
    assert_eq!(x.calls(), vec!["StoppedFalling() physics=Byte(14)"]);
    x.calls.borrow_mut().clear();
    x.set(pawn, "bNotifyBeginFalling", Value::Bool(true));
    x.physics(pawn, 2).unwrap();
    assert_eq!(x.calls(), vec!["BeganFalling(14) physics=Byte(2)"]);
    x.calls.borrow_mut().clear();
    // Walking is 1 or 12: from 12 to 14 with bNotifyStopWalking.
    let walker = x.obj("WillowGame.ToyWillowPawn");
    x.set(walker, "Physics", Value::Byte(12));
    x.set(walker, "bNotifyStopWalking", Value::Bool(true));
    x.physics(walker, CUSTOM).unwrap();
    assert_eq!(x.calls(), vec!["StoppedWalking() physics=Byte(14)"]);
    // Without the flag, no event.
    x.calls.borrow_mut().clear();
    let quiet = x.obj("WillowGame.ToyWillowPawn");
    x.set(quiet, "Physics", Value::Byte(2));
    x.physics(quiet, CUSTOM).unwrap();
    assert!(x.calls().is_empty());
}

/// The note's tests 6, 7 and 8: the transition refusals, before any write.
#[test]
fn the_transition_refusals_come_before_any_write() {
    let mut x = fx();
    let plain = x.obj("Engine.ToyPlainPawn");
    x.set(plain, "Physics", Value::Byte(2));
    // The base search (lane IMPL-LINK34-AFTER-MIND): this world has no collision service.
    assert_eq!(not_yet(x.same(plain, 1)), (Missing::LayerAbsent(BASE_TRACE_LAYER), BASE_TRACE.to_string()));
    assert_eq!(x.get(plain, "bForceFloorCheck"), Value::Bool(false));
    let pawn = x.obj("WillowGame.ToyAIPawn");
    let base = x.obj("Engine.Actor");
    x.set(pawn, "Base", Value::Object(Some(base)));
    assert_eq!(not_yet(x.same(pawn, CUSTOM)), (Missing::NotRead, BASE_CLEAR.to_string()));
    let pawn = x.obj("WillowGame.ToyAIPawn");
    x.set(pawn, "Role", Value::Byte(AUTHORITY));
    let component = x.obj("Engine.Actor");
    x.set(pawn, "CollisionComponent", Value::Object(Some(component)));
    assert_eq!(not_yet(x.same(pawn, 10)), (Missing::LayerAbsent(RIGID_LAYER), RIGID_BODY.to_string()));
    let below = x.obj("WillowGame.ToyAIPawn");
    x.set(below, "Role", Value::Byte(SIMULATED));
    assert_eq!(not_yet(x.same(below, 10)), (Missing::NotRead, RIGID_EVENT.to_string()));
    assert_eq!(not_yet(x.same(below, 12)), (Missing::NotRead, NAV_MESH.to_string()));
}

/// The note's test 9: a resident `PhysicsVolume`'s spy event runs with the actor, after `Physics` is written.
#[test]
fn the_volume_event_runs_after_the_write() {
    let mut x = fx();
    let pawn = x.obj("WillowGame.ToyAIPawn");
    let volume = x.obj("Engine.PhysicsVolume");
    x.set(pawn, "PhysicsVolume", Value::Object(Some(volume)));
    x.physics(pawn, CUSTOM).unwrap();
    assert_eq!(x.calls(), vec!["PhysicsChangedFor(actor) physics=Byte(14)"]);
}

/// The note's test 10: the event boundaries, each with `Physics` unchanged.
#[test]
fn the_event_boundaries_refuse_before_any_write() {
    let mut x = fx();
    for (class, needle) in [("Engine.ToyNoEventVolume", "declares no PhysicsChangedFor"), ("Engine.ToyWrongVolume", "other parameters")] {
        let pawn = x.obj("WillowGame.ToyAIPawn");
        let volume = x.obj(class);
        x.put(pawn, "PhysicsVolume", Value::Object(Some(volume)));
        script_error(x.same(pawn, CUSTOM), needle);
    }
    let pawn = x.obj("WillowGame.ToyAIPawn");
    let cold = x.cold("Engine.ToyColdVolume");
    x.set(pawn, "PhysicsVolume", Value::Object(Some(cold)));
    let (missing, text) = not_yet(x.same(pawn, CUSTOM));
    assert_eq!((missing, text), (Missing::NotRead, code_not_loaded("PhysicsChangedFor", "Engine.ToyColdVolume")));
    // The same two for StoppedFalling, on a WillowPawn whose condition holds.
    for (variant, needle) in [(Variant::NoStoppedFalling, "declares no StoppedFalling"), (Variant::WrongStoppedFalling, "other parameters")] {
        let mut y = fx_variant(ParityMode::Off, variant);
        let pawn = y.obj("WillowGame.ToyWillowPawn");
        y.set(pawn, "Physics", Value::Byte(2));
        y.set(pawn, "bNotifyStopFalling", Value::Bool(true));
        script_error(y.same(pawn, CUSTOM), needle);
    }
    let cold_pawn = x.cold("WillowGame.ToyColdWillowPawn");
    x.set(cold_pawn, "Physics", Value::Byte(2));
    x.set(cold_pawn, "bNotifyStopFalling", Value::Bool(true));
    let (missing, text) = not_yet(x.same(cold_pawn, CUSTOM));
    assert_eq!((missing, text), (Missing::NotRead, code_not_loaded("StoppedFalling", "WillowGame.ToyColdWillowPawn")));
}

/// The note's test 11: residency only on a path that uses the object.
#[test]
fn residency_is_checked_only_where_used() {
    let mut x = fx();
    let (volume, thing) = (x.held("Volume"), x.held("Thing"));
    let pawn = x.obj("WillowGame.ToyAIPawn");
    x.set(pawn, "PhysicsVolume", Value::Object(Some(volume)));
    let (missing, text) = not_yet(x.same(pawn, CUSTOM));
    assert_eq!((missing, text), (Missing::NotRead, NOT_LOADED.to_string()));
    assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, volume), "nothing loaded");
    // A same-mode request, and a bHardAttach stop, with a lazy volume and a lazy base: success, nothing loaded.
    x.set(pawn, "Base", Value::Object(Some(thing)));
    assert_eq!(x.same(pawn, 0).unwrap(), Value::None);
    x.set(pawn, "bHardAttach", Value::Bool(true));
    assert_eq!(x.same(pawn, CUSTOM).unwrap(), Value::None);
    for o in [volume, thing] {
        assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, o), "nothing loaded");
    }
}

/// The note's tests 12 and 13: an unlisted native class (and `WillowPlayerPawn`, not read) is the class `NotRead`;
/// `Physics` of another kind is a script error naming it.
#[test]
fn unlisted_classes_and_a_wrong_physics_kind_are_refused() {
    let mut x = fx();
    for class in ["GearboxFramework.ToyNativePawn"] {
        let pawn = x.obj(class);
        let (missing, text) = not_yet(x.same(pawn, CUSTOM));
        assert_eq!((missing, text), (Missing::NotRead, format!("Actor.SetPhysics: the physics change of {class}")));
    }
    let pawn = x.obj("WillowGame.ToyAIPawn");
    let class = x.world.vm.space.heap.get(pawn).unwrap().class;
    let p = x.world.vm.space.prop(class, "Physics").unwrap();
    *x.vm().space.slot_mut(pawn, PropAddr::scalar(p)).unwrap() = Value::Int(0);
    script_error(x.same(pawn, CUSTOM), "Physics");
}

/// The note's test 14: a volume event that answers a typed `NotYet` after the write: "stopped after partial writes",
/// with `Physics` keeping the new mode.
#[test]
fn a_refusal_after_the_write_is_marked() {
    let mut x = fx();
    let pawn = x.obj("WillowGame.ToyAIPawn");
    let volume = x.obj("Engine.PhysicsVolume");
    x.set(pawn, "PhysicsVolume", Value::Object(Some(volume)));
    *x.fail_volume.borrow_mut() = true;
    let (missing, text) = not_yet(x.physics(pawn, CUSTOM));
    assert_eq!(missing, Missing::NotRead);
    assert!(text.starts_with("stopped after partial writes: a toy native the volume event reaches"), "{text}");
    assert_eq!(x.get(pawn, "Physics"), Value::Byte(CUSTOM));
}

/// The native's key and the note's test 16 in this crate: Reference binds nothing here.
#[test]
fn off_binds_the_key_and_reference_binds_nothing() {
    let mut x = fx();
    let f = x.vm().find_function("Engine.Actor.SetPhysics").unwrap();
    assert_eq!(x.world.vm.function(f).unwrap().native_key.as_deref(), Some(KEY));
    assert!(x.world.vm.is_bound(f));
    let r = fx_variant(ParityMode::Reference, Variant::Good);
    assert!(!r.world.vm.has_native(KEY));
}

/// Review r1 of #696, item 1: in state `Landing`, whose parent state `Falling` overrides `StoppedFalling`, the event the
/// commit will call is the parent state's (as Off event dispatch finds it), not the class body's.
#[test]
fn an_override_in_a_parent_state_is_the_event_called() {
    let mut x = fx();
    // The Core natives, GotoState among them (World::new does not register them).
    ow_vm::natives_core::register(x.vm()).unwrap();
    let pawn = x.obj("WillowGame.ToyStatePawn");
    let landing = Value::Name(x.vm().space.names.intern("Landing"));
    let f = x.vm().find_function("Engine.Object.GotoState").unwrap();
    x.vm().call(f, Some(pawn), vec![landing]).unwrap();
    x.set(pawn, "Physics", Value::Byte(2));
    x.set(pawn, "bNotifyStopFalling", Value::Bool(true));
    let plan = preflight(&x.world.vm, pawn, CUSTOM).unwrap().unwrap();
    let paths: Vec<String> =
        plan.events.iter().map(|(name, shape, _)| {
            let f = event(&x.world.vm, pawn, name, *shape, pawn).unwrap();
            x.world.vm.function(f).unwrap().path.as_str().to_string()
        }).collect();
    assert_eq!(paths, ["WillowGame.ToyStatePawn.Falling.StoppedFalling"]);
    // The commit runs it (a script body): Physics 14 and no spy call, since the class body's spy is not the one called.
    x.physics(pawn, CUSTOM).unwrap();
    assert_eq!(x.get(pawn, "Physics"), Value::Byte(CUSTOM));
    assert!(x.calls().is_empty(), "{:?}", x.calls());
}

/// Review r1 of #696, item 2: a static-array `BeganFalling(byte[2])`, and a `PhysicsChangedFor` whose object parameter
/// does not accept the actor, are script errors before any write.
#[test]
fn array_and_unrelated_parameters_are_refused_before_any_write() {
    let mut y = fx_variant(ParityMode::Off, Variant::ArrayBeganFalling);
    let pawn = y.obj("WillowGame.ToyWillowPawn");
    y.set(pawn, "bNotifyBeginFalling", Value::Bool(true));
    script_error(y.same(pawn, 2), "other parameters");
    let mut x = fx();
    let pawn = x.obj("WillowGame.ToyAIPawn");
    let volume = x.obj("Engine.ToyUnrelatedVolume");
    x.put(pawn, "PhysicsVolume", Value::Object(Some(volume)));
    script_error(x.same(pawn, CUSTOM), "other parameters");
}

/// Review r1 of #696, item 3: the rule is bound for `Engine`'s function by its package-qualified identity, not
/// VM-wide.
#[test]
fn the_rule_is_bound_for_engine_only() {
    let x = fx();
    assert!(x.world.vm.is_bound_in(PACKAGE, KEY));
    assert!(!x.world.vm.native_keys().any(|k| k == KEY), "no VM-wide binding");
    assert!(!x.world.vm.is_bound_in("SomeOtherPackage", KEY), "another package's same-keyed function stays unbound");
}

/// Review r1 of #696, item 4: the AI layer answers before `Physics` is read, so a malformed `Physics` does not turn a
/// nav-mesh request or a `bHardAttach` stop into a script error.
#[test]
fn the_ai_layer_answers_before_physics_is_read() {
    let mut x = fx();
    let pawn = x.obj("WillowGame.ToyAIPawn");
    x.put(pawn, "Physics", Value::Int(0));
    assert_eq!(not_yet(x.same(pawn, 12)), (Missing::NotRead, NAV_MESH.to_string()));
    x.set(pawn, "bHardAttach", Value::Bool(true));
    assert_eq!(x.same(pawn, CUSTOM).unwrap(), Value::None);
}

/// Review r2 of #696: the volume's `PhysicsChangedFor` moves the pawn into state `Landing` (whose parent `Falling`
/// overrides `StoppedFalling`), so the pawn event dispatched next is the new state's override, resolved again at
/// dispatch, not the class body's handler the preflight saw.
#[test]
fn a_callback_s_state_change_is_followed_at_dispatch() {
    let mut x = fx();
    ow_vm::natives_core::register(x.vm()).unwrap();
    let pawn = x.obj("WillowGame.ToyStatePawn");
    let volume = x.obj("Engine.PhysicsVolume");
    x.set(pawn, "PhysicsVolume", Value::Object(Some(volume)));
    x.set(pawn, "Physics", Value::Byte(2));
    x.set(pawn, "bNotifyStopFalling", Value::Bool(true));
    let plan = preflight(&x.world.vm, pawn, CUSTOM).unwrap().unwrap();
    assert_eq!(plan.events.iter().map(|e| e.0).collect::<Vec<_>>(), ["StoppedFalling"], "validated in preflight");
    *x.goto_on_volume.borrow_mut() = Some("Landing");
    x.physics(pawn, CUSTOM).unwrap();
    assert_eq!(x.calls(), vec!["PhysicsChangedFor(actor) physics=Byte(14)"], "the class body's spy is not called");
    let landing = x.world.vm.space.names.find("Landing");
    assert_eq!(x.world.vm.space.heap.get(pawn).unwrap().state, landing, "the pawn is in the new state");
}

/// NATIVE_LINK34_AFTER_MIND.md section 3 (test 2; lane IMPL-LINK34-AFTER-MIND): a `WillowPlayerPawn` has `WillowPawn`'s
/// layers. A change writes `Physics`; `PHYS_NavMeshWalking` meets no AI step.
#[test]
fn a_player_pawn_has_the_willow_pawn_layers() {
    for mode in [CUSTOM, 12] {
        let mut x = fx();
        let pawn = x.obj("WillowGame.WillowPlayerPawn");
        x.physics(pawn, mode).unwrap();
        assert_eq!(x.get(pawn, "Physics"), Value::Byte(mode), "{mode}");
    }
}

/// A collision service with geometry (the base search cannot run its trace there).
struct WithGeometry;

impl crate::placement::SpawnCollision for WithGeometry {
    fn box_test(&self, _: [f32; 3], _: [f32; 3], _: crate::placement::Mode) -> Result<crate::placement::Test, VmError> {
        Ok(crate::placement::Test::Clear)
    }
    fn box_sweep(&self, _: [f32; 3], _: [f32; 3], _: [f32; 3]) -> Result<crate::placement::Sweep, VmError> {
        Ok(crate::placement::Sweep::Clear)
    }
    fn point_sweep(&self, _: [f32; 3], _: [f32; 3]) -> Result<crate::placement::Sweep, VmError> {
        Ok(crate::placement::Sweep::Clear)
    }
}

/// NATIVE_LINK34_AFTER_MIND.md section 2 (test 1; lane IMPL-LINK34-AFTER-MIND): the base search. No collision service
/// is the trace's `LayerAbsent`; a service with geometry its `NotRead`; under `NoLevel` with no `Base` the mode is
/// written, the vectors zeroed and the stand-in logged; with a `Base` set the base setter's `NotRead`. Each refusal
/// writes nothing.
#[test]
fn the_base_search() {
    let mut x = fx();
    let pawn = x.obj("Engine.ToyPlainPawn");
    x.set(pawn, "Physics", Value::Byte(2));
    assert_eq!(not_yet(x.same(pawn, 0)), (Missing::LayerAbsent(BASE_TRACE_LAYER), BASE_TRACE.to_string()));
    let mut x = fx();
    crate::placement::install_collision(x.vm(), Rc::new(WithGeometry)).unwrap();
    let pawn = x.obj("Engine.ToyPlainPawn");
    x.set(pawn, "Physics", Value::Byte(2));
    assert_eq!(not_yet(x.same(pawn, 0)), (Missing::NotRead, BASE_TRACE_FLAGS.to_string()));
    // The stand-in: into PHYS_None, the vectors zeroed and the line logged.
    let mut x = fx();
    crate::placement::install_collision(x.vm(), Rc::new(crate::placement::NoLevel)).unwrap();
    let pawn = x.obj("Engine.ToyPlainPawn");
    x.set(pawn, "Physics", Value::Byte(2));
    let moving = {
        let mut v = x.get(pawn, "Velocity");
        for f in v.struct_mut().unwrap().fields.iter_mut() {
            *f = Value::Float(3.0);
        }
        v
    };
    x.set(pawn, "Velocity", moving);
    x.physics(pawn, 0).unwrap();
    assert_eq!(x.get(pawn, "Physics"), Value::Byte(0));
    let zero = x.get(pawn, "Velocity");
    assert!(zero.as_struct().unwrap().fields.iter().all(|f| *f == Value::Float(0.0)), "{zero:?}");
    assert!(x.vm().log.iter().any(|l| l == BASE_STAND_IN), "logged");
    // Into PHYS_Walking: bForceFloorCheck set, the vectors kept.
    let pawn = x.obj("Engine.ToyPlainPawn");
    x.set(pawn, "Physics", Value::Byte(2));
    x.physics(pawn, 1).unwrap();
    assert_eq!((x.get(pawn, "Physics"), x.get(pawn, "bForceFloorCheck")), (Value::Byte(1), Value::Bool(true)));
    // A Base set: the base setter's NotRead, nothing written.
    let pawn = x.obj("Engine.ToyPlainPawn");
    let base = x.obj("Engine.ToyPlainPawn");
    x.set(pawn, "Physics", Value::Byte(2));
    x.set(pawn, "Base", Value::Object(Some(base)));
    assert_eq!(not_yet(x.same(pawn, 0)), (Missing::NotRead, BASE_SETTER.to_string()));
}
