//! Lanes IMPL-RESOLVE-CONTEXT (NATIVE_RESOLVE_CONTEXT.md section 5, test 12) and IMPL-ADD-ATTRIBUTE-MODIFIER
//! (NATIVE_ADD_ATTRIBUTE_MODIFIER.md section 3, test 10): the action-skill pack's bindings of
//! `AttributeDefinitionBase.ResolveContext` and `AddAttributeModifier` and the equip pack's meet in either order. Each keeps the binding made
//! first (the keep-existing rule), neither fails, and the key ends bound once. Invented: one declared native only.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::ObjectSpace;
use ow_vm::api::{FunctionDecl, Vm};

use ow_inventory::equip::natives::{ADD_MODIFIER, ADD_MODIFIER_FUNCTION, RESOLVE_CONTEXT, RESOLVE_CONTEXT_FUNCTION};

fn vm(mode: ParityMode) -> Vm {
    let mut vm = Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    for (path, key) in [(RESOLVE_CONTEXT_FUNCTION, RESOLVE_CONTEXT), (ADD_MODIFIER_FUNCTION, ADD_MODIFIER)] {
        vm.declare_function(FunctionDecl::native(ObjectPath::parse(path).unwrap(), key)).unwrap();
    }
    vm
}

#[test]
fn the_action_skill_and_equip_packs_bind_resolve_context_once_in_either_order() {
    for action_first in [true, false] {
        let mut vm = vm(ParityMode::Off);
        assert!(!vm.has_native(RESOLVE_CONTEXT));
        if action_first {
            super::register_cast_resolve_context(&mut vm).unwrap();
            super::register_cast_add_modifier(&mut vm).unwrap();
            ow_inventory::equip::natives::register(&mut vm).unwrap();
        } else {
            ow_inventory::equip::natives::register(&mut vm).unwrap();
            super::register_cast_resolve_context(&mut vm).unwrap();
            super::register_cast_add_modifier(&mut vm).unwrap();
        }
        assert!(vm.has_native(RESOLVE_CONTEXT) && vm.has_native(ADD_MODIFIER), "action first: {action_first}");
        // The action-skill side is keep-existing, so a second call after either order is a no-op.
        super::register_cast_resolve_context(&mut vm).unwrap();
        super::register_cast_add_modifier(&mut vm).unwrap();
        assert!(vm.has_native(RESOLVE_CONTEXT) && vm.has_native(ADD_MODIFIER));
    }
}

#[test]
fn reference_binds_no_resolve_context_from_the_action_skill_pack() {
    let mut vm = vm(ParityMode::Reference);
    super::register_cast_resolve_context(&mut vm).unwrap();
    super::register_cast_add_modifier(&mut vm).unwrap();
    assert!(!vm.has_native(RESOLVE_CONTEXT) && !vm.has_native(ADD_MODIFIER));
}
