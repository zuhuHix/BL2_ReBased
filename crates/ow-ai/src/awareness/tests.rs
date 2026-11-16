//! Tests of the awareness rules on invented data (AGENTS.md rule 9): invented actors in an object space, their facts
//! and the engine's answers given by a table ([`Toy`]). AI-assisted (Claude Code).
//!
//! The rules have no stand-in (no AI runs in the C++ reference), so they do not depend on the parity mode; the VM
//! adapter's tests (`vm/tests.rs`) run in both modes.

use ow_core::units::Rotator;
use ow_core::{DetMap, Missing};
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::ObjRef;
use ow_vm::error::VmError;

use super::record::{ActorFacts, ActorKind, Component, Exposure, Opinion, Prioritizer, Settings, ThreatLevel};
use super::targets::{Effect, valid_target};
use super::threat::{is_aware, mind_attacked_by, set_threat};
use super::tick::tick;
use super::{Senses, Vec3};

/// Invented actors and the engine's answers about them.
struct Toy {
    facts: DetMap<ObjRef, ActorFacts>,
    opinions: DetMap<(ObjRef, ObjRef), Opinion>,
    exposure: DetMap<ObjRef, f32>,
    registry: Vec<ObjRef>,
    targeted: DetMap<ObjRef, i32>,
}

impl Senses for Toy {
    fn facts(&mut self, actor: ObjRef) -> Result<ActorFacts, VmError> {
        Ok(self.facts[&actor])
    }
    fn opinion(&mut self, pawn: ObjRef, other: ObjRef) -> Result<Opinion, VmError> {
        Ok(self.opinions.get(&(pawn, other)).copied().unwrap_or(Opinion::Enemy))
    }
    fn exposure(&mut self, _viewer: ObjRef, target: ObjRef) -> Result<f32, VmError> {
        Ok(self.exposure.get(&target).copied().unwrap_or(0.0))
    }
    fn targetables(&mut self) -> Result<Vec<ObjRef>, VmError> {
        Ok(self.registry.clone())
    }
    fn targeted_by(&mut self, actor: ObjRef) -> Result<i32, VmError> {
        Ok(self.targeted.get(&actor).copied().unwrap_or(0))
    }
}

const PAWN: ActorKind = ActorKind::Pawn {
    controlled: true,
    driving: false,
    weapon_seat: false,
};

/// An invented AI pawn at the origin facing +X, a player and a second AI, all registered.
struct World {
    toy: Toy,
    ai: ObjRef,
    player: ObjRef,
    other: ObjRef,
    space: ObjectSpace,
    class: ow_object::ids::ClassId,
}

fn world() -> World {
    let mut space = ObjectSpace::new();
    let class = space.classes.define("Toy.ToyActor", None, vec![]).unwrap();
    let mut actor = |name: &str| space.alloc(class, name, None, Origin::Runtime).unwrap();
    let (ai, player, other) = (actor("ToyBeast_0"), actor("ToyPlayer_0"), actor("ToyBeast_1"));
    let mut facts = DetMap::new();
    facts.insert(ai, ActorFacts::new([0.0; 3], Rotator::new(0, 0, 0), PAWN));
    facts.insert(player, ActorFacts::new([3000.0, 0.0, 0.0], Rotator::new(0, 32768, 0), PAWN));
    facts.insert(other, ActorFacts::new([0.0, 500.0, 0.0], Rotator::new(0, 0, 0), PAWN));
    let mut opinions = DetMap::new();
    opinions.insert((ai, other), Opinion::Friendly);
    World {
        toy: Toy {
            facts,
            opinions,
            exposure: DetMap::new(),
            registry: vec![player, other],
            targeted: DetMap::new(),
        },
        ai,
        player,
        other,
        space,
        class,
    }
}

/// A lone component with the live bullymong settings and an invented collision radius of 100.
fn component(w: &World) -> Component {
    Component::new(w.ai, Settings::live_bullymong(100.0))
}

fn place(toy: &mut Toy, actor: ObjRef, at: Vec3) {
    toy.facts.get_mut(&actor).unwrap().location = at;
}

/// Frames of 1/16 s (exact in binary) from `from` (exclusive) to `to` (inclusive); the 0.2 s update then runs every
/// fourth frame. Returns every effect.
fn run(c: &mut Component, w: &mut World, from: f32, to: f32) -> Vec<Effect> {
    let mut effects = Vec::new();
    let mut t = from;
    loop {
        t += 0.0625;
        if t > to {
            return effects;
        }
        effects.extend(tick(c, &mut w.toy, t).unwrap().effects);
    }
}

fn level(c: &Component, target: ObjRef) -> Option<ThreatLevel> {
    c.record(target).map(|r| r.threat)
}

// ---------------------------------------------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn the_exposure_byte_follows_the_two_thresholds() {
    for (f, e) in [
        (-1.0, Exposure::None),
        (0.0, Exposure::None),
        (0.39, Exposure::None),
        (0.4, Exposure::Half),
        (0.5, Exposure::Half),
        (0.89, Exposure::Half),
        (0.9, Exposure::Full),
        (1.0, Exposure::Full),
    ] {
        assert_eq!(Exposure::of(f), e, "{f}");
    }
}

#[test]
fn threat_levels_are_named_and_ordered() {
    assert_eq!(ThreatLevel::from_name("etl_provoked"), Some(ThreatLevel::Provoked));
    assert_eq!(ThreatLevel::from_name("ETL_Unknown"), None);
    assert!(ThreatLevel::SeenInThreatRadius.seen() && !ThreatLevel::InThreatRadius.seen());
}

// ---------------------------------------------------------------------------------------------------------------
// Threat
// ---------------------------------------------------------------------------------------------------------------

/// The level only rises; the first level of 2 or more stamps `AggroTime`; 2 and 3 push `ForgetTime` out by their
/// times; no record, nothing.
#[test]
fn set_threat_stamps_once_and_only_rises() {
    let mut w = world();
    let mut c = component(&w);
    set_threat(&mut c, 1.0, w.player, ThreatLevel::Provoked).unwrap();
    assert!(c.record(w.player).is_none(), "no record, nothing");
    c.records.push(super::Record::new(w.player));
    assert_eq!(level(&c, w.player), Some(ThreatLevel::InThreatRadius), "a fresh record shows 1");
    set_threat(&mut c, 2.0, w.player, ThreatLevel::SeenInThreatRadius).unwrap();
    let r = c.record(w.player).unwrap();
    assert_eq!((r.threat, r.aggro_time, r.forget_time), (ThreatLevel::SeenInThreatRadius, 2.0, 4.5));
    set_threat(&mut c, 3.0, w.player, ThreatLevel::Provoked).unwrap();
    let r = c.record(w.player).unwrap();
    assert_eq!((r.threat, r.aggro_time, r.forget_time), (ThreatLevel::Provoked, 2.0, 13.0));
    set_threat(&mut c, 4.0, w.player, ThreatLevel::SeenInThreatRadius).unwrap();
    let r = c.record(w.player).unwrap();
    assert_eq!((r.threat, r.forget_time), (ThreatLevel::Provoked, 13.0), "no fall, ForgetTime only grows");
    let _ = &mut w;
}

/// No record: not aware. A fresh record (`AggroTime` 0): aware once the 0.1 s delay has passed. After a stamp: not
/// aware for 0.1 s.
#[test]
fn awareness_needs_a_record_and_the_aggro_delay() {
    let w = world();
    let mut c = component(&w);
    assert!(!is_aware(&c, 5.0, w.player).unwrap());
    c.records.push(super::Record::new(w.player));
    assert!(is_aware(&c, 5.0, w.player).unwrap(), "fresh record");
    set_threat(&mut c, 5.0, w.player, ThreatLevel::SeenInThreatRadius).unwrap();
    assert!(!is_aware(&c, 5.05, w.player).unwrap());
    assert!(!is_aware(&c, 5.1, w.player).unwrap(), "strict");
    assert!(is_aware(&c, 5.15, w.player).unwrap());
}

// ---------------------------------------------------------------------------------------------------------------
// Noticing
// ---------------------------------------------------------------------------------------------------------------

/// In front, in the radius, exposed: the record appears at the first update with the exposure still none (the threat
/// iterator runs first), and the level and the target come one update later.
#[test]
fn the_sight_rule_notices_one_update_after_the_exposure() {
    let mut w = world();
    w.toy.exposure.insert(w.player, 1.0);
    let mut c = component(&w);
    let first = tick(&mut c, &mut w.toy, 0.2).unwrap();
    assert_eq!(first.added, vec![w.player], "the friendly bullymong is not a target");
    assert_eq!(level(&c, w.player), Some(ThreatLevel::InThreatRadius));
    assert_eq!(c.current, None);
    let second = tick(&mut c, &mut w.toy, 0.4).unwrap();
    assert_eq!(level(&c, w.player), Some(ThreatLevel::SeenInThreatRadius));
    assert_eq!(c.current, Some(w.player));
    let r = c.record(w.player).unwrap();
    assert_eq!((r.aggro_time, r.forget_time), (0.4, 0.4 + 2.5));
    assert_eq!(
        second.effects,
        vec![
            Effect::TargetAcquired(w.player),
            Effect::TimedStartsReArmed,
            Effect::Provoked { location: [0.0; 3] },
            Effect::LookAt(Some(w.player)),
            Effect::TargetingCount { actor: w.player, delta: 1 },
        ]
    );
    assert!(c.provoked);
}

/// Behind the pawn the cone fails; out of the radius the sight rule fails; not exposed it fails.
#[test]
fn the_sight_rule_needs_the_cone_the_radius_and_the_exposure() {
    for (at, exposure, noticed) in [
        ([-3000.0, 0.0, 0.0], 1.0, false),
        // Exactly side-on (dot 0) passes: the live threshold is just below 0 (cos 90 degrees in floats). The game's
        // edge is UNVERIFIED (open question 17).
        ([0.0, 3000.0, 0.0], 1.0, true),
        ([-10.0, 3000.0, 0.0], 1.0, false),
        ([3000.0, 10.0, 0.0], 1.0, true),
        ([4090.0, 0.0, 0.0], 1.0, true),
        ([3000.0, 0.0, 0.0], 0.3, false),
        ([3000.0, 0.0, 0.0], 0.5, true),
    ] {
        let mut w = world();
        place(&mut w.toy, w.player, at);
        w.toy.exposure.insert(w.player, exposure);
        let mut c = component(&w);
        run(&mut c, &mut w, 0.0, 1.0);
        assert_eq!(c.current == Some(w.player), noticed, "{at:?} exposure {exposure}");
    }
}

/// Behind and unexposed, close: the proximity rule notices at the record's first update (3D distance minus the
/// collision radius below 768); at exactly 768 it does not.
#[test]
fn the_proximity_rule_needs_no_cone_and_no_exposure() {
    for (x, noticed) in [(-800.0, true), (-867.0, true), (-868.0, false), (-2000.0, false)] {
        let mut w = world();
        place(&mut w.toy, w.player, [x, 0.0, 0.0]);
        let mut c = component(&w);
        tick(&mut c, &mut w.toy, 0.2).unwrap();
        assert_eq!(c.current == Some(w.player), noticed, "{x}");
    }
    // Height counts for the 3D distance: 500 out and 600 up is 781 (681 after the radius: close); 800 up is 943
    // (843: not close).
    for (up, noticed) in [(600.0, true), (800.0, false)] {
        let mut w = world();
        place(&mut w.toy, w.player, [-500.0, 0.0, up]);
        let mut c = component(&w);
        tick(&mut c, &mut w.toy, 0.2).unwrap();
        assert_eq!(c.current == Some(w.player), noticed, "{up}");
    }
}

/// Beyond the search radius (horizontal) there is no record at all; height does not count for being in the list.
#[test]
fn the_list_takes_targets_within_the_horizontal_search_radius() {
    let mut w = world();
    place(&mut w.toy, w.player, [4100.0, 0.0, 0.0]);
    let mut c = component(&w);
    run(&mut c, &mut w, 0.0, 1.0);
    assert!(c.record(w.player).is_none());
    place(&mut w.toy, w.player, [4000.0, 0.0, 3000.0]);
    tick(&mut c, &mut w.toy, 1.2).unwrap();
    assert!(c.record(w.player).is_some(), "3,000 above, 4,000 out: in the list");
}

/// The target is kept beyond the radius while the level holds; the level falls at the first update after
/// `ForgetTime`, and the target goes in that same update.
#[test]
fn a_target_is_kept_until_the_level_falls() {
    let mut w = world();
    w.toy.exposure.insert(w.player, 1.0);
    let mut c = component(&w);
    run(&mut c, &mut w, 0.0, 0.5);
    assert_eq!(c.current, Some(w.player));
    // She stays seen for 1 s: ForgetTime is refreshed at every update, to that update's time plus 2.5.
    run(&mut c, &mut w, 0.5, 1.5);
    let forget = c.record(w.player).unwrap().forget_time;
    assert_eq!(forget, c.last_target_update + 2.5);
    // Then she leaves: far, unexposed. The record stays (no distance cut), the target too until ForgetTime.
    place(&mut w.toy, w.player, [20000.0, 0.0, 0.0]);
    w.toy.exposure.insert(w.player, 0.0);
    run(&mut c, &mut w, 1.5, forget);
    assert_eq!(c.current, Some(w.player));
    let effects = run(&mut c, &mut w, forget, forget + 0.25);
    assert_eq!(c.current, None);
    assert_eq!(level(&c, w.player), Some(ThreatLevel::InThreatRadius));
    assert_eq!(c.record(w.player).unwrap().aggro_time, 0.0);
    assert_eq!(
        effects,
        vec![
            Effect::TargetLost(w.player),
            Effect::LookAt(None),
            Effect::TargetingCount { actor: w.player, delta: -1 },
        ]
    );
}

/// Once provoked, the threat origin is `ProvokedLocation`: the pawn walking away does not move it.
#[test]
fn a_provoked_component_measures_from_where_it_was_provoked() {
    let mut w = world();
    w.toy.exposure.insert(w.player, 1.0);
    let mut c = component(&w);
    run(&mut c, &mut w, 0.0, 0.5);
    assert!(c.provoked);
    // The pawn walks 3,000 towards +X; the player is now 4,000 from where it was provoked (and 1,000 from it).
    place(&mut w.toy, w.ai, [3000.0, 0.0, 0.0]);
    place(&mut w.toy, w.player, [4000.0, 0.0, 0.0]);
    run(&mut c, &mut w, 0.5, 1.0);
    let r = c.record(w.player).unwrap();
    assert_eq!(r.forget_time, c.last_target_update + 2.5, "refreshed: 4,000 from the origin is inside the radius");
    place(&mut w.toy, w.player, [4200.0, 0.0, 0.0]);
    run(&mut c, &mut w, 1.0, 1.25);
    let before = c.record(w.player).unwrap().forget_time;
    run(&mut c, &mut w, 1.25, 1.75);
    assert_eq!(c.record(w.player).unwrap().forget_time, before, "4,200 from the origin: no refresh");
}

// ---------------------------------------------------------------------------------------------------------------
// Attacked
// ---------------------------------------------------------------------------------------------------------------

/// Shot from outside the list: the attacker becomes a target at level 3 with the provoked stamps, and the next update
/// picks it.
#[test]
fn an_attack_provokes_from_any_distance() {
    let mut w = world();
    place(&mut w.toy, w.player, [-6000.0, 0.0, 0.0]);
    let mut c = component(&w);
    run(&mut c, &mut w, 0.0, 0.5);
    assert!(c.record(w.player).is_none());
    mind_attacked_by(&mut c, &mut w.toy, 0.5, Some(w.player), true).unwrap();
    let r = c.record(w.player).unwrap();
    assert_eq!((r.threat, r.aggro_time, r.forget_time), (ThreatLevel::Provoked, 0.5, 10.5));
    assert_eq!((c.last_attacked_time, c.last_hurt_time, c.last_attacker), (0.5, 0.5, Some(w.player)));
    run(&mut c, &mut w, 0.5, 0.75);
    assert_eq!(c.current, Some(w.player));
    // A friendly attacker is a target only with bCanTargetFriendlies: no record, and no field writes.
    let mut c = component(&w);
    mind_attacked_by(&mut c, &mut w.toy, 1.0, Some(w.other), false).unwrap();
    assert!(c.record(w.other).is_none());
    assert_eq!(c.last_attacker, None);
}

// ---------------------------------------------------------------------------------------------------------------
// Valid targets
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn valid_target_follows_the_recorded_answers() {
    let mut w = world();
    let c = component(&w);
    assert!(valid_target(&c, &mut w.toy, Some(w.player)).unwrap(), "the player");
    assert!(!valid_target(&c, &mut w.toy, None).unwrap(), "None");
    assert!(!valid_target(&c, &mut w.toy, Some(w.ai)).unwrap(), "itself");
    assert!(!valid_target(&c, &mut w.toy, Some(w.other)).unwrap(), "a friendly pawn");
    w.toy.facts.get_mut(&w.player).unwrap().cloaked = true;
    assert!(!valid_target(&c, &mut w.toy, Some(w.player)).unwrap(), "cloaked");
    w.toy.facts.get_mut(&w.player).unwrap().cloaked = false;
    w.toy.facts.get_mut(&w.ai).unwrap().has_allegiance = false;
    assert!(!valid_target(&c, &mut w.toy, Some(w.player)).unwrap(), "no allegiance");
    w.toy.facts.get_mut(&w.ai).unwrap().has_allegiance = true;
    w.toy.facts.get_mut(&w.player).unwrap().kind = ActorKind::Pawn {
        controlled: false,
        driving: false,
        weapon_seat: false,
    };
    assert!(!valid_target(&c, &mut w.toy, Some(w.player)).unwrap(), "no controller");
    w.toy.facts.get_mut(&w.player).unwrap().kind = PAWN;
    // A Neutral pawn is valid (only Friendly is refused) but FindTargets skips it.
    w.toy.opinions.insert((w.ai, w.player), Opinion::Neutral);
    assert!(valid_target(&c, &mut w.toy, Some(w.player)).unwrap(), "neutral is valid");
    let mut c = component(&w);
    w.toy.exposure.insert(w.player, 1.0);
    run(&mut c, &mut w, 0.0, 1.0);
    assert!(c.record(w.player).is_none(), "FindTargets skips a neutral entry");
}

/// A target that becomes invalid (controller lost) is removed at the next update, and the current target goes.
#[test]
fn an_invalid_target_is_removed_and_lost() {
    let mut w = world();
    w.toy.exposure.insert(w.player, 1.0);
    let mut c = component(&w);
    run(&mut c, &mut w, 0.0, 0.5);
    assert_eq!(c.current, Some(w.player));
    w.toy.facts.get_mut(&w.player).unwrap().kind = ActorKind::Pawn {
        controlled: false,
        driving: false,
        weapon_seat: false,
    };
    let report = tick(&mut c, &mut w.toy, 0.75).unwrap();
    assert_eq!(report.removed, vec![w.player]);
    assert!(c.records.is_empty());
    assert_eq!(c.current, None);
    assert!(report.effects.contains(&Effect::TargetLost(w.player)));
}

/// A hidden actor is not a current target; a scripted target is the only one.
#[test]
fn hidden_and_scripted_targets() {
    let mut w = world();
    w.toy.exposure.insert(w.player, 1.0);
    w.toy.facts.get_mut(&w.player).unwrap().hidden = true;
    let mut c = component(&w);
    run(&mut c, &mut w, 0.0, 1.0);
    assert_eq!(level(&c, w.player), Some(ThreatLevel::SeenInThreatRadius));
    assert_eq!(c.current, None, "hidden");
    w.toy.facts.get_mut(&w.player).unwrap().hidden = false;
    c.scripted_target = Some(w.other);
    run(&mut c, &mut w, 1.0, 1.5);
    assert_eq!(c.current, None, "only the scripted target is valid");
}

// ---------------------------------------------------------------------------------------------------------------
// Priority
// ---------------------------------------------------------------------------------------------------------------

/// The weighted mean of the live definition: (nearby + exposed + 0.4 x current + bonus) / 3.4, with the 3D distance
/// and the raw exposure (an invented -1.0 passes unclamped).
#[test]
fn the_priority_is_the_weighted_mean() {
    let mut w = world();
    let mut c = component(&w);
    c.records.push(super::Record::new(w.player));
    for (distance, exposure, current, expected) in [
        (1024.0f32, 1.0f32, true, (0.75 + 1.0 + 0.4) / 3.4),
        (2048.0, 0.5, false, (0.5 + 0.5) / 3.4),
        (8192.0, 0.0, true, 0.4 / 3.4),
        (3072.0, -1.0, false, (0.25 - 1.0) / 3.4),
    ] {
        c.current = current.then_some(w.player);
        let r = c.record_mut(w.player).unwrap();
        r.distance = distance;
        r.exposure_to_me = exposure;
        super::iterators::prioritize(&mut c, &mut w.toy, 0).unwrap();
        let p = c.record(w.player).unwrap().priority;
        assert!((p - expected).abs() < 1e-6, "{distance} {exposure}: {p} against {expected}");
    }
}

/// The highest valid record wins; a closer, more exposed target takes over from the current one only when its
/// priority beats the current one's 0.4 bonus.
#[test]
fn the_best_valid_record_is_chosen() {
    let mut w = world();
    let second = w.space.alloc(w.class, "ToyPlayer_1", None, Origin::Runtime).unwrap();
    w.toy.facts.insert(second, ActorFacts::new([2000.0, 100.0, 0.0], Rotator::new(0, 0, 0), PAWN));
    w.toy.registry.push(second);
    w.toy.exposure.insert(w.player, 1.0);
    w.toy.exposure.insert(second, 1.0);
    let mut c = component(&w);
    run(&mut c, &mut w, 0.0, 0.5);
    assert_eq!(c.current, Some(second), "nearer: (1 - 2000/4096) beats (1 - 3000/4096)");
    // The current one carries the 0.4 bonus: (1 - 2002.5/4096 + 1 + 0.4) / 3.4 = 0.562. The other at 1,500 uu scores
    // (1 - 1500/4096 + 1) / 3.4 = 0.481: no switch. At 100 uu it scores 0.581: the switch, lost before acquired.
    place(&mut w.toy, w.player, [1500.0, 0.0, 0.0]);
    run(&mut c, &mut w, 0.5, 1.0);
    assert_eq!(c.current, Some(second));
    place(&mut w.toy, w.player, [100.0, 0.0, 0.0]);
    let effects = run(&mut c, &mut w, 1.0, 1.25);
    assert_eq!(c.current, Some(w.player));
    assert_eq!(effects[..2], [Effect::TargetLost(second), Effect::TargetAcquired(w.player)]);
    assert!(!effects.contains(&Effect::TimedStartsReArmed), "a switch is not an acquisition from none");
}

/// The cooked weights give Idle and Attacking a weight; their inputs are not read here, so `NotYet`.
#[test]
fn an_unread_prioritizer_with_a_weight_is_not_yet() {
    let mut w = world();
    let mut c = component(&w);
    c.records.push(super::Record::new(w.player));
    c.settings.prioritization[1].1 = 0.25;
    let e = super::iterators::prioritize(&mut c, &mut w.toy, 0).unwrap_err();
    assert!(matches!(e, VmError::NotYet(n) if n.missing == Missing::NotPorted));
    c.settings.prioritization[1].1 = 0.0;
    c.settings.prioritization[0].1 = 0.5;
    assert!(super::iterators::prioritize(&mut c, &mut w.toy, 0).is_err(), "Idle without a maximum");
    c.settings.prioritization[0].0 = Prioritizer::Idle { max_targeting: Some(4.0) };
    w.toy.targeted.insert(w.player, 2);
    c.records[0].distance = 0.0;
    super::iterators::prioritize(&mut c, &mut w.toy, 0).unwrap();
    // (0.5 x (1 - 2/4) + 1 x 1 + 1 x 0 + 1 x 0) / 3.9
    assert!((c.records[0].priority - 1.25 / 3.9).abs() < 1e-6, "{}", c.records[0].priority);
}

// ---------------------------------------------------------------------------------------------------------------
// The tick
// ---------------------------------------------------------------------------------------------------------------

/// The target update runs at the first tick at or after `LastStateUpdateTime + UpdateRate`, and both times are
/// written together; on hold nothing runs.
#[test]
fn the_cadence_follows_the_state_update_time() {
    let mut w = world();
    let mut c = component(&w);
    let mut ran = Vec::new();
    for t in [0.0f32, 0.1, 0.19, 0.2, 0.3, 0.39, 0.45, 0.6, 0.64, 0.66] {
        if tick(&mut c, &mut w.toy, t).unwrap().target_update {
            ran.push(t);
            assert_eq!(c.last_target_update, c.last_state_update);
        }
    }
    assert_eq!(ran, vec![0.2, 0.45, 0.66]);
    c.on_hold = true;
    let held = tick(&mut c, &mut w.toy, 5.0).unwrap();
    assert!(!held.target_update && held.pending.iter().any(|p| p.contains("on hold")));
}

/// A component with a parent (a den member) is `NotYet`: the den's rules are not ported.
#[test]
fn a_den_member_is_not_yet() {
    let mut w = world();
    let mut c = component(&w);
    c.settings.has_parent = true;
    let e = tick(&mut c, &mut w.toy, 0.2).unwrap_err();
    assert!(matches!(e, VmError::NotYet(n) if n.missing == Missing::NotPorted));
}
