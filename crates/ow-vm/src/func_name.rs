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
//! VM's stack of running script functions and the name table. With no script function running (a native called from
//! outside bytecode), the answer is not read: `NotYet { NotRead }` ([`NO_FRAME`]).

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
    let path = c.vm.function_path(f);
    let name = path.rsplit('.').next().unwrap_or(path.as_str()).to_string();
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
