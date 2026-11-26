//! Owner: lane IMPL-LINKS-3-4-TAIL (decision 2026-11-26-NNN). `AdjustDamage`'s shield stage (10) and protection timer
//! (15), in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINKS_3_4_TAIL.md section 9 (#812 as landed), with the
//! stage rules of NATIVE_DAMAGE_DEATH.md section 1 (stages 10 and 15) and NATIVE_DOWN_TIMERS.md (the protection timer's
//! fields), alone; this lane did not read the executable. **UNVERIFIED in the running game.**
//!
//! **Stage 10** ([`plan_shield`], reads only): a victim whose `ShieldArmor` (a pool reference, declared) resolves to a
//! resident pool with `CurrentValue` above 0, and a damage that is not healing. The effective shield damage is the damage
//! times the damage type's `ShieldDamageModifier` (an initialization data value, evaluated with the instigator) times the
//! pawn damage-type modifier (`damage::resistance`, with its own Off refusal); the shield absorbs
//! `min(effective, shield)`; `DamageDealtToShields` is that; the damage that carries on is `(effective - absorbed) /
//! factor` (0 for a factor not above 0); `ShieldReduction` is the difference. Decided in the plan, before anything of
//! either stage is written:
//! - a player instigator (its controller's damage-number notification and the shield-penetration roll): [`NOTIFY`];
//! - a shield that would reach 0: [`DEPLETED`] (`OnShieldDepleted`);
//! - any damage absorbed: [`TRACKER`], because the victim's recent-damage record of the shield side names a tracker
//!   field the note does not name.
//!
//! The pool's new value is written by [`commit_shield`] at the end of the pass, after stage 15's decision.
//!
//! **Stage 15** ([`protection`], reads only): a `WillowPlayerPawn` victim hit by an AI instigator. The globals
//! definition resident (else `NotRead`), its three protection-timer fields declared as floats. With the timer not active
//! (`bIsProtectionTimerActive`), a hit that would take health below `HealthMax` × `ProtectionTimerThreshold` is
//! [`TIMER_START`]; with it active, the damage cannot take health below `MinimumHealthMaintainedByProtectionTimer`, and
//! the summary's `ProtectionTimerReduction` is what that took.

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::combat_music::{bad, is_a, object, read};
use crate::links34_tail::{boolean, float, resident};

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_LINKS_3_4_TAIL.md (section 9); NATIVE_DAMAGE_DEATH.md stages 10 and 15";
pub const NOTIFY: &str = "AdjustDamage stage 10: a player instigator's damage-number notification (and the shield-penetration roll)";
pub const DEPLETED: &str = "AdjustDamage stage 10: the shield-depleted handler (OnShieldDepleted)";
pub const TRACKER: &str =
    "AdjustDamage stage 10: the victim's recent-damage record of the shield side (the tracker field is not named)";
pub const TIMER_START: &str = "AdjustDamage stage 15: the protection timer's start (EnableProtectionTimer)";
const WHAT: &str = "WillowDamagePipeline.AdjustDamage";

/// Stage 10's numbers: the pool, its new value, and the damage that carries on.
pub struct ShieldPlan {
    pub pool: ObjRef,
    pub new_value: f32,
    pub absorbed: f32,
    pub carry: f32,
}

/// Stage 10's plan (module documentation); `Ok(None)` when the stage does not apply. Reads only.
pub fn plan_shield(
    vm: &mut Vm,
    victim: ObjRef,
    instigator: Option<ObjRef>,
    player_instigator: bool,
    type_definition: Option<ObjRef>,
    damage: f32,
) -> Result<Option<ShieldPlan>, VmError> {
    resident(vm, victim)?;
    let (reference, kind) = read(vm, victim, "ShieldArmor", WHAT)?;
    if !matches!(kind, PropKind::Struct(_)) {
        return Err(bad(WHAT, "ShieldArmor is not declared as a struct"));
    }
    let r = ow_world::pools::read_ref(&vm.space, &reference)?;
    if let Some(manager) = r.manager {
        resident(vm, manager)?;
    }
    let (_, Some(pool)) = ow_world::pools::resolve(&vm.space, r)? else { return Ok(None) };
    resident(vm, pool)?;
    let shield = float(vm, pool, "CurrentValue", WHAT)?;
    if shield <= 0.0 {
        return Ok(None);
    }
    if let Some(t) = type_definition {
        resident(vm, t)?;
        if crate::link34::is_healing(vm, t)? {
            return Ok(None);
        }
    }
    if player_instigator {
        return Err(NotYet::not_read(NOTIFY, RECORD).into());
    }
    let factor = match type_definition {
        Some(t) => {
            let (_, kind) = read(vm, t, "ShieldDamageModifier", WHAT)?;
            if !matches!(kind, PropKind::Struct(_)) {
                return Err(bad(WHAT, "ShieldDamageModifier is not declared as a struct"));
            }
            crate::damage::surface_modifier(vm, t, 3, instigator)?
        }
        None => 1.0,
    };
    let pawn_modifier = match type_definition {
        Some(t) => crate::damage::resistance(vm, victim, t)?,
        None => 1.0,
    };
    let effective = damage * factor * pawn_modifier;
    let absorbed = effective.min(shield);
    let new_value = shield - absorbed;
    let carry = if factor > 0.0 { (effective - absorbed) / factor } else { 0.0 };
    if new_value <= 0.0 {
        return Err(NotYet::not_read(DEPLETED, RECORD).into());
    }
    if absorbed > 0.0 {
        return Err(NotYet::not_read(TRACKER, RECORD).into());
    }
    Ok(Some(ShieldPlan { pool, new_value, absorbed, carry }))
}

/// Stage 10's write: the pool's new value (the pools rule's setter).
pub fn commit_shield(vm: &mut Vm, plan: &ShieldPlan) -> Result<(), VmError> {
    ow_world::pools::set_current(vm, plan.pool, plan.new_value)
}

/// The world's globals definition, resident (else `NotRead`). Reads only.
fn globals_definition(vm: &Vm) -> Result<ObjRef, VmError> {
    let globals = match vm.service::<ow_world::query::WorldQuery>() {
        Some(q) => q.try_borrow().map_err(|_| VmError::ServiceBorrowed("WorldQuery"))?.globals(),
        None => None,
    };
    let Some(globals) = globals else {
        return Err(NotYet::not_read("AdjustDamage stage 15: the world has no globals object", RECORD).into());
    };
    resident(vm, globals)?;
    match object(vm, globals, "TheGlobalsDefinition", WHAT)? {
        Some(d) => {
            resident(vm, d)?;
            Ok(d)
        }
        None => Err(NotYet::not_read(crate::links34_tail::DEFINITION_COLD, RECORD).into()),
    }
}

/// Stage 15 (module documentation): the damage after the stage. Reads only; the caller writes the reduction.
pub fn protection(vm: &Vm, victim: ObjRef, damage: f32, health: f32, max_health: f32) -> Result<f32, VmError> {
    if !is_a(vm, victim, "WillowGame.WillowPlayerPawn")? {
        return Ok(damage);
    }
    let definition = globals_definition(vm)?;
    let threshold = float(vm, definition, "ProtectionTimerThreshold", WHAT)?;
    float(vm, definition, "ProtectionTimerMaintainedMaxHealthPct", WHAT)?;
    float(vm, definition, "ProtectionTimerDurationInSeconds", WHAT)?;
    if !boolean(vm, victim, "bIsProtectionTimerActive", WHAT)? {
        if health - damage < max_health * threshold {
            return Err(NotYet::not_read(TIMER_START, RECORD).into());
        }
        return Ok(damage);
    }
    let floor = float(vm, victim, "MinimumHealthMaintainedByProtectionTimer", WHAT)?;
    if health - damage < floor {
        return Ok((health - floor).max(0.0));
    }
    Ok(damage)
}

#[cfg(test)]
#[path = "shield_stage_tests.rs"]
mod tests;
