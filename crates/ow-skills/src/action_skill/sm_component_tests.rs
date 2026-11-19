//! Tests of `GearboxPawn.GetSMComponent` (NATIVE_SPECIAL_MOVE_COMPONENT.md; lane FIX-GETSMCOMPONENT) on invented
//! classes and objects (AGENTS.md rule 9), in both parity modes.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::{GET_SM_COMPONENT, RECORD_SM_COMPONENT, register_sm_component};

/// A VM with `Core.Object`, a `Test.GearboxPawn` (its `SMComponent`, an object) and a `Test.SpecialMoveComponent`, the
/// native declared on the pawn class under its key, and the bindings of `mode`.
fn vm(mode: ParityMode) -> (Vm, FuncId) {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let sm = PropertyDecl::new(space.names.intern("SMComponent"), PropKind::Object { class: object });
    let pawn = space.classes.define("Test.GearboxPawn", Some(object), vec![sm]).unwrap();
    space.classes.define("Test.SpecialMoveComponent", Some(object), vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    let path = ObjectPath::parse("Test.GearboxPawn.GetSMComponent").unwrap();
    let ret = PropertyDecl::new(vm.space.names.intern("ReturnValue"), PropKind::Object { class: object })
        .with_flags(PropertyDecl::PARAM | PropertyDecl::OUT | PropertyDecl::RETURN);
    let f = vm
        .declare_function(FunctionDecl::native(path, GET_SM_COMPONENT).with_owner(pawn).with_params(vec![ret]))
        .unwrap();
    register_sm_component(&mut vm).unwrap();
    (vm, f)
}

fn make(vm: &mut Vm, class: &str, name: &str, outer: Option<ObjRef>) -> ObjRef {
    let c = vm.find_class(class).unwrap();
    vm.space.alloc(c, name, outer, Origin::Runtime).unwrap()
}

fn set_component(vm: &mut Vm, pawn: ObjRef, component: Option<ObjRef>) {
    let class = vm.space.heap.get(pawn).unwrap().class;
    let p = vm.space.prop(class, "SMComponent").unwrap();
    vm.space.write(pawn, PropAddr::scalar(p), Value::Object(component)).unwrap();
}

/// The pawn's own component is answered, the same on every call; None is None; a component that is not the pawn's
/// own (a shared template sub-object) is `NotYet { NotRead }` naming the open question.
#[test]
fn get_sm_component_reads_the_pawn_s_own_component() {
    let (mut vm, f) = vm(ParityMode::Off);
    let pawn = make(&mut vm, "Test.GearboxPawn", "Maya", None);
    let call = |vm: &mut Vm, pawn: ObjRef| -> Result<Value, VmError> { Ok(vm.call(f, Some(pawn), vec![])?.ret) };
    assert_eq!(call(&mut vm, pawn).unwrap(), Value::Object(None), "no component");
    let own = make(&mut vm, "Test.SpecialMoveComponent", "SpecialMoveComponent0", Some(pawn));
    set_component(&mut vm, pawn, Some(own));
    assert_eq!(call(&mut vm, pawn).unwrap(), Value::Object(Some(own)));
    assert_eq!(call(&mut vm, pawn).unwrap(), Value::Object(Some(own)), "the same one on every call");
    let template = make(&mut vm, "Test.GearboxPawn", "Default__GearboxPawn", None);
    let shared = make(&mut vm, "Test.SpecialMoveComponent", "SpecialMoveComponent0", Some(template));
    let other = make(&mut vm, "Test.GearboxPawn", "Other", None);
    set_component(&mut vm, other, Some(shared));
    match call(&mut vm, other).unwrap_err() {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert_eq!(n.record, RECORD_SM_COMPONENT);
            assert!(n.what.contains("open question 1"), "{n}");
        }
        e => panic!("expected NotYet, got {e}"),
    }
}

/// Reference: the C++ has no stand-in, so the native stays unbound (R3).
#[test]
fn get_sm_component_is_unbound_in_reference_mode() {
    let (vm, _) = vm(ParityMode::Reference);
    assert!(!vm.has_native(GET_SM_COMPONENT));
    let (vm, _) = vm_off();
    assert!(vm.has_native(GET_SM_COMPONENT));
}

fn vm_off() -> (Vm, FuncId) {
    vm(ParityMode::Off)
}
