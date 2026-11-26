//! NATIVE_LINKS_3_4_TAIL.md section 11, test 5a (stage 15), on invented classes (AGENTS.md rule 9: the class and field
//! names are the packages'; every value is invented). Stage 10 is tested through the combat run (`run/tests.rs`).
//! AI-assisted (Claude Code).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
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

fn make(vm: &mut Vm, path: &str) -> ObjRef {
    let c = vm.space.classes.by_path(path).unwrap();
    vm.space.alloc(c, "Toy", None, Origin::Runtime).unwrap()
}

fn set(vm: &mut Vm, o: ObjRef, name: &str, v: Value) {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.write(o, PropAddr::scalar(p), v).unwrap();
}

/// A world with globals and a definition (threshold 0.5), and a player pawn; answers (vm, globals, definition, pawn).
fn world() -> (Vm, ObjRef, ObjRef, ObjRef) {
    let mut vm = Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.space.classes.define("Core.Object", None, vec![]).unwrap();
    let o = PropKind::Object { class: object };
    let actor = vm.space.classes.define("Engine.Actor", Some(object), vec![]).unwrap();
    let props = vec![
        decl(&mut vm, "bIsProtectionTimerActive", PropKind::Bool),
        decl(&mut vm, "MinimumHealthMaintainedByProtectionTimer", PropKind::Float),
    ];
    vm.space.classes.define("WillowGame.WillowPlayerPawn", Some(actor), props).unwrap();
    let mut props = Vec::new();
    for f in ["ProtectionTimerThreshold", "ProtectionTimerMaintainedMaxHealthPct", "ProtectionTimerDurationInSeconds"] {
        props.push(decl(&mut vm, f, PropKind::Float));
    }
    vm.space.classes.define("WillowGame.GlobalsDefinition", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "TheGlobalsDefinition", o)];
    vm.space.classes.define("WillowGame.WillowGlobals", Some(object), props).unwrap();
    let globals = make(&mut vm, "WillowGame.WillowGlobals");
    let definition = make(&mut vm, "WillowGame.GlobalsDefinition");
    set(&mut vm, definition, "ProtectionTimerThreshold", Value::Float(0.5));
    set(&mut vm, globals, "TheGlobalsDefinition", Value::Object(Some(definition)));
    let q = ow_world::query::install(&mut vm).unwrap();
    q.borrow_mut().set_globals(globals);
    let pawn = make(&mut vm, "WillowGame.WillowPlayerPawn");
    (vm, globals, definition, pawn)
}

#[test]
fn the_protection_timer_s_start_refuses_and_an_active_timer_keeps_the_floor() {
    let (mut vm, globals, _, pawn) = world();
    // Health 100 of 200: a 10 hit stays above half: nothing.
    assert_eq!(protection(&vm, pawn, 10.0, 150.0, 200.0).unwrap(), 10.0);
    // A hit that would take health below half with the timer off: the start's NotRead, nothing written.
    let before = vm.space.heap.committed_writes();
    match protection(&vm, pawn, 60.0, 150.0, 200.0) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, TIMER_START)),
        other => panic!("{other:?}"),
    }
    assert_eq!(vm.space.heap.committed_writes(), before);
    // Active, with a floor of 20: health cannot go below it.
    set(&mut vm, pawn, "bIsProtectionTimerActive", Value::Bool(true));
    set(&mut vm, pawn, "MinimumHealthMaintainedByProtectionTimer", Value::Float(20.0));
    assert_eq!(protection(&vm, pawn, 60.0, 50.0, 200.0).unwrap(), 30.0);
    assert_eq!(protection(&vm, pawn, 10.0, 50.0, 200.0).unwrap(), 10.0);
    // Not a player pawn: unchanged.
    let other = make(&mut vm, "Engine.Actor");
    assert_eq!(protection(&vm, other, 60.0, 50.0, 200.0).unwrap(), 60.0);
    // No globals definition: NotRead.
    set(&mut vm, globals, "TheGlobalsDefinition", Value::Object(None));
    assert!(matches!(protection(&vm, pawn, 10.0, 150.0, 200.0), Err(VmError::NotYet(_))));
}

#[test]
fn a_mistyped_protection_field_is_a_data_error() {
    let (mut vm, globals, _, pawn) = world();
    let object = vm.space.classes.by_path("Core.Object").unwrap();
    let props = vec![decl(&mut vm, "ProtectionTimerThreshold", PropKind::Int)];
    vm.space.classes.define("Test.BadDefinition", Some(object), props).unwrap();
    let bad = make(&mut vm, "Test.BadDefinition");
    set(&mut vm, globals, "TheGlobalsDefinition", Value::Object(Some(bad)));
    assert!(matches!(protection(&vm, pawn, 10.0, 150.0, 200.0), Err(VmError::Script { .. })));
}
