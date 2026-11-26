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
    t.prop("Object", pawn, "InvManager", 0, actor);
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
    native(&mut t, dropped, "Pickupable_IsEnabled", &[], Some(("Bool", 0)));
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
    willow_game_with(Some("Bool"))
}

/// `WillowGame` with `WillowPickup`'s `bPickupable` declared of kind `field`, or not at all for `None` (invented, for
/// the declaration checks of review of #765, item 1).
fn willow_game_with(field: Option<&str>) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let controller = t.p.add_import("Core", "Class", engine, "Controller");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let dropped = t.p.add_import("Core", "Class", engine, "DroppedPickup");
    // `WillowPawn` between, as in the game (review of #778: the on-foot rule's other class).
    let willow_pawn = t.class("WillowPawn", pawn);
    let player = t.class("WillowPlayerPawn", willow_pawn);
    t.class("ToyPlayerPawnSubclass", player);
    let player = t.class("WillowPlayerController", controller);
    native(&mut t, player, "GetInventoryPawn", &[], Some(("Object", pawn)));
    native(&mut t, player, "GetPawnInventoryManager", &[], Some(("Object", object)));
    // GetPawnInventoryManager's answer class (invented here as a plain class of the package).
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let manager = t.class("WillowInventoryManager", actor);
    // One invented field, so a manager the store has not built (slots empty) is told from a built one (review of #778).
    t.prop("Object", manager, "ToyField", 0, object);
    let globals = t.class("WillowGlobals", object);
    native(&mut t, globals, "RemovePickup", &[Param { type_ref: object, ..Param::new("Object", "inPickup") }], None);
    let pickup = t.class("WillowPickup", dropped);
    if let Some(kind) = field {
        t.prop(kind, pickup, "bPickupable", 0, 0);
    }
    native(&mut t, pickup, "Pickupable_IsEnabled", &[], Some(("Bool", 0)));
    // Invented: a subclass with no field of its own (NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 5's third test).
    t.class("ToyWillowPickupSubclass", pickup);
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
    fx_with(mode, Some("Bool"))
}

fn fx_with(mode: ParityMode, field: Option<&str>) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-pickup-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow_game_with(field));
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

/// `AddToNavigation` does nothing. `RemovePickup` is bound since IMPL-LINK6-AFTER-TAKE (decision 2026-11-26-57; its
/// removal is tested in `link6_tests.rs`); this fixture's globals declare no `PickupList`, so it is a `Script` error
/// before it changes anything (the pickup keeps its inventory).
#[test]
fn add_to_navigation_is_a_no_op_and_remove_pickup_checks_the_list_first() {
    let mut x = fx(ParityMode::Off);
    let pickup = x.obj("WillowGame.WillowPickup");
    let thing = x.obj("Engine.Actor");
    x.set(pickup, "Inventory", Value::Object(Some(thing)));
    assert_eq!(x.call("Engine.DroppedPickup.AddToNavigation", pickup, vec![]).unwrap(), Value::None);
    let globals = x.obj("WillowGame.WillowGlobals");
    match x.call("WillowGame.WillowGlobals.RemovePickup", globals, vec![Value::Object(Some(pickup))]).unwrap_err() {
        VmError::Script { message, .. } => assert!(message.contains("PickupList"), "{message}"),
        e => panic!("expected a Script error, got {e}"),
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
        GET_PAWN_INVENTORY_MANAGER,
        PICKUPABLE_IS_ENABLED[0],
        PICKUPABLE_IS_ENABLED[1],
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

// ---- Pickupable_IsEnabled (lane IMPL-PICKUPABLE-ISENABLED, NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 5) ------------

const ENABLED_WILLOW: &str = "WillowGame.WillowPickup.Pickupable_IsEnabled";
const ENABLED_DROPPED: &str = "Engine.DroppedPickup.Pickupable_IsEnabled";

fn enabled_not_read(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!((n.missing, n.record), (Missing::NotRead, RECORD_ENABLED), "{n}");
            n.what.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// Section 5's first test: a toy `WillowPickup` with `bPickupable` set answers true, and cleared answers false, by
/// either key. No write, draw, load or name is counted.
#[test]
fn a_willow_pickup_answers_its_b_pickupable() {
    let mut x = fx(ParityMode::Off);
    let pickup = x.obj("WillowGame.WillowPickup");
    for enabled in [true, false] {
        x.set(pickup, "bPickupable", Value::Bool(enabled));
        let before = marks(&x.vm);
        for path in [ENABLED_WILLOW, ENABLED_DROPPED] {
            assert_eq!(x.call(path, pickup, vec![]).unwrap(), Value::Bool(enabled), "{path}");
        }
        assert_eq!(marks(&x.vm), before, "no code load, write, draw, store load or name");
    }
}

/// Section 5's second test: a toy `DroppedPickup` that is not a `WillowPickup` is the `NotRead` (its own answer was not
/// read), and nothing is counted.
#[test]
fn a_dropped_pickup_that_is_not_a_willow_pickup_is_not_read() {
    let mut x = fx(ParityMode::Off);
    let dropped = x.obj("Engine.DroppedPickup");
    let before = marks(&x.vm);
    let what = enabled_not_read(x.call(ENABLED_DROPPED, dropped, vec![]));
    assert_eq!(what, format!("{PICKUPABLE_OTHER} Engine.DroppedPickup: its own answer is not read"));
    assert_eq!(marks(&x.vm), before);
}

/// Section 5's third test: a subclass of `WillowPickup` with no field of its own reads the inherited `bPickupable`.
#[test]
fn a_willow_pickup_subclass_reads_the_inherited_field() {
    let mut x = fx(ParityMode::Off);
    let pickup = x.obj("WillowGame.ToyWillowPickupSubclass");
    x.set(pickup, "bPickupable", Value::Bool(true));
    assert_eq!(x.call(ENABLED_WILLOW, pickup, vec![]).unwrap(), Value::Bool(true));
    x.set(pickup, "bPickupable", Value::Bool(false));
    assert_eq!(x.call(ENABLED_WILLOW, pickup, vec![]).unwrap(), Value::Bool(false));
}

/// Where the note is silent: a `WillowPickup` the store has not loaded (here a slotless stand-in) is the `NotRead`,
/// its slots not being the package's values; nothing is counted.
#[test]
fn an_unloaded_willow_pickup_is_not_read() {
    let mut x = fx(ParityMode::Off);
    let pickup = x.obj("WillowGame.WillowPickup");
    x.vm.space.heap.get_mut(pickup).unwrap().slots.clear();
    let before = marks(&x.vm);
    assert_eq!(enabled_not_read(x.call(ENABLED_WILLOW, pickup, vec![])), PICKUPABLE_UNLOADED);
    assert_eq!(marks(&x.vm), before);
}

fn enabled_script(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::Script { function, message }) => {
            assert_eq!(function, "Pickupable_IsEnabled");
            message
        }
        other => panic!("expected VmError::Script, got {other:?}"),
    }
}

/// Review of #765, item 1: a `WillowPickup` class that declares no `bPickupable` is the data error, by either key, and
/// it is found before the loaded check: an unloaded receiver of that class is the data error too, not the `NotRead`.
/// Nothing is counted.
#[test]
fn a_willow_pickup_class_without_b_pickupable_is_a_script_error() {
    let mut x = fx_with(ParityMode::Off, None);
    let pickup = x.obj("WillowGame.WillowPickup");
    let before = marks(&x.vm);
    for path in [ENABLED_WILLOW, ENABLED_DROPPED] {
        let message = enabled_script(x.call(path, pickup, vec![]));
        assert!(message.contains("declares no bPickupable"), "{path}: {message}");
    }
    x.vm.space.heap.get_mut(pickup).unwrap().slots.clear();
    let message = enabled_script(x.call(ENABLED_WILLOW, pickup, vec![]));
    assert!(message.contains("declares no bPickupable"), "{message}");
    assert_eq!(marks(&x.vm), before);
}

/// Review of #765, item 1: a `bPickupable` declared as an int is the data error, before the loaded check too.
#[test]
fn a_non_bool_b_pickupable_is_a_script_error() {
    let mut x = fx_with(ParityMode::Off, Some("Int"));
    let pickup = x.obj("WillowGame.ToyWillowPickupSubclass");
    let before = marks(&x.vm);
    let message = enabled_script(x.call(ENABLED_WILLOW, pickup, vec![]));
    assert!(message.contains("not a bool"), "{message}");
    x.vm.space.heap.get_mut(pickup).unwrap().slots.clear();
    let message = enabled_script(x.call(ENABLED_WILLOW, pickup, vec![]));
    assert!(message.contains("not a bool"), "{message}");
    assert_eq!(marks(&x.vm), before);
}

// ---- GetPawnInventoryManager (lane IMPL-GETPAWNINVENTORYMANAGER, NATIVE_CONTROLLER_HELPERS.md) ----------------------

const PAWN_MANAGER: &str = "WillowGame.WillowPlayerController.GetPawnInventoryManager";

/// The rule for an on-foot pawn: no pawn, None; the pawn's `InvManager` when it is a `WillowInventoryManager`, else
/// None (an empty slot, or another class). Nothing is written, drawn, loaded or interned.
#[test]
fn get_pawn_inventory_manager_answers_an_on_foot_pawn_s_manager() {
    let mut x = fx(ParityMode::Off);
    let controller = x.obj("WillowGame.WillowPlayerController");
    let maya = x.obj("WillowGame.WillowPlayerPawn");
    let manager = x.obj("WillowGame.WillowInventoryManager");
    let other = x.obj("Engine.Actor");
    let ask = |x: &mut Fx| {
        let before = marks(&x.vm);
        let r = x.call(PAWN_MANAGER, controller, vec![]).unwrap();
        assert_eq!(marks(&x.vm), before, "nothing written, drawn, loaded or interned");
        r
    };
    assert_eq!(ask(&mut x), Value::Object(None), "no pawn");
    x.set(controller, "Pawn", Value::Object(Some(maya)));
    assert_eq!(ask(&mut x), Value::Object(None), "no manager");
    x.set(maya, "InvManager", Value::Object(Some(manager)));
    assert_eq!(ask(&mut x), Value::Object(Some(manager)));
    x.set(maya, "InvManager", Value::Object(Some(other)));
    assert_eq!(ask(&mut x), Value::Object(None), "not a WillowInventoryManager");
}

/// Review of #778: the rule's other on-foot class, a `WillowPawn` that is not a player pawn, answers its manager the
/// same way.
#[test]
fn get_pawn_inventory_manager_answers_a_willow_pawn_s_manager() {
    let mut x = fx(ParityMode::Off);
    let controller = x.obj("WillowGame.WillowPlayerController");
    let pawn = x.obj("WillowGame.WillowPawn");
    let manager = x.obj("WillowGame.WillowInventoryManager");
    x.set(controller, "Pawn", Value::Object(Some(pawn)));
    x.set(pawn, "InvManager", Value::Object(Some(manager)));
    let before = marks(&x.vm);
    assert_eq!(x.call(PAWN_MANAGER, controller, vec![]).unwrap(), Value::Object(Some(manager)));
    assert_eq!(marks(&x.vm), before, "nothing written, drawn, loaded or interned");
}

/// Review of #778: an on-foot pawn whose `InvManager` the store has not loaded is the not-loaded `NotRead`, nothing
/// counted.
#[test]
fn get_pawn_inventory_manager_refuses_a_cold_manager() {
    let mut x = fx(ParityMode::Off);
    let controller = x.obj("WillowGame.WillowPlayerController");
    let maya = x.obj("WillowGame.WillowPlayerPawn");
    let (manager, _) = cold(&mut x, "WillowGame.WillowInventoryManager");
    x.vm.space.heap.get_mut(manager).unwrap().slots.clear();
    x.set(controller, "Pawn", Value::Object(Some(maya)));
    x.set(maya, "InvManager", Value::Object(Some(manager)));
    let before = marks(&x.vm);
    match x.call(PAWN_MANAGER, controller, vec![]) {
        Err(VmError::NotYet(n)) => {
            assert_eq!((n.missing, n.record), (Missing::NotRead, RECORD_HELPERS), "{n}");
            assert_eq!(n.what.as_ref(), INVENTORY_MANAGER_UNLOADED);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(marks(&x.vm), before);
}

/// The note's refusals: a vehicle (or any pawn whose first native class is not an on-foot Willow pawn) is the
/// `NotRead` naming it; a pawn the store has not loaded is the not-loaded `NotRead`. Nothing is counted.
#[test]
fn get_pawn_inventory_manager_refuses_a_vehicle_and_a_cold_pawn() {
    let mut x = fx(ParityMode::Off);
    let controller = x.obj("WillowGame.WillowPlayerController");
    let car = x.obj("Engine.Vehicle");
    x.set(controller, "Pawn", Value::Object(Some(car)));
    let before = marks(&x.vm);
    match x.call(PAWN_MANAGER, controller, vec![]) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(marks(&x.vm), before);
            assert_eq!((n.missing, n.record), (Missing::NotRead, RECORD_HELPERS), "{n}");
            assert!(n.what.starts_with(&format!("{INVENTORY_MANAGER_OTHER} Engine.Vehicle ")), "{}", n.what);
        }
        other => panic!("{other:?}"),
    }
    let (cold_pawn, _) = cold(&mut x, "WillowGame.WillowPlayerPawn");
    x.vm.space.heap.get_mut(cold_pawn).unwrap().slots.clear();
    x.set(controller, "Pawn", Value::Object(Some(cold_pawn)));
    let before_cold = marks(&x.vm);
    match x.call(PAWN_MANAGER, controller, vec![]) {
        Err(VmError::NotYet(n)) => assert_eq!(n.what.as_ref(), INVENTORY_MANAGER_UNLOADED),
        other => panic!("{other:?}"),
    }
    assert_eq!(marks(&x.vm), before_cold);
}
