//! Owner: lane W7-AI (part 1). The target list and the current target: `ValidTarget`, `FindTargets`, `AddTarget`,
//! the removal pass, `ValidCurrentTarget`, `ChooseBestTarget`, `SetTarget` and `TargetChanged` (NATIVE_AI_MELEE.md,
//! "Target list" and "ChooseBestTarget"; NATIVE_AI_AWARENESS.md, the Willow overrides). AI-assisted (Claude Code).
//!
//! What a rule changes outside the component (the script events, the pawn's head look, another pawn's counter) is
//! returned as an [`Effect`]; the VM adapter applies the ones it can and reports the rest.

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_vm::error::VmError;

use super::iterators::{calc_target_info, prioritize};
use super::record::{ActorKind, Component, Opinion, Record};
use super::threat::{get_threat, no_parent};
use super::{RECORD_AWARENESS, RECORD_MELEE, Senses, Vec3, length_2d, sub};

/// What a target change does outside the component's own fields.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// The AI event `Target_Lost`, to the running action and then every other action of the tree (L8).
    TargetLost(ObjRef),
    /// The AI event `Target_Acquired`, to the same actions (L8).
    TargetAcquired(ObjRef),
    /// From no target to one: every tree node's timed start is re-armed (G25; live, L9 section 7).
    TimedStartsReArmed,
    /// From no target to one: `bProvoked` set and `ProvokedLocation` the pawn's location (live, L9 section 5).
    Provoked { location: Vec3 },
    /// The pawn's script event `SetLookAtTarget(target, zero offset)` (live, L9 section 5); not sent while on hold.
    LookAt(Option<ObjRef>),
    /// A target's `NumAICurrentlyTargeting` goes up or down by one (live, L9 section 5).
    TargetingCount { actor: ObjRef, delta: i32 },
}

/// `ValidTarget(actor)`, base rule then the Willow override, in the order G26 reads (first failure is false). The
/// caller passes a pawn: a controller is mapped to its pawn by the VM adapter (step 2). The global "no target all
/// enemies" cheat (step 3) is off: nothing sets it here. Live, L9 section 4: the player true; None, the AI's own pawn
/// and a same-allegiance bullymong false; a cloaked player false; an AI with its allegiance cleared false.
pub fn valid_target(c: &Component, senses: &mut dyn Senses, actor: Option<ObjRef>) -> Result<bool, VmError> {
    let Some(actor) = actor else {
        return Ok(false);
    };
    let it = senses.facts(actor)?;
    let me = senses.facts(c.pawn)?;
    // 4. the actor can be targeted, and the AI has an allegiance (an AI without one has no valid target).
    if !it.can_be_targeted || !me.has_allegiance {
        return Ok(false);
    }
    // 5. not the AI's own pawn.
    if actor == c.pawn {
        return Ok(false);
    }
    // 6. a target the AI regards as Friendly (not "not an Enemy": a Neutral one passes) only with
    // bCanTargetFriendlies, and the CanTargetFriendliesIf evaluator when there is one.
    if senses.opinion(c.pawn, actor)? == Opinion::Friendly {
        if !c.settings.can_target_friendlies {
            return Ok(false);
        }
        if c.settings.can_target_friendlies_if {
            return Err(NotYet::not_ported(
                "ValidTarget with a CanTargetFriendliesIf evaluator",
                RECORD_AWARENESS,
            )
            .into());
        }
    }
    // 7. the AI's own pawn is a registered targetable with an allegiance; a pawn that drives a vehicle is not a direct
    // target (its vehicle is).
    if !me.can_be_targeted {
        return Ok(false);
    }
    // 8. the Base-chain exclusion accepts a normal pawn at once (G26, UNVERIFIED reading).
    // The Willow override.
    match it.kind {
        ActorKind::Pawn {
            controlled,
            driving,
            weapon_seat,
        } => {
            if driving || !controlled || weapon_seat {
                return Ok(false);
            }
        }
        ActorKind::Vehicle { occupied } => {
            if !occupied {
                return Ok(false);
            }
        }
        ActorKind::Other => {}
    }
    // The group leader rule needs a forced target (a target leader), which nothing sets for a lone component.
    Ok(!it.cloaked)
}

/// `AddTarget(actor)`: the existing record, or, when `ValidTarget` holds, a new one appended to `Targets` and filled by
/// `CalcTargetInfo` at once (G25). Returns the record's index and whether it is new; `None` when the actor is not a
/// valid target.
pub fn add_target(
    c: &mut Component,
    senses: &mut dyn Senses,
    now: f32,
    actor: ObjRef,
) -> Result<Option<(usize, bool)>, VmError> {
    no_parent(c)?;
    if let Some(i) = c.records.iter().position(|r| r.target == actor) {
        return Ok(Some((i, false)));
    }
    if !valid_target(c, senses, Some(actor))? {
        return Ok(None);
    }
    c.records.push(Record::new(actor));
    let index = c.records.len() - 1;
    calc_target_info(c, senses, now, index)?;
    Ok(Some((index, true)))
}

/// `FindTargets` for a lone component: every entry of the world's registry of targetables, in order, that can be
/// targeted, that the AI (when it has an allegiance) does not regard as Neutral, whose horizontal distance is within
/// the search radius, and that is a valid target, is added (G26).
///
/// UNVERIFIED for a lone component (open question 17): live, the list of a den member was not cut at the 4,096 radius
/// (records to about 7 km, L8 section 3.4), because a den member's list comes from the den; a lone pawn's comes from
/// this rule. Within means "not farther than" (the comparison's edge is not read).
pub fn find_targets(c: &mut Component, senses: &mut dyn Senses, now: f32) -> Result<Vec<usize>, VmError> {
    no_parent(c)?;
    let me = senses.facts(c.pawn)?;
    let mut added = Vec::new();
    for entry in senses.targetables()? {
        let it = senses.facts(entry)?;
        if !it.can_be_targeted {
            continue;
        }
        if me.has_allegiance && senses.opinion(c.pawn, entry)? == Opinion::Neutral {
            continue;
        }
        if length_2d(sub(me.location, it.location)) > c.settings.search_radius {
            continue;
        }
        if let Some((index, true)) = add_target(c, senses, now, entry)? {
            added.push(index);
        }
    }
    Ok(added)
}

/// The removal pass at the start of a target update: every record whose target is no longer a valid target is
/// removed (`ShouldRemoveTarget` without a parent), from the end of the list to the front (G25). `TargetRemoved` hands
/// the threat to a replacement actor only for vehicles and their drivers (G26); a pawn on foot has none. Returns the
/// removed targets.
pub fn remove_stale(c: &mut Component, senses: &mut dyn Senses) -> Result<Vec<ObjRef>, VmError> {
    no_parent(c)?;
    let mut stale = Vec::new();
    for r in &c.records {
        if !valid_target(c, senses, Some(r.target))? {
            stale.push(r.target);
        }
    }
    for target in stale.iter().rev() {
        let kind = senses.facts(*target)?.kind;
        let hands_over = match kind {
            ActorKind::Vehicle { .. } => true,
            ActorKind::Pawn {
                driving,
                weapon_seat,
                ..
            } => driving || weapon_seat,
            ActorKind::Other => false,
        };
        if hands_over {
            return Err(NotYet::not_ported(
                "TargetRemoved's hand-off between a vehicle and its occupant (vehicles are not in the Rust world)",
                RECORD_AWARENESS,
            )
            .into());
        }
        if let Some(i) = c.records.iter().rposition(|r| r.target == *target) {
            c.records.remove(i);
        }
    }
    Ok(stale)
}

/// `ValidCurrentTarget(record)`: the base rule (`StopTargetingIf` decides keeping the current target, `CanTargetIf`
/// acquiring another; a `ScriptedTarget` is the only valid one), then the Willow additions: the actor is not
/// `bHidden` and `GetThreatLevel` is at least `ETL_SeenInThreatRadius` (G26; the threat gate is live, L9 section 10:
/// no current target for minutes at level 1, set in the update that raised it to 2). The evaluators are `NotYet`.
pub fn valid_current_target(c: &Component, senses: &mut dyn Senses, index: usize) -> Result<bool, VmError> {
    let target = c.records[index].target;
    if c.current == Some(target) {
        if c.settings.stop_targeting_if {
            return Err(NotYet::not_ported("ValidCurrentTarget with a StopTargetingIf evaluator", RECORD_MELEE).into());
        }
    } else if c.settings.can_target_if {
        return Err(NotYet::not_ported("ValidCurrentTarget with a CanTargetIf evaluator", RECORD_MELEE).into());
    }
    if c.scripted_target.is_some_and(|s| s != target) {
        return Ok(false);
    }
    if senses.facts(target)?.hidden {
        return Ok(false);
    }
    Ok(get_threat(c, senses, target)?.seen())
}

/// `ChooseBestTarget`: every record is prioritised, the list is taken highest priority first, and the first record
/// that is a valid current target is the best; when it differs from `CurrentTarget`, `SetTarget` runs (G25). The
/// game's sort is read as not stable; ties keep the list order here (UNVERIFIED).
pub fn choose_best(c: &mut Component, senses: &mut dyn Senses, effects: &mut Vec<Effect>) -> Result<(), VmError> {
    for i in 0..c.records.len() {
        prioritize(c, senses, i)?;
    }
    let mut order: Vec<usize> = (0..c.records.len()).collect();
    order.sort_by(|&a, &b| c.records[b].priority.total_cmp(&c.records[a].priority));
    let mut best = None;
    for i in order {
        if valid_current_target(c, senses, i)? {
            best = Some(c.records[i].target);
            break;
        }
    }
    if best != c.current {
        set_target(c, senses, best, effects)?;
    }
    Ok(())
}

/// `SetTarget(actor)`: when it differs from `CurrentTarget`, `Target_Lost` for the old target, the new one stored,
/// `Target_Acquired` for it, then `TargetChanged(old, new)` (G25; the lost-before-acquired order on a switch is
/// UNVERIFIED, L8).
pub fn set_target(
    c: &mut Component,
    senses: &mut dyn Senses,
    new: Option<ObjRef>,
    effects: &mut Vec<Effect>,
) -> Result<(), VmError> {
    let old = c.current;
    if new == old {
        return Ok(());
    }
    if let Some(o) = old {
        effects.push(Effect::TargetLost(o));
    }
    c.current = new;
    if let Some(n) = new {
        effects.push(Effect::TargetAcquired(n));
    }
    target_changed(c, senses, old, new, effects)
}

/// `TargetChanged(old, new)`, base then Willow (G26; live, L9 section 5): from no target to one, the tree's timed
/// starts are re-armed, `bProvoked` is set and `ProvokedLocation` is the pawn's location; unless the component is on
/// hold the pawn looks at the new target; the old target's counter goes down and the new one's up. The three pawn
/// notifications, the started/stopped-targeting calls and the presentation cues are not read.
pub fn target_changed(
    c: &mut Component,
    senses: &mut dyn Senses,
    old: Option<ObjRef>,
    new: Option<ObjRef>,
    effects: &mut Vec<Effect>,
) -> Result<(), VmError> {
    if old.is_none() && new.is_some() {
        effects.push(Effect::TimedStartsReArmed);
        let location = senses.facts(c.pawn)?.location;
        c.provoked = true;
        c.provoked_location = location;
        effects.push(Effect::Provoked { location });
    }
    if !c.on_hold {
        effects.push(Effect::LookAt(new));
    }
    if let Some(o) = old {
        effects.push(Effect::TargetingCount { actor: o, delta: -1 });
    }
    if let Some(n) = new {
        effects.push(Effect::TargetingCount { actor: n, delta: 1 });
    }
    Ok(())
}
