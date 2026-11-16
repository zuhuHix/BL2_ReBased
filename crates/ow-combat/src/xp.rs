//! Owner: lane W6-COMBAT. The kill-experience glue (plan step S-d): the two `WillowExperiencePipeline` natives bound to
//! the rules of `ow-balance` (decision 2026-11-15-18: `kill_xp_award`, `kill_xp`, `KillEvent`), and the pawn's
//! experience multiplier. Port of `src/combat_xp.cpp`. AI-assisted (Claude Code).
//!
//! Records: NATIVE_COMBAT_GROUNDWORK.md sections 5.1 and 5.2, COMBAT_KILL_XP.md. **The rules are `ow-balance`'s**
//! (architecture R2: one implementation); this module finds the world's inputs (the globals definition, the party
//! from the game's replication info, the game's `EffectiveNumPlayers`, the levels, the playthrough of the world's
//! balance state), calls them, keeps the record of each kill for the reports and calls each player's `ExpEarn`.
//!
//! **Where it differs from the C++** (each a refusal where the C++ logs and goes on): a vehicle victim, a killer in a
//! vehicle, a critical kill by a killer whose class has no `ExpScaleForCriticalHitKills`, and a non-empty
//! `DamageTypeExpModifiers` list are `NotYet` from `ow-balance` in both modes (the C++ logs `UNIMPLEMENTED ...` and
//! continues with the factor left out). None is reached by the slice's kills.
//!
//! **The player count.** `ow_balance::types::PlayerCount` holds 1 to 4; the game's `EffectiveNumPlayers` can be 0 (the
//! `WillowGameInfo` default), and a count outside the weight table gives bonus 0 and weight 1 (the record). This
//! module passes a count the table does not reach for that case, which `kill_xp` reads as outside the table; a
//! table of four rows or more leaves no such count, and the credit is then `NotYet` (a contract gap of
//! `KillCredit::players`, reported in the lane's handoff).

use ow_attr::api::EvalCx;
use ow_balance::types::{BalanceError, ExpLevel, PlayerCount, Playthrough};
use ow_balance::xp::{KillCredit, KillEvent, kill_xp, kill_xp_award};
use ow_core::NotYet;
use ow_core::fmt::fmt_f;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::hit::access::{array, error, field, get, integer, object, object_of, set};
use crate::hit::{
    arg_number, arg_object, arg_truth, bind_fn, call_method, describe, globals,
    is_player_controller, level_of, not_implemented, note, with_state,
};

/// The records of the kill experience.
pub const RECORD: &str = "NATIVE_COMBAT_GROUNDWORK.md sections 5.1 and 5.2; COMBAT_KILL_XP.md";

/// One player's credit of a kill: the controller, its level at that moment, and the integer its `ExpEarn` got.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Credit {
    pub controller: ObjRef,
    pub player_level: i32,
    pub amount: i32,
}

/// What the two natives did for one kill: the award for the party (before the split), the victim's level, and the
/// credits.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Kill {
    pub killed: Option<ObjRef>,
    pub killed_level: i32,
    pub award: f32,
    pub credits: Vec<Credit>,
}

/// A balance-rule error as a VM error (its `NotYet` stays one).
pub fn balance_error(e: BalanceError) -> VmError {
    match e {
        BalanceError::NotYet(n) => VmError::NotYet(n),
        BalanceError::Attr(a) => VmError::Attr(a),
        other => error(format!("combat xp: {other}")),
    }
}

/// The pawn's `ExperiencePointMultiplier`, the attribute the base kill experience scales (NATIVE_COMBAT_GROUNDWORK.md
/// section 5.1; C++ `applyExperienceMultiplier`): the stock spawn writes the AI class's starting values into the
/// pawn's attributes (`PopulationFactoryBalancedAIPawn.ApplyAttributeStartingValues`, unread), and the class lists
/// `Attribute_ExperienceMultiplier` with an `XPMultiplier_*` definition. That entry is evaluated with the pawn as the
/// context source and written to the pawn's property and its base value. UNVERIFIED reading of the native; the live
/// pawns read 0.5, 0.75, 1.25, ... (COMBAT_GOLDEN_TRACE.md, "Open-rule checks"). A pawn or class without these fields
/// is left as it is.
pub fn apply_experience_multiplier(vm: &mut Vm, pawn: ObjRef, mind: ObjRef) -> Result<(), VmError> {
    let _ = mind; // the C++ context names the mind too; the resolvers reach it from the pawn
    if get(vm, pawn, "ExperiencePointMultiplier")?.is_none() {
        return Ok(());
    }
    let Some(class) = object(vm, pawn, "AIClass")? else {
        return Ok(());
    };
    let Some(starting) = get(vm, class, "AttributeStartingValues")? else {
        return Ok(());
    };
    for entry in starting.elements().unwrap_or(&[]) {
        let attribute = field(vm, entry, "Attribute").as_ref().and_then(object_of);
        if !crate::hit::access::path_of(vm, attribute).ends_with("Attribute_ExperienceMultiplier") {
            continue;
        }
        let Some(base) = field(vm, entry, "BaseValue") else {
            return Err(error(
                "combat xp: an AttributeStartingValues entry has no BaseValue",
            ));
        };
        let value = crate::hit::evaluate_for(vm, &base, Some(pawn))?;
        set(vm, pawn, "ExperiencePointMultiplier", Value::Float(value))?;
        set(
            vm,
            pawn,
            "ExperiencePointMultiplierBaseValue",
            Value::Float(value),
        )?;
        return Ok(());
    }
    Ok(())
}

/// The world's playthrough (its balance state; the first without one).
fn playthrough(vm: &Vm) -> Result<Playthrough, VmError> {
    match vm.service::<ow_world::world::BalanceState>() {
        Some(b) => Ok(b
            .try_borrow()
            .map_err(|_| VmError::ServiceBorrowed("BalanceState"))?
            .snapshot
            .playthrough),
        None => Ok(Playthrough::NORMAL),
    }
}

/// The `PlayerCount` that makes `kill_xp` read the weight row the game's count `n` selects (module documentation).
fn player_count(vm: &mut Vm, g: ObjRef, n: i64) -> Result<PlayerCount, VmError> {
    let rows = array(vm, g, "ExpAwardWeights")?.len();
    let inside = n >= 1 && usize::try_from(n).is_ok_and(|n| n <= rows);
    if inside && let Ok(c) = PlayerCount::new(i32::try_from(n).unwrap_or(0)) {
        return Ok(c);
    }
    // Outside the table: a count past the table's rows reads the same (bonus 0, weight 1).
    let past = usize::from(PlayerCount::MAX);
    if rows < past {
        return PlayerCount::new(i32::from(PlayerCount::MAX)).map_err(balance_error);
    }
    Err(NotYet::not_ported(
        format!("the kill credit for EffectiveNumPlayers {n} with a weight table of {rows} rows (PlayerCount holds 1 to 4)"),
        RECORD,
    )
    .into())
}

pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    // WillowExperiencePipeline.CalculateExperiencePointsForKill(Killer, KilledPawn, bKilledByCritical,
    // DamageSourceClass, DamageTypeDef) -> float, static: `ow_balance::xp::kill_xp_award`.
    bind_fn(
        vm,
        "WillowGame.WillowExperiencePipeline.CalculateExperiencePointsForKill",
        |c| {
            let killer = arg_object(c, 0)?;
            let killed = arg_object(c, 1)?;
            let critical = arg_truth(c, 2)?;
            let source = match c.arg(3)? {
                Value::Class(k) => *k,
                _ => None,
            };
            let damage_type = arg_object(c, 4)?;
            let vm = &mut *c.vm;
            let mut record = Kill {
                killed,
                killed_level: 0,
                award: 0.0,
                credits: Vec::new(),
            };
            if let Some(killed) = killed {
                if describe(vm, Some(killed))?.is_none() {
                    not_implemented(
                        vm,
                        "CalculateExperiencePointsForKill: a killed actor that is not an AI pawn",
                        RECORD,
                    )?;
                }
                let g = globals(vm)?;
                let attrs = vm.attrs.clone();
                let kill =
                    KillEvent::new(killed, killer, critical).with_damage(source, damage_type);
                record.award =
                    kill_xp_award(&attrs, &mut EvalCx::new(vm), g, &kill).map_err(balance_error)?;
                record.killed_level = level_of(vm, Some(killed))?;
            }
            let award = record.award;
            with_state(vm, |s| s.kills.push(record))?;
            note(
                vm,
                format!(
                    "CalculateExperiencePointsForKill award={}",
                    fmt_f(f64::from(award), 6)
                ),
            )?;
            Ok(Value::Float(award))
        },
    )?;

    // WillowExperiencePipeline.AwardCombatExperienceToParty(ExperiencePoints, Killer, KilledActor), static: for each
    // player replication info of the game's replication info whose owner is a player controller, in list order, the
    // credit of `ow_balance::xp::kill_xp`, given to the controller's ExpEarn(amount, 1, 1).
    bind_fn(
        vm,
        "WillowGame.WillowExperiencePipeline.AwardCombatExperienceToParty",
        |c| {
            // N5: the experience is a float argument; the rule works in single precision.
            let points = crate::hit::access::to_f32(arg_number(c, 0)?);
            let killer = arg_object(c, 1)?;
            let killed = arg_object(c, 2)?;
            let vm = &mut *c.vm;
            let world = with_state(vm, |s| s.world)?;
            let players = match world {
                Some(w) => match object(vm, w, "GRI")? {
                    Some(gri) => get(vm, gri, "PRIArray")?,
                    None => None,
                },
                None => None,
            };
            let Some(players) = players else {
                note(
                    vm,
                    "AwardCombatExperienceToParty: the world has no replication info (nothing awarded)",
                )?;
                return Ok(Value::None);
            };
            let mut count = 1;
            if let Some(w) = world
                && let Some(game) = object(vm, w, "Game")?
                && let Some(effective) = get(vm, game, "EffectiveNumPlayers")?
            {
                count = integer(&effective);
            }
            let g = globals(vm)?;
            let victim_level = match killed {
                Some(k) => Some(level_of(vm, Some(k))?),
                None => None,
            };
            let through = playthrough(vm)?;
            for info in players.elements().unwrap_or(&[]).to_vec() {
                let Some(info) = object_of(&info) else {
                    continue;
                };
                let owner = object(vm, info, "Owner")?;
                if !is_player_controller(vm, owner)? {
                    continue;
                }
                let Some(controller) = owner else { continue };
                let player_level = level_of(vm, Some(controller))?;
                let credit = KillCredit::new(
                    points,
                    player_count(vm, g, count)?,
                    Some(controller) == killer,
                    Some(ExpLevel::new(player_level)),
                    victim_level.map(ExpLevel::new),
                    through,
                );
                let attrs = vm.attrs.clone();
                let amount =
                    kill_xp(&attrs, &mut EvalCx::new(vm), g, &credit).map_err(balance_error)?;
                with_state(vm, |s| {
                    if let Some(last) = s.kills.last_mut()
                        && last.killed == killed
                    {
                        last.credits.push(Credit {
                            controller,
                            player_level,
                            amount,
                        });
                    }
                })?;
                note(
                    vm,
                    format!(
                        "AwardCombatExperienceToParty player_level={player_level} victim_level={} ExpEarn({amount}, 1, 1)",
                        victim_level.unwrap_or(0)
                    ),
                )?;
                let args = vec![Value::Int(amount), Value::Byte(1), Value::Byte(1)];
                if call_method(vm, controller, "ExpEarn", args)?.is_none() {
                    return Err(error("combat xp: the player controller has no ExpEarn"));
                }
            }
            Ok(Value::None)
        },
    )
}

#[cfg(test)]
mod tests;
