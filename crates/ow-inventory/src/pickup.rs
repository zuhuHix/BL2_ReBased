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
//! - **`DroppedPickup.TouchPickupTrace(StartLoc, EndLoc)`** (section 3; the Off contract): `NotYet { NotRead }`. What
//!   blocks the trace inside the native was not read, so a touch's validity cannot be settled in Off.
//! - **`DroppedPickup.AddToNavigation()`** (high confidence): does nothing in this build.
//! - **`WillowGlobals.RemovePickup(Pickup)`** (medium confidence): `NotYet { NotRead }`, decided before anything
//!   changes. The note says it removes the pickup from the globals' pickup list and fires `OnPickupDetached`, and
//!   `OnAllPickupsDetached` when no other pickup of the same holder is left, but names neither the list nor the holder
//!   nor how the two global events are delivered.
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

/// The native keys [`register`] binds in Off.
pub const GET_INVENTORY_PAWN: &str = "WillowPlayerController.GetInventoryPawn()";
pub const ADD_TO_NAVIGATION: &str = "DroppedPickup.AddToNavigation()";
pub const REMOVE_PICKUP: &str = "WillowGlobals.RemovePickup(object)";
pub const TOUCH_PICKUP_TRACE: &str = "DroppedPickup.TouchPickupTrace(vector,vector)";

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
    let natives: [(&str, NativeFn); 4] = [
        (GET_INVENTORY_PAWN, Rc::new(get_inventory_pawn)),
        (ADD_TO_NAVIGATION, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::None))),
        (
            REMOVE_PICKUP,
            Rc::new(|_: &mut NativeCall<'_>| {
                Err(NotYet::not_read("the pickup events of RemovePickup", crate::globals::RECORD).into())
            }),
        ),
        (
            TOUCH_PICKUP_TRACE,
            Rc::new(|_: &mut NativeCall<'_>| {
                Err(NotYet::not_read(
                    "DroppedPickup.TouchPickupTrace (what blocks the touch trace inside the native was not read, so a \
                     touch's validity cannot be settled in Off)",
                    RECORD,
                )
                .into())
            }),
        ),
    ];
    for (key, f) in natives {
        vm.register_native(key, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "pickup_tests.rs"]
mod tests;
