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

/// Part 1's world with the toy hold behavior; the objects its script saw.
fn world(mode: ParityMode) -> (Env, ObjRef, Rc<RefCell<Vec<ObjRef>>>) {
    let mut e = env_with(mode, &[("ToyHolds", toy_holds())]);
    let vm = &mut e.vm;
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
                s.borrow_mut().push(*o);
            }
            Ok(Value::None)
        }),
    )
    .unwrap();
    (e, behavior, seen)
}

/// A context with no AI to reach runs the installed-shaped script with itself as `ContextObject`; one whose
/// `Controller` is set is `NotYet` (the pawn question), with nothing run.
#[test]
fn the_script_runs_unless_the_context_has_an_ai() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut e, behavior, seen) = world(mode);
        let vm = &mut e.vm;
        let pawn_class = vm.space.heap.get(e.ai).unwrap().class;
        let dummy = make(vm, pawn_class, "ToyDummy_0");
        let plain = make(vm, e.object, "ToyThing_0");
        apply(vm, behavior, &[plain]).unwrap();
        assert!(matches!(apply(vm, behavior, &[dummy]), Err(VmError::NotYet(_))));
        assert_eq!(*seen.borrow(), [plain], "{mode:?}");
        let err = apply(vm, behavior, &[e.ai]).unwrap_err();
        assert!(matches!(&err, VmError::NotYet(n) if n.missing == Missing::NotRead), "{mode:?}: {err}");
        assert!(err.to_string().contains("Pawn IAIInterface"), "{err}");
        assert_eq!(seen.borrow().len(), 1, "nothing run for the pawn with a controller");
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

#[test]
fn context_order_and_script_errors_propagate_without_running_later_contexts() {
    let (mut e, behavior, seen) = world(ParityMode::Off);
    let vm = &mut e.vm;
    let first = make(vm, e.object, "ToyFirst");
    let last = make(vm, e.object, "ToyLast");
    assert!(apply(vm, behavior, &[first, e.ai, last]).is_err());
    assert_eq!(*seen.borrow(), [first]);
    let mut failing = env_with(ParityMode::Off, &[("ToyHolds", toy_holds())]);
    let vm = &mut failing.vm;
    let behavior_class = vm.find_class("ToyHolds.ToyHoldBehavior").unwrap();
    let behavior = make(vm, behavior_class, "ToyFailingBehavior");
    let last = make(vm, failing.object, "ToyFailingContext");
    let f = vm.find_function("ToyHolds.ToyHoldBehavior.Seen").unwrap();
    let key = vm.function(f).unwrap().native_key.clone().unwrap();
    vm.register_native(&key, Rc::new(|_| Err(VmError::Script {
        function: "ToySeen".into(), message: "invented failure".into(),
    }))).unwrap();
    let err = apply(vm, behavior, &[last]).unwrap_err();
    assert!(err.to_string().contains("invented failure"), "{err}");
    assert_eq!(*seen.borrow(), [first]);
}
