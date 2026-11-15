//! Owner: lane W3-BALANCE. The playthrough modifiers: `WillowGlobals.PlayThroughBasedBalanceModifiers` (one
//! `BalanceModifierDefinition` per playthrough) and the XP multiplier helper that reads them for the mission reward and
//! the kill credit (NATIVE_PROGRESSION.md section 2, "playthrough multiplier"; NATIVE_COMBAT_GROUNDWORK.md section
//! 5.2, step 3).
//!
//! The helper, as read (`kind` 1 combat, 2 quests):
//!
//! 1. playthrough index 2 or more (UVHM): the last `BalanceModifiers` entry of the playthrough's modifier whose
//!    `MinEffectiveLevel` is at most the player's level: its `XPGainedFromCombatMultiplier` or
//!    `XPGainedFromQuestsMultiplier`;
//! 2. otherwise, if the player's level is at least 50 and the playthrough-3 modifier exists:
//!    `ModifierToXPGainedTowardsNewLevelsInEarlierPlaythroughs[playthrough index]`;
//! 3. otherwise **1.0**.
//!
//! Only step 3 is built. Steps 1 and 2 need the modifier entry of a playthrough, and which entry of
//! `PlayThroughBasedBalanceModifiers` serves which playthrough (by position, or by its `PlaythroughToBalance`, whose
//! numbering is not read), the type of the multiplier fields and the identity of the "level cap" input are open
//! (NATIVE_COMBAT_GROUNDWORK.md 5.2 "Open"; NATIVE_PROGRESSION.md "What was not read"). They answer
//! `NotYet { NotRead }`. The two readings of the step-2 threshold (50 in NATIVE_PROGRESSION, "the level cap" in
//! NATIVE_COMBAT_GROUNDWORK) agree below 50, the only levels where this port answers 1.0. The player's level is
//! `WillowPlayerReplicationInfo.ExpLevel`. **Status: UNVERIFIED**; the 1.0 of the first playthrough below level 50
//! is behind the live Fire mission XP of 395 (REALGAME_GROUND_TRUTH.md).

use ow_attr::api::EvalCx;
use ow_core::NotYet;
use ow_object::ids::ObjRef;

use crate::types::{BalanceError, ExpLevel, Playthrough};

/// The record the multiplier comes from.
pub const RECORD: &str = "NATIVE_PROGRESSION.md section 2";

/// The level from which step 2 of the helper applies (NATIVE_PROGRESSION.md section 2: "the player's level is
/// ≥ 50"). Below it, the first two playthroughs take 1.0.
pub const EARLIER_PLAYTHROUGH_LEVEL: i32 = 50;

/// Which XP the multiplier scales: the helper's `kind` argument.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XpKind {
    /// Kind 1: combat (the kill credit), `XPGainedFromCombatMultiplier` in UVHM.
    Combat,
    /// Kind 2: quests (the mission reward), `XPGainedFromQuestsMultiplier` in UVHM.
    Quests,
}

impl XpKind {
    /// The helper's kind number.
    pub fn number(self) -> u8 {
        match self {
            XpKind::Combat => 1,
            XpKind::Quests => 2,
        }
    }
}

/// The `PlayThroughBasedBalanceModifiers` entry (a `BalanceModifierDefinition`) of playthrough `pt`, read from the
/// world's globals object.
///
/// `NotYet { NotRead }` for every playthrough: which entry serves which playthrough is not read (module
/// documentation). The world is taken so the signature does not change when the selection is read.
pub fn playthrough_modifier(
    cx: &mut EvalCx<'_>,
    pt: Playthrough,
) -> Result<Option<ObjRef>, BalanceError> {
    let _ = cx;
    Err(NotYet::not_read(
        format!(
            "which PlayThroughBasedBalanceModifiers entry serves playthrough index {} (by position or by its PlaythroughToBalance, whose numbering is not read)",
            pt.index()
        ),
        RECORD,
    )
    .into())
}

/// The XP multiplier `m` of the mission reward ([`XpKind::Quests`]) and the kill credit ([`XpKind::Combat`]) for a
/// player at `player_level` in playthrough `pt` (module documentation): 1.0 in the first two playthroughs below level
/// 50; `NotYet { NotRead }` in UVHM and at level 50 or more.
pub fn xp_multiplier(
    kind: XpKind,
    pt: Playthrough,
    player_level: ExpLevel,
) -> Result<f32, BalanceError> {
    if pt.index() >= 2 {
        return Err(NotYet::not_read(
            format!(
                "the kind-{} XP multiplier in playthrough index {}: the BalanceModifiers entry by MinEffectiveLevel",
                kind.number(),
                pt.index()
            ),
            RECORD,
        )
        .into());
    }
    if player_level.get() >= EARLIER_PLAYTHROUGH_LEVEL {
        return Err(NotYet::not_read(
            format!(
                "the kind-{} XP multiplier at level {} in playthrough index {}: ModifierToXPGainedTowardsNewLevelsInEarlierPlaythroughs",
                kind.number(),
                player_level.get(),
                pt.index()
            ),
            RECORD,
        )
        .into());
    }
    Ok(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::fixture::Fx;
    use ow_core::notyet::Missing;

    fn not_read(r: Result<f32, BalanceError>) -> bool {
        matches!(r, Err(BalanceError::NotYet(n)) if n.missing == Missing::NotRead && n.record == RECORD)
    }

    #[test]
    fn one_below_fifty_in_the_first_two_playthroughs() {
        // NATIVE_PROGRESSION.md section 2: "otherwise 1.0. The slice (playthrough 1, level < 50) is in this case."
        for kind in [XpKind::Combat, XpKind::Quests] {
            for pt in [Playthrough::NORMAL, Playthrough::TVHM] {
                for level in [1, 8, 49] {
                    assert_eq!(xp_multiplier(kind, pt, ExpLevel::new(level)).unwrap(), 1.0);
                }
            }
        }
    }

    #[test]
    fn uvhm_and_level_fifty_are_not_read() {
        for kind in [XpKind::Combat, XpKind::Quests] {
            assert!(not_read(xp_multiplier(
                kind,
                Playthrough::UVHM,
                ExpLevel::new(8)
            )));
            assert!(not_read(xp_multiplier(
                kind,
                Playthrough::UVHM,
                ExpLevel::new(72)
            )));
            for pt in [Playthrough::NORMAL, Playthrough::TVHM] {
                assert!(not_read(xp_multiplier(kind, pt, ExpLevel::new(50))));
                assert!(not_read(xp_multiplier(kind, pt, ExpLevel::new(80))));
            }
        }
    }

    #[test]
    fn the_modifier_entry_is_not_read() {
        let fx = Fx::new();
        let (_sys, mut world) = fx.into_world();
        let mut cx = EvalCx::new(&mut world);
        for pt in [Playthrough::NORMAL, Playthrough::TVHM, Playthrough::UVHM] {
            assert!(matches!(
                playthrough_modifier(&mut cx, pt),
                Err(BalanceError::NotYet(n)) if n.missing == Missing::NotRead
            ));
        }
    }
}
