//! Owner: lane W7-AI (part 3). The movement rules on an invented "Core" package (AGENTS.md rule 9): a `ToyAction`
//! (an `Object`, not an actor, with the action-sequence fields) whose states call the waits, a `ToyHandle`, a
//! `ToyComponent` with target records and a `ToyMind`. The natives are registered by their keys with this module's
//! bodies (the stock paths are checked separately). Every value is invented.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::Missing;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::PropAddr;
use ow_package::testkit::reflect::toy_array_of;
use ow_package::testkit::script::{Asm, CPF_OPT, FunctionSpec, Param, ScriptToy};
use ow_package::testkit::{Export, ints};
use ow_vm::api::StubPolicy;

use super::*;
use crate::awareness::Vec3;
use crate::awareness::actions::install_locomotion;

const FUNC_FINAL: u32 = 0x1;
const FUNC_NATIVE: u32 = 0x400;
const FUNC_PUBLIC: u32 = 0x20000;
const NATIVE: u32 = FUNC_NATIVE | FUNC_FINAL | FUNC_PUBLIC;
const OPERATOR: u32 = FUNC_NATIVE | 0x1000 | 0x23000;

/// A quarter second (exact in binary).
const DT: f32 = 0.25;

fn param(kind: &str, name: &str, flags: u64) -> Param {
    Param { flags, ..Param::new(kind, name) }
}

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], ret: Option<(&str, i32)>, number: u16) -> i32 {
    let mut s = FunctionSpec::new(NATIVE, name);
    s.native = number;
    t.function(owner, name, params, ret, &[], &s, |_| None).function
}

/// `Trace = Trace $ text`.
fn append(a: &mut Asm, trace: i32, text: &str) {
    let mut constant = vec![0x1F];
    constant.extend_from_slice(text.as_bytes());
    constant.push(0);
    a.stmt().raw(&[0x0F]).instance(trace).native(112, |a| {
        a.instance(trace).raw(&constant);
    });
}

/// `GotoState(name)`.
fn goto(t: &mut ScriptToy, a: &mut Asm, name: &str) {
    let mut constant = vec![0x21];
    constant.extend_from_slice(&t.p.fname(name, 0));
    a.stmt().native(113, |a| {
        a.raw(&constant);
    });
}

/// A float constant.
fn float(a: &mut Asm, v: f32) {
    a.raw(&[0x1E]).raw(&v.to_le_bytes());
}

fn finish(t: &mut ScriptToy, mut a: Asm, labels: &[(&str, u32)]) -> (Asm, u16) {
    let at = a.here();
    a.stmt().raw(&[0x0C]);
    for &(name, offset) in labels {
        let f = t.p.fname(name, 0);
        a.raw(&f).i32(i32::try_from(offset).unwrap());
    }
    let none = t.none;
    a.raw(&none).i32(0);
    a.end();
    (a, u16::try_from(at + 1).unwrap())
}

fn state(t: &mut ScriptToy, owner: i32, name: &str, code: &(Asm, u16)) -> i32 {
    let none = t.none;
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&none);
    payload.extend_from_slice(&ints(&[0, 0, 0, 0, 0, 0, 0]));
    let (a, label) = code;
    payload.extend_from_slice(&ints(&[i32::try_from(a.memory_size()).unwrap(), i32::try_from(a.bytes.len()).unwrap()]));
    payload.extend_from_slice(&a.bytes);
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&label.to_le_bytes());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&ints(&[0]));
    let mut export = Export::new(t.import("State"), name, payload);
    export.outer = owner;
    t.p.add_export_with(export)
}

/// One state of `ToyAction`: Begin: "a"; `call`; "b"; Stop.
fn wait_state(t: &mut ScriptToy, action: i32, trace: i32, name: &str, call: impl FnOnce(&mut Asm)) {
    let mut a = Asm::new();
    let begin = a.here();
    append(&mut a, trace, "a");
    call(a.stmt());
    append(&mut a, trace, "b");
    a.stmt().raw(&[0x08]);
    let code = finish(t, a, &[("Begin", begin)]);
    state(t, action, name, &code);
}

/// The package (module documentation). `ToyAction`'s states: `Sleeping` (Sleep(0.5)), `Uninit` (`Uninit()` clears
/// `bInitialized`), `Path` (WaitForPath), `Follow` (FollowPath), `FinishP` (FinishPath), `Turn` (WaitForPawnToTurn),
/// `Stopping` (WaitForPawnToStop), and `PingA` / `PingB`, which append "x" and go to each other.
fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class", "Function", "State", "StrProperty", "NameProperty", "FloatProperty", "BoolProperty", "ObjectProperty",
        "ArrayProperty",
    ]);
    let object = t.class("Object", 0);
    let mut concat = FunctionSpec::new(OPERATOR, "$");
    concat.native = 112;
    t.function(object, "Concat_StrStr", &[param("Str", "A", 0), param("Str", "B", 0)], Some(("Str", 0)), &[], &concat, |_| None);
    native(
        &mut t,
        object,
        "GotoState",
        &[param("Name", "NewState", CPF_OPT), param("Name", "Label", CPF_OPT), param("Bool", "bForceEvents", CPF_OPT), param("Bool", "bKeepStack", CPF_OPT)],
        None,
        113,
    );
    let handle = t.class("ToyHandle", object);
    t.prop("Bool", handle, "bInitialized", 0, 0);
    let mind = t.class("ToyMind", object);
    for name in ["Pawn", "FollowActor", "ScriptedMoveTarget", "ScriptedFocus", "MyWillowPawn", "AIClass"] {
        t.prop("Object", mind, name, 0, object);
    }
    t.prop("Bool", mind, "bScriptedHoldPosition", 0, 0);
    let record = t.class("ToyRecord", object);
    t.prop("Object", record, "Target", 0, object);
    let component = t.class("ToyComponent", object);
    t.prop("Object", component, "CurrentTarget", 0, object);
    native(&mut t, component, "GetTargetRecord", &[param("Bool", "bParent", CPF_OPT)], Some(("Object", object)), 0);
    native(&mut t, component, "GetTargetRecordFor", &[Param { type_ref: object, ..param("Object", "A", 0) }, param("Bool", "bParent", CPF_OPT)], Some(("Object", object)), 0);
    toy_array_of(&mut t, component, "Targets", "Object", object);
    for name in ["IsTooFarFromHome", "IsInScriptedMovement", "ShouldLookAtPlayer"] {
        native(&mut t, mind, name, &[], Some(("Bool", 0)), 0);
    }

    let action = t.class("ToyAction", object);
    let trace = t.prop("Str", action, "Trace", 0, 0);
    t.prop("Bool", action, "bInitialized", 0, 0);
    t.prop("Float", action, "LatentFloat", 0, 0);
    for name in ["ParentSequence", "MyNavHandle", "MyWillowPawn", "MyWillowMind", "MyGearboxPawn"] {
        t.prop("Object", action, name, 0, object);
    }
    let sleep = native(&mut t, action, "Sleep", &[param("Float", "Seconds", 0)], None, 0);
    native(&mut t, action, "InterruptLatentAction", &[], None, 0);
    let uninit = native(&mut t, action, "Uninit", &[], None, 0);
    let waits: Vec<(&str, i32)> = ["WaitForPath", "FollowPath", "FinishPath", "WaitForPawnToTurn", "WaitForPawnToStop"]
        .iter()
        .map(|n| (*n, native(&mut t, action, n, &[], None, 0)))
        .collect();
    wait_state(&mut t, action, trace, "Sleeping", |a| {
        a.final_function(sleep, |a| float(a, 0.5));
    });
    wait_state(&mut t, action, trace, "Uninit", |a| {
        a.final_function(uninit, |_| {});
    });
    for (state_name, (_, f)) in ["Path", "Follow", "FinishP", "Turn", "Stopping"].iter().zip(&waits) {
        let f = *f;
        wait_state(&mut t, action, trace, state_name, |a| {
            a.final_function(f, |_| {});
        });
    }
    for (name, next) in [("PingA", "PingB"), ("PingB", "PingA")] {
        let mut a = Asm::new();
        let begin = a.here();
        append(&mut a, trace, "x");
        goto(&mut t, &mut a, next);
        let code = finish(&mut t, a, &[("Begin", begin)]);
        state(&mut t, action, name, &code);
    }
    t.build()
}

/// A movement host whose path is valid once `valid` is set.
#[derive(Default)]
struct Paths {
    valid: bool,
}

impl Locomotion for Paths {
    fn create_path(&mut self, _: &mut Vm, _: ObjRef, _: Vec3) -> Result<bool, VmError> {
        Ok(false)
    }
    fn clear_path(&mut self, _: &mut Vm, _: ObjRef, _: bool) -> Result<bool, VmError> {
        Ok(false)
    }
    fn path_is_valid(&mut self, _: &mut Vm, _: ObjRef) -> Result<bool, VmError> {
        Ok(self.valid)
    }
    fn invalidate_path(&mut self, _: &mut Vm, _: ObjRef) -> Result<(), VmError> {
        Ok(())
    }
    fn reached_destination(&mut self, _: &mut Vm, _: ObjRef) -> Result<bool, VmError> {
        Ok(false)
    }
    fn can_move_to(&mut self, _: &mut Vm, _: ObjRef, _: Vec3) -> Result<bool, VmError> {
        Ok(false)
    }
    fn set_movement_speed(&mut self, _: &mut Vm, _: ObjRef, _: u8) -> Result<(), VmError> {
        Ok(())
    }
    fn nearest_on_navmesh(&mut self, _: &mut Vm, _: ObjRef) -> Result<Option<Vec3>, VmError> {
        Ok(None)
    }
}

struct Run {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

static NEXT: AtomicU32 = AtomicU32::new(0);

fn start() -> Run {
    let dir = std::env::temp_dir().join(format!("ow-ai-movement-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let defs = DefStore::from_synthetic(&dir, &[("Core.upk", &package())], IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
        .with_stub_policy(StubPolicy::refuse_all());
    ow_vm::natives_core::register(&mut vm).unwrap();
    let bodies: [(&str, NativeFn); 15] = [
        ("ToyAction.Sleep(float)", Rc::new(n_sleep)),
        ("ToyAction.InterruptLatentAction()", Rc::new(n_interrupt)),
        ("ToyAction.WaitForPath()", Rc::new(n_wait_for_path)),
        ("ToyAction.FollowPath()", Rc::new(n_follow_path)),
        ("ToyAction.FinishPath()", Rc::new(n_finish_path)),
        ("ToyAction.WaitForPawnToTurn()", Rc::new(n_wait_for_turn)),
        ("ToyAction.WaitForPawnToStop()", Rc::new(n_wait_for_stop)),
        (
            "ToyAction.Uninit()",
            Rc::new(|c: &mut NativeCall<'_>| {
                let me = this(c)?;
                write(c.vm, me, "bInitialized", Value::Bool(false))?;
                Ok(Value::None)
            }),
        ),
        ("ToyComponent.GetTargetRecord(bool)", Rc::new(n_target_record)),
        ("ToyComponent.GetTargetRecordFor(object,bool)", Rc::new(n_target_record_for)),
        ("ToyMind.IsTooFarFromHome()", Rc::new(n_too_far_from_home)),
        ("ToyMind.IsInScriptedMovement()", Rc::new(n_in_scripted_movement)),
        ("ToyMind.ShouldLookAtPlayer()", Rc::new(n_look_at_player)),
        ("ToyMind.CheckForPerch()", Rc::new(n_check_for_perch)),
        ("ToyMind.GetPopOpDen()", Rc::new(|_c| Err(den_not_read()))),
    ];
    for (key, body) in bodies {
        if key.starts_with("ToyMind.CheckForPerch") || key.starts_with("ToyMind.GetPopOpDen") {
            continue; // not declared by the toy: their bodies are tested directly
        }
        vm.register_native(key, body).unwrap();
    }
    Run { vm, dir }
}

impl Run {
    fn make(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(&format!("Core.{class}")).unwrap();
        self.vm.instantiate(c, "").unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let prop = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(prop), v).unwrap();
    }

    fn read(&mut self, o: ObjRef, name: &str) -> Value {
        get(&mut self.vm, o, name).unwrap().unwrap()
    }

    /// A `ToyAction` with `bInitialized` set, in `state`.
    fn action(&mut self, state: &str) -> ObjRef {
        let o = self.make("ToyAction");
        self.set(o, "bInitialized", Value::Bool(true));
        self.goto(o, state);
        o
    }

    fn goto(&mut self, o: ObjRef, state: &str) {
        let f = self.vm.find_function("Core.Object.GotoState").unwrap();
        let state = Value::Name(self.vm.space.names.intern(state));
        self.vm.call(f, Some(o), vec![state]).unwrap();
    }

    fn trace(&mut self, o: ObjRef) -> String {
        match self.read(o, "Trace") {
            Value::Str(s) => s.to_string(),
            other => panic!("Trace is {other:?}"),
        }
    }

    fn step(&mut self, o: ObjRef) -> Result<(), VmError> {
        step_action(&mut self.vm, o, DT)
    }

    fn call(&mut self, o: ObjRef, path: &str, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path).unwrap();
        Ok(self.vm.call(f, Some(o), args)?.ret)
    }
}

fn missing(e: VmError) -> Missing {
    match e {
        VmError::NotYet(n) => {
            assert_eq!(n.record, RECORD_MOVEMENT, "{n}");
            n.missing
        }
        other => panic!("expected NotYet, got {other}"),
    }
}

/// `Sleep(0.5)` at a quarter-second step: started in the first step, polled from the next; the remainder goes 0.5,
/// 0.25, then 0 < 0.125 ends it, and the end writes 0.
#[test]
fn sleep_waits_on_latent_float_and_ends_below_half_the_delta() {
    let mut r = start();
    let o = r.action("Sleeping");
    r.step(o).unwrap();
    assert_eq!((r.trace(o), r.read(o, "LatentFloat")), ("a".into(), Value::Float(0.5)));
    r.step(o).unwrap();
    assert_eq!((r.trace(o), r.read(o, "LatentFloat")), ("a".into(), Value::Float(0.25)));
    r.step(o).unwrap();
    assert_eq!((r.trace(o), r.read(o, "LatentFloat")), ("ab".into(), Value::Float(0.0)));
}

/// With `bInitialized` clear the step runs no code and the sleep's poll changes nothing; set again, both go on.
#[test]
fn the_step_and_the_sleep_are_gated_by_b_initialized() {
    let mut r = start();
    let o = r.action("Sleeping");
    r.set(o, "bInitialized", Value::Bool(false));
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "", "no code runs while bInitialized is clear");
    r.set(o, "bInitialized", Value::Bool(true));
    r.step(o).unwrap();
    assert_eq!(r.read(o, "LatentFloat"), Value::Float(0.5));
    r.set(o, "bInitialized", Value::Bool(false));
    r.step(o).unwrap();
    assert_eq!(r.read(o, "LatentFloat"), Value::Float(0.5), "the poll changes nothing while clear");
    // Cleared by the code itself: the run stops after that statement and resumes after it once set again.
    let u = r.action("Uninit");
    r.step(u).unwrap();
    assert_eq!(r.trace(u), "a");
    r.set(u, "bInitialized", Value::Bool(true));
    r.step(u).unwrap();
    assert_eq!(r.trace(u), "ab");
}

/// The action step ends at the fifth state change (five runs); the actor step allows five restarts (six runs).
#[test]
fn the_fifth_state_change_ends_an_action_step() {
    let mut r = start();
    let o = r.action("PingA");
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "xxxxx");
    let p = r.action("PingA");
    ow_vm::states::process_state(&mut r.vm, p, DT).unwrap();
    assert_eq!(r.trace(p), "xxxxxx", "the actor step, for comparison");
}

/// `InterruptLatentAction` ends the wait at once: the next step continues after it, and `LatentFloat` is 0.
#[test]
fn interrupt_ends_the_wait_and_clears_latent_float() {
    let mut r = start();
    let o = r.action("Sleeping");
    r.step(o).unwrap();
    r.call(o, "Core.ToyAction.InterruptLatentAction", vec![]).unwrap();
    assert_eq!(r.read(o, "LatentFloat"), Value::Float(0.0));
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "ab");
}

/// `WaitForPath`: no handle, or a handle not initialised, is no wait; otherwise it ends on the first valid poll;
/// with no movement host the poll is `NotYet { LayerAbsent }`.
#[test]
fn wait_for_path_follows_the_handle_and_the_path_validity() {
    let mut r = start();
    let o = r.action("Path");
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "ab", "no handle: no wait");
    let h = r.make("ToyHandle");
    let o = r.action("Path");
    r.set(o, "MyNavHandle", Value::Object(Some(h)));
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "ab", "a handle not initialised: no wait");
    r.set(h, "bInitialized", Value::Bool(true));
    let o = r.action("Path");
    let pawn = r.make("ToyHandle");
    r.set(o, "MyNavHandle", Value::Object(Some(h)));
    r.set(o, "MyWillowPawn", Value::Object(Some(pawn)));
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "a");
    assert!(matches!(missing(r.step(o).unwrap_err()), Missing::LayerAbsent(_)), "no movement host");
    let paths = Rc::new(RefCell::new(Paths::default()));
    install_locomotion(&mut r.vm, paths.clone()).unwrap();
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "a", "the path is not valid yet");
    paths.borrow_mut().valid = true;
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "ab");
}

/// `FollowPath`: an invalid path is no wait; a valid one needs the nav mesh. `FinishPath`: no handle is no wait; one
/// needs the nav mesh.
#[test]
fn follow_and_finish_path_stop_at_the_nav_mesh() {
    let mut r = start();
    let paths = Rc::new(RefCell::new(Paths::default()));
    install_locomotion(&mut r.vm, paths.clone()).unwrap();
    let pawn = r.make("ToyHandle");
    let o = r.action("Follow");
    r.set(o, "MyWillowPawn", Value::Object(Some(pawn)));
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "ab", "an invalid path: no wait");
    paths.borrow_mut().valid = true;
    let o = r.action("Follow");
    r.set(o, "MyWillowPawn", Value::Object(Some(pawn)));
    assert!(matches!(missing(r.step(o).unwrap_err()), Missing::LayerAbsent(l) if l == NAV_MESH));
    let o = r.action("FinishP");
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "ab", "no handle: no wait");
    let h = r.make("ToyHandle");
    let o = r.action("FinishP");
    r.set(o, "MyNavHandle", Value::Object(Some(h)));
    assert!(matches!(missing(r.step(o).unwrap_err()), Missing::LayerAbsent(l) if l == NAV_MESH));
}

/// `WaitForPawnToTurn` always waits; with no pawn the first poll ends it; with a pawn the direction is not read.
/// `WaitForPawnToStop` with no `MyGearboxPawn` is refused at the poll.
#[test]
fn the_pawn_waits_end_or_name_what_is_not_read() {
    let mut r = start();
    let o = r.action("Turn");
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "a", "it always waits");
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "ab", "no pawn: the first poll ends it");
    let pawn = r.make("ToyHandle");
    let o = r.action("Turn");
    r.set(o, "MyWillowPawn", Value::Object(Some(pawn)));
    assert_eq!(missing(r.step(o).unwrap_err()), Missing::NotRead);
    let o = r.action("Stopping");
    r.step(o).unwrap();
    assert_eq!(r.trace(o), "a");
    assert_eq!(missing(r.step(o).unwrap_err()), Missing::NotRead);
}

/// The turn rule: within 255 units it ends; a larger error on the same side as before continues; a sign change within
/// a quarter turn ends it; beyond a quarter turn it continues. Yaw is atan2 in 65535 units per turn.
#[test]
fn the_turn_wait_rule_and_the_yaw() {
    assert!(!turn_wait_continues(1000, 800, 0), "within 255");
    assert!(turn_wait_continues(1000, 0, -500), "same side, error 1000");
    assert!(!turn_wait_continues(1000, 2000, 0), "error -1000 after +1000: crossed zero");
    assert!(turn_wait_continues(0, 20000, -100), "error beyond a quarter turn");
    assert!(turn_wait_continues(0, -300, 40000), "a wrapped previous error beyond a quarter turn");
    assert_eq!(yaw_of(1.0, 0.0), 0);
    assert_eq!(yaw_of(0.0, 1.0), 16384);
    assert_eq!(yaw_of(-1.0, 0.0), 32768);
}

/// The step's parent notice when the code runs out under a `ParentSequence` is not read.
#[test]
fn a_child_running_out_of_code_names_the_parent_notice() {
    let mut r = start();
    let parent = r.make("ToyAction");
    let o = r.action("Uninit");
    r.set(o, "ParentSequence", Value::Object(Some(parent)));
    r.step(o).unwrap();
    r.set(o, "bInitialized", Value::Bool(true));
    assert_eq!(missing(r.step(o).unwrap_err()), Missing::NotRead);
    // Without a parent the code simply runs out.
    let o = r.action("Uninit");
    r.step(o).unwrap();
    r.set(o, "bInitialized", Value::Bool(true));
    r.step(o).unwrap();
    assert!(!ow_vm::states::has_state_code(&mut r.vm, o).unwrap());
}

/// `GetTargetRecord` is `GetTargetRecordFor(CurrentTarget)`; the first record whose `Target` is the actor, or None.
#[test]
fn target_records_are_found_in_targets() {
    let mut r = start();
    let c = r.make("ToyComponent");
    let (a, b) = (r.make("ToyHandle"), r.make("ToyHandle"));
    let (ra, rb) = (r.make("ToyRecord"), r.make("ToyRecord"));
    r.set(ra, "Target", Value::Object(Some(a)));
    r.set(rb, "Target", Value::Object(Some(b)));
    r.set(c, "Targets", Value::array(vec![Value::Object(None), Value::Object(Some(ra)), Value::Object(Some(rb))]));
    let get = |r: &mut Run, args| r.call(c, "Core.ToyComponent.GetTargetRecordFor", args).unwrap();
    assert_eq!(get(&mut r, vec![Value::Object(Some(b))]), Value::Object(Some(rb)));
    assert_eq!(get(&mut r, vec![Value::Object(Some(b)), Value::Bool(true)]), Value::Object(Some(rb)), "no parent here");
    assert_eq!(get(&mut r, vec![Value::Object(None)]), Value::Object(None));
    assert_eq!(get(&mut r, vec![Value::Object(Some(c))]), Value::Object(None), "no record for it");
    assert_eq!(r.call(c, "Core.ToyComponent.GetTargetRecord", vec![]).unwrap(), Value::Object(None), "no current target");
    r.set(c, "CurrentTarget", Value::Object(Some(a)));
    assert_eq!(r.call(c, "Core.ToyComponent.GetTargetRecord", vec![]).unwrap(), Value::Object(Some(ra)));
}

/// The mind's queries: the branches the note settles, and `NotYet { NotRead }` past them.
#[test]
fn the_mind_queries_answer_what_the_note_settles() {
    let mut r = start();
    let m = r.make("ToyMind");
    let x = r.make("ToyHandle");
    let ask = |r: &mut Run, name: &str| r.call(m, &format!("Core.ToyMind.{name}"), vec![]);
    assert_eq!(ask(&mut r, "IsTooFarFromHome").unwrap(), Value::Bool(false), "no Pawn");
    r.set(m, "Pawn", Value::Object(Some(x)));
    assert_eq!(missing(ask(&mut r, "IsTooFarFromHome").unwrap_err()), Missing::NotRead, "the den");
    assert_eq!(ask(&mut r, "IsInScriptedMovement").unwrap(), Value::Bool(false));
    r.set(m, "bScriptedHoldPosition", Value::Bool(true));
    assert_eq!(ask(&mut r, "IsInScriptedMovement").unwrap(), Value::Bool(true));
    r.set(m, "FollowActor", Value::Object(Some(x)));
    assert_eq!(missing(ask(&mut r, "IsInScriptedMovement").unwrap_err()), Missing::NotRead);
    assert_eq!(ask(&mut r, "ShouldLookAtPlayer").unwrap(), Value::Bool(false), "no MyWillowPawn or AIClass");
    r.set(m, "MyWillowPawn", Value::Object(Some(x)));
    r.set(m, "AIClass", Value::Object(Some(x)));
    assert_eq!(missing(ask(&mut r, "ShouldLookAtPlayer").unwrap_err()), Missing::NotRead);
    assert_eq!(missing(den_not_read()), Missing::NotRead);
}

/// `register` binds the stock paths in Off and nothing under Reference (its list, [`bodies`]).
#[test]
fn register_binds_the_stock_paths_in_off_only() {
    let mut r = start();
    let off: Vec<&str> = bodies(&r.vm).into_iter().map(|(p, _)| p).collect();
    assert_eq!(off.len(), 14);
    for path in [SLEEP, WAIT_FOR_PATH, FOLLOW_PATH, FINISH_PATH, WAIT_FOR_TURN, WAIT_FOR_STOP, TARGET_RECORD, POP_OP_DEN] {
        assert!(off.iter().any(|p| *p == path), "{path}");
    }
    r.vm.parity = ParityMode::Reference;
    assert!(bodies(&r.vm).is_empty(), "nothing under Reference");
}

/// Review r1 of #397: `bParent` on a component with a parent is `NotYet` (the den's records are not modelled); without
/// `bParent`, or on a lone component, the component's own list answers. The natives ask the awareness state, which
/// knows no parent for a component it does not hold.
#[test]
fn b_parent_on_a_component_with_a_parent_is_not_yet() {
    let mut r = start();
    let c = r.make("ToyComponent");
    let a = r.make("ToyHandle");
    let ra = r.make("ToyRecord");
    r.set(ra, "Target", Value::Object(Some(a)));
    r.set(c, "Targets", Value::array(vec![Value::Object(Some(ra))]));
    let parent = record_for_parent(&mut r.vm, c, Some(a), true, true).unwrap_err();
    assert_eq!(missing(parent), Missing::NotPorted);
    assert_eq!(record_for_parent(&mut r.vm, c, Some(a), false, true).unwrap(), Value::Object(Some(ra)), "no bParent");
    assert_eq!(record_for_parent(&mut r.vm, c, Some(a), true, false).unwrap(), Value::Object(Some(ra)), "a lone component");
    assert_eq!(record_for_parent(&mut r.vm, c, None, true, true).unwrap(), Value::Object(None), "no actor");
    assert!(!crate::awareness::vm::component_has_parent(&mut r.vm, c).unwrap(), "no awareness state: lone");
    let got = r.call(c, "Core.ToyComponent.GetTargetRecordFor", vec![Value::Object(Some(a)), Value::Bool(true)]).unwrap();
    assert_eq!(got, Value::Object(Some(ra)), "through the native, lone");
}
