//! Owner: lane IMPL-LINK67-KEY (queue 0ah). Link 6's `key`, the pickup's cost: `WillowPlayerController`'s
//! `CanAffordToPickUpPickupable(Pickupable)` and `PayForPickupable(Pickupable)`, and `UpdateAmmoCounts(bSilent)`'s
//! refusal. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINK67_KEY_AND_LOOK.md sections 2 and 4 alone (lane
//! ANALYST-LINK67-KEY-AND-LOOK, #772 with the hub's review fixes, on main c345fdb); this lane did not read the
//! executable. **Every rule is UNVERIFIED in the running game.** Off only; Reference binds nothing (the stub policy
//! answers).
//!
//! **`CanAffordToPickUpPickupable`** (section 2.4), a preflight that writes, loads and interns nothing:
//! 1. no pickupable: true;
//! 2. a pickupable the store has not loaded: [`PICKUPABLE_NOT_LOADED`] (`NotRead`);
//!
//!    (Each shared refusal text starts with the asking native's name, [`CAN_AFFORD_NAME`] or [`PAY_FOR_NAME`]:
//!    review of #774.)
//! 3. **the cost query, by fit (UNVERIFIED):** the pickupable's own `bCostsToPickUp` (bool), `CostsToPickUpType`
//!    (byte) and `CostsToPickUpAmount` (int), read by name. A class that declares any of them otherwise, or not at all,
//!    is [`COST_QUERY`] (`NotRead`): its own answer was not read;
//! 4. no cost: true;
//! 5. golden keys (type 3): [`GOLDEN_KEY_COUNT`] (`NotRead`);
//! 6. the controller's `PlayerReplicationInfo`: a handle the store has not loaded is [`INFO_NOT_LOADED`] (`NotRead`),
//!    before the class test; None, or not a `WillowPlayerReplicationInfo`, true; otherwise `amount <=
//!    GetCurrencyOnHand(type)`, asked through the VM (the bound native).
//!
//! **`PayForPickupable`** (section 2.2): steps 1 to 4 and step 6's not-loaded check and cast only, not step 5. Golden
//! keys raise the controller's `SpendGoldenKey` once per unit (an amount of 0 or less: none). Any other type, with the
//! cast replication info, calls `AddCurrencyOnHand(type, -amount)` last; a failed cast does nothing. Every refusal
//! of the checks comes before the first call. A raised `SpendGoldenKey` is a write, and so is whatever a handler does
//! before it refuses: an error of either call is marked "stopped after partial writes" when a key was raised before it
//! or #518's counters moved since the native was entered (reviews of #774, rounds 1 and 2; `pickup_rb`'s
//! `WriteCounters`). The note's `GetCurrencyOnHand` read whose answer is dropped is not made (it writes
//! nothing).
//!
//! **`UpdateAmmoCounts(bSilent)`** (section 4): [`AMMO_REFRESH`] (`NotRead`), nothing written.
//!
//! A method the VM resolves from a class whose code is not loaded is a named `NotRead` (a class load is a write).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use crate::equip::{bad, object_of, read};

/// The record of the rules.
pub const RECORD: &str = "NATIVE_LINK67_KEY_AND_LOOK.md";
/// The native keys.
pub const CAN_AFFORD: &str = "WillowPlayerController.CanAffordToPickUpPickupable(interface)";
pub const PAY_FOR: &str = "WillowPlayerController.PayForPickupable(interface)";
pub const UPDATE_AMMO_COUNTS: &str = "WillowPlayerController.UpdateAmmoCounts(bool)";
/// The refusals.
/// The two natives' names, which start their shared refusals' texts (`"<name>: <text>"`).
pub const CAN_AFFORD_NAME: &str = "CanAffordToPickUpPickupable";
pub const PAY_FOR_NAME: &str = "PayForPickupable";
pub const PICKUPABLE_NOT_LOADED: &str = "a pickupable the store has not loaded (a load is a write)";
/// `"<name>: the cost query of <class>"`.
pub const COST_QUERY: &str = "the cost query of";
pub const GOLDEN_KEY_COUNT: &str = "CanAffordToPickUpPickupable: the controller's golden-key count";
pub const INFO_NOT_LOADED: &str = "a replication info the store has not loaded (a load is a write)";
pub const AMMO_REFRESH: &str = "UpdateAmmoCounts: the controller's ammo refresh";
/// The currency type of golden keys.
pub const GOLDEN_KEYS: u8 = 3;
/// The replication info class the cast asks for.
pub const REPLICATION_INFO: &str = "WillowGame.WillowPlayerReplicationInfo";

/// The pickupable's cost, as its fields hold it (by fit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cost {
    currency: u8,
    amount: i32,
}

fn unloaded(vm: &Vm, o: ObjRef) -> bool {
    ow_world::touch_trace::is_unloaded(vm, o) || vm.defs.is_failed(&vm.space, o)
}

/// Steps 1 to 4 for the native `name`: `Ok(None)` for no pickupable or no cost.
fn cost_of(vm: &Vm, pickupable: Option<ObjRef>, name: &str) -> Result<Option<Cost>, VmError> {
    let Some(p) = pickupable else {
        return Ok(None);
    };
    if unloaded(vm, p) {
        return Err(NotYet::not_read(format!("{name}: {PICKUPABLE_NOT_LOADED}"), RECORD).into());
    }
    let class = vm.space.heap.get(p)?.class;
    let declared = |name: &str, ok: fn(&PropKind) -> bool| {
        vm.space.prop(class, name).and_then(|id| vm.space.classes.decl(id)).is_some_and(|d| ok(&d.kind))
    };
    let shaped = declared("bCostsToPickUp", |k| matches!(k, PropKind::Bool))
        && declared("CostsToPickUpType", |k| matches!(k, PropKind::Byte { .. }))
        && declared("CostsToPickUpAmount", |k| matches!(k, PropKind::Int));
    if !shaped {
        let path = ow_attr::eval::class_path(&vm.space, p);
        return Err(NotYet::not_read(format!("{name}: {COST_QUERY} {path}"), RECORD).into());
    }
    let what = format!("WillowPlayerController.{name}");
    let what = what.as_str();
    let costs = match read(vm, p, "bCostsToPickUp")? {
        Some(Value::Bool(b)) => b,
        other => return Err(bad(what, format!("bCostsToPickUp holds {other:?}"))),
    };
    if !costs {
        return Ok(None);
    }
    let currency = match read(vm, p, "CostsToPickUpType")? {
        Some(Value::Byte(b)) => b,
        other => return Err(bad(what, format!("CostsToPickUpType holds {other:?}"))),
    };
    let amount = match read(vm, p, "CostsToPickUpAmount")? {
        Some(Value::Int(i)) => i,
        other => return Err(bad(what, format!("CostsToPickUpAmount holds {other:?}"))),
    };
    Ok(Some(Cost { currency, amount }))
}

/// Step 6's not-loaded check and cast: the controller's `PlayerReplicationInfo` when it is a
/// `WillowPlayerReplicationInfo` (by the class table, nothing loaded), else `None`.
fn replication_info(vm: &Vm, controller: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let Some(info) = read(vm, controller, "PlayerReplicationInfo")?.map(|v| object_of(&v, "PlayerReplicationInfo")).transpose()?.flatten()
    else {
        return Ok(None);
    };
    if unloaded(vm, info) {
        return Err(NotYet::not_read(format!("{name}: {INFO_NOT_LOADED}"), RECORD).into());
    }
    let class = vm.space.heap.get(info)?.class;
    let cast = vm.space.classes.by_path(REPLICATION_INFO).is_some_and(|w| vm.space.classes.is_child_of(class, w));
    Ok(cast.then_some(info))
}

/// `o`'s method `name`, resolved without a load: a class of the chain whose code is not loaded is a named `NotRead`;
/// no such method, a data error.
fn method(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<ow_object::ids::FuncId, VmError> {
    let (class, state) = {
        let x = vm.space.heap.get(o)?;
        (x.class, x.state)
    };
    match vm.find_loaded_method(class, name, state) {
        None => Err(NotYet::not_read(
            format!("{what}: {name} on {}, whose class code is not loaded (a class load is a write)", ow_attr::eval::class_path(&vm.space, o)),
            RECORD,
        )
        .into()),
        Some(None) => Err(bad(what, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o)))),
        Some(Some(f)) => Ok(f),
    }
}

fn int_of(v: &Value, what: &str) -> Result<i64, VmError> {
    match v {
        Value::Int(i) => Ok(i64::from(*i)),
        Value::Byte(b) => Ok(i64::from(*b)),
        other => Err(bad(what, format!("GetCurrencyOnHand answered {}, not an int", other.kind_name()))),
    }
}

fn pickupable_arg(c: &NativeCall<'_>, what: &str) -> Result<Option<ObjRef>, VmError> {
    object_of(c.arg(0)?, what)
}

/// `CanAffordToPickUpPickupable(Pickupable)` (module documentation).
fn can_afford(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerController.CanAffordToPickUpPickupable";
    let controller = c.this.ok_or_else(|| bad(WHAT, "called without a controller"))?;
    let pickupable = pickupable_arg(c, WHAT)?;
    let Some(cost) = cost_of(c.vm, pickupable, CAN_AFFORD_NAME)? else {
        return Ok(Value::Bool(true));
    };
    if cost.currency == GOLDEN_KEYS {
        return Err(NotYet::not_read(GOLDEN_KEY_COUNT, RECORD).into());
    }
    let Some(info) = replication_info(c.vm, controller, CAN_AFFORD_NAME)? else {
        return Ok(Value::Bool(true));
    };
    let f = method(c.vm, info, "GetCurrencyOnHand", WHAT)?;
    let on_hand = c.vm.call(f, Some(info), vec![Value::Byte(cost.currency)])?.ret;
    Ok(Value::Bool(i64::from(cost.amount) <= int_of(&on_hand, WHAT)?))
}

/// `PayForPickupable(Pickupable)` (module documentation).
fn pay_for(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerController.PayForPickupable";
    let counters = crate::attach_mesh::pickup_rb::WriteCounters::of(c.vm);
    let controller = c.this.ok_or_else(|| bad(WHAT, "called without a controller"))?;
    let pickupable = pickupable_arg(c, WHAT)?;
    let Some(cost) = cost_of(c.vm, pickupable, PAY_FOR_NAME)? else {
        return Ok(Value::None);
    };
    if cost.currency == GOLDEN_KEYS {
        let units = cost.amount.max(0);
        if units > 0 {
            let f = method(c.vm, controller, "SpendGoldenKey", WHAT)?;
            for raised in 0..units {
                // A raised event is a write, and so is anything a handler did before refusing (review of #774).
                if let Err(e) = c.vm.call(f, Some(controller), vec![]) {
                    return Err(if raised > 0 { after_writes(e) } else { counters.mark(c.vm, e) });
                }
            }
        }
        return Ok(Value::None);
    }
    let Some(info) = replication_info(c.vm, controller, PAY_FOR_NAME)? else {
        return Ok(Value::None);
    };
    let f = method(c.vm, info, "AddCurrencyOnHand", WHAT)?;
    let delta = cost.amount.checked_neg().ok_or_else(|| bad(WHAT, "the cost amount cannot be negated"))?;
    if let Err(e) = c.vm.call(f, Some(info), vec![Value::Byte(cost.currency), Value::Int(delta)]) {
        return Err(counters.mark(c.vm, e));
    }
    Ok(Value::None)
}

/// `e` after a write: a `NotYet` marked once, its kind and record kept; any other error unchanged.
fn after_writes(e: VmError) -> VmError {
    ow_world::world::component_setters::after_writes(e)
}

/// Binds the three keys in Off (the caller checks the mode). No pack bound them before (checked by grep), so a
/// duplicate is a registration error.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    let natives: [(&str, NativeFn); 3] = [
        (CAN_AFFORD, Rc::new(can_afford)),
        (PAY_FOR, Rc::new(pay_for)),
        (UPDATE_AMMO_COUNTS, Rc::new(|_: &mut NativeCall<'_>| Err(NotYet::not_read(AMMO_REFRESH, RECORD).into()))),
    ];
    for (key, f) in natives {
        vm.register_native(key, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "pickup_cost_tests.rs"]
mod tests;
