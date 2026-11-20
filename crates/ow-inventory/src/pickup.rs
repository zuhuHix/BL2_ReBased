//! Owner: lane IMPL-PICKUP. AI-assisted (Claude Code). The natives a dropped item's pickup reaches (slice links 6 and
//! 7), coded from NATIVE_PICKUP.md alone (decision 2026-11-19-68). This lane did not read the natives. Every rule is
//! the note's reading of the executable, **UNVERIFIED** in the running game. The pickup itself is installed script
//! (the touch state, the key, `GiveTo`, `AddInventory`, the destroy), and the VM runs it.
//!
//! - **`WillowPlayerController.GetInventoryPawn()`** (section 7's table, medium confidence; the Off contract): the
//!   controller's `Pawn`, None without one. Only a plain player pawn is supported: one of the concrete class
//!   `WillowPlayerPawn`, which answers as no vehicle. Any other pawn, a subclass included, is `NotYet { NotRead }`,
//!   because the note leaves the vehicle test open (question 4) and a vehicle gives its `Driver` instead. A failed
//!   class lookup is that error, not the refusal (review r2 of #452).
//! - **`DroppedPickup.TouchPickupTrace(StartLoc, EndLoc)`** (lane IMPL-TOUCH-PICKUP-TRACE, decision 2026-11-19-NNN),
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
//!     the note does not say the native runs an override.
//!
//! **Only in `ParityMode::Off`.** The C++ reference has no stand-in, so Reference binds nothing here (R3).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
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
pub const ADD_TO_NAVIGATION: &str = "DroppedPickup.AddToNavigation()";
pub const REMOVE_PICKUP: &str = "WillowGlobals.RemovePickup(object)";
pub const TOUCH_PICKUP_TRACE: &str = "DroppedPickup.TouchPickupTrace(vector,vector)";
pub const GET_PICKUPABLE_INVENTORY: &str = "DroppedPickup.GetPickupableInventory()";
pub const GET_PICKUPABLE_INVENTORY_DEFINITION: &str = "DroppedPickup.GetPickupableInventoryDefinition()";

/// The record of `TouchPickupTrace`.
pub const RECORD_TOUCH: &str = ow_world::trace_filter::RECORD_TOUCH;
/// The two classes whose table holds the read implementation (note section 1).
pub const TOUCH_CLASSES: [&str; 2] = ["Engine.DroppedPickup", "WillowGame.WillowPickup"];
/// The prefix of the exact-class refusal.
pub const TOUCH_TRACE_OF: &str = "the touch trace of";
/// The refusal of an unloaded receiver.
pub const TOUCH_UNLOADED: &str = "the touch trace of a pickup the store has not loaded (the walk needs its Base chain)";

/// Argument `i` as a `Vector` struct's three floats, else a data error.
fn vector_arg(c: &NativeCall<'_>, i: usize, what: &str) -> Result<[f32; 3], VmError> {
    let v = c.arg(i)?;
    let not = || bad(what, format!("argument {i} is not a Vector"));
    let s = v.as_struct().ok_or_else(not)?;
    let classes = &c.vm.space.classes;
    let layout = classes.struct_layout(s.ty).ok_or_else(not)?;
    if !layout.path().name().eq_ignore_ascii_case("Vector") {
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
    let Some(f) = c.vm.find_method(class, "GetInventoryDefinition", state)? else {
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

fn get_inventory_pawn(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerController.GetInventoryPawn";
    let controller = c.this.ok_or_else(|| bad(WHAT, "called without a controller"))?;
    let Some(pawn) = object_of(&required(c.vm, controller, "Pawn", WHAT)?, WHAT)? else {
        return Ok(Value::Object(None));
    };
    let player = c.vm.find_class("WillowGame.WillowPlayerPawn")?;
    let class = c.vm.space.heap.get(pawn)?.class;
    if class != player {
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

/// Binds the module's natives in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let natives: [(&str, NativeFn); 6] = [
        (GET_INVENTORY_PAWN, Rc::new(get_inventory_pawn)),
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
    Ok(())
}

#[cfg(test)]
#[path = "pickup_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pickup_touch_tests.rs"]
mod touch_tests;
