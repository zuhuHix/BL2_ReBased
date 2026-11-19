//! Tests of the hold behavior's run and of the pack's bindings, on invented classes (AGENTS.md rule 9), in both
//! parity modes, over part 1's invented world (`super::super::vm::tests`). The hold rule's own tests are
//! `ow_combat::holds`'s. AI-assisted (Claude Code).

use std::cell::RefCell;
use std::rc::Rc;

use ow_core::{Missing, ObjectPath, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{FunctionDecl, NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

use super::super::vm::tests::{Env, env_with, make};
use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;

/// `ToyHolds.ToyHoldBehavior`: an invented hold behavior whose `ApplyBehaviorToContext(ContextObject)` script passes
/// its context to the native `Seen(Object)`.
fn toy_holds() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Object"]);
    let core = t.p.add_import("Core", "Package", 0, "Core");
    let object = t.p.add_import("Core", "Class", core, "Object");
    let class = t.class("ToyHoldBehavior", object);
    let param = |name: &str| {
        let mut p = Param::new("Object", name);
        p.type_ref = object;
        p
    };
    let seen = t
        .function(class, "Seen", &[param("What")], None, &[], &FunctionSpec::new(NATIVE, "Seen"), |_| None)
        .function;
    t.function(
        class,
        "ApplyBehaviorToContext",
        &[param("ContextObject")],
        None,
        &[],
        &FunctionSpec::new(SCRIPT, "ApplyBehaviorToContext"),
        |ids| {
            let mut a = Asm::new();
            let context = ids.get("ContextObject");
            a.stmt().final_function(seen, |a| {
                a.local(context);
            });
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    t.class("ToyNotABehavior", object);
    t.build()
}

/// Part 1's world with the toy hold behavior and an invented `GearboxFramework.IAIInterface` class; the objects its
/// script saw.
fn world(mode: ParityMode) -> (Env, ObjRef, Rc<RefCell<Vec<ObjRef>>>) {
    world_where(mode, Rc::new(|_| None))
}

/// [`world`], with `refuse` asked first on each object the script passes to `Seen`: its error is the native's, and
/// the object is not seen.
fn world_where(mode: ParityMode, refuse: Rc<dyn Fn(ObjRef) -> Option<VmError>>) -> (Env, ObjRef, Rc<RefCell<Vec<ObjRef>>>) {
    let mut e = env_with(mode, &[("ToyHolds", toy_holds())]);
    let vm = &mut e.vm;
    let root = vm.find_class("Core.Object").unwrap();
    vm.space.classes.define("GearboxFramework.IAIInterface", Some(root), vec![]).unwrap();
    let class = vm.find_class("ToyHolds.ToyHoldBehavior").unwrap();
    let behavior = make(vm, class, "Hold_57");
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let f = vm.find_function("ToyHolds.ToyHoldBehavior.Seen").unwrap();
    let key = vm.function(f).unwrap().native_key.clone().unwrap();
    vm.register_native(
        &key,
        Rc::new(move |c: &mut NativeCall<'_>| {
            if let Value::Object(Some(o)) = c.arg(0)? {
                if let Some(e) = refuse(*o) {
                    return Err(e);
                }
                s.borrow_mut().push(*o);
            }
            Ok(Value::None)
        }),
    )
    .unwrap();
    (e, behavior, seen)
}

/// A context with no AI to reach runs the installed-shaped script with itself as `ContextObject`. A pawn: in Off,
/// `Pawn`'s `QueryInterface` rule (lane IMPL-PAWN-AI-INTERFACE): a pawn with no controller, vehicle or mesh answers
/// none and the script runs; one whose `Controller` is set asks that controller, whose own answer is not read, so it
/// is `NotYet` with nothing run. Under Reference every pawn is `NotYet`, as before.
#[test]
fn the_script_runs_unless_the_context_has_an_ai() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut e, behavior, seen) = world(mode);
        let vm = &mut e.vm;
        Pack.register(vm).unwrap();
        let pawn_class = vm.space.heap.get(e.ai).unwrap().class;
        let dummy = make(vm, pawn_class, "ToyDummy_0");
        let plain = make(vm, e.object, "ToyThing_0");
        apply(vm, behavior, &[plain]).unwrap();
        if mode == ParityMode::Off {
            apply(vm, behavior, &[dummy]).unwrap();
            assert_eq!(*seen.borrow(), [plain, dummy], "the pawn's rule answered none: the script ran");
        } else {
            assert!(matches!(apply(vm, behavior, &[dummy]), Err(VmError::NotYet(_))));
            assert_eq!(*seen.borrow(), [plain], "{mode:?}");
        }
        let ran = seen.borrow().len();
        let err = apply(vm, behavior, &[e.ai]).unwrap_err();
        assert!(matches!(&err, VmError::NotYet(n) if n.missing == Missing::NotRead), "{mode:?}: {err}");
        let words = if mode == ParityMode::Off {
            "goes to its Controller, a WillowGame.WillowMind"
        } else {
            "Pawn IAIInterface"
        };
        assert!(err.to_string().contains(words), "{mode:?}: {err}");
        assert_eq!(seen.borrow().len(), ran, "nothing run for the pawn with a controller");
        let other = vm.find_class("ToyHolds.ToyNotABehavior").unwrap();
        let wrong = make(vm, other, "NotAHold");
        assert!(apply(vm, wrong, &[plain]).unwrap_err().to_string().contains("no ApplyBehaviorToContext"));
    }
}

fn declare(vm: &mut Vm, owner: ow_object::ids::ClassId, path: &str) -> ow_object::ids::FuncId {
    let d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), path).with_owner(owner);
    vm.declare_function(d).unwrap()
}

/// The pack binds the four kinds (`ow_combat::holds`, whose tests hold the rule) and `GetAIComponent` in Off, and
/// nothing under Reference; a key another pack bound first keeps its binding.
#[test]
fn the_pack_binds_the_hold_kinds_in_off_only() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut e, _, _) = world(mode);
        let vm = &mut e.vm;
        let hold = declare(vm, e.object, "GearboxFramework.AIComponent.HoldMovement");
        let get_component = declare(vm, e.object, "GearboxFramework.GearboxMind.GetAIComponent");
        let on_hold = declare(vm, e.object, "GearboxFramework.AIComponent.GodOnHold");
        let taken = vm.function(on_hold).unwrap().native_key.clone().unwrap();
        vm.register_native(&taken, Rc::new(|_c: &mut NativeCall<'_>| Ok(Value::Bool(true)))).unwrap();
        Pack.register(vm).unwrap();
        let key = vm.function(hold).unwrap().native_key.clone().unwrap();
        let component = make(vm, e.object, "ToyComponent_9");
        match mode {
            ParityMode::Off => {
                assert!(vm.has_native(&key), "bound in Off");
                let err = vm.call(get_component, Some(e.mind), vec![]).unwrap_err();
                assert!(matches!(&err, VmError::NotYet(n) if n.missing == Missing::NotRead), "{err}");
                assert!(err.to_string().contains("NATIVE_AI_HOLDS.md"), "{err}");
            }
            _ => assert!(!vm.has_native(&key), "unbound under Reference"),
        }
        assert_eq!(vm.call(on_hold, Some(component), vec![]).unwrap().ret, Value::Bool(true), "{mode:?}: kept");
    }
}

/// Off judges every context before any script runs (review r1 of #456, item 7): with the pack registered, a pawn
/// whose controller's own answer is not read stops the run with the named `NotYet { NotRead }` and no script call,
/// the contexts ahead of it included. Under Reference each context is judged just before its script (item 5): the
/// plain context ahead of the pawn runs, then the pawn refuses.
#[test]
fn off_judges_every_context_first_and_reference_keeps_its_order() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut e, behavior, seen) = world(mode);
        let vm = &mut e.vm;
        Pack.register(vm).unwrap();
        let first = make(vm, e.object, "ToyFirst");
        let last = make(vm, e.object, "ToyLast");
        let err = apply(vm, behavior, &[first, e.ai, last]).unwrap_err();
        assert!(matches!(&err, VmError::NotYet(n) if n.missing == Missing::NotRead), "{mode:?}: {err}");
        assert!(!err.to_string().contains(ow_combat::holds::PARTIAL_WRITES), "{mode:?}: {err}");
        match mode {
            ParityMode::Off => {
                assert!(err.to_string().contains("goes to its Controller, a WillowGame.WillowMind"), "{err}");
                assert_eq!(*seen.borrow(), [], "no script call");
            }
            _ => assert_eq!(*seen.borrow(), [first], "Reference: the first context ran"),
        }
    }
}

/// A script's own refusal after an earlier context's script ran (review r1 of #456, item 2): in Off a `NotYet` is
/// terminal, prefixed "stopped after partial writes" with its kind kept; on the first context it is not; another
/// error is unchanged; Reference keeps the plain `NotYet`. The invented `Seen` native stands in for the unread
/// `GetAIComponent`: it refuses on the context named `ToyUnread`.
#[test]
fn a_script_refusal_after_an_earlier_context_ran_is_terminal_in_off() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let names: Rc<RefCell<Vec<(ObjRef, &str)>>> = Rc::new(RefCell::new(Vec::new()));
        let n = names.clone();
        let (mut e, behavior, seen) = world_where(
            mode,
            Rc::new(move |o| match n.borrow().iter().find(|(x, _)| *x == o).map(|(_, w)| *w) {
                Some("unread") => Some(ow_core::NotYet::not_read("an invented unread answer", "invented").into()),
                Some("failing") => {
                    Some(VmError::Script { function: "Seen".into(), message: "invented failure".into() })
                }
                _ => None,
            }),
        );
        let vm = &mut e.vm;
        Pack.register(vm).unwrap();
        let first = make(vm, e.object, "ToyFirst");
        let unread = make(vm, e.object, "ToyUnread");
        let failing = make(vm, e.object, "ToyFailing");
        names.borrow_mut().extend([(unread, "unread"), (failing, "failing")]);
        let err = apply(vm, behavior, &[first, unread]).unwrap_err();
        let VmError::NotYet(n) = &err else { panic!("{mode:?}: {err}") };
        assert_eq!(n.missing, Missing::NotRead, "{mode:?}: the kind is kept");
        let terminal = n.what.starts_with(ow_combat::holds::PARTIAL_WRITES);
        assert_eq!(terminal, mode == ParityMode::Off, "{mode:?}: {err}");
        assert_eq!(*seen.borrow(), [first], "{mode:?}");
        let err = apply(vm, behavior, &[unread]).unwrap_err();
        assert!(!err.to_string().contains(ow_combat::holds::PARTIAL_WRITES), "{mode:?}: first context: {err}");
        let err = apply(vm, behavior, &[first, failing]).unwrap_err();
        assert!(
            matches!(&err, VmError::Script { message, .. } if message == "invented failure"),
            "{mode:?}: another error is unchanged: {err}"
        );
    }
}
