//! Owner: lane VM-LATENT-HOST. AI-assisted (Claude Code). Host latent actions on an invented "Core" package
//! (AGENTS.md rule 9): a `Walker` actor whose state code calls two host latent natives, `MoveToward(Goal)` (finished by
//! its poll) and `FinishAnim()` (finished by the host from outside), over a fake host world that moves one unit per
//! second. Every value here is invented.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{Asm, CPF_OPT, FunctionSpec, Param, ScriptToy};
use ow_package::testkit::{Export, ints};

use super::{
    HostLatentKind, HostLatentPoll, LatentAction, LatentStatus, LatentTicket, finish_host_latent,
    host_latent_kind, host_latent_of, register_host_latent, start_host_latent,
};
use crate::api::{StubPolicy, Vm};
use crate::error::VmError;
use crate::natives_core;
use crate::states::{self, process_state};

const FUNC_FINAL: u32 = 0x1;
const FUNC_NATIVE: u32 = 0x400;
const FUNC_PUBLIC: u32 = 0x20000;
const NATIVE: u32 = FUNC_NATIVE | FUNC_FINAL | FUNC_PUBLIC;
const OPERATOR: u32 = FUNC_NATIVE | 0x1000 | 0x23000;

/// A quarter second: exact in binary, so the positions below are exact.
const DT: f32 = 0.25;

/// The fake host's speed, units per second.
const SPEED: f32 = 1.0;

fn param(kind: &str, name: &str, flags: u64) -> Param {
    Param {
        flags,
        ..Param::new(kind, name)
    }
}

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], number: u16) -> i32 {
    let mut s = FunctionSpec::new(NATIVE, name);
    s.native = number;
    t.function(owner, name, params, None, &[], &s, |_| None).function
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

/// The label table and `EndOfScript`; the body and the label table offset (one past the `LabelTable` token).
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

/// A State export of `owner` with no parent and code `code`, in the layout `states::read_state_body` reads.
fn state(t: &mut ScriptToy, owner: i32, name: &str, code: &(Asm, u16)) -> i32 {
    let none = t.none;
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&none);
    payload.extend_from_slice(&ints(&[0, 0, 0, 0, 0, 0, 0]));
    let (a, label) = code;
    payload.extend_from_slice(&ints(&[
        i32::try_from(a.memory_size()).unwrap(),
        i32::try_from(a.bytes.len()).unwrap(),
    ]));
    payload.extend_from_slice(&a.bytes);
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&label.to_le_bytes());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&ints(&[0]));
    let mut export = Export::new(t.import("State"), name, payload);
    export.outer = owner;
    t.p.add_export_with(export)
}

/// `Object` (`$`, `GotoState`), `Actor`, and `Walker` (`Trace`, `Goal`, the natives `MoveToward(float)` and
/// `FinishAnim()`, and two states). Walking: Begin: "a"; MoveToward(Goal); "b"; FinishAnim(); "c"; Stop.
/// Idle: Begin: "i"; Stop.
fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "State", "StrProperty", "NameProperty",
        "FloatProperty", "BoolProperty"]);
    let object = t.class("Object", 0);
    let mut concat = FunctionSpec::new(OPERATOR, "$");
    concat.native = 112;
    t.function(
        object,
        "Concat_StrStr",
        &[param("Str", "A", 0), param("Str", "B", 0)],
        Some(("Str", 0)),
        &[],
        &concat,
        |_| None,
    );
    native(
        &mut t,
        object,
        "GotoState",
        &[
            param("Name", "NewState", CPF_OPT),
            param("Name", "Label", CPF_OPT),
            param("Bool", "bForceEvents", CPF_OPT),
            param("Bool", "bKeepStack", CPF_OPT),
        ],
        113,
    );
    let actor = t.class("Actor", object);
    let walker = t.class("Walker", actor);
    let trace = t.prop("Str", walker, "Trace", 0, 0);
    let goal = t.prop("Float", walker, "Goal", 0, 0);
    let move_toward = native(&mut t, walker, "MoveToward", &[param("Float", "NewGoal", 0)], 0);
    let finish_anim = native(&mut t, walker, "FinishAnim", &[], 0);
    let move_twice = native(&mut t, walker, "MoveTwice", &[param("Float", "NewGoal", 0)], 0);

    let walking = {
        let mut a = Asm::new();
        let begin = a.here();
        append(&mut a, trace, "a");
        a.stmt().final_function(move_toward, |a| {
            a.instance(goal);
        });
        append(&mut a, trace, "b");
        a.stmt().final_function(finish_anim, |_| {});
        append(&mut a, trace, "c");
        a.stmt().raw(&[0x08]);
        finish(&mut t, a, &[("Begin", begin)])
    };
    state(&mut t, walker, "Walking", &walking);
    let idle = {
        let mut a = Asm::new();
        let begin = a.here();
        append(&mut a, trace, "i");
        a.stmt().raw(&[0x08]);
        finish(&mut t, a, &[("Begin", begin)])
    };
    state(&mut t, walker, "Idle", &idle);
    // Double: Begin: "a"; MoveTwice(Goal) (one native that starts two host calls); "b"; Stop.
    let double = {
        let mut a = Asm::new();
        let begin = a.here();
        append(&mut a, trace, "a");
        a.stmt().final_function(move_twice, |a| {
            a.instance(goal);
        });
        append(&mut a, trace, "b");
        a.stmt().raw(&[0x08]);
        finish(&mut t, a, &[("Begin", begin)])
    };
    state(&mut t, walker, "Double", &double);
    t.build()
}

/// What the move poll does besides moving the walker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Poll {
    /// Moves, and answers `Done` at the goal.
    #[default]
    Normal,
    /// Sends the walker to `Idle` instead of moving it, and answers `Pending`.
    Redirect,
    /// Finishes its own call with `finish_host_latent`, and answers `Pending`.
    FinishSelf,
    /// Answers `Err` on its first poll, then moves normally.
    ErrOnce,
    /// Marks its walker destroyed, and answers `Pending`.
    DestroySelf,
    /// Looks a kind up and registers a new one (re-entering the registry), then moves normally.
    Reenter,
}

/// The fake host world: each walker's position and goal, its label, and a log of every start and poll.
#[derive(Default)]
struct World {
    walkers: Vec<(ObjRef, String, f32, f32)>,
    anim: Vec<(ObjRef, LatentTicket)>,
    log: Vec<String>,
    poll: Poll,
    polls: u32,
}

impl World {
    fn label(&self, o: ObjRef) -> String {
        self.walkers
            .iter()
            .find(|w| w.0 == o)
            .map_or_else(|| "?".to_string(), |w| w.1.clone())
    }
}

/// Registers the two host latent kinds and their natives, as a host would.
fn host(vm: &mut Vm, poll: Poll) -> Rc<RefCell<World>> {
    let world = Rc::new(RefCell::new(World {
        poll,
        ..World::default()
    }));

    let w = Rc::clone(&world);
    let move_kind = register_host_latent(
        vm,
        "MoveToward",
        Rc::new(move |vm: &mut Vm, o, ticket, delta| {
            let (mode, first) = {
                let mut w = w.borrow_mut();
                w.polls += 1;
                (w.poll, w.polls == 1)
            };
            match mode {
                Poll::Normal => {}
                Poll::Redirect => {
                    w.borrow_mut().log.push(format!("redirect#{}", ticket.get()));
                    let f = vm.find_function("Core.Object.GotoState")?;
                    let idle = Value::Name(vm.space.names.intern("Idle"));
                    vm.call(f, Some(o), vec![idle])?;
                    return Ok(LatentStatus::Pending);
                }
                Poll::FinishSelf => {
                    let finished = finish_host_latent(vm, o, ticket)?;
                    w.borrow_mut().log.push(format!("finish-self#{}:{finished}", ticket.get()));
                    return Ok(LatentStatus::Pending);
                }
                Poll::ErrOnce if first => {
                    w.borrow_mut().log.push(format!("err#{}", ticket.get()));
                    return Err(VmError::Script {
                        function: "the test host".to_string(),
                        message: "an invented host failure".to_string(),
                    });
                }
                Poll::ErrOnce => {}
                Poll::DestroySelf => {
                    vm.space.heap.get_mut(o)?.destroyed = true;
                    w.borrow_mut().log.push(format!("destroy#{}", ticket.get()));
                    return Ok(LatentStatus::Pending);
                }
                Poll::Reenter => {
                    let known = host_latent_kind(vm, "FinishAnim")?.is_some();
                    let name = format!("Extra{}", ticket.get());
                    let poll: HostLatentPoll = Rc::new(|_: &mut Vm, _, _, _| Ok(LatentStatus::Done));
                    register_host_latent(vm, &name, poll)?;
                    let added = host_latent_kind(vm, &name)?.is_some();
                    w.borrow_mut().log.push(format!("reenter#{}:{known}:{added}", ticket.get()));
                }
            }
            let mut w = w.borrow_mut();
            let label = w.label(o);
            let entry = w.walkers.iter_mut().find(|e| e.0 == o).expect("a walker");
            entry.2 = (entry.2 + SPEED * delta).min(entry.3);
            let (pos, done) = (entry.2, entry.2 >= entry.3);
            w.log.push(format!("{label}:move#{}@{pos}", ticket.get()));
            Ok(if done {
                LatentStatus::Done
            } else {
                LatentStatus::Pending
            })
        }),
    )
    .unwrap();
    let w = Rc::clone(&world);
    vm.register_native(
        "Walker.MoveToward(float)",
        Rc::new(move |c| {
            let Value::Float(goal) = *c.arg(0)? else {
                panic!("MoveToward takes a float");
            };
            let o = c.this.expect("an object");
            let ticket = start_host_latent(c, move_kind)?;
            let mut w = w.borrow_mut();
            let label = w.label(o);
            let entry = w.walkers.iter_mut().find(|e| e.0 == o).expect("a walker");
            entry.3 = goal;
            w.log.push(format!("{label}:start-move#{}", ticket.get()));
            Ok(Value::None)
        }),
    )
    .unwrap();

    let w = Rc::clone(&world);
    vm.register_native(
        "Walker.MoveTwice(float)",
        Rc::new(move |c| {
            let Value::Float(goal) = *c.arg(0)? else {
                panic!("MoveTwice takes a float");
            };
            let o = c.this.expect("an object");
            let first = start_host_latent(c, move_kind)?;
            let second = start_host_latent(c, move_kind)?;
            let mut w = w.borrow_mut();
            let label = w.label(o);
            let entry = w.walkers.iter_mut().find(|e| e.0 == o).expect("a walker");
            entry.3 = goal;
            w.log.push(format!("{label}:start-twice#{}#{}", first.get(), second.get()));
            Ok(Value::None)
        }),
    )
    .unwrap();

    let w = Rc::clone(&world);
    let anim_kind = register_host_latent(
        vm,
        "FinishAnim",
        Rc::new(move |_: &mut Vm, o, ticket, _| {
            let mut w = w.borrow_mut();
            let label = w.label(o);
            w.log.push(format!("{label}:anim#{}", ticket.get()));
            Ok(LatentStatus::Pending)
        }),
    )
    .unwrap();
    let w = Rc::clone(&world);
    vm.register_native(
        "Walker.FinishAnim()",
        Rc::new(move |c| {
            let o = c.this.expect("an object");
            let ticket = start_host_latent(c, anim_kind)?;
            let mut w = w.borrow_mut();
            let label = w.label(o);
            w.anim.push((o, ticket));
            w.log.push(format!("{label}:start-anim#{}", ticket.get()));
            Ok(Value::None)
        }),
    )
    .unwrap();
    world
}

struct Run {
    vm: Vm,
    world: Rc<RefCell<World>>,
    dir: PathBuf,
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A run in `mode`. The host registers its kinds and natives only outside Reference (registration is refused there);
/// under Reference the world stays empty and the latent natives unbound.
fn start_with(mode: ParityMode, poll: Poll) -> Run {
    let dir = std::env::temp_dir().join(format!(
        "ow-vm-latent-host-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Core.upk", &package())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    )
    .with_stub_policy(StubPolicy::refuse_all());
    natives_core::register(&mut vm).unwrap();
    let world = if mode == ParityMode::Reference {
        Rc::new(RefCell::new(World::default()))
    } else {
        host(&mut vm, poll)
    };
    Run { vm, world, dir }
}

fn start(mode: ParityMode) -> Run {
    start_with(mode, Poll::Normal)
}

impl Run {
    /// A walker labelled `label` with goal `goal`, sent to `Walking`.
    fn walker(&mut self, label: &str, goal: f32) -> ObjRef {
        let c = self.vm.find_class("Core.Walker").unwrap();
        let o = self.vm.instantiate(c, "").unwrap();
        self.world
            .borrow_mut()
            .walkers
            .push((o, label.to_string(), 0.0, 0.0));
        self.set(o, "Goal", Value::Float(goal));
        self.goto(o, "Walking");
        o
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let prop = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(prop), v).unwrap();
    }

    fn goto(&mut self, o: ObjRef, state: &str) {
        let f = self.vm.find_function("Core.Object.GotoState").unwrap();
        let state = Value::Name(self.vm.space.names.intern(state));
        self.vm.call(f, Some(o), vec![state]).unwrap();
    }

    fn trace(&self, o: ObjRef) -> String {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let prop = self.vm.space.prop(class, "Trace").unwrap();
        match self.vm.space.read(o, PropAddr::scalar(prop)).unwrap() {
            Value::Str(s) => s.to_string(),
            other => panic!("Trace is {other:?}"),
        }
    }

    fn step(&mut self, o: ObjRef) {
        process_state(&mut self.vm, o, DT).unwrap();
    }

    fn entries(&self) -> Vec<String> {
        self.world.borrow().log.clone()
    }

    fn anim_ticket(&self, o: ObjRef) -> LatentTicket {
        self.world
            .borrow()
            .anim
            .iter()
            .rev()
            .find(|a| a.0 == o)
            .expect("an animation started")
            .1
    }
}

#[test]
fn state_code_suspends_at_a_host_latent_call_and_resumes_when_the_host_completes_it() {
    let mut r = start(ParityMode::Off);
    let w = r.walker("w", 0.5);
    assert_eq!(r.trace(w), "", "state code runs on the next step, not in GotoState");

    // Step 1: "a", then MoveToward starts and the code yields; the poll is not called in the starting step.
    r.step(w);
    assert_eq!(r.trace(w), "a");
    let h = host_latent_of(&mut r.vm, w).unwrap().expect("a pending host call");
    assert_eq!(h.ticket().get(), 1);
    assert!(!h.is_finished());
    assert_eq!(states::latent_of(&mut r.vm, w).unwrap(), Some(LatentAction::Host(h)));
    assert_eq!(r.entries(), ["w:start-move#1"]);

    // Step 2: the poll moves 0.25 of 0.5: still pending.
    r.step(w);
    assert_eq!(r.trace(w), "a");
    // Step 3: the poll reaches the goal: the call ends and the code continues in this step to FinishAnim.
    r.step(w);
    assert_eq!(r.trace(w), "ab");
    assert_eq!(host_latent_of(&mut r.vm, w).unwrap().unwrap().ticket().get(), 2);

    // Step 4: the animation poll answers pending.
    r.step(w);
    assert_eq!(r.trace(w), "ab");

    // The host completes it from outside: the next step continues without polling it.
    let anim = r.anim_ticket(w);
    assert!(finish_host_latent(&mut r.vm, w, anim).unwrap());
    assert!(host_latent_of(&mut r.vm, w).unwrap().unwrap().is_finished());
    assert_eq!(r.trace(w), "ab", "finishing does not run code by itself");
    r.step(w);
    assert_eq!(r.trace(w), "abc");
    assert_eq!(host_latent_of(&mut r.vm, w).unwrap(), None);
    assert!(!states::latent_pending(&mut r.vm, w).unwrap());

    // The code ran out: later steps do nothing and call no poll.
    r.step(w);
    assert_eq!(r.trace(w), "abc");
    assert_eq!(
        r.entries(),
        [
            "w:start-move#1",
            "w:move#1@0.25",
            "w:move#1@0.5",
            "w:start-anim#2",
            "w:anim#2",
        ]
    );
}

#[test]
fn a_state_change_abandons_the_host_call_and_its_ticket() {
    let mut r = start(ParityMode::Off);
    let w = r.walker("w", 0.5);
    r.step(w);
    let ticket = host_latent_of(&mut r.vm, w).unwrap().unwrap().ticket();
    r.goto(w, "Idle");
    assert_eq!(host_latent_of(&mut r.vm, w).unwrap(), None, "GotoState clears the latent action");
    assert!(
        !finish_host_latent(&mut r.vm, w, ticket).unwrap(),
        "an abandoned ticket is not current"
    );
    r.step(w);
    assert_eq!(r.trace(w), "ai");
    assert_eq!(r.entries(), ["w:start-move#1"], "the abandoned call is never polled");
}

#[test]
fn a_state_change_made_by_the_poll_wins_over_its_answer() {
    let mut r = start_with(ParityMode::Off, Poll::Redirect);
    let w = r.walker("w", 0.5);
    r.step(w);
    assert_eq!(r.trace(w), "a");
    // The poll sends the walker to Idle and answers pending: the code continues in Idle in the same step.
    r.step(w);
    assert_eq!(r.trace(w), "ai");
    assert_eq!(host_latent_of(&mut r.vm, w).unwrap(), None);
    assert_eq!(r.entries(), ["w:start-move#1", "redirect#1"]);
}

#[test]
fn a_host_latent_call_is_refused_outside_state_code() {
    let mut r = start(ParityMode::Off);
    let w = r.walker("w", 0.5);
    let f = r.vm.find_function("Core.Walker.MoveToward").unwrap();
    let got = r.vm.call(f, Some(w), vec![Value::Float(1.0)]);
    assert!(
        matches!(&got, Err(VmError::Script { message, .. }) if message.contains("only in the object's state code")),
        "a latent function called from outside state code: {got:?}"
    );
    assert!(r.entries().is_empty(), "the host set-up is not reached");
    assert_eq!(host_latent_of(&mut r.vm, w).unwrap(), None);
}

#[test]
fn host_latent_kinds_are_named_once() {
    let mut r = start(ParityMode::Off);
    let found = host_latent_kind(&mut r.vm, "FinishAnim").unwrap();
    assert!(found.is_some());
    assert_ne!(found, host_latent_kind(&mut r.vm, "MoveToward").unwrap());
    assert_eq!(host_latent_kind(&mut r.vm, "FinishRotation").unwrap(), None);
    let again = register_host_latent(
        &mut r.vm,
        "MoveToward",
        Rc::new(|_: &mut Vm, _, _, _| Ok(LatentStatus::Done)),
    );
    assert!(matches!(again, Err(VmError::Registration { .. })));
    let _: Option<HostLatentKind> = found;
}

#[test]
fn under_reference_no_state_code_runs_so_no_host_call_starts() {
    let mut r = start(ParityMode::Reference);
    let w = r.walker("w", 0.5);
    for _ in 0..3 {
        r.step(w);
    }
    assert_eq!(r.trace(w), "");
    assert!(r.entries().is_empty());
    // A kind cannot be registered there (as `Actor.Sleep` stays unbound), and a lookup finds none.
    let poll: HostLatentPoll = Rc::new(|_: &mut Vm, _, _, _| Ok(LatentStatus::Done));
    assert!(matches!(
        register_host_latent(&mut r.vm, "MoveToward", poll),
        Err(VmError::Registration { .. })
    ));
    assert_eq!(host_latent_kind(&mut r.vm, "MoveToward").unwrap(), None);
}

/// Three walkers with invented goals stepped `steps` times in `order`: the host log and each walker's trace.
fn run_walkers(order: &[usize], steps: usize) -> (Vec<String>, Vec<String>) {
    let mut r = start(ParityMode::Off);
    let walkers: Vec<ObjRef> = [("w0", 0.25), ("w1", 0.75), ("w2", 0.5)]
        .iter()
        .map(|&(label, goal)| r.walker(label, goal))
        .collect();
    for _ in 0..steps {
        for &i in order {
            r.step(walkers[i]);
        }
    }
    let traces = walkers.iter().map(|&w| r.trace(w)).collect();
    (r.entries(), traces)
}

#[test]
fn host_latent_calls_run_in_the_fixed_step_order_and_repeat_exactly() {
    let (log, traces) = run_walkers(&[0, 1, 2], 4);
    assert_eq!(
        log,
        [
            // Step 1: each starts its move; tickets follow the step order.
            "w0:start-move#1",
            "w1:start-move#2",
            "w2:start-move#3",
            // Step 2: w0 arrives and starts its animation in the same step.
            "w0:move#1@0.25",
            "w0:start-anim#4",
            "w1:move#2@0.25",
            "w2:move#3@0.25",
            // Step 3: w2 arrives.
            "w0:anim#4",
            "w1:move#2@0.5",
            "w2:move#3@0.5",
            "w2:start-anim#5",
            // Step 4: w1 arrives.
            "w0:anim#4",
            "w1:move#2@0.75",
            "w1:start-anim#6",
            "w2:anim#5",
        ]
    );
    assert_eq!(traces, ["ab", "ab", "ab"]);

    // The same run again gives the same log, ticket for ticket.
    assert_eq!(run_walkers(&[0, 1, 2], 4), (log, traces));

    // The order is the caller's: stepping in reverse hands out the tickets in reverse.
    let (reversed, _) = run_walkers(&[2, 1, 0], 1);
    assert_eq!(reversed, ["w2:start-move#1", "w1:start-move#2", "w0:start-move#3"]);
}

// ---- review follow-ups of #389 (regression tests) -------------------------------------------------------------

#[test]
fn a_lookup_does_not_add_the_registry_service() {
    let mut r = start(ParityMode::Reference);
    assert_eq!(host_latent_kind(&mut r.vm, "MoveToward").unwrap(), None);
    assert!(r.vm.service::<super::HostLatents>().is_none(), "the lookup made no registry");
}

#[test]
fn a_poll_that_finishes_its_own_call_continues_in_the_same_step() {
    let mut r = start_with(ParityMode::Off, Poll::FinishSelf);
    let w = r.walker("w", 0.5);
    r.step(w);
    assert_eq!(r.trace(w), "a");
    // The poll marks its own ticket finished and answers Pending: finished wins, the code goes on to FinishAnim.
    r.step(w);
    assert_eq!(r.trace(w), "ab");
    assert_eq!(r.entries(), ["w:start-move#1", "finish-self#1:true", "w:start-anim#2"]);
}

#[test]
fn a_poll_error_ends_the_step_and_leaves_the_call_pending_for_the_next() {
    let mut r = start_with(ParityMode::Off, Poll::ErrOnce);
    let w = r.walker("w", 0.25);
    r.step(w);
    let got = process_state(&mut r.vm, w, DT);
    assert!(matches!(got, Err(VmError::Script { .. })), "{got:?}");
    assert_eq!(r.trace(w), "a");
    let h = host_latent_of(&mut r.vm, w).unwrap().expect("still pending");
    assert_eq!(h.ticket().get(), 1);
    assert!(!h.is_finished());
    // The next step polls the same call again; it reaches the goal and the code continues.
    r.step(w);
    assert_eq!(r.trace(w), "ab");
    assert_eq!(
        r.entries(),
        ["w:start-move#1", "err#1", "w:move#1@0.25", "w:start-anim#2"]
    );
}

#[test]
fn an_object_destroyed_while_waiting_reports_no_call_and_is_not_polled() {
    let mut r = start(ParityMode::Off);
    let w = r.walker("w", 0.5);
    r.step(w);
    let ticket = host_latent_of(&mut r.vm, w).unwrap().unwrap().ticket();
    r.vm.space.heap.get_mut(w).unwrap().destroyed = true;
    assert_eq!(host_latent_of(&mut r.vm, w).unwrap(), None, "destroyed, not yet forgotten");
    assert!(!finish_host_latent(&mut r.vm, w, ticket).unwrap());
    r.step(w);
    assert_eq!(r.trace(w), "a");
    assert_eq!(r.entries(), ["w:start-move#1"], "never polled");
}

#[test]
fn an_object_destroyed_inside_its_poll_stops_there() {
    let mut r = start_with(ParityMode::Off, Poll::DestroySelf);
    let w = r.walker("w", 0.5);
    r.step(w);
    let ticket = host_latent_of(&mut r.vm, w).unwrap().unwrap().ticket();
    r.step(w);
    assert_eq!(r.trace(w), "a");
    assert_eq!(host_latent_of(&mut r.vm, w).unwrap(), None);
    assert!(!finish_host_latent(&mut r.vm, w, ticket).unwrap());
    r.step(w);
    assert_eq!(r.entries(), ["w:start-move#1", "destroy#1"], "polled once, then never");
}

#[test]
fn a_poll_may_re_enter_the_registry() {
    let mut r = start_with(ParityMode::Off, Poll::Reenter);
    let w = r.walker("w", 0.25);
    r.step(w);
    r.step(w);
    assert_eq!(r.trace(w), "ab");
    assert_eq!(
        r.entries(),
        ["w:start-move#1", "reenter#1:true:true", "w:move#1@0.25", "w:start-anim#2"]
    );
    assert!(host_latent_kind(&mut r.vm, "Extra1").unwrap().is_some());
}

#[test]
fn a_second_start_replaces_the_pending_call() {
    let mut r = start(ParityMode::Off);
    let w = r.walker("w", 0.25);
    r.goto(w, "Double");
    r.step(w);
    assert_eq!(r.trace(w), "a");
    let h = host_latent_of(&mut r.vm, w).unwrap().unwrap();
    assert_eq!(h.ticket().get(), 2, "the second start's call is current");
    assert!(!finish_host_latent(&mut r.vm, w, LatentTicket(1)).unwrap(), "the first ticket is not");
    r.step(w);
    assert_eq!(r.trace(w), "ab");
    assert_eq!(r.entries(), ["w:start-twice#1#2", "w:move#2@0.25"], "only the second call is polled");
}

#[test]
fn a_ticket_is_refused_on_another_object() {
    let mut r = start(ParityMode::Off);
    let (w1, w2) = (r.walker("w1", 0.5), r.walker("w2", 0.5));
    r.step(w1);
    r.step(w2);
    let t1 = host_latent_of(&mut r.vm, w1).unwrap().unwrap().ticket();
    let before = host_latent_of(&mut r.vm, w2).unwrap();
    assert!(!finish_host_latent(&mut r.vm, w2, t1).unwrap());
    assert_eq!(host_latent_of(&mut r.vm, w2).unwrap(), before, "w2's call is unchanged");
    assert!(!host_latent_of(&mut r.vm, w1).unwrap().unwrap().is_finished(), "and w1's too");
}
