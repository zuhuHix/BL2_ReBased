//! Tests of the pickup natives (NATIVE_PICKUP.md; lane IMPL-PICKUP, decision 2026-11-19-68) on two invented packages
//! built with `ow_package::testkit` (AGENTS.md rule 9): `Engine` (`Actor`, `Pawn`, `Vehicle`, `Controller`,
//! `DroppedPickup` with its native `AddToNavigation`) and `WillowGame` (`WillowPlayerController` with
//! `GetInventoryPawn`, `WillowGlobals` with `RemovePickup`, `WillowPickup`). Every native is reached through the VM.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 7] =
    ["Class", "Function", "ScriptStruct", "ObjectProperty", "BoolProperty", "IntProperty", "StructProperty"];

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], result: Option<(&str, i32)>) {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(NATIVE, name), |_| None);
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let vector = ow_package::testkit::reflect::toy_struct(&mut t, object, "Vector", None);
    let actor = t.class("Actor", object);
    let pawn = t.class("Pawn", actor);
    t.class("Vehicle", pawn);
    let controller = t.class("Controller", actor);
    t.prop("Object", controller, "Pawn", 0, pawn);
    let dropped = t.class("DroppedPickup", actor);
    t.prop("Object", dropped, "Inventory", 0, actor);
    native(&mut t, dropped, "AddToNavigation", &[], None);
    let at = |name: &str| Param { type_ref: vector, ..Param::new("Struct", name) };
    native(&mut t, dropped, "TouchPickupTrace", &[at("StartLoc"), at("EndLoc")], Some(("Bool", 0)));
    // The packages' WillowInventory is an Engine class (lane IMPL-PICKUP found `definition` binding WillowGame paths).
    let inventory = t.class("WillowInventory", actor);
    native(&mut t, inventory, "GetManufacturer", &[], Some(("Object", object)));
    native(&mut t, inventory, "ComputeMonetaryValue", &[], Some(("Int", 0)));
    native(&mut t, inventory, "GenerateUniqueID", &[], Some(("Int", 0)));
    t.build()
}

fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let controller = t.p.add_import("Core", "Class", engine, "Controller");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let dropped = t.p.add_import("Core", "Class", engine, "DroppedPickup");
    t.class("WillowPlayerPawn", pawn);
    let player = t.class("WillowPlayerController", controller);
    native(&mut t, player, "GetInventoryPawn", &[], Some(("Object", pawn)));
    let globals = t.class("WillowGlobals", object);
    native(&mut t, globals, "RemovePickup", &[Param { type_ref: object, ..Param::new("Object", "inPickup") }], None);
    t.class("WillowPickup", dropped);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-pickup-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow_game());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    // The classes' code is loaded before the keys are registered (the VM resolves a native by its key at the call).
    for c in ["WillowGame.WillowPlayerController", "WillowGame.WillowGlobals", "WillowGame.WillowPickup"] {
        vm.find_class(c).unwrap();
    }
    register(&mut vm).unwrap();
    Fx { vm, dir }
}

impl Fx {
    fn obj(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn call(&mut self, path: &str, me: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        Ok(self.vm.call(f, Some(me), args)?.ret)
    }
}

fn not_read(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!((n.missing.clone(), n.record), (Missing::NotRead, RECORD), "{n}");
            n.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// `GetInventoryPawn` (the Off contract): the controller's plain player pawn, None without one; any other pawn,
/// a vehicle or a plain `Pawn`, is `NotYet { NotRead }` (open question 4).
#[test]
fn get_inventory_pawn_supports_only_a_plain_player_pawn() {
    let mut x = fx(ParityMode::Off);
    let controller = x.obj("WillowGame.WillowPlayerController");
    let path = "WillowGame.WillowPlayerController.GetInventoryPawn";
    assert_eq!(x.call(path, controller, vec![]).unwrap(), Value::Object(None));
    let maya = x.obj("WillowGame.WillowPlayerPawn");
    x.set(controller, "Pawn", Value::Object(Some(maya)));
    assert_eq!(x.call(path, controller, vec![]).unwrap(), Value::Object(Some(maya)));
    for class in ["Engine.Vehicle", "Engine.Pawn"] {
        let other = x.obj(class);
        x.set(controller, "Pawn", Value::Object(Some(other)));
        assert!(not_read(x.call(path, controller, vec![])).contains("open question 4"), "{class}");
    }
}

/// `TouchPickupTrace` (the Off contract): what blocks the trace is not read, so a touch's validity is a named
/// `NotYet`.
#[test]
fn the_touch_trace_is_not_yet() {
    let mut x = fx(ParityMode::Off);
    let pickup = x.obj("WillowGame.WillowPickup");
    let f = x.vm.find_function("Engine.DroppedPickup.TouchPickupTrace").unwrap();
    let args: Vec<Value> = x.vm.function(f).unwrap().params.iter().map(|p| x.vm.space.classes.default_value(&p.kind).unwrap()).collect();
    let r = x.vm.call(f, Some(pickup), args).map(|r| r.ret);
    assert!(not_read(r).contains("TouchPickupTrace"));
}

/// `AddToNavigation` does nothing; `RemovePickup` is `NotYet { NotRead }` before it changes anything (the pickup keeps
/// its inventory).
#[test]
fn add_to_navigation_is_a_no_op_and_remove_pickup_is_not_yet() {
    let mut x = fx(ParityMode::Off);
    let pickup = x.obj("WillowGame.WillowPickup");
    let thing = x.obj("Engine.Actor");
    x.set(pickup, "Inventory", Value::Object(Some(thing)));
    assert_eq!(x.call("Engine.DroppedPickup.AddToNavigation", pickup, vec![]).unwrap(), Value::None);
    let globals = x.obj("WillowGame.WillowGlobals");
    match x.call("WillowGame.WillowGlobals.RemovePickup", globals, vec![Value::Object(Some(pickup))]).unwrap_err() {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert_eq!(n.record, RECORD);
        }
        e => panic!("expected NotYet, got {e}"),
    }
    let class = x.vm.space.heap.get(pickup).unwrap().class;
    let p = x.vm.space.prop(class, "Inventory").unwrap();
    assert_eq!(x.vm.space.read(pickup, PropAddr::scalar(p)).unwrap(), &Value::Object(Some(thing)));
}

/// Reference binds none of the three (no C++ stand-in).
#[test]
fn reference_binds_nothing() {
    let x = fx(ParityMode::Reference);
    for key in [GET_INVENTORY_PAWN, ADD_TO_NAVIGATION, REMOVE_PICKUP, TOUCH_PICKUP_TRACE] {
        assert!(!x.vm.has_native(key), "{key}");
    }
}

/// `crate::definition` binds its three natives on an `Engine.WillowInventory`, the class's package in the game's data
/// (before lane IMPL-PICKUP its `WillowGame.` paths bound nothing there).
#[test]
fn the_definition_natives_bind_on_the_engine_class() {
    let mut x = fx(ParityMode::Off);
    x.vm.find_class("Engine.WillowInventory").unwrap();
    crate::definition::register(&mut x.vm).unwrap();
    for key in ["WillowInventory.GetManufacturer()", "WillowInventory.ComputeMonetaryValue()", "WillowInventory.GenerateUniqueID()"] {
        assert!(x.vm.has_native(key), "{key}");
    }
}
