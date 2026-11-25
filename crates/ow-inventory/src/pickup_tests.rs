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
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 8] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "BoolProperty",
    "IntProperty",
    "StructProperty",
    "FloatProperty",
];

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], result: Option<(&str, i32)>) {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(NATIVE, name), |_| None);
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let vector = ow_package::testkit::reflect::toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
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
    native(&mut t, dropped, "GetPickupableInventory", &[], Some(("Object", actor)));
    native(&mut t, dropped, "GetPickupableInventoryDefinition", &[], Some(("Object", object)));
    // The packages' WillowInventory is an Engine class (lane IMPL-PICKUP found `definition` binding WillowGame paths).
    let inventory = t.class("WillowInventory", actor);
    native(&mut t, inventory, "GetManufacturer", &[], Some(("Object", object)));
    native(&mut t, inventory, "ComputeMonetaryValue", &[], Some(("Int", 0)));
    native(&mut t, inventory, "GenerateUniqueID", &[], Some(("Int", 0)));
    native(&mut t, inventory, "GetInventoryDefinition", &[], Some(("Object", object)));
    // Invented: a subclass that inherits the native query, and one that overrides it in script (review r1 of #487).
    t.class("ToyInheritingInventory", inventory);
    let scripted = t.class("ToyScriptedInventory", inventory);
    t.function(scripted, "GetInventoryDefinition", &[], Some(("Object", object)), &[], &FunctionSpec::new(SCRIPT, "GetInventoryDefinition"), |_| {
        let mut a = ow_package::testkit::script::Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.build()
}

fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let controller = t.p.add_import("Core", "Class", engine, "Controller");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let dropped = t.p.add_import("Core", "Class", engine, "DroppedPickup");
    let player = t.class("WillowPlayerPawn", pawn);
    t.class("ToyPlayerPawnSubclass", player);
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

/// `GetInventoryPawn` (the Off contract): the controller's plain player pawn, None without one; any other pawn, a
/// vehicle, a plain `Pawn` or a subclass of the player pawn (review r2 of #452), is `NotYet { NotRead }` (open
/// question 4).
#[test]
fn get_inventory_pawn_supports_only_a_plain_player_pawn() {
    let mut x = fx(ParityMode::Off);
    let controller = x.obj("WillowGame.WillowPlayerController");
    let path = "WillowGame.WillowPlayerController.GetInventoryPawn";
    assert_eq!(x.call(path, controller, vec![]).unwrap(), Value::Object(None));
    let maya = x.obj("WillowGame.WillowPlayerPawn");
    x.set(controller, "Pawn", Value::Object(Some(maya)));
    assert_eq!(x.call(path, controller, vec![]).unwrap(), Value::Object(Some(maya)));
    for class in ["Engine.Vehicle", "Engine.Pawn", "WillowGame.ToyPlayerPawnSubclass"] {
        let other = x.obj(class);
        x.set(controller, "Pawn", Value::Object(Some(other)));
        assert!(not_read(x.call(path, controller, vec![])).contains("open question 4"), "{class}");
    }
}

/// `TouchPickupTrace` in a world with no collision service (lane IMPL-TOUCH-PICKUP-TRACE; the full cases are in
/// `pickup_touch_tests.rs`): the note's `LayerAbsent`, never "clear".
#[test]
fn the_touch_trace_without_a_collision_service_is_layer_absent() {
    let mut x = fx(ParityMode::Off);
    let pickup = x.obj("WillowGame.WillowPickup");
    let f = x.vm.find_function("Engine.DroppedPickup.TouchPickupTrace").unwrap();
    let args: Vec<Value> = x.vm.function(f).unwrap().params.iter().map(|p| x.vm.space.classes.default_value(&p.kind).unwrap()).collect();
    match x.vm.call(f, Some(pickup), args).map(|r| r.ret) {
        Err(VmError::NotYet(n)) => {
            assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            assert_eq!(n.what.as_ref(), ow_world::touch_trace::TOUCH_TRACE);
        }
        other => panic!("{other:?}"),
    }
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
            assert_eq!(n.record, crate::globals::RECORD);
        }
        e => panic!("expected NotYet, got {e}"),
    }
    let class = x.vm.space.heap.get(pickup).unwrap().class;
    let p = x.vm.space.prop(class, "Inventory").unwrap();
    assert_eq!(x.vm.space.read(pickup, PropAddr::scalar(p)).unwrap(), &Value::Object(Some(thing)));
}

/// The two pickup accessors (decision 2026-11-19-40, assigned -103): the pickup's `Inventory`, and that inventory's own
/// native `GetInventoryDefinition()` through the VM (a subclass that inherits it included). A pickup with no inventory
/// and an inventory class without the query are named `NotRead`s. Nothing is written.
#[test]
fn the_pickup_accessors_read_the_inventory_and_ask_its_definition() {
    let mut x = fx(ParityMode::Off);
    let definition = x.obj("Engine.Object");
    let key = "WillowInventory.GetInventoryDefinition()";
    x.vm
        .register_native(key, Rc::new(move |_c: &mut ow_vm::api::NativeCall<'_>| Ok(Value::Object(Some(definition)))))
        .unwrap();
    let pickup = x.obj("WillowGame.WillowPickup");
    assert_eq!(x.call(INVENTORY_OF, pickup, vec![]).unwrap(), Value::Object(None));
    expect_not_read(x.call(DEFINITION_OF, pickup, vec![]), NO_INVENTORY);
    for class in ["Engine.WillowInventory", "Engine.ToyInheritingInventory"] {
        let item = x.obj(class);
        x.set(pickup, "Inventory", Value::Object(Some(item)));
        assert_eq!(x.call(INVENTORY_OF, pickup, vec![]).unwrap(), Value::Object(Some(item)), "{class}");
        assert_eq!(x.call(DEFINITION_OF, pickup, vec![]).unwrap(), Value::Object(Some(definition)), "{class}");
    }
    let plain = x.obj("Engine.Actor");
    x.set(pickup, "Inventory", Value::Object(Some(plain)));
    expect_not_read(x.call(DEFINITION_OF, pickup, vec![]), NO_DEFINITION_QUERY);
    assert_eq!(inventory_of(&x, pickup), Some(plain), "nothing written");
}

/// Review r1 of #487, item 1: with nothing registered under the native's key, a subclass that inherits it is the
/// callee's own `UnboundNative`, unchanged.
#[test]
fn an_inherited_unbound_definition_query_is_unbound_native() {
    let mut x = fx(ParityMode::Off);
    let pickup = x.obj("WillowGame.WillowPickup");
    let item = x.obj("Engine.ToyInheritingInventory");
    x.set(pickup, "Inventory", Value::Object(Some(item)));
    match x.call(DEFINITION_OF, pickup, vec![]) {
        Err(VmError::UnboundNative(k)) => assert!(k.contains("GetInventoryDefinition"), "{k}"),
        other => panic!("{other:?}"),
    }
}

/// Review r1 of #487, item 2: a subclass that overrides the query in script gets the named `NotRead` before anything
/// runs, and nothing is written.
#[test]
fn a_script_override_of_the_definition_query_is_not_read() {
    let mut x = fx(ParityMode::Off);
    let pickup = x.obj("WillowGame.WillowPickup");
    let item = x.obj("Engine.ToyScriptedInventory");
    x.set(pickup, "Inventory", Value::Object(Some(item)));
    expect_not_read(x.call(DEFINITION_OF, pickup, vec![]), NO_NATIVE_DEFINITION_QUERY);
    assert_eq!(inventory_of(&x, pickup), Some(item), "nothing written");
}

const INVENTORY_OF: &str = "Engine.DroppedPickup.GetPickupableInventory";
const DEFINITION_OF: &str = "Engine.DroppedPickup.GetPickupableInventoryDefinition";

fn expect_not_read(r: Result<Value, VmError>, what: &str) {
    match r {
        Err(VmError::NotYet(n)) => assert!(n.missing == Missing::NotRead && n.what == what && n.record == RECORD_EQUIP, "{n}"),
        other => panic!("expected the NotRead {what:?}, got {other:?}"),
    }
}

fn inventory_of(x: &Fx, pickup: ObjRef) -> Option<ObjRef> {
    let class = x.vm.space.heap.get(pickup).unwrap().class;
    let p = x.vm.space.prop(class, "Inventory").unwrap();
    match x.vm.space.read(pickup, PropAddr::scalar(p)).unwrap() {
        Value::Object(o) => *o,
        other => panic!("{other:?}"),
    }
}

/// Reference binds none of them (no C++ stand-in).
#[test]
fn reference_binds_nothing() {
    let x = fx(ParityMode::Reference);
    for key in [
        GET_INVENTORY_PAWN,
        ADD_TO_NAVIGATION,
        REMOVE_PICKUP,
        TOUCH_PICKUP_TRACE,
        GET_PICKUPABLE_INVENTORY,
        GET_PICKUPABLE_INVENTORY_DEFINITION,
    ] {
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

// ---- COLD-CLASS-AUDIT (lane COLD-CLASS-AUDIT-WORLD-INVENTORY) ---------------------------------------------------------

/// An object of the class at `path`, its layout defined through the code index and its code NOT loaded.
fn cold(x: &mut Fx, path: &str) -> (ObjRef, ow_object::ids::ClassId) {
    let vm = &mut x.vm;
    let (code, store) = vm.defs.code_mut();
    let key = code.find_class(store, path).unwrap();
    let c = code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap();
    assert!(!vm.class_code_loaded(c), "{path} starts cold");
    (vm.space.alloc(c, "Cold", None, ow_object::heap::Origin::Runtime).unwrap(), c)
}

/// Code loads, #518's three counters and the name table.
fn marks(vm: &Vm) -> (u64, u64, u64, u64, usize) {
    (vm.class_code_loads(), vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
}

/// `GetInventoryPawn` with a pawn whose class's code is cold (and the player pawn class not warmed): the refusal for
/// a player pawn subclass, with no class code loaded (lane COLD-LOAD-FIXES: the class table by path, not `find_class`).
#[test]
fn get_inventory_pawn_on_a_cold_pawn_loads_no_code() {
    let mut x = fx(ParityMode::Off);
    let controller = x.obj("WillowGame.WillowPlayerController");
    let (pawn, _) = cold(&mut x, "WillowGame.ToyPlayerPawnSubclass");
    x.set(controller, "Pawn", Value::Object(Some(pawn)));
    let before = marks(&x.vm);
    let r = x.call("WillowGame.WillowPlayerController.GetInventoryPawn", controller, vec![]);
    assert!(not_read(r).contains("open question 4"));
    assert_eq!(marks(&x.vm), before, "no code load, write, draw, store load or name");
}

/// `GetPickupableInventoryDefinition` with an inventory whose class's code is cold: the query cannot be resolved
/// without a load (lane COLD-LOAD-FIXES, `find_loaded_method`), so it is the named `NotRead` for that class, with no
/// code load, write, draw, store load or name. The warm sibling answers the inherited query's `UnboundNative`.
#[test]
fn the_definition_query_on_a_cold_inventory_loads_no_code() {
    let mut x = fx(ParityMode::Off);
    let pickup = x.obj("WillowGame.WillowPickup");
    let (item, _) = cold(&mut x, "Engine.ToyInheritingInventory");
    x.set(pickup, "Inventory", Value::Object(Some(item)));
    let before = marks(&x.vm);
    match x.call(DEFINITION_OF, pickup, vec![]) {
        Err(VmError::NotYet(n)) => {
            assert_eq!((n.missing.clone(), n.record), (Missing::NotRead, crate::pickup::RECORD_EQUIP), "{n}");
            assert!(n.what.contains("the class Engine.ToyInheritingInventory, not loaded (a class load is a write)"), "{n}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(marks(&x.vm), before, "no code load, write, draw, store load or name");
}
