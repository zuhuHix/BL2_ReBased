//! The class-level part's declaration checks (lane IMPL-LINKS-13-14; NATIVE_LINKS_13_14.md section 8, test 1) on
//! invented classes (AGENTS.md rule 9). Its writes are tested in `pools/tests.rs`, mod `class_part`.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

/// A controller, a pawn whose `EncumbranceResistance` is declared `pawn_kind`, and a class that declares
/// `BaseEncumbranceResistance` only when `with_field`.
fn world(pawn_kind: PropKind, with_field: bool) -> (Vm, ObjRef) {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::standard().unwrap()), ParityMode::Off);
    let o = PropKind::Object { class: object };
    let fields = vec![
        decl(&mut vm, "BaseValueConstant", PropKind::Float),
        decl(&mut vm, "BaseValueAttribute", o.clone()),
        decl(&mut vm, "InitializationDefinition", o.clone()),
        decl(&mut vm, "BaseValueScaleConstant", PropKind::Float),
    ];
    let init = vm.space.classes.define_struct("Engine.ToyClass.AttributeInitializationData", None, fields).unwrap();
    let props = if with_field { vec![decl(&mut vm, "BaseEncumbranceResistance", PropKind::Struct(init))] } else { vec![] };
    let class = vm.space.classes.define("Engine.ToyClass", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "EncumbranceResistance", pawn_kind)];
    let pawn_class = vm.space.classes.define("Engine.Pawn", Some(object), props).unwrap();
    let props = vec![
        decl(&mut vm, "Pawn", o.clone()),
        decl(&mut vm, "CharacterClass", o.clone()),
        decl(&mut vm, "ResourcePoolManager", o.clone()),
    ];
    let controller_class = vm.space.classes.define("Engine.Controller", Some(object), props).unwrap();
    let alloc = |vm: &mut Vm, c, n: &str| vm.space.alloc(c, n, None, Origin::Runtime).unwrap();
    let (c, p, k) = (alloc(&mut vm, controller_class, "C"), alloc(&mut vm, pawn_class, "P"), alloc(&mut vm, class, "K"));
    let set = |vm: &mut Vm, o: ObjRef, n: &str, v: Value| {
        let pr = vm.space.prop(vm.space.heap.get(o).unwrap().class, n).unwrap();
        vm.space.write(o, PropAddr::scalar(pr), v).unwrap();
    };
    set(&mut vm, c, "Pawn", Value::Object(Some(p)));
    set(&mut vm, c, "CharacterClass", Value::Object(Some(k)));
    (vm, c)
}

/// A class that declares no `BaseEncumbranceResistance`, and a pawn whose `EncumbranceResistance` is an int: each a
/// script error, before any write.
#[test]
fn mistyped_or_missing_fields_are_script_errors_before_any_write() {
    for (kind, with_field) in [(PropKind::Float, false), (PropKind::Int, true)] {
        let (mut vm, c) = world(kind.clone(), with_field);
        let before = vm.space.heap.committed_writes();
        let e = recalculate_attribute_initialized_state(&mut vm, c).unwrap_err();
        assert!(matches!(&e, VmError::Script { .. }), "{kind:?} {with_field}: {e:?}");
        assert_eq!(vm.space.heap.committed_writes(), before);
    }
}
