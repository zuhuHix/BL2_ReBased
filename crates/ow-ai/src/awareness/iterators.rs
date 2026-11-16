//! Owner: lane W7-AI (part 1). The targeting iterators: `CalcTargetInfo` runs the Knowledge iterators in the
//! definition's order, `PrioritizeTarget` the Prioritization ones (NATIVE_AI_MELEE.md, "CalcTargetInfo and
//! PrioritizeTarget" and "The iterators"; NATIVE_AI_AWARENESS.md, "WillowTI_CalcThreat"). AI-assisted (Claude Code).
//!
//! **Order matters** (L9 section 0): `WillowTI_CalcThreat` runs before `WillowTI_CalcExposure` and
//! `WillowTI_CalcDistances` in the live definition, so the threat rules read the exposure and the facing dot product
//! of the previous update. They are read here from the record in the same way, whatever the definition's order is.

use ow_core::NotYet;
use ow_vm::error::VmError;

use super::record::{ActorKind, Component, Exposure, Knowledge, Opinion, Prioritizer, ThreatLevel};
use super::threat::{get_threat, set_threat};
use super::{RECORD_MELEE, Senses, dot_unit, facing, length, length_2d, sub};

/// `CalcTargetInfo(record)`: every Knowledge iterator of the definition, in order, on record `index`.
pub fn calc_target_info(
    c: &mut Component,
    senses: &mut dyn Senses,
    now: f32,
    index: usize,
) -> Result<(), VmError> {
    for k in c.settings.knowledge.clone() {
        match k {
            Knowledge::Threat => calc_threat(c, senses, now, index)?,
            Knowledge::CanFire => calc_can_fire(c, index),
            Knowledge::Exposure => calc_exposure(c, senses, now, index)?,
            Knowledge::Distances => calc_distances(c, senses, index)?,
            Knowledge::Combat | Knowledge::Empty => {}
        }
    }
    Ok(())
}

/// `WillowTI_CalcDistances`: `Distance` (3D), `Distance2D` (X and Y), `DistanceVertical` (absolute height
/// difference), `DotToTarget` (the AI pawn's facing with the direction to the target), `DotFromTarget` (the target's
/// facing with the direction back) and `DotTargetFacing` (the two facings; UNVERIFIED reading). Live, L8: the
/// distances equal the geometry. `LeftDotToTarget` and `LastKnownLocation` are not filled (their reading is open).
pub fn calc_distances(c: &mut Component, senses: &mut dyn Senses, index: usize) -> Result<(), VmError> {
    let target = c.records[index].target;
    let me = senses.facts(c.pawn)?;
    let it = senses.facts(target)?;
    let to = sub(me.location, it.location);
    let back = sub(it.location, me.location);
    let (mine, theirs) = (facing(me.rotation), facing(it.rotation));
    let r = &mut c.records[index];
    r.distance = length(to);
    r.distance_2d = length_2d(to);
    r.distance_vertical = (it.location[2] - me.location[2]).abs();
    r.dot_to_target = dot_unit(mine, to);
    r.dot_from_target = dot_unit(theirs, back);
    r.dot_target_facing = mine[0] * theirs[0] + mine[1] * theirs[1] + mine[2] * theirs[2];
    Ok(())
}

/// `WillowTI_CalcExposure`: `TargetExposureToMe` is the engine's exposure of the target to the AI pawn (asked through
/// [`Senses::exposure`]), `Exposure` its three-level byte ([`Exposure::of`]), and `ExposureChangeTime` becomes `now`
/// when the exposure changes between none and some (G25; live values 0.0, 0.5, 1.0 and -1.0, L8).
pub fn calc_exposure(
    c: &mut Component,
    senses: &mut dyn Senses,
    now: f32,
    index: usize,
) -> Result<(), VmError> {
    let target = c.records[index].target;
    let fraction = senses.exposure(c.pawn, target)?;
    // A non-finite answer would read as full sight (NaN compares false with every threshold); the game shows 0 to 1
    // and -1, so anything not finite is the host's error.
    if !fraction.is_finite() {
        return Err(VmError::Script {
            function: "ai awareness".to_string(),
            message: format!("the host's exposure answer is not finite ({fraction}); expected 0 to 1, or -1"),
        });
    }
    let r = &mut c.records[index];
    let level = Exposure::of(fraction);
    if (r.exposure == Exposure::None) != (level == Exposure::None) {
        r.exposure_change_time = now;
    }
    r.exposure_to_me = fraction;
    r.exposure = level;
    Ok(())
}

/// `WillowTI_CalcCanFire`: `bCanFireAt` and `bCanSeeOrHearTarget` are set when `Exposure` is not none (G25; live,
/// L8: the flag followed line of sight). `WeaponDotToTarget` (the weapon's aim at eye height) is not filled: its
/// inputs are not read.
pub fn calc_can_fire(c: &mut Component, index: usize) {
    let r = &mut c.records[index];
    let some = r.exposure != Exposure::None;
    r.can_fire_at = some;
    r.can_see_or_hear = some;
}

/// `WillowTI_CalcThreat` for a lone component (G26; confirmed live with a den as the origin, L9 sections 0 and 10):
/// the record's level becomes `GetThreatLevel(target)`; then, when the AI pawn regards the target as an Enemy and
/// has its collision component, the level is raised to `ETL_SeenInThreatRadius` (with the stamps of
/// [`set_threat`]) when either rule holds:
/// - **sight**: the record's `Exposure` (the previous update's) is not none, its `DotToTarget` (the previous
///   update's) is above `SightViewCone`, and the horizontal distance from the threat origin to the target is below the
///   threat radius;
/// - **proximity** (not for a vehicle): the horizontal distance from the origin is below the patrol limit and the 3D
///   distance from the pawn to the target minus the pawn's collision radius is below `AICloseThreatRadius`.
///
/// A lone component's origin is the pawn's location, or `ProvokedLocation` while `bProvoked` is set; its threat radius
/// is the AI's search radius; it has no patrol limit (G26; **UNVERIFIED**: every live pawn had a den, and the lone case
/// is the architecture's open question 17). The cone is strict (`>`): the game was tested only by sign (+0.91 fires,
/// -0.67 does not), so the edge is UNVERIFIED (question 17, "cone edge"). The proximity distance is the live one, not
/// the record's: a fresh record's distance is still 0, and the game raised no level for records added kilometres away.
pub fn calc_threat(
    c: &mut Component,
    senses: &mut dyn Senses,
    now: f32,
    index: usize,
) -> Result<(), VmError> {
    let target = c.records[index].target;
    let level = get_threat(c, senses, target)?;
    c.records[index].threat = level;
    if senses.opinion(c.pawn, target)? != Opinion::Enemy {
        return Ok(());
    }
    let Some(collision_radius) = c.settings.collision_radius else {
        return Ok(());
    };
    let me = senses.facts(c.pawn)?;
    let it = senses.facts(target)?;
    let origin = if c.provoked {
        c.provoked_location
    } else {
        me.location
    };
    let from_origin = length_2d(sub(origin, it.location));
    let r = &c.records[index];
    let sight = r.exposure != Exposure::None
        && r.dot_to_target > c.settings.sight_view_cone
        && from_origin < c.settings.search_radius;
    let vehicle = matches!(it.kind, ActorKind::Vehicle { .. });
    let proximity = !vehicle
        && length(sub(me.location, it.location)) - collision_radius < c.settings.close_threat_radius;
    if sight || proximity {
        set_threat(c, now, target, ThreatLevel::SeenInThreatRadius)?;
    }
    Ok(())
}

/// `PrioritizeTarget(record)`: `Priority = sum(OnePriority * Weight) / sum(Weight)` over the Prioritization
/// iterators; the record keeps its priority when the weight sum is not positive (G25; live, L8: 134 samples to 5e-8
/// with the weights 1, 1, 0.4 and 1 and the Idle and Attacking iterators at weight 0). One whose inputs are not read
/// answers `NotYet` when its weight is not 0. A priority that is not finite is refused (it would win any sort).
///
/// **An iterator with weight 0 is skipped** (UNVERIFIED): it adds nothing to the priority, but in the game it still
/// runs. That matters only for `WillowTI_PrioritizeAttacking`, whose run keeps the record's decaying damage meter
/// (`TotalDamagePct`, an update-state iterator): that meter is not modelled here (its hit history is not read), so it
/// stays 0, as L8 read it in a fight where the player did no damage.
pub fn prioritize(
    c: &mut Component,
    senses: &mut dyn Senses,
    index: usize,
) -> Result<(), VmError> {
    let target = c.records[index].target;
    let mut sum = 0.0f32;
    let mut weights = 0.0f32;
    let mut last = 0.0f32;
    for (p, weight) in c.settings.prioritization.clone() {
        if weight == 0.0 {
            continue;
        }
        let r = &c.records[index];
        let one = match p {
            Prioritizer::Exposed => r.exposure_to_me,
            Prioritizer::Nearby { max_distance } => {
                if max_distance <= 0.0 {
                    return Err(NotYet::not_read(
                        "TargetIteratorPrioritizeNearbyTarget with a MaxDistance of 0 or less",
                        RECORD_MELEE,
                    )
                    .into());
                }
                (1.0 - r.distance / max_distance).clamp(0.0, 1.0)
            }
            Prioritizer::Previous => {
                if c.current == Some(target) {
                    1.0
                } else {
                    0.0
                }
            }
            Prioritizer::Bonus => r.bonus_priority,
            Prioritizer::Idle { max_targeting } => {
                let Some(max) = max_targeting.filter(|m| *m > 0.0) else {
                    return Err(NotYet::not_read(
                        "WillowTI_PrioritizeIdle with a weight and no positive MaxAICurrentlyTargeting",
                        RECORD_MELEE,
                    )
                    .into());
                };
                let mut count = senses.targeted_by(target)?;
                if c.current == Some(target) {
                    count = count.saturating_sub(1);
                }
                (1.0 - count as f32 / max).clamp(0.0, 1.0)
            }
            Prioritizer::Attacking => {
                return Err(NotYet::not_ported(
                    "WillowTI_PrioritizeAttacking with a weight: the hit history it reads is not modelled",
                    RECORD_MELEE,
                )
                .into());
            }
        };
        last = one;
        sum += one * weight;
        weights += weight;
    }
    let r = &mut c.records[index];
    r.one_priority = last;
    if weights > 0.0 {
        let priority = sum / weights;
        if !priority.is_finite() {
            return Err(VmError::Script {
                function: "ai awareness".to_string(),
                message: format!("a target's priority is not finite ({priority})"),
            });
        }
        r.priority = priority;
    }
    Ok(())
}
