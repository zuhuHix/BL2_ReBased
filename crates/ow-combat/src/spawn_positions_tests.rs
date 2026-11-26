//! The combat run's spawn positions (lane IMPL-COMBAT-WORLD-COLLISION, the hub's ruling), on invented classes
//! (AGENTS.md rule 9). AI-assisted (Claude Code).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::ObjRef;
use ow_object::layout::{PropKind, PropertyDecl};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;
use crate::hit::access::{field, number};

/// A world with an `Engine.Actor` that declares a vector `Location`, and a class without one.
fn vm_in(mode: ParityMode) -> Vm {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    let axes = ["X", "Y", "Z"].map(|n| PropertyDecl::new(vm.space.names.intern(n), PropKind::Float)).to_vec();
    let vector = vm.space.classes.define_struct("Core.Object.Vector", None, axes).unwrap();
    let location = PropertyDecl::new(vm.space.names.intern("Location"), PropKind::Struct(vector));
    vm.space.classes.define("Engine.Actor", Some(object), vec![location]).unwrap();
    vm.space.classes.define("Engine.ToyNoLocation", Some(object), vec![]).unwrap();
    vm
}

fn make(vm: &mut Vm, class: &str) -> ObjRef {
    let c = vm.space.classes.by_path(class).unwrap();
    vm.space.alloc(c, "Toy", None, Origin::Runtime).unwrap()
}

fn at(vm: &mut Vm, o: ObjRef) -> [f64; 3] {
    let v = get(vm, o, "Location").unwrap().unwrap();
    ["X", "Y", "Z"].map(|n| number(&field(vm, &v, n).unwrap()))
}

/// Off: nothing is placed until the setting is on; then the n-th placed pawn stands at (1000 n, 0, 0), each placement
/// logged once after the set-up line. A second enable is refused, and a class without `Location` is a script error.
#[test]
fn each_placed_pawn_stands_apart_in_off() {
    let mut vm = vm_in(ParityMode::Off);
    let before = make(&mut vm, "Engine.Actor");
    assert_eq!(place(&mut vm, before).unwrap(), None, "off by default");
    assert_eq!(at(&mut vm, before), [0.0, 0.0, 0.0]);
    assert!(vm.log.is_empty());
    enable_spawn_positions(&mut vm).unwrap();
    assert!(matches!(enable_spawn_positions(&mut vm), Err(VmError::Registration { .. })), "twice");
    let (first, second) = (make(&mut vm, "Engine.Actor"), make(&mut vm, "Engine.Actor"));
    assert_eq!(place(&mut vm, first).unwrap(), Some([1000.0, 0.0, 0.0]));
    assert_eq!(place(&mut vm, second).unwrap(), Some([2000.0, 0.0, 0.0]));
    assert_eq!(at(&mut vm, first), [1000.0, 0.0, 0.0]);
    assert_eq!(at(&mut vm, second), [2000.0, 0.0, 0.0]);
    let class = class_name(&vm, first);
    assert_eq!(vm.log, [SETUP_LINE.to_string(), placed_line(&class, 1000.0), placed_line(&class, 2000.0)]);
    let bare = make(&mut vm, "Engine.ToyNoLocation");
    assert!(matches!(place(&mut vm, bare), Err(VmError::Script { .. })), "no Location");
    // Review of #807, LOW 3: the failed placement did not move the counter, so the next pawn takes the third place.
    let third = make(&mut vm, "Engine.Actor");
    assert_eq!(place(&mut vm, third).unwrap(), Some([3000.0, 0.0, 0.0]));
}

/// Review of #807, LOW 2: the set-up line states the spacing the code uses.
#[test]
fn the_set_up_line_states_the_spacing() {
    let spacing = format!("({} n, 0, 0)", SPACING);
    assert!(SETUP_LINE.contains(&spacing), "{SETUP_LINE} does not contain {spacing}");
}

/// Reference: the setting is refused, and even a directly added service places nothing.
#[test]
fn reference_places_nothing() {
    let mut vm = vm_in(ParityMode::Reference);
    assert!(matches!(enable_spawn_positions(&mut vm), Err(VmError::Registration { .. })));
    vm.add_service(SpawnPositions::default()).unwrap();
    let pawn = make(&mut vm, "Engine.Actor");
    assert_eq!(place(&mut vm, pawn).unwrap(), None);
    assert_eq!(at(&mut vm, pawn), [0.0, 0.0, 0.0]);
    assert!(vm.log.is_empty());
}
