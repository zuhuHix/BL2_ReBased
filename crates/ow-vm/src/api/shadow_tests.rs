//! The native shadow audit (lane PLACEHOLDER-SHADOWS): `Vm::audit_native_shadows`, `audit_new_vms` and
//! `Vm::native_shadows`. The registration under test lives in another source file (`func_name.rs`, keep-existing on
//! `Object.GetFuncName()`), so this file's checks are "another file". AI-assisted (Claude Code).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::ObjectSpace;
use ow_object::value::Value;

use super::{FunctionDecl, NativeShadow, Vm, audit_new_vms};
use crate::func_name::{self, KEY};

fn vm() -> Vm {
    Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
}

#[test]
fn a_check_from_another_file_is_a_shadow_and_the_registrant_s_own_is_not() {
    let mut vm = vm();
    vm.audit_native_shadows();
    func_name::register(&mut vm).unwrap();
    // func_name.rs checks its own key again (keep-existing): the same file, no shadow.
    func_name::register(&mut vm).unwrap();
    assert!(vm.native_shadows().is_empty());
    // A keep-existing check from this file: recorded once, however often it is asked.
    assert!(vm.has_native(KEY));
    assert!(vm.has_native(KEY));
    let shadows = vm.native_shadows();
    assert_eq!(shadows.len(), 1, "{shadows:?}");
    let NativeShadow { key, bound_by, checked_by } = &shadows[0];
    assert_eq!(key, KEY);
    assert!(bound_by.replace('\\', "/").ends_with("ow-vm/src/func_name.rs"), "{bound_by}");
    assert!(checked_by.replace('\\', "/").ends_with("ow-vm/src/api/shadow_tests.rs"), "{checked_by}");
    // An unbound key is no shadow.
    assert!(!vm.has_native("Toy.Unbound()"));
    assert_eq!(vm.native_shadows().len(), 1);
}

#[test]
fn a_swallowed_duplicate_is_recorded_and_the_audit_is_off_by_default() {
    let mut vm = vm();
    func_name::register(&mut vm).unwrap();
    assert!(vm.has_native(KEY));
    assert!(vm.native_shadows().is_empty(), "off by default");
    vm.audit_native_shadows();
    let refused = vm.register_native(KEY, Rc::new(|_| Ok(Value::None)));
    assert!(refused.is_err(), "still a hard duplicate");
    assert_eq!(vm.native_shadows().len(), 1, "recorded, in case the caller swallows the refusal");
}

#[test]
fn audit_new_vms_turns_the_audit_on_for_this_thread_s_new_vms() {
    audit_new_vms(true);
    let mut on = vm();
    audit_new_vms(false);
    let mut off = vm();
    for vm in [&mut on, &mut off] {
        func_name::register(vm).unwrap();
        assert!(vm.has_native(KEY));
    }
    assert_eq!(on.native_shadows().len(), 1);
    assert!(off.native_shadows().is_empty());
}

/// `is_bound_unaudited` (lane SHADOW-AUDIT-FOLLOWUPS-2): a read-only check records nothing under the audit, where
/// `is_bound` from the same file records the shadow.
#[test]
fn is_bound_unaudited_records_nothing_under_the_audit() {
    let mut vm = vm();
    vm.audit_native_shadows();
    func_name::register(&mut vm).unwrap();
    let f = vm.declare_function(FunctionDecl::native(ObjectPath::parse("Core.Object.GetFuncName").unwrap(), KEY)).unwrap();
    assert!(vm.is_bound_unaudited(f));
    assert!(vm.native_shadows().is_empty(), "nothing recorded");
    assert!(vm.is_bound(f));
    assert_eq!(vm.native_shadows().len(), 1, "is_bound from another file is recorded");
}
