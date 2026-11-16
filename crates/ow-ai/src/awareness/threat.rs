//! Owner: lane W7-AI (part 1). Threat levels: `SetThreatLevel`, `GetThreatLevel`, the fall after `ForgetTime`,
//! `IsAwareOfTarget` and the two `NotifyAttackedBy` natives (NATIVE_AI_AWARENESS.md, AI_AWARENESS_LIVE.md sections 2,
//! 3 and 10). AI-assisted (Claude Code).
//!
//! A lone component is its own root: the records it holds are the ones the rules stamp. (For a den member the den's
//! component holds them, L9; that case is `NotYet`, [`no_parent`].)

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_vm::error::VmError;

use super::record::{Component, Opinion, ThreatLevel};
use super::{RECORD_LIVE_AWARENESS, Senses};

/// `NotYet { NotPorted }` for a component with a parent: a den member reads and stamps the den's records and uses
/// the den's origin and radii (L9 section 0); population dens are not in the Rust world yet.
pub fn no_parent(c: &Component) -> Result<(), VmError> {
    if c.settings.has_parent {
        return Err(NotYet::not_ported(
            "an AI component with a parent (a population den member): the den's records, origin and radii",
            RECORD_LIVE_AWARENESS,
        )
        .into());
    }
    Ok(())
}

/// `GetThreatLevel(Target)`: `ETL_SeenInThreatRadius` for a target the AI's pawn (with an allegiance) regards as
/// Friendly; otherwise the record's level, or `ETL_NoThreat` when there is no record (G26; the "no record" answer and
/// the chain to the root are live, L9; the friendly case is not).
pub fn get_threat(c: &Component, senses: &mut dyn Senses, target: ObjRef) -> Result<ThreatLevel, VmError> {
    no_parent(c)?;
    if senses.facts(c.pawn)?.has_allegiance && senses.opinion(c.pawn, target)? == Opinion::Friendly {
        return Ok(ThreatLevel::SeenInThreatRadius);
    }
    Ok(c.record(target).map_or(ThreatLevel::NoThreat, |r| r.threat))
}

/// `SetThreatLevel(Target, NewThreat)` (G26 steps 2 and 4 to 6; live on a den's record, L9 sections 2, 3 and 10):
/// nothing without a record; the level only rises; `AggroTime` is stamped with `now` once, by the first level of 2 or
/// more; a level of 2 sets `ForgetTime` to at least `now + seen_time`, a level of 3 to at least `now +
/// provoked_time`. The den alert of step 3 needs a den owner, which a lone component does not have.
///
/// UNVERIFIED: the note reads the stamp condition as "`AggroTime` is still (almost) zero"; the tolerance is not read,
/// so this tests for exactly zero (a world time is zero only at the very start).
pub fn set_threat(c: &mut Component, now: f32, target: ObjRef, level: ThreatLevel) -> Result<(), VmError> {
    no_parent(c)?;
    let (seen_time, provoked_time) = (c.settings.seen_time, c.settings.provoked_time);
    let Some(r) = c.record_mut(target) else {
        return Ok(());
    };
    r.threat = r.threat.max(level);
    if level.seen() && r.aggro_time == 0.0 {
        r.aggro_time = now;
    }
    match level {
        ThreatLevel::SeenInThreatRadius => r.forget_time = r.forget_time.max(now + seen_time),
        ThreatLevel::Provoked => r.forget_time = r.forget_time.max(now + provoked_time),
        _ => {}
    }
    Ok(())
}

/// The fall after `ForgetTime` (observed, L9 section 2; the code that does it was not located): at the root's target
/// update after `now` passes `ForgetTime`, a record at level 2 or 3 falls to `ETL_InThreatRadius` in place,
/// `AggroTime` back to 0, `ForgetTime` kept. The game also showed the record removed instead, with the player beyond
/// the den's list radius; a lone component's list never drops a record by distance ([`super::targets`]), so that
/// branch does not arise here. UNVERIFIED for a lone component (row 17 of the architecture's open questions).
pub fn forget(c: &mut Component, now: f32) {
    for r in &mut c.records {
        if r.threat.seen() && now > r.forget_time {
            r.threat = ThreatLevel::InThreatRadius;
            r.aggro_time = 0.0;
        }
    }
}

/// `WillowMind.IsAwareOfTarget(PotentialTarget)`: false without a record; else
/// `MinAggroTimeToConsiderAwareOfTarget + AggroTime < now` (G26; live, L9 section 2: a fresh record, `AggroTime` 0,
/// answers true; after a stamp it is false for 0.1 s).
pub fn is_aware(c: &Component, now: f32, actor: ObjRef) -> Result<bool, VmError> {
    no_parent(c)?;
    Ok(c.record(actor)
        .is_some_and(|r| c.settings.min_aggro_time + r.aggro_time < now))
}

/// `WillowAIComponent.NotifyAttackedBy(Attacker)`: the attacker becomes a target (`AddTarget`, which applies
/// `ValidTarget`), then `SetThreatLevel(Attacker, ETL_Provoked)` (G26; live, L9 section 3: level 3, `AggroTime` the
/// call's time, `ForgetTime` + the provoked time).
pub fn component_attacked_by(
    c: &mut Component,
    senses: &mut dyn Senses,
    now: f32,
    attacker: Option<ObjRef>,
) -> Result<(), VmError> {
    let Some(attacker) = attacker else {
        return Ok(());
    };
    no_parent(c)?;
    super::targets::add_target(c, senses, now, attacker)?;
    set_threat(c, now, attacker, ThreatLevel::Provoked)
}

/// `WillowMind.NotifyAttackedBy(Attacker, bDamaged)`: the component's rule, then, unless the pawn regards the attacker
/// as Friendly, `LastAttackedTime = now`, `LastAttacker = Attacker` and (when `bDamaged`) `LastHurtTime = now` (G26;
/// the call is live, L9 section 3; the three writes and the friendly gate were not read live).
pub fn mind_attacked_by(
    c: &mut Component,
    senses: &mut dyn Senses,
    now: f32,
    attacker: Option<ObjRef>,
    damaged: bool,
) -> Result<(), VmError> {
    component_attacked_by(c, senses, now, attacker)?;
    let Some(attacker) = attacker else {
        return Ok(());
    };
    if senses.opinion(c.pawn, attacker)? != Opinion::Friendly {
        c.last_attacked_time = now;
        c.last_attacker = Some(attacker);
        if damaged {
            c.last_hurt_time = now;
        }
    }
    Ok(())
}
