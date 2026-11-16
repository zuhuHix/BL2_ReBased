//! Owner: lane W6-COMBAT. The player goes down (plan step S-f): the natives the stock `injured` state of a player pawn
//! reaches, the currency the respawn fee reads and charges, and the player pawn's preparation. Port of
//! `src/combat_down.cpp`. AI-assisted (Claude Code).
//!
//! Records: NATIVE_DAMAGE_DEATH.md section 10, COMBAT_DOWN_RESPAWN.md, NATIVE_DOWN_TIMERS.md (read later than the C++)
//! and NATIVE_CONTROLLER_HELPERS.md. Every body is **UNVERIFIED** in the running game. The injured state's timing is
//! script (`Tick`) and needs none of them.
//!
//! **Where the record and the C++ differ** (architecture R3: the C++ reading under `ParityMode::Reference`, the
//! record's under `ParityMode::Off`):
//! - `SetSecondWindReason`: the C++ stores the byte unconditionally; the record (NATIVE_DOWN_TIMERS.md) stores it only
//!   when the pawn answers `IsInjured` (its virtual) and the stored byte is still 0, so the first reason of an injury
//!   wins and a reason given to a healthy pawn is lost.
//! - `IsInjured`: the C++ answers `bIsInjured`; the record's `WillowAIPawn` override also answers the AI pawn's own
//!   `bInjured` byte (when the class declares it; an invented class that does not has no such byte to add).
//!
//! **The mission bridge.** `maya`, `bleed`, `cash` and the injured definition need W6-MISSION's player
//! (`run.rs` refuses those steps until it lands); the natives and [`prepare_player_pawn`] are here for it.

use ow_core::ParityMode;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_vm::interp::instantiate_from_defaults;

use crate::hit::access::{
    error, field, get, integer, is_a, number, object, object_of, set, set_field, truth,
};
use crate::hit::{arg_number, arg_object, arg_truth, bind_fn, call_method, note, this, with_state};

/// The records of the down state.
pub const RECORD: &str =
    "NATIVE_DAMAGE_DEATH.md section 10; COMBAT_DOWN_RESPAWN.md; NATIVE_DOWN_TIMERS.md";
/// The record of the currency natives.
pub const RECORD_CURRENCY: &str =
    "NATIVE_CONTROLLER_HELPERS.md (GetCurrencyOnHand, AddCurrencyOnHand)";

/// The currency types of `WillowPlayerReplicationInfo.Currency` (a static array of 13 `CurrencyState`).
pub const CURRENCY_TYPES: i64 = 13;
/// The currency type the VM has no count of (golden keys: the game asks the owner controller).
pub const GOLDEN_KEYS: i64 = 3;
/// `AddCurrencyOnHand`'s caps: credits, Eridium, every other type.
pub const CREDITS_CAP: i64 = 99_999_999;
pub const ERIDIUM_CAP: i64 = 500;
pub const OTHER_CAP: i64 = 999;

/// The `CurrentAmount` of `Currency[ty]` on a replication info, with where to write it back; `None` outside the
/// table or for an info without the field.
fn currency_slot(vm: &mut Vm, info: ObjRef, ty: i64) -> Result<Option<(PropAddr, Value)>, VmError> {
    if !(0..CURRENCY_TYPES).contains(&ty) {
        return Ok(None);
    }
    let class = vm.space.heap.get(info)?.class;
    let Some(p) = vm.space.prop(class, "Currency") else {
        return Ok(None);
    };
    let dim = vm.space.classes.decl(p).map_or(1, |d| d.array_dim);
    // 0 <= ty < 13 checked above.
    let index = ty as u32;
    if index >= u32::try_from(dim).unwrap_or(1) {
        return Ok(None);
    }
    let at = PropAddr { prop: p, index };
    let entry = vm.space.read(info, at)?.clone();
    Ok(Some((at, entry)))
}

/// The amount of a currency on hand, as `GetCurrencyOnHand` answers it: 0 outside the table and for golden keys.
pub fn currency_on_hand(vm: &mut Vm, info: ObjRef, ty: i64) -> Result<i64, VmError> {
    if ty == GOLDEN_KEYS {
        return Ok(0);
    }
    Ok(match currency_slot(vm, info, ty)? {
        Some((_, entry)) => field(vm, &entry, "CurrentAmount").map_or(0, |v| integer(&v)),
        None => 0,
    })
}

/// Sets `Currency[ty].CurrentAmount` (a state input: the run's cash). An info without that slot is refused.
pub fn set_currency_on_hand(
    vm: &mut Vm,
    info: ObjRef,
    ty: i64,
    amount: i64,
) -> Result<(), VmError> {
    let Some((at, mut entry)) = currency_slot(vm, info, ty)? else {
        return Err(error(format!(
            "combat: the replication info has no Currency slot {ty}"
        )));
    };
    // The amount is an int field; the callers keep it inside the caps above.
    let amount = i32::try_from(amount)
        .map_err(|_| error(format!("combat: currency amount {amount} out of range")))?;
    if !set_field(vm, &mut entry, "CurrentAmount", Value::Int(amount)) {
        return Err(error(format!(
            "combat: the replication info has no Currency slot {ty}"
        )));
    }
    vm.space.write(info, at, entry)?;
    Ok(())
}

/// What the injured state's script reads of the pawn it runs on and a bare player pawn lacks (C++
/// `preparePlayerPawn`): the world and authority, a `PlayerStart` at the origin as the controller's `StartSpot` when
/// it has none (the respawn point: the game's choice is a travel station of the level, which the VM world does not
/// have; UNVERIFIED stand-in, plan step S-f), and the controller's replication info. Idempotent.
pub fn prepare_player_pawn(vm: &mut Vm, pawn: ObjRef) -> Result<(), VmError> {
    let (world, authority) = with_state(vm, |s| (s.world, s.authority))?;
    set(vm, pawn, "WorldInfo", Value::Object(world))?;
    set(vm, pawn, "Role", Value::Byte(authority))?;
    if let Some(controller) = object(vm, pawn, "Controller")? {
        if let Some(spot) = get(vm, controller, "StartSpot")?
            && object_of(&spot).is_none()
            && let Ok(start) = vm.find_class("Engine.PlayerStart")
        {
            let made = instantiate_from_defaults(vm, start)?;
            set(vm, controller, "StartSpot", Value::Object(Some(made)))?;
        }
        if let Some(info) = object(vm, controller, "PlayerReplicationInfo")? {
            set(vm, pawn, "PlayerReplicationInfo", Value::Object(Some(info)))?;
        }
    }
    Ok(())
}

/// `Pawn.IsInjured()` (module documentation).
pub fn is_injured(vm: &mut Vm, pawn: ObjRef) -> Result<bool, VmError> {
    let flag = get(vm, pawn, "bIsInjured")?.is_some_and(|v| truth(vm, &v));
    if flag || vm.parity == ParityMode::Reference {
        return Ok(flag);
    }
    if is_a(vm, pawn, "WillowGame.WillowAIPawn")?
        && let Some(own) = get(vm, pawn, "bInjured")?
    {
        return Ok(integer(&own) != 0 || truth(vm, &own));
    }
    Ok(false)
}

pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    // WillowPlayerController.GetInjuredDefinition: the injured definition of the player's class, which the owner (the
    // mission bridge) supplies; None without one.
    bind_fn(
        vm,
        "WillowGame.WillowPlayerController.GetInjuredDefinition",
        |c| {
            let hook = with_state(c.vm, |s| s.injured_definition.clone())?;
            Ok(Value::Object(match hook {
                Some(f) => f(c.vm)?,
                None => None,
            }))
        },
    )?;

    // WillowPawn.SetSecondWindReason(Reason) / ClearSecondWindReason (module documentation).
    bind_fn(vm, "WillowGame.WillowPawn.SetSecondWindReason", |c| {
        let me = this(c, "SetSecondWindReason")?;
        // An ESecondWindReason byte; the C++ makeByte keeps the low 8 bits.
        let reason = crate::hit::access::low_u8(integer(c.arg(0)?));
        let stores = match c.vm.parity {
            ParityMode::Reference => true,
            ParityMode::Off => {
                // The pawn's native virtual (no script class overrides it), not a script call.
                let injured = is_injured(c.vm, me)?;
                let stored = get(c.vm, me, "SecondWindReason")?.map_or(0, |v| integer(&v));
                injured && stored == 0
            }
        };
        if stores {
            set(c.vm, me, "SecondWindReason", Value::Byte(reason))?;
        }
        note(c.vm, format!("SetSecondWindReason {reason}"))?;
        Ok(Value::None)
    })?;
    bind_fn(vm, "WillowGame.WillowPawn.ClearSecondWindReason", |c| {
        let me = this(c, "ClearSecondWindReason")?;
        set(c.vm, me, "SecondWindReason", Value::Byte(0))?;
        Ok(Value::None)
    })?;

    // GetRevivePct / SetRevivePct: the percentage the HUD shows, stored on the player's replication info when the
    // pawn has one (dropped otherwise).
    for path in [
        "WillowGame.WillowPawn.SetRevivePct",
        "WillowGame.WillowPlayerPawn.SetRevivePct",
    ] {
        bind_fn(vm, path, |c| {
            let me = this(c, "SetRevivePct")?;
            let pct = arg_number(c, 0)?;
            if let Some(info) = object(c.vm, me, "PlayerReplicationInfo")? {
                set(
                    c.vm,
                    info,
                    "RevivePct",
                    Value::Float(crate::hit::access::to_f32(pct)),
                )?;
            }
            Ok(Value::None)
        })?;
    }
    for path in [
        "WillowGame.WillowPawn.GetRevivePct",
        "WillowGame.WillowPlayerPawn.GetRevivePct",
    ] {
        bind_fn(vm, path, |c| {
            let me = this(c, "GetRevivePct")?;
            let pct = match object(c.vm, me, "PlayerReplicationInfo")? {
                Some(info) => get(c.vm, info, "RevivePct")?.map_or(0.0, |v| number(&v)),
                None => 0.0,
            };
            Ok(Value::Float(crate::hit::access::to_f32(pct)))
        })?;
    }

    // Actor.SetCanTarget(bool) / CanTarget() / GetCanTarget(): the actor's bCanTarget flag (the cheat flag is not
    // read). Stand-in for the targeting layer, as the C++.
    bind_fn(vm, "Engine.Actor.SetCanTarget", |c| {
        let me = this(c, "SetCanTarget")?;
        let on = arg_truth(c, 0)?;
        set(c.vm, me, "bCanTarget", Value::Bool(on))?;
        note(c.vm, format!("SetCanTarget {on}"))?;
        Ok(Value::None)
    })?;
    for path in ["Engine.Actor.CanTarget", "Engine.Actor.GetCanTarget"] {
        bind_fn(vm, path, |c| {
            let me = this(c, "CanTarget")?;
            Ok(Value::Bool(
                get(c.vm, me, "bCanTarget")?.is_some_and(|v| truth(c.vm, &v)),
            ))
        })?;
    }

    // WillowPlayerPawn.HasActiveDuel(Other): the VM has no versus duel, so no pawn is in one.
    bind_fn(vm, "WillowGame.WillowPlayerPawn.HasActiveDuel", |_| {
        Ok(Value::Bool(false))
    })?;

    // WillowPlayerReplicationInfo.GetCurrencyOnHand / AddCurrencyOnHand (NATIVE_CONTROLLER_HELPERS.md, UNVERIFIED): Add
    // ignores a type outside the table and golden keys; a negative delta lowers the amount, never below 0; a positive
    // one raises it to at most the type's cap; then the script event NotifyCurrencyDelegates fires on the info (even
    // when nothing changed). The replication flag bit has no VM counterpart.
    bind_fn(
        vm,
        "WillowGame.WillowPlayerReplicationInfo.GetCurrencyOnHand",
        |c| {
            let me = this(c, "GetCurrencyOnHand")?;
            let ty = integer(c.arg(0)?);
            let amount = currency_on_hand(c.vm, me, ty)?;
            // An amount set through this module fits an int.
            Ok(Value::Int(crate::hit::access::low_i32(amount)))
        },
    )?;
    bind_fn(
        vm,
        "WillowGame.WillowPlayerReplicationInfo.AddCurrencyOnHand",
        |c| {
            let me = this(c, "AddCurrencyOnHand")?;
            let ty = integer(c.arg(0)?);
            let delta = integer(c.arg(1)?);
            if !(0..CURRENCY_TYPES).contains(&ty) || ty == GOLDEN_KEYS {
                return Ok(Value::None);
            }
            let cap = match ty {
                0 => CREDITS_CAP,
                1 => ERIDIUM_CAP,
                _ => OTHER_CAP,
            };
            let before = currency_on_hand(c.vm, me, ty)?;
            let after = if delta <= 0 {
                before.saturating_add(delta).max(0)
            } else {
                before.saturating_add(delta).min(cap)
            };
            set_currency_on_hand(c.vm, me, ty, after)?;
            note(
                c.vm,
                format!("AddCurrencyOnHand type {ty} delta {delta}: {before} -> {after}"),
            )?;
            call_method(c.vm, me, "NotifyCurrencyDelegates", Vec::new())?;
            Ok(Value::None)
        },
    )?;

    // WillowPlayerController.NoFailSetPawnLocation(Pawn, Location): puts the pawn at the location whatever is there.
    // Stand-in, as the C++: the Location is written and the call succeeds; no collision test, no anchor.
    bind_fn(
        vm,
        "WillowGame.WillowPlayerController.NoFailSetPawnLocation",
        |c| {
            if let Some(pawn) = arg_object(c, 0)? {
                let location = c.arg(1)?.clone();
                set(c.vm, pawn, "Location", location)?;
            }
            note(c.vm, "NoFailSetPawnLocation")?;
            Ok(Value::Bool(true))
        },
    )?;
    // Pawn.IsInjured() (module documentation).
    bind_fn(vm, "Engine.Pawn.IsInjured", |c| {
        let me = this(c, "IsInjured")?;
        Ok(Value::Bool(is_injured(c.vm, me)?))
    })
}

#[cfg(test)]
mod tests;
