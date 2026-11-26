//! Owner: lane IMPL-PICKUP. AI-assisted (Claude Code). The natives a dropped item's pickup reaches (slice links 6 and
//! 7), coded from NATIVE_PICKUP.md alone (decision 2026-11-19-68). This lane did not read the natives. Every rule is
//! the note's reading of the executable, **UNVERIFIED** in the running game. The pickup itself is installed script
//! (the touch state, the key, `GiveTo`, `AddInventory`, the destroy), and the VM runs it.
//!
//! - **`WillowPlayerController.GetInventoryPawn()`** (section 7's table, medium confidence; the Off contract): the
//!   controller's `Pawn`, None without one. Only a plain player pawn is supported: one of the concrete class
//!   `WillowPlayerPawn`, which answers as no vehicle. Any other pawn, a subclass included, is `NotYet { NotRead }`,
//!   because the note leaves the vehicle test open (question 4) and a vehicle gives its `Driver` instead. The class
//!   is compared through the class table by path, so no class code is loaded (lane COLD-LOAD-FIXES); a table that does
//!   not hold `WillowPlayerPawn` answers the refusal.
//! - **`DroppedPickup.TouchPickupTrace(StartLoc, EndLoc)`** (lane IMPL-TOUCH-PICKUP-TRACE, decision 2026-11-19-250),
//!   coded from NATIVE_TOUCH_PICKUP_TRACE.md section 4 alone (#624, accepted at 6dbdea1e; clean room: analyst lane
//!   ANALYST-LINKS-6-7-PAST-TOUCH read the native, this lane did not), UNVERIFIED. It replaces the earlier
//!   `NotYet { NotRead }` stub. Steps 1 to 4 are here, each before anything else is read:
//!   1. no receiver: a data error;
//!   2. the receiver's exact class neither `Engine.DroppedPickup` nor `WillowGame.WillowPickup` (looked up through the
//!      class table's `by_path` only): `NotYet { NotRead }`, "the touch trace of <class>". No other subclass's table
//!      was read;
//!   3. the two arguments not `Vector` structs: a data error;
//!   4. an unloaded receiver: `NotYet { NotRead }` (the walk needs its `Base` chain).
//!
//!   Steps 5 to 9 (the collision service, the level part from `EndLoc` to `StartLoc`, the actor walk with the pickup
//!   as the source actor) are `ow_world::touch_trace::touch_line`. Nothing is written, loaded or drawn.
//! - **`DroppedPickup.AddToNavigation()`** (high confidence): does nothing in this build.
//! - **`WillowGlobals.RemovePickup(Pickup)`** (medium confidence): `NotYet { NotRead }`, decided before anything
//!   changes. The note says it removes the pickup from the globals' pickup list and fires `OnPickupDetached`, and
//!   `OnAllPickupsDetached` when no other pickup of the same holder is left, but names neither the list nor the holder
//!   nor how the two global events are delivered.
//!
//! - **`DroppedPickup.GetPickupableInventory()` and `GetPickupableInventoryDefinition()`** (lane LINKS-6-8-NEXT,
//!   decision 2026-11-19-40, assigned -103), coded from NATIVE_INVENTORY_EQUIP.md, "Pickup interface accessors"
//!   (analyst lane G12, medium confidence: the implementers behind the interface table were not each read). They
//!   write nothing.
//!   - `GetPickupableInventory`: the pickup's `Inventory`, as stored (None included).
//!   - `GetPickupableInventoryDefinition`: "that inventory's definition". It is read as the inventory's own
//!     `GetInventoryDefinition()`, dispatched through the VM on the inventory's class, only when that resolves to the
//!     native `Engine.WillowInventory.GetInventoryDefinition` (the guard of #467's `native_getter`, review r1 of
//!     #487). An unbound one is `VmError::UnboundNative`, unchanged (UNVERIFIED that the native asks that virtual).
//!   - Where the note is silent, a named `NotYet { NotRead }`: a pickup with no `Inventory` ([`NO_INVENTORY`]); an
//!     inventory whose class declares no `GetInventoryDefinition` ([`NO_DEFINITION_QUERY`]); and one whose query
//!     resolves to anything but that native, a script or state override included ([`NO_NATIVE_DEFINITION_QUERY`]):
//!     the note does not say the native runs an override. The query is resolved without a load
//!     (`Vm::find_loaded_method`, lane COLD-LOAD-FIXES): a class of the chain whose code is not loaded is a named
//!     `NotYet { NotRead }`, "the class <path>, not loaded (a class load is a write)".
//!
//! - **`DroppedPickup.Pickupable_IsEnabled()` and `WillowPickup.Pickupable_IsEnabled()`** (lane
//!   IMPL-PICKUPABLE-ISENABLED, decision 2026-11-26-22), coded from NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 5
//!   alone (clean room: analyst lane F read the native, this lane did not), UNVERIFIED. Both keys share one rule:
//!   - a receiver of class `WillowGame.WillowPickup` or a subclass (the class table by path, so no code is loaded):
//!     its `bPickupable`, read **by name** through the class layout, never by offset (the note places the field after
//!     `RBState` under a fitted 16-byte alignment, UNVERIFIED). A class that declares no `bPickupable`, or one whose
//!     `bPickupable` is not a bool, is a data error (`VmError::Script`), checked from the class before the loaded test
//!     below (review of #765, item 1);
//!   - any other receiver, a `DroppedPickup` that is not a `WillowPickup` among them: `NotYet { NotRead }`
//!     ([`PICKUPABLE_OTHER`]): `DroppedPickup`'s own answer was not read;
//!   - a receiver the store has not loaded: `NotYet { NotRead }` ([`PICKUPABLE_UNLOADED`]). The note does not cover
//!     it, and a handle's slots are not the package's values.
//!
//!   Nothing is written, loaded or drawn.
//!
//! - **`WillowPlayerController.GetPawnInventoryManager()`** (lane IMPL-GETPAWNINVENTORYMANAGER, decision
//!   2026-11-26-NNN), coded from NATIVE_CONTROLLER_HELPERS.md, "WillowPlayerController.GetPawnInventoryManager" alone
//!   (high confidence; the vehicle seats medium), UNVERIFIED. It writes and loads nothing:
//!   - no `Pawn`: None;
//!   - the pawn's first native class `WillowGame.WillowPawn` or `WillowGame.WillowPlayerPawn` (whose "vehicle/base"
//!     virtual is the base `WillowPawn`'s do-nothing stub, so the pawn is on foot): its `InvManager` when that is a
//!     `WillowInventoryManager`, else None;
//!   - any other pawn class, a vehicle among them: [`INVENTORY_MANAGER_OTHER`] (`NotRead`): its virtual, and so
//!     whether its `Driver` answers, was not read;
//!   - a pawn or manager the store has not loaded: [`INVENTORY_MANAGER_UNLOADED`] (`NotRead`).
//!
//! **Only in `ParityMode::Off`.** The C++ reference has no stand-in, so Reference binds nothing here (R3).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ClassId;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use crate::equip::{bad, object_of, required};

/// The record these natives follow.
pub const RECORD: &str = "NATIVE_PICKUP.md";

/// The record of the two pickup accessors (lane G12).
pub const RECORD_EQUIP: &str = "NATIVE_INVENTORY_EQUIP.md";
/// The `NotYet` of a pickup with no inventory (the note is silent).
pub const NO_INVENTORY: &str = "DroppedPickup.GetPickupableInventoryDefinition on a pickup with no Inventory";
/// The `NotYet` of an inventory whose class has no definition query (the note is silent).
pub const NO_DEFINITION_QUERY: &str =
    "DroppedPickup.GetPickupableInventoryDefinition on an inventory whose class declares no GetInventoryDefinition";
/// The `NotYet` of an inventory whose query resolves to anything but the native (the note is silent on overrides).
pub const NO_NATIVE_DEFINITION_QUERY: &str = "DroppedPickup.GetPickupableInventoryDefinition on an inventory whose \
     GetInventoryDefinition is not the native Engine.WillowInventory.GetInventoryDefinition (an override)";
/// The native declaration the definition query must resolve to (the one path, from `crate::definition`).
pub const NATIVE_DEFINITION_QUERY: &str = crate::definition::GET_INVENTORY_DEFINITION;

/// The native keys [`register`] binds in Off.
pub const GET_INVENTORY_PAWN: &str = "WillowPlayerController.GetInventoryPawn()";
pub const GET_PAWN_INVENTORY_MANAGER: &str = "WillowPlayerController.GetPawnInventoryManager()";
/// `GetPawnInventoryManager`'s record.
pub const RECORD_HELPERS: &str = "NATIVE_CONTROLLER_HELPERS.md";
/// The first native pawn classes whose "vehicle/base" virtual is the base stub (an on-foot pawn).
pub const ON_FOOT_PAWNS: [&str; 2] = ["WillowGame.WillowPawn", "WillowGame.WillowPlayerPawn"];
/// The start of the `NotRead` for any other pawn: "<this> <class> (...)".
pub const INVENTORY_MANAGER_OTHER: &str = "GetPawnInventoryManager on a";
/// The `NotRead` for a pawn or manager the store has not loaded.
pub const INVENTORY_MANAGER_UNLOADED: &str =
    "GetPawnInventoryManager: a pawn or inventory manager the store has not loaded (a load is a write)";
/// The class `GetPawnInventoryManager` answers.
pub const INVENTORY_MANAGER: &str = "WillowGame.WillowInventoryManager";
pub const ADD_TO_NAVIGATION: &str = "DroppedPickup.AddToNavigation()";
pub const REMOVE_PICKUP: &str = "WillowGlobals.RemovePickup(object)";
pub const TOUCH_PICKUP_TRACE: &str = "DroppedPickup.TouchPickupTrace(vector,vector)";
pub const GET_PICKUPABLE_INVENTORY: &str = "DroppedPickup.GetPickupableInventory()";
pub const GET_PICKUPABLE_INVENTORY_DEFINITION: &str = "DroppedPickup.GetPickupableInventoryDefinition()";
/// The two `Pickupable_IsEnabled` keys (NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 5), one rule.
pub const PICKUPABLE_IS_ENABLED: [&str; 2] =
    ["DroppedPickup.Pickupable_IsEnabled()", "WillowPickup.Pickupable_IsEnabled()"];

/// The record `Pickupable_IsEnabled` follows.
pub const RECORD_ENABLED: &str = "NATIVE_TOUCH_COMPONENT_TEMPLATE.md";
/// The class whose answer is read: it and its subclasses answer their `bPickupable`.
pub const WILLOW_PICKUP: &str = "WillowGame.WillowPickup";
/// The start of the `NotRead` for any other receiver: "Pickupable_IsEnabled on <class>: its own answer is not read".
pub const PICKUPABLE_OTHER: &str = "Pickupable_IsEnabled on";
/// The `NotRead` for a receiver the store has not loaded.
pub const PICKUPABLE_UNLOADED: &str = "Pickupable_IsEnabled on a pickup the store has not loaded";

/// The record of `TouchPickupTrace`.
pub const RECORD_TOUCH: &str = ow_world::trace_filter::RECORD_TOUCH;
/// The two classes whose table holds the read implementation (note section 1).
pub const TOUCH_CLASSES: [&str; 2] = ["Engine.DroppedPickup", "WillowGame.WillowPickup"];
/// The prefix of the exact-class refusal.
pub const TOUCH_TRACE_OF: &str = "the touch trace of";
/// The refusal of an unloaded receiver.
pub const TOUCH_UNLOADED: &str = "the touch trace of a pickup the store has not loaded (the walk needs its Base chain)";

/// Argument `i` as the declared `Vector` struct's three floats, else a data error: a struct of any other id (another
/// struct named `Vector` included) and a script `None` (kept by [`Vm::keep_struct_none`]) are refused (review r1 of
/// #638, item 2).
fn vector_arg(c: &NativeCall<'_>, i: usize, what: &str) -> Result<[f32; 3], VmError> {
    let v = c.arg(i)?;
    let not = || bad(what, format!("argument {i} is not a Vector"));
    let s = v.as_struct().ok_or_else(not)?;
    let classes = &c.vm.space.classes;
    let declared = c.vm.function(c.function).and_then(|f| f.params.get(i)).map(|p| p.kind.clone());
    let Some(PropKind::Struct(want)) = declared else {
        return Err(bad(what, format!("parameter {i} is not declared a struct")));
    };
    let layout = classes.struct_layout(want).ok_or_else(not)?;
    if s.ty != want || !layout.path().name().eq_ignore_ascii_case("Vector") {
        return Err(not());
    }
    let mut out = [0.0; 3];
    for (slot, axis) in out.iter_mut().zip(["X", "Y", "Z"]) {
        let id = c.vm.space.names.find(axis).ok_or_else(not)?;
        let (k, _) = layout.field(id).ok_or_else(not)?;
        *slot = match s.fields.get(k as usize) {
            Some(Value::Float(x)) => *x,
            _ => return Err(not()),
        };
    }
    Ok(out)
}

fn touch_pickup_trace(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "DroppedPickup.TouchPickupTrace";
    // 1. The receiver.
    let pickup = c.this.ok_or_else(|| bad(WHAT, "called without a pickup"))?;
    // 2. Its exact class, by the class table alone.
    let class = c.vm.space.heap.get(pickup)?.class;
    if !TOUCH_CLASSES.iter().any(|path| c.vm.space.classes.by_path(path) == Some(class)) {
        let path = ow_attr::eval::class_path(&c.vm.space, pickup);
        return Err(NotYet::not_read(format!("{TOUCH_TRACE_OF} {path}"), RECORD_TOUCH).into());
    }
    // 3. The arguments.
    let start = vector_arg(c, 0, WHAT)?;
    let end = vector_arg(c, 1, WHAT)?;
    // 4. A receiver the store has not loaded.
    if ow_world::touch_trace::is_unloaded(c.vm, pickup) {
        return Err(NotYet::not_read(TOUCH_UNLOADED, RECORD_TOUCH).into());
    }
    Ok(Value::Bool(ow_world::touch_trace::touch_line(c.vm, pickup, start, end)?))
}

/// `Pickupable_IsEnabled()` (module documentation): a `WillowPickup`'s `bPickupable`, read by name.
fn pickupable_is_enabled(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "Pickupable_IsEnabled";
    let pickup = c.this.ok_or_else(|| bad(WHAT, "called without a pickup"))?;
    let class = c.vm.space.heap.get(pickup)?.class;
    let willow = c.vm.space.classes.by_path(WILLOW_PICKUP);
    if !willow.is_some_and(|w| c.vm.space.classes.is_child_of(class, w)) {
        let path = ow_attr::eval::class_path(&c.vm.space, pickup);
        return Err(
            NotYet::not_read(format!("{PICKUPABLE_OTHER} {path}: its own answer is not read"), RECORD_ENABLED).into()
        );
    }
    // The declaration first (review of #765, item 1): a subclass that declares no bool `bPickupable` is the data
    // error the module documentation names, loaded or not.
    let declared = c.vm.space.prop(class, "bPickupable").and_then(|p| c.vm.space.classes.decl(p));
    match declared {
        None => return Err(bad(WHAT, "the pickup's class declares no bPickupable")),
        Some(d) if !matches!(d.kind, PropKind::Bool) => {
            return Err(bad(WHAT, format!("bPickupable is declared as {:?}, not a bool", d.kind)));
        }
        Some(_) => {}
    }
    if ow_world::touch_trace::is_unloaded(c.vm, pickup) {
        return Err(NotYet::not_read(PICKUPABLE_UNLOADED, RECORD_ENABLED).into());
    }
    match required(c.vm, pickup, "bPickupable", WHAT)? {
        Value::Bool(enabled) => Ok(Value::Bool(enabled)),
        other => Err(bad(WHAT, format!("bPickupable is not a bool: {other:?}"))),
    }
}

fn pickup_inventory(c: &mut NativeCall<'_>, what: &str) -> Result<Option<ow_object::ids::ObjRef>, VmError> {
    let pickup = c.this.ok_or_else(|| bad(what, "called without a pickup"))?;
    object_of(&required(c.vm, pickup, "Inventory", what)?, what)
}

fn get_pickupable_inventory(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Ok(Value::Object(pickup_inventory(c, "DroppedPickup.GetPickupableInventory")?))
}

fn get_pickupable_inventory_definition(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "DroppedPickup.GetPickupableInventoryDefinition";
    // No inventory is this native's named NotYet. `pickup_targeting::definition_of` answers Ok(None) there, following
    // NATIVE_PICKUP_TARGETING.md, not this native: the two are not to be aligned (review r1 of #487).
    let Some(inventory) = pickup_inventory(c, WHAT)? else {
        return Err(NotYet::not_read(NO_INVENTORY, RECORD_EQUIP).into());
    };
    let (class, state) = {
        let x = c.vm.space.heap.get(inventory)?;
        (x.class, x.state)
    };
    // Resolved without a load (lane COLD-LOAD-FIXES): a class of the chain whose package code is not loaded makes the
    // answer need a load, which is a write before the dispatch check.
    let Some(found) = c.vm.find_loaded_method(class, "GetInventoryDefinition", state) else {
        return Err(cold_class(c.vm, class));
    };
    let Some(f) = found else {
        return Err(NotYet::not_read(NO_DEFINITION_QUERY, RECORD_EQUIP).into());
    };
    // Checked before anything runs: only the native declaration is dispatched.
    let native = c
        .vm
        .function(f)
        .is_some_and(|d| d.native_key.is_some() && d.path.as_str().eq_ignore_ascii_case(NATIVE_DEFINITION_QUERY));
    if !native {
        return Err(NotYet::not_read(NO_NATIVE_DEFINITION_QUERY, RECORD_EQUIP).into());
    }
    Ok(c.vm.call(f, Some(inventory), vec![])?.ret)
}

/// The first class of `class`'s chain whose package code is not loaded, as a named `NotYet { NotRead }`: a class load
/// is a write (lane COLD-LOAD-FIXES).
fn cold_class(vm: &Vm, class: ClassId) -> VmError {
    let mut cursor = Some(class);
    let mut cold = class;
    while let Some(k) = cursor {
        if vm.defs.code().key_of(k).is_some() && !vm.class_code_loaded(k) {
            cold = k;
            break;
        }
        cursor = vm.space.classes.get_layout(k).and_then(|l| l.super_class());
    }
    let path = vm.space.classes.get_layout(cold).map_or_else(|| format!("{cold:?}"), |l| l.path().as_str().to_string());
    NotYet::not_read(
        format!("GetPickupableInventoryDefinition: the class {path}, not loaded (a class load is a write)"),
        RECORD_EQUIP,
    )
    .into()
}

fn get_inventory_pawn(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerController.GetInventoryPawn";
    let controller = c.this.ok_or_else(|| bad(WHAT, "called without a controller"))?;
    let Some(pawn) = object_of(&required(c.vm, controller, "Pawn", WHAT)?, WHAT)? else {
        return Ok(Value::Object(None));
    };
    // The class table by path, read only (lane COLD-LOAD-FIXES): the pawn's class is resident, so when it is
    // `WillowPlayerPawn` the table holds that path; a table without it means the pawn is another class. No class code
    // is loaded.
    let class = c.vm.space.heap.get(pawn)?.class;
    if c.vm.space.classes.by_path("WillowGame.WillowPlayerPawn") != Some(class) {
        return Err(NotYet::not_read(
            format!(
                "GetInventoryPawn on a {} (only a plain player pawn is supported: the vehicle test, and so whether its \
                 Driver answers, is open question 4)",
                ow_attr::eval::class_path(&c.vm.space, pawn)
            ),
            RECORD,
        )
        .into());
    }
    Ok(Value::Object(Some(pawn)))
}

/// `GetPawnInventoryManager()` (module documentation).
fn get_pawn_inventory_manager(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerController.GetPawnInventoryManager";
    let controller = c.this.ok_or_else(|| bad(WHAT, "called without a controller"))?;
    let vm: &Vm = c.vm;
    let Some(pawn) = object_of(&required(vm, controller, "Pawn", WHAT)?, WHAT)? else {
        return Ok(Value::Object(None));
    };
    let unloaded = |o| ow_world::touch_trace::is_unloaded(vm, o) || vm.defs.is_failed(&vm.space, o);
    if unloaded(pawn) {
        return Err(NotYet::not_read(INVENTORY_MANAGER_UNLOADED, RECORD_HELPERS).into());
    }
    let class = vm.space.heap.get(pawn)?.class;
    let native = ow_world::world::render_hooks::first_native(vm, class);
    if !native.as_deref().is_some_and(|n| ON_FOOT_PAWNS.iter().any(|p| n.eq_ignore_ascii_case(p))) {
        let named = native.unwrap_or_else(|| ow_attr::eval::class_path(&vm.space, pawn));
        return Err(NotYet::not_read(
            format!(
                "{INVENTORY_MANAGER_OTHER} {named} (its vehicle/base virtual, and so whether its Driver answers, is not \
                 read)"
            ),
            RECORD_HELPERS,
        )
        .into());
    }
    let Some(manager) = object_of(&required(vm, pawn, "InvManager", WHAT)?, WHAT)? else {
        return Ok(Value::Object(None));
    };
    if unloaded(manager) {
        return Err(NotYet::not_read(INVENTORY_MANAGER_UNLOADED, RECORD_HELPERS).into());
    }
    let manager_class = vm.space.heap.get(manager)?.class;
    let is_manager =
        vm.space.classes.by_path(INVENTORY_MANAGER).is_some_and(|m| vm.space.classes.is_child_of(manager_class, m));
    Ok(Value::Object(is_manager.then_some(manager)))
}

/// Binds the module's natives in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let natives: [(&str, NativeFn); 7] = [
        (GET_INVENTORY_PAWN, Rc::new(get_inventory_pawn)),
        (GET_PAWN_INVENTORY_MANAGER, Rc::new(get_pawn_inventory_manager)),
        (GET_PICKUPABLE_INVENTORY, Rc::new(get_pickupable_inventory)),
        (GET_PICKUPABLE_INVENTORY_DEFINITION, Rc::new(get_pickupable_inventory_definition)),
        (ADD_TO_NAVIGATION, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::None))),
        (
            REMOVE_PICKUP,
            Rc::new(|_: &mut NativeCall<'_>| {
                Err(NotYet::not_read("the pickup events of RemovePickup", crate::globals::RECORD).into())
            }),
        ),
        (TOUCH_PICKUP_TRACE, Rc::new(touch_pickup_trace)),
    ];
    for (key, f) in natives {
        vm.register_native(key, f)?;
    }
    for key in PICKUPABLE_IS_ENABLED {
        vm.register_native(key, Rc::new(pickupable_is_enabled))?;
    }
    // Link 6's key: the pickup's cost and the ammo refresh (NATIVE_LINK67_KEY_AND_LOOK.md; lane IMPL-LINK67-KEY).
    crate::pickup_cost::register(vm)?;
    // The touch trace refuses a script None in place of a vector (step 3), so the interpreter hands it over.
    vm.keep_struct_none(TOUCH_PICKUP_TRACE);
    Ok(())
}

#[cfg(test)]
#[path = "pickup_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pickup_touch_tests.rs"]
mod touch_tests;
