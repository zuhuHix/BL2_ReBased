//! Owner: lane IMPL-GET-FUNC-NAME (decision 2026-11-19-NNN, number pending). `Object.GetFuncName()`, the stock UE3 Core
//! native that answers the name of the script function calling it. AI-assisted (Claude Code).
//!
//! **The rule** (public UE3 semantics, not a Gearbox rule; no analyst note): a static final native whose answer is
//! the name of the function whose frame executes the call, the innermost script function running. On slice link 9,
//! `LiftActionSkill.OnSelectedTarget` passes it as the event name of
//! `BehaviorKernel.ActivateBehaviorEventFromScript`, so the answer there is `OnSelectedTarget`. **UNVERIFIED** in the
//! running game.
//!
//! **Off only** (Reference keeps its stub policy: the C++ VM has no binding), first-wins. Write-free: it reads the
//! VM's stack of running script functions and the name table. Refusals, each `NotYet { NotRead }` (review r1 of #564):
//! - no script function running (a native called from outside bytecode): [`NO_FRAME`];
//! - the innermost frame is a state's code (W5-STATES runs it as a function whose path ends `.<state code>`): its name
//!   in the game is the state's, which the port does not guess ([`STATE_CODE`]);
//! - the innermost frame's function is not declared in the VM: [`UNKNOWN`].
//!
//! The name is the function path's last component (`ObjectPath::name`, which splits at `.` and `:`).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::value::Value;

use crate::api::{NativeCall, Vm};
use crate::error::VmError;

/// The native key.
pub const KEY: &str = "Object.GetFuncName()";
/// The record of the rule.
pub const RECORD: &str = "public UE3 Object.GetFuncName";
/// The `NotYet { NotRead }` with no script function running.
pub const NO_FRAME: &str =
    "Object.GetFuncName with no script function running (its answer outside a script frame is not read)";

/// The `NotYet { NotRead }` inside state code.
pub const STATE_CODE: &str = "GetFuncName: inside state code";
/// The `NotYet { NotRead }` for a frame whose function the VM does not declare.
pub const UNKNOWN: &str = "GetFuncName: the running function is not declared in the VM";
/// The path suffix of a state's code (`crate::states`).
const STATE_CODE_SUFFIX: &str = ".<state code>";

/// Binds `Object.GetFuncName()` in Off, unless another pack bound it first.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off || vm.has_native(KEY) {
        return Ok(());
    }
    vm.register_native(KEY, Rc::new(get_func_name))
}

/// The rule (module documentation).
pub(crate) fn get_func_name(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(&f) = c.vm.code.stack.last() else {
        return Err(NotYet::not_read(NO_FRAME, RECORD).into());
    };
    if crate::interp::is_state_code(c.vm, f) {
        return Err(NotYet::not_read(STATE_CODE, RECORD).into());
    }
    let Some(decl) = c.vm.function(f) else {
        return Err(NotYet::not_read(UNKNOWN, RECORD).into());
    };
    if decl.path.as_str().ends_with(STATE_CODE_SUFFIX) {
        return Err(NotYet::not_read(STATE_CODE, RECORD).into());
    }
    let name = decl.path.name().to_string();
    let id = match c.vm.space.names.find(&name) {
        Some(id) => id,
        None => c.vm.space.names.intern(&name),
    };
    Ok(Value::Name(id))
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use ow_attr::AttributeSystem;
    use ow_core::{Missing, ObjectPath, ParityMode};
    use ow_defs::DefStore;
    use ow_object::heap::ObjectSpace;
    use ow_object::value::Value;

    use super::*;
    use crate::api::FunctionDecl;

    fn vm(mode: ParityMode) -> Vm {
        Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode)
    }

    /// The innermost running script function's name, as a name; the outer one when the inner returned. Invented
    /// functions stand for the frames (the test pushes them as the interpreter does around a script function).
    #[test]
    fn the_answer_is_the_innermost_script_function_s_name() {
        let mut vm = vm(ParityMode::Off);
        let outer = vm.declare_function(FunctionDecl::script(ObjectPath::parse("Toy.ToySkill.OnToyEvent").unwrap())).unwrap();
        let inner = vm.declare_function(FunctionDecl::script(ObjectPath::parse("Toy.ToySkill.Helper").unwrap())).unwrap();
        let native = vm.declare_function(FunctionDecl::native(ObjectPath::parse("Core.Object.GetFuncName").unwrap(), KEY)).unwrap();
        vm.code.stack.push(outer);
        let mut c = NativeCall::new(&mut vm, native, None, Vec::new());
        let got = get_func_name(&mut c).unwrap();
        let Value::Name(n) = got else { panic!("{got:?}") };
        assert_eq!(vm.space.names.text(n), "OnToyEvent");
        vm.code.stack.push(inner);
        let mut c = NativeCall::new(&mut vm, native, None, Vec::new());
        let Value::Name(n) = get_func_name(&mut c).unwrap() else { panic!() };
        assert_eq!(vm.space.names.text(n), "Helper", "the innermost");
        vm.code.stack.pop();
        let mut c = NativeCall::new(&mut vm, native, None, Vec::new());
        let Value::Name(n) = get_func_name(&mut c).unwrap() else { panic!() };
        assert_eq!(vm.space.names.text(n), "OnToyEvent", "back to the outer");
        vm.code.stack.clear();
    }

    /// With no script function running, the named `NotRead`; the heap is unchanged.
    #[test]
    fn with_no_script_frame_it_is_not_read() {
        let mut vm = vm(ParityMode::Off);
        let native = vm.declare_function(FunctionDecl::native(ObjectPath::parse("Core.Object.GetFuncName").unwrap(), KEY)).unwrap();
        let heap = format!("{:?}", vm.space.heap);
        let mut c = NativeCall::new(&mut vm, native, None, Vec::new());
        match get_func_name(&mut c).unwrap_err() {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert_eq!(n.what, NO_FRAME);
                assert_eq!(n.record, RECORD);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(format!("{:?}", vm.space.heap), heap);
    }

    /// Review r1 of #564: a state-scoped function answers its own name; state code is the named `NotRead`, not a
    /// made-up name; a frame whose function the VM does not declare is `NotRead` too.
    #[test]
    fn state_functions_state_code_and_unknown_frames() {
        let mut vm = vm(ParityMode::Off);
        let in_state = vm
            .declare_function(FunctionDecl::script(ObjectPath::parse("Toy.ToySkill.ToyState.OnToyEvent").unwrap()))
            .unwrap();
        let state_code = vm
            .declare_function(FunctionDecl::script(ObjectPath::parse("Toy.ToySkill.ToyState.<state code>").unwrap()))
            .unwrap();
        let native = vm.declare_function(FunctionDecl::native(ObjectPath::parse("Core.Object.GetFuncName").unwrap(), KEY)).unwrap();
        vm.code.stack.push(in_state);
        let mut c = NativeCall::new(&mut vm, native, None, Vec::new());
        let Value::Name(n) = get_func_name(&mut c).unwrap() else { panic!() };
        assert_eq!(vm.space.names.text(n), "OnToyEvent", "a state-scoped function");
        vm.code.stack.push(state_code);
        let mut c = NativeCall::new(&mut vm, native, None, Vec::new());
        match get_func_name(&mut c).unwrap_err() {
            VmError::NotYet(n) => assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, STATE_CODE)),
            other => panic!("{other:?}"),
        }
        assert!(vm.space.names.find("<state code>").is_none(), "no made-up name interned");
        vm.code.stack.clear();
        vm.code.stack.push(ow_object::ids::FuncId::new(9_999));
        let mut c = NativeCall::new(&mut vm, native, None, Vec::new());
        match get_func_name(&mut c).unwrap_err() {
            VmError::NotYet(n) => assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, UNKNOWN)),
            other => panic!("{other:?}"),
        }
        vm.code.stack.clear();
    }

    /// Review r1 of #564: through the registered binding, from bytecode. An invented `Core.upk`: `Object` with the
    /// native `GetFuncName`, a native `Bounce`, and the script functions `Probe` (returns `GetFuncName()`), `Inner`
    /// (the same) and `Outer` (returns `Bounce()`). `Bounce` calls `Inner` back from native code, then asks
    /// `GetFuncName` itself: the callee's name first, the caller's (`Outer`) after the callee returned.
    #[test]
    fn through_the_binding_from_bytecode_and_a_native_calling_back() {
        use std::cell::RefCell;

        use ow_defs::identity::IdentityPolicy;
        use ow_package::testkit::script::{FunctionSpec, ScriptToy};

        const NATIVE: u32 = 0x400 | 0x20000 | 0x1;
        const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
        let mut t = ScriptToy::with_imports(&["Class", "Function", "NameProperty"]);
        let object = t.class("Object", 0);
        let get = t.function(object, "GetFuncName", &[], Some(("Name", 0)), &[], &FunctionSpec::new(NATIVE | 0x2000, "GetFuncName"), |_| None);
        let bounce = t.function(object, "Bounce", &[], Some(("Name", 0)), &[], &FunctionSpec::new(NATIVE, "Bounce"), |_| None);
        let body = |callee: i32| {
            move |_: &ow_package::testkit::script::FunctionIds| {
                let mut a = ow_package::testkit::script::Asm::new();
                a.stmt().raw(&[0x04]);
                a.final_function(callee, |_| {});
                a.end();
                Some(a)
            }
        };
        t.function(object, "Probe", &[], Some(("Name", 0)), &[], &FunctionSpec::new(SCRIPT, "Probe"), body(get.function));
        t.function(object, "Inner", &[], Some(("Name", 0)), &[], &FunctionSpec::new(SCRIPT, "Inner"), body(get.function));
        t.function(object, "Outer", &[], Some(("Name", 0)), &[], &FunctionSpec::new(SCRIPT, "Outer"), body(bounce.function));
        let bytes = t.build();
        let dir = std::env::temp_dir().join(format!("ow-vm-funcname-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let defs = DefStore::from_synthetic(&dir, &[("Core.upk", bytes.as_slice())], IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
        register(&mut vm).unwrap();
        let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let record = seen.clone();
        vm.register_native(
            "Object.Bounce()",
            Rc::new(move |c: &mut NativeCall<'_>| {
                let inner = c.vm.find_function("Core.Object.Inner")?;
                let got = c.call_script(inner, None, vec![])?.ret;
                let Value::Name(n) = got else { panic!("{got:?}") };
                record.borrow_mut().push(c.vm.space.names.text(n).to_string());
                let me = get_func_name(c)?;
                let Value::Name(m) = me else { panic!() };
                record.borrow_mut().push(c.vm.space.names.text(m).to_string());
                Ok(me)
            }),
        )
        .unwrap();
        let probe = vm.find_function("Core.Object.Probe").unwrap();
        let Value::Name(n) = vm.call(probe, None, vec![]).unwrap().ret else { panic!() };
        assert_eq!(vm.space.names.text(n), "Probe", "the binding, from bytecode");
        let outer = vm.find_function("Core.Object.Outer").unwrap();
        let Value::Name(n) = vm.call(outer, None, vec![]).unwrap().ret else { panic!() };
        assert_eq!(vm.space.names.text(n), "Outer");
        assert_eq!(*seen.borrow(), ["Inner", "Outer"], "the callee's name, then the caller's after it returned");
        drop(vm);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Off binds it, first-wins; Reference leaves it unbound.
    #[test]
    fn off_binds_it_and_reference_does_not() {
        let mut off = vm(ParityMode::Off);
        register(&mut off).unwrap();
        assert!(off.has_native(KEY));
        register(&mut off).unwrap();
        let mut reference = vm(ParityMode::Reference);
        register(&mut reference).unwrap();
        assert!(!reference.has_native(KEY));
    }
}
