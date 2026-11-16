//! Tests of the action adapter on invented classes (AGENTS.md rule 9), in both parity modes, over part 1's invented
//! world (`super::super::vm::tests`). The action events are script of an invented package class calling invented
//! natives.
//! AI-assisted (Claude Code).

use std::cell::RefCell;
use std::rc::Rc;

use ow_combat::hit::access::{get, set};
use ow_core::{DetMap, Missing, ObjectPath, ParityMode};
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};

use super::super::vm::tests::{Env, class, decl, env, env_with, host, make};
use super::*;

/// What the invented actions answer and what they saw.
#[derive(Default)]
struct Script {
    log: Vec<String>,
    /// Action name -> its CanRun failure while there is no target.
    fails: DetMap<String, String>,
    has_target: bool,
    /// Whether the patrol finishes at its next Update.
    patrol_done: bool,
    /// Whether the patrol's Start calls Finish at once.
    start_finishes: bool,
    /// Whether the patrol's Start enters the state `Busy`, and whether Stop leaves it again.
    start_enters: bool,
    stop_resets: bool,
    /// Whether the patrol finishes when it hears `Target_Acquired`.
    finish_on_acquired: bool,
}

fn name_of(vm: &Vm, o: ObjRef) -> String {
    vm.space.heap.get(o).map(|h| vm.space.names.text(h.name).into_owned()).unwrap_or_default()
}

fn declare(
    vm: &mut Vm,
    owner: ClassId,
    path: &str,
    params: Vec<PropertyDecl>,
    result: Option<PropKind>,
) -> ow_object::ids::FuncId {
    let mut d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), path).with_owner(owner);
    d.params = params.into_iter().map(|p| p.with_flags(PropertyDecl::PARAM)).collect();
    d.result = result.map(|k| decl(vm, "ReturnValue", k).with_flags(PropertyDecl::RETURN));
    vm.declare_function(d).unwrap()
}

struct World {
    e: Env,
    script: Rc<RefCell<Script>>,
    action_class: ClassId,
}

const NATIVE: u32 = 0x400 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;

/// `ToyActions.ToyAction`: an invented action class whose events are script. Each event calls the native
/// `Note(Name)` with its own name (ActivateEvent with the event's name), and `CanRun` returns the native `Answer()`.
/// `ToyActions.ToyTimedAction`, its subclass, declares an `Update(float DeltaTime)` the stock events do not have.
fn toy_actions() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Object", "Name", "Bool", "Float"]);
    let core = t.p.add_import("Core", "Package", 0, "Core");
    let object = t.p.add_import("Core", "Class", core, "Object");
    let class = t.class("ToyAction", object);
    t.prop("Object", class, "MyWillowPawn", 0, object);
    t.prop("Object", class, "MyWillowMind", 0, object);
    let note = t
        .function(class, "Note", &[Param::new("Name", "What")], None, &[], &FunctionSpec::new(NATIVE, "Note"), |_| None)
        .function;
    let answer = t
        .function(class, "Answer", &[], Some(("Name", 0)), &[], &FunctionSpec::new(NATIVE, "Answer"), |_| None)
        .function;
    for event in ["Init", "Start", "Stop", "Update"] {
        let mut p = t.p.clone();
        let name = p.fname(event, 0);
        let _ = p;
        t.function(class, event, &[], None, &[], &FunctionSpec::new(SCRIPT, event), |_| {
            let mut a = Asm::new();
            a.stmt().final_function(note, |a| {
                a.raw(&[0x21]).raw(&name);
            });
            a.stmt().return_nothing();
            a.end();
            Some(a)
        });
    }
    t.function(
        class,
        "ActivateEvent",
        &[Param::new("Name", "EventName")],
        None,
        &[],
        &FunctionSpec::new(SCRIPT, "ActivateEvent"),
        |ids| {
            let mut a = Asm::new();
            let param = ids.get("EventName");
            a.stmt().final_function(note, |a| {
                a.local(param);
            });
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    t.function(class, "CanRun", &[], Some(("Name", 0)), &[], &FunctionSpec::new(SCRIPT, "CanRun"), |_| {
        let mut a = Asm::new();
        a.stmt().raw(&[0x04]).final_function(answer, |_| {});
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    let timed = t.class("ToyTimedAction", class);
    t.function(timed, "Update", &[Param::new("Float", "DeltaTime")], None, &[], &FunctionSpec::new(SCRIPT, "Update"), |_| {
        let mut a = Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.build()
}

/// Part 1's world, a tree of a root priority node over Attack (node 1) and Patrol (node 2) of the toy action class:
/// Attack cannot run without a target; Patrol always can; the patrol finishes from its Update when told to.
fn world(mode: ParityMode) -> World {
    let mut e = env_with(mode, &[("ToyActions", toy_actions())]);
    let vm = &mut e.vm;
    let action_class = vm.find_class("ToyActions.ToyAction").unwrap();
    let script = Rc::new(RefCell::new(Script::default()));
    script.borrow_mut().fails.insert("Attack".to_string(), "NoTarget".to_string());
    let s = script.clone();
    let note = vm.find_function("ToyActions.ToyAction.Note").unwrap();
    let key = vm.function(note).unwrap().native_key.clone().unwrap();
    vm.register_native(
        &key,
        Rc::new(move |c: &mut NativeCall<'_>| {
            let me = c.this.unwrap();
            let name = name_of(c.vm, me);
            let what = match c.arg(0)? {
                Value::Name(n) => c.vm.space.names.text(*n).into_owned(),
                _ => String::new(),
            };
            if what == "Target_Acquired" {
                s.borrow_mut().has_target = true;
            }
            s.borrow_mut().log.push(format!("{what} {name}"));
            let (enters, resets, finishes, on_acquired) = {
                let s = s.borrow();
                (s.start_enters, s.stop_resets, s.start_finishes, s.finish_on_acquired)
            };
            if name == "Patrol" && what == "Target_Acquired" && on_acquired {
                let f = c.vm.find_function(FINISH)?;
                c.vm.call(f, Some(me), vec![Value::Bool(false)])?;
            }
            if name == "Patrol" && what == "Start" && enters {
                let busy = c.vm.space.names.intern("Busy");
                c.vm.space.heap.get_mut(me)?.state = Some(busy);
            }
            if name == "Patrol" && what == "Stop" && resets {
                c.vm.space.heap.get_mut(me)?.state = None;
            }
            if name == "Patrol" && what == "Start" && finishes {
                let f = c.vm.find_function(FINISH)?;
                c.vm.call(f, Some(me), vec![Value::Bool(false)])?;
            }
            // The patrol's script finishes it from Update when told to (as `AttackFinished` calls Finish).
            if what == "Update" && name == "Patrol" && s.borrow().patrol_done {
                let f = c.vm.find_function(FINISH)?;
                c.vm.call(f, Some(me), vec![Value::Bool(false)])?;
            }
            Ok(Value::None)
        }),
    )
    .unwrap();
    let s = script.clone();
    let answer = vm.find_function("ToyActions.ToyAction.Answer").unwrap();
    let key = vm.function(answer).unwrap().native_key.clone().unwrap();
    vm.register_native(
        &key,
        Rc::new(move |c: &mut NativeCall<'_>| {
            let name = name_of(c.vm, c.this.unwrap());
            let fail = if s.borrow().has_target { None } else { s.borrow().fails.get(&name).cloned() };
            Ok(Value::Name(c.vm.space.names.intern(fail.as_deref().unwrap_or("None"))))
        }),
    )
    .unwrap();
    let failed = decl(vm, "bFailed", PropKind::Bool).with_flags(PropertyDecl::OPTIONAL);
    declare(vm, action_class, FINISH, vec![failed], None);
    Pack.register(vm).unwrap();
    // The tree.
    let attack = make(vm, action_class, "Attack");
    let patrol = make(vm, action_class, "Patrol");
    let node = |vm: &mut Vm, kind: u8, children: Vec<i32>, action: Option<ObjRef>| {
        let mut v = vm.space.classes.default_value(&PropKind::Struct(e.tree_node)).unwrap();
        ow_combat::hit::access::set_field(vm, &mut v, "Type", Value::Byte(kind));
        let conds = vec![Value::Object(None); children.len()];
        ow_combat::hit::access::set_field(
            vm,
            &mut v,
            "Children",
            Value::Array(Rc::new(children.into_iter().map(Value::Int).collect())),
        );
        ow_combat::hit::access::set_field(vm, &mut v, "Conditions", Value::Array(Rc::new(conds)));
        ow_combat::hit::access::set_field(vm, &mut v, "bInterrupt", Value::Bool(true));
        ow_combat::hit::access::set_field(vm, &mut v, "Action", Value::Object(action));
        v
    };
    let nodes =
        vec![node(vm, 3, vec![1, 2], None), node(vm, 1, vec![], Some(attack)), node(vm, 1, vec![], Some(patrol))];
    set(vm, e.aidef, "NodeList", Value::Array(Rc::new(nodes))).unwrap();
    World { e, script, action_class }
}

fn brain_at(w: &mut World, h: &mut super::super::vm::tests::ToyHost, t: f32) -> Result<BrainReport, VmError> {
    let dt = t - w.e.vm.clock.time_seconds();
    w.e.vm.clock.advance(dt);
    tick_brain(&mut w.e.vm, h, w.e.mind, w.e.globals)
}

/// Perceive, choose, start: patrol without a target; on acquisition the events go to the running action first; the
/// state update then needs the lock rule (NotYet, clean room) while the dry selection names the attack; the patrol's
/// own Finish hands over to the attack at once.
#[test]
fn the_brain_perceives_chooses_and_starts() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut w = world(mode);
        let mut h = host(&w.e, 1.0);
        let first = brain_at(&mut w, &mut h, 0.25).unwrap();
        assert!(first.built);
        assert_eq!(
            first.changes.last().and_then(|c| c.started),
            Some(2),
            "{mode:?}: patrol; log {:?}; report {:?}",
            w.script.borrow().log,
            first
        );
        let b = brain(&w.e.vm, w.e.mind).unwrap();
        let patrol = b.actions[2].unwrap();
        assert_eq!(get(&mut w.e.vm, patrol, "MyWillowPawn").unwrap(), Some(Value::Object(Some(w.e.ai))));
        assert_eq!(w.script.borrow().log[..2], ["Init Attack", "Init Patrol"]);
        // The player is noticed: the events, running action first; the state update stops at the lock rule.
        let err = brain_at(&mut w, &mut h, 0.5).unwrap_err();
        assert!(matches!(&err, VmError::NotYet(n) if n.missing == Missing::NotPorted), "{mode:?}: {err}");
        let log = w.script.borrow().log.clone();
        let acquired: Vec<&String> = log.iter().filter(|l| l.starts_with("Target_Acquired")).collect();
        assert_eq!(acquired, ["Target_Acquired Patrol", "Target_Acquired Attack"]);
        assert_eq!(brain(&w.e.vm, w.e.mind).unwrap().tree.current, Some(2), "nothing lost");
        let (picked, fails) = dry_select(&mut w.e.vm, w.e.mind).unwrap();
        assert_eq!((picked, fails), (Some(1), vec![]), "the attack would be picked");
        // The patrol finishes from its own script: the attack starts at once (no lock question on that path).
        w.script.borrow_mut().patrol_done = true;
        let _ = brain_at(&mut w, &mut h, 0.625);
        assert_eq!(brain(&w.e.vm, w.e.mind).unwrap().tree.current, Some(1), "{mode:?}");
        assert!(w.script.borrow().log.iter().any(|l| l == "Start Attack"));
    }
}

/// The component's holds (G26): a hold needs a tag and is not doubled; a release removes the pair.
#[test]
fn holds_are_name_and_tag_pairs() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut e = env(mode);
        let vm = &mut e.vm;
        let (hn, tg) = (decl(vm, "HoldName", PropKind::Name), decl(vm, "Tag", PropKind::Object { class: e.object }));
        let hold = vm.space.classes.define_struct("Toy.ToyObject.ToyHold", None, vec![hn, tg]).unwrap();
        let holder =
            class(vm, "Toy.ToyHolder", e.object, &[("Holds", PropKind::Array(Box::new(PropKind::Struct(hold))))]);
        let name = decl(vm, "HoldName", PropKind::Name);
        let tag = decl(vm, "Tag", PropKind::Object { class: e.object });
        let hold_f = declare(vm, holder, "GearboxFramework.AIComponent.Hold", vec![name.clone(), tag.clone()], None);
        let release_f = declare(vm, holder, "GearboxFramework.AIComponent.Release", vec![name, tag], None);
        let on_hold_f = declare(vm, holder, "GearboxFramework.AIComponent.OnHold", vec![], Some(PropKind::Bool));
        Pack.register(vm).unwrap();
        let component = make(vm, holder, "ToyComponent_9");
        let t = make(vm, e.object, "ToyTag");
        let n = Value::Name(vm.space.names.intern("Attack"));
        let on = |vm: &mut Vm| vm.call(on_hold_f, Some(component), vec![]).unwrap().ret;
        assert_eq!(on(vm), Value::Bool(false));
        vm.call(hold_f, Some(component), vec![n.clone(), Value::Object(None)]).unwrap();
        assert_eq!(on(vm), Value::Bool(false), "{mode:?}: no tag, no hold");
        vm.call(hold_f, Some(component), vec![n.clone(), Value::Object(Some(t))]).unwrap();
        vm.call(hold_f, Some(component), vec![n.clone(), Value::Object(Some(t))]).unwrap();
        assert_eq!(ow_combat::hit::access::array(vm, component, "Holds").unwrap().len(), 1, "not doubled");
        assert_eq!(on(vm), Value::Bool(true));
        vm.call(release_f, Some(component), vec![n, Value::Object(Some(t))]).unwrap();
        assert_eq!(on(vm), Value::Bool(false));
    }
}

struct ToyMoves {
    calls: Vec<String>,
}

impl Locomotion for ToyMoves {
    fn create_path(&mut self, _vm: &mut Vm, _handle: ObjRef, dest: Vec3) -> Result<bool, VmError> {
        self.calls.push(format!("path {dest:?}"));
        Ok(true)
    }
    fn clear_path(&mut self, _vm: &mut Vm, _handle: ObjRef, _stop: bool) -> Result<bool, VmError> {
        Ok(true)
    }
    fn path_is_valid(&mut self, _vm: &mut Vm, _pawn: ObjRef) -> Result<bool, VmError> {
        Ok(true)
    }
    fn invalidate_path(&mut self, _vm: &mut Vm, _pawn: ObjRef) -> Result<(), VmError> {
        Ok(())
    }
    fn reached_destination(&mut self, _vm: &mut Vm, _pawn: ObjRef) -> Result<bool, VmError> {
        Ok(false)
    }
    fn can_move_to(&mut self, vm: &mut Vm, pawn: ObjRef, goal: Vec3) -> Result<bool, VmError> {
        self.calls.push(format!("can move {} to {goal:?}", name_of(vm, pawn)));
        Ok(true)
    }
    fn set_movement_speed(&mut self, _vm: &mut Vm, _pawn: ObjRef, speed: u8) -> Result<(), VmError> {
        self.calls.push(format!("speed {speed}"));
        Ok(())
    }
    fn nearest_on_navmesh(&mut self, _vm: &mut Vm, _pawn: ObjRef) -> Result<Option<Vec3>, VmError> {
        Ok(None)
    }
}

/// Movement goes to the host for the action's pawn; a latent move is `NotYet` (an ow-vm seam); the natives this lane
/// read as analyst are `NotYet` outside Reference and unbound under it.
#[test]
fn movement_goes_to_the_host_and_the_unread_is_not_yet() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut w = world(mode);
        let vm = &mut w.e.vm;
        let goal = decl(vm, "GoalPoint", PropKind::Struct(w.e.vector));
        let can_move = declare(
            vm,
            w.action_class,
            "WillowGame.WillowActionSequencePawn.CanMoveTo",
            vec![goal],
            Some(PropKind::Bool),
        );
        let follow = declare(vm, w.action_class, "WillowGame.WillowActionSequencePawn.FollowPath", vec![], None);
        let atomic = declare(vm, w.action_class, "GearboxFramework.ActionSequence.StartAtomic", vec![], None);
        Pack.register(vm).unwrap();
        let action = make(vm, w.action_class, "Mover");
        set(vm, action, "MyWillowPawn", Value::Object(Some(w.e.ai))).unwrap();
        let goal = super::super::vm::tests::vec3(w.e.vector, [100.0, 0.0, 0.0]);
        let err = vm.call(can_move, Some(action), vec![goal.clone()]).unwrap_err();
        assert!(matches!(err, VmError::NotYet(n) if matches!(n.missing, Missing::LayerAbsent(_))), "no host yet");
        let moves = Rc::new(RefCell::new(ToyMoves { calls: vec![] }));
        install_locomotion(vm, moves.clone()).unwrap();
        assert_eq!(vm.call(can_move, Some(action), vec![goal]).unwrap().ret, Value::Bool(true));
        assert_eq!(moves.borrow().calls, ["can move ToyBeast_0 to [100.0, 0.0, 0.0]"]);
        let err = vm.call(follow, Some(action), vec![]).unwrap_err();
        assert!(matches!(err, VmError::NotYet(n) if matches!(n.missing, Missing::LayerAbsent(_))), "{mode:?}: latent");
        let key = vm.function(atomic).unwrap().native_key.clone().unwrap();
        match mode {
            ParityMode::Off => {
                let err = vm.call(atomic, Some(action), vec![]).unwrap_err();
                assert!(matches!(err, VmError::NotYet(n) if n.missing == Missing::NotPorted));
            }
            _ => assert!(!vm.has_native(&key), "unbound under Reference"),
        }
    }
}

/// A Start that calls Finish at once, with no TimedReset, would be selected and finish again for ever: the
/// application of the queued finishes is bounded and names the action (review r1 M3).
#[test]
fn a_start_that_finishes_at_once_is_bounded() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut w = world(mode);
        let mut h = host(&w.e, 1.0);
        w.script.borrow_mut().start_finishes = true;
        let err = brain_at(&mut w, &mut h, 0.25).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("ToyAction") && text.contains("node 2") && text.contains("finished more than"), "{text}");
        let starts = w.script.borrow().log.iter().filter(|l| *l == "Start Patrol").count();
        assert!(starts > 1 && starts < 30, "{mode:?}: {starts} starts");
    }
}

/// After Stop the engine puts the action back in its default state (not read): an action Stop leaves in another
/// state than its rest state is NotYet, and the tree stays as it was; one Stop brings back is stopped (review r1 M2).
#[test]
fn stop_must_leave_the_action_in_its_rest_state() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut w = world(mode);
        let mut h = host(&w.e, 1.0);
        w.script.borrow_mut().start_enters = true;
        brain_at(&mut w, &mut h, 0.25).unwrap();
        assert_eq!(brain(&w.e.vm, w.e.mind).unwrap().tree.current, Some(2));
        w.script.borrow_mut().patrol_done = true;
        let err = brain_at(&mut w, &mut h, 0.3125).unwrap_err();
        assert!(matches!(&err, VmError::NotYet(n) if n.missing == Missing::NotRead), "{mode:?}: {err}");
        assert!(err.to_string().contains("in state Busy after Stop"), "{err}");
        assert_eq!(brain(&w.e.vm, w.e.mind).unwrap().tree.current, Some(2), "{mode:?}: the tree as it was");
        w.script.borrow_mut().stop_resets = true;
        let report = brain_at(&mut w, &mut h, 0.375).unwrap();
        assert_eq!(report.changes.first().and_then(|c| c.stopped), Some((2, false)), "{mode:?}: {report:?}");
    }
}

/// A brain that fails to build is not built again each tick (review r1 L3): the error is answered again and the
/// archetypes are not copied again.
#[test]
fn a_failed_build_is_kept() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut w = world(mode);
        let mut h = host(&w.e, 1.0);
        let vm = &mut w.e.vm;
        let mut nodes = ow_combat::hit::access::array(vm, w.e.aidef, "NodeList").unwrap();
        ow_combat::hit::access::set_field(vm, &mut nodes[2], "Type", Value::Byte(9));
        set(vm, w.e.aidef, "NodeList", Value::Array(Rc::new(nodes))).unwrap();
        let first = brain_at(&mut w, &mut h, 0.25).unwrap_err();
        assert!(first.to_string().contains("node 2 has the node type 9"), "{first}");
        let objects = w.e.vm.space.heap.len();
        let again = brain_at(&mut w, &mut h, 0.5).unwrap_err();
        assert_eq!(again, first, "{mode:?}");
        assert_eq!(w.e.vm.space.heap.len(), objects, "{mode:?}: nothing copied again");
        assert!(w.script.borrow().log.is_empty(), "no Init raised");
    }
}

/// An event is raised with exactly the parameters it declares: one that declares more is an error, not a silent
/// zero (review r1 M1: the stock `Update` declares none).
#[test]
fn an_event_with_undeclared_parameters_is_an_error() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut w = world(mode);
        let vm = &mut w.e.vm;
        let timed = vm.find_class("ToyActions.ToyTimedAction").unwrap();
        let action = make(vm, timed, "Timed");
        let err = event(vm, action, "Update", vec![]).unwrap_err();
        assert!(err.to_string().contains("Update declares 1 parameters and the adapter passes 0"), "{mode:?}: {err}");
        assert_eq!(event(vm, action, "Update", vec![Value::Float(0.0625)]).unwrap(), Value::None);
    }
}

/// A Finish called from an event handler takes effect before the running action's Update: the finished patrol gets
/// no Update, the attack it hands over to does (review r1 L6).
#[test]
fn a_finish_from_an_event_comes_before_the_update() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut w = world(mode);
        let mut h = host(&w.e, 1.0);
        brain_at(&mut w, &mut h, 0.25).unwrap();
        w.script.borrow_mut().finish_on_acquired = true;
        w.script.borrow_mut().log.clear();
        let _ = brain_at(&mut w, &mut h, 0.5);
        let log = w.script.borrow().log.clone();
        let at = log.iter().position(|l| l == "Target_Acquired Patrol").expect("the event");
        assert!(!log[at..].iter().any(|l| l == "Update Patrol"), "{mode:?}: {log:?}");
        assert!(log[at..].iter().any(|l| l == "Update Attack"), "{mode:?}: {log:?}");
    }
}
