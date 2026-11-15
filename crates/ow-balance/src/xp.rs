//! Owner: lane W3-BALANCE. The experience formulas: the mission reward (`MissionDefinition.GetExperienceReward`,
//! NATIVE_PROGRESSION.md section 2), the kill award and its per-player credit
//! (`WillowExperiencePipeline.CalculateExperiencePointsForKill` and `AwardCombatExperienceToParty`,
//! NATIVE_COMBAT_GROUNDWORK.md sections 5.1 and 5.2, COMBAT_KILL_XP.md), and the skill points of a level-up (the
//! script `WillowPlayerController.ExpLevelUp`, NATIVE_PROGRESSION.md section 3).
//!
//! Every amount is read from the data through the one evaluator (`ow-attr`); no closed form is coded. The C++ oracles
//! are `src/mission_script.cpp` `experienceReward` and `src/combat_xp.cpp`; where this port reaches further than they
//! do, the doc comment says so.
//!
//! **Status.** Verified live: the Fire mission reward 395 at stage 8 (REALGAME_GROUND_TRUTH.md); seven kill rows of
//! COMBAT_KILL_XP.md (award and credit, first playthrough, one player); the skill points `max(0, L - 4)` at levels 2,
//! 8, 17 and 70. Everything else here is UNVERIFIED.

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, EvalCx, InitData};
use ow_core::NotYet;
use ow_core::num::{trunc_i32, trunc_i64_wrap_i32};
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use crate::curve::experience_span;
use ow_attr::eval::{elements, field, init_data, number, object, property, required};

use crate::offline::data::is_a;
use crate::playthrough::{XpKind, xp_multiplier};
use crate::types::{BalanceError, ExpLevel, GameStage, PlayerCount, Playthrough};

/// The record of the mission reward.
pub const MISSION_RECORD: &str = "NATIVE_PROGRESSION.md section 2";
/// The record of the kill experience.
pub const KILL_RECORD: &str = "NATIVE_COMBAT_GROUNDWORK.md section 5";

// ---------------------------------------------------------------------------------------------------------------
// The mission reward
// ---------------------------------------------------------------------------------------------------------------

/// What one `GetExperienceReward` call needs.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct MissionXp {
    /// The mission's level `L`: its game stage (the region stage unless the mission's stage is locked).
    pub stage: GameStage,
    /// `Reward.ExperienceRewardPercentage` (or `AlternativeReward`'s), as data.
    pub percentage: InitData,
    /// The `OptionalExperienceRewardPercentage` of each optional objective the mission tracker reports complete.
    pub optional: Vec<InitData>,
    /// The player controller: the source the percentages are evaluated with.
    pub controller: Option<ObjRef>,
    /// The current playthrough.
    pub playthrough: Playthrough,
    /// The player's level (`WillowPlayerReplicationInfo.ExpLevel`), for the playthrough multiplier.
    pub player_level: ExpLevel,
}

impl MissionXp {
    /// A reward with no completed optional objective.
    pub fn new(
        stage: GameStage,
        percentage: InitData,
        controller: Option<ObjRef>,
        playthrough: Playthrough,
        player_level: ExpLevel,
    ) -> Self {
        MissionXp {
            stage,
            percentage,
            optional: Vec::new(),
            controller,
            playthrough,
            player_level,
        }
    }

    /// The same reward with these completed optional objectives' percentages.
    pub fn with_optional(mut self, optional: Vec<InitData>) -> Self {
        self.optional = optional;
        self
    }
}

/// `MissionDefinition.GetExperienceReward` (NATIVE_PROGRESSION.md section 2):
///
/// ```text
/// span   = R(L + 1) - R(L)                         (integers, `curve`)
/// amount = trunc(float(span x pct x m))            (extended precision, stored as float, truncated toward zero)
/// ```
///
/// plus the same term for each completed optional objective, each truncated on its own. `pct` is evaluated with the
/// player controller as the source; `m` is [`xp_multiplier`] for quests (1.0 in the first two playthroughs below level
/// 50, else `NotYet`). The C++ oracle refuses every playthrough but the first and any optional objective; the
/// record reads both, so this port computes them. The edge case "objectives but no mission tracker gives 0" is the
/// caller's (it knows whether a tracker exists). Status: verified at stage 8 (395, the Fire mission, live), the
/// rest UNVERIFIED.
pub fn mission_xp(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    curve: ObjRef,
    reward: &MissionXp,
) -> Result<i32, BalanceError> {
    let m = xp_multiplier(XpKind::Quests, reward.playthrough, reward.player_level)?;
    let span = experience_span(sys, cx, curve, ExpLevel::new(reward.stage.get()))?;
    let mut total = mission_term(sys, cx, span, &reward.percentage, m, reward.controller)?;
    for pct in &reward.optional {
        let term = mission_term(sys, cx, span, pct, m, reward.controller)?;
        total = total.checked_add(term).ok_or(BalanceError::OutOfRange {
            what: "mission experience reward (the sum of the terms overflows an int)",
            value: i64::from(total),
        })?;
    }
    Ok(total)
}

/// One term: `trunc(float(span x pct x m))`.
fn mission_term(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    span: i64,
    pct: &InitData,
    m: f32,
    controller: Option<ObjRef>,
) -> Result<i32, BalanceError> {
    let pct = sys.evaluate_init(pct, controller, None, cx)?;
    // f64 mirrors the native's extended-precision product (the C++ `double(span) * double(pct)`); span is far below
    // 2^53, so the conversion is exact. The product is then stored as a float (rounded to nearest) and truncated.
    let product = span as f64 * f64::from(pct) * f64::from(m);
    Ok(trunc_i32(product as f32))
}

// ---------------------------------------------------------------------------------------------------------------
// The kill award (for the whole party)
// ---------------------------------------------------------------------------------------------------------------

/// `WillowExperiencePipeline.CalculateExperiencePointsForKill` (NATIVE_COMBAT_GROUNDWORK.md section 5.1): the amount
/// for the whole party before the split, single precision, not rounded.
///
/// 1. `GlobalsDefinition.BaseEnemyExperienceFormula` evaluated with the **killed pawn** as the source (the data gives
///    the pawn's `ExperiencePointMultiplier` scaled by `Init_BaseEnemyExperience`).
/// 2. The vehicle adjustment: a vehicle victim (`Engine.Vehicle`) or a killer whose pawn drives a vehicle is
///    `NotYet { NotPorted }`, as the C++ oracle refuses them (the record reads them at medium confidence).
/// 3. A critical kill with a killer multiplies by the killer's `ExpScaleForCriticalHitKills` (its class must declare
///    it: `NotYet { NotRead }` otherwise). The `DamageTypeExpModifiers` factors need the damage source and type: a
///    non-empty list is `NotYet { NotPorted }` (empty in the game's data).
///
/// Status: UNVERIFIED as a reading of the native; the awards of seven live kills are reproduced through the data
/// chain (COMBAT_KILL_XP.md).
pub fn kill_xp_award(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    globals_definition: ObjRef,
    killed: ObjRef,
    killer: Option<ObjRef>,
    critical: bool,
) -> Result<f32, BalanceError> {
    let formula = required(cx, globals_definition, "BaseEnemyExperienceFormula")?;
    let data = init_data(cx.world.parts().space, &formula)?;
    if is_a(cx.world.parts().space, killed, "Engine.Vehicle")? {
        return Err(NotYet::not_ported(
            "CalculateExperiencePointsForKill: a vehicle victim (0 without an occupant)",
            KILL_RECORD,
        )
        .into());
    }
    if let Some(k) = killer
        && let Some(pawn) = property(cx, k, "Pawn")?
        && let Some(pawn) = object("Pawn", &pawn)?
        && let Some(driven) = property(cx, pawn, "DrivenVehicle")?
        && object("DrivenVehicle", &driven)?.is_some()
    {
        return Err(NotYet::not_ported(
            "CalculateExperiencePointsForKill: a killer in a vehicle (VehicleVSCharacterExpModifier)",
            KILL_RECORD,
        )
        .into());
    }
    let mut total = sys.evaluate_init(&data, Some(killed), None, cx)?;
    if critical && let Some(k) = killer {
        let Some(scale) = property(cx, k, "ExpScaleForCriticalHitKills")? else {
            return Err(NotYet::not_read(
                "CalculateExperiencePointsForKill: a critical kill by a killer whose class declares no ExpScaleForCriticalHitKills",
                KILL_RECORD,
            )
            .into());
        };
        total *= number("ExpScaleForCriticalHitKills", &scale)?;
    }
    if let Some(list) = property(cx, globals_definition, "DamageTypeExpModifiers")?
        && !elements("DamageTypeExpModifiers", &list)?.is_empty()
    {
        return Err(NotYet::not_ported(
            "CalculateExperiencePointsForKill: DamageTypeExpModifiers entries (the damage source and type factors)",
            KILL_RECORD,
        )
        .into());
    }
    Ok(total)
}

// ---------------------------------------------------------------------------------------------------------------
// The kill credit (one player's share)
// ---------------------------------------------------------------------------------------------------------------

/// One player's side of `AwardCombatExperienceToParty`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KillCredit {
    /// The award for the whole party ([`kill_xp_award`]).
    pub points: f32,
    /// The game's `EffectiveNumPlayers` (1 with no game info, and the single-player row for the class default 0).
    pub players: PlayerCount,
    /// Whether this player's controller is the killer.
    pub is_killer: bool,
    /// The player controller's level (its replication info's `ExpLevel`); `None` when it has no level source.
    pub player_level: Option<ExpLevel>,
    /// The killed actor's level (its balanced-actor `ExpLevel`); `None` when it has no level source.
    pub victim_level: Option<ExpLevel>,
    /// The current playthrough.
    pub playthrough: Playthrough,
}

impl KillCredit {
    pub fn new(
        points: f32,
        players: PlayerCount,
        is_killer: bool,
        player_level: Option<ExpLevel>,
        victim_level: Option<ExpLevel>,
        playthrough: Playthrough,
    ) -> Self {
        KillCredit {
            points,
            players,
            is_killer,
            player_level,
            victim_level,
            playthrough,
        }
    }
}

/// The experience one player is credited for a kill (NATIVE_COMBAT_GROUNDWORK.md section 5.2, step 2; the integer the
/// party loop hands to that controller's `ExpEarn(amount, 1, 1)`), single precision:
///
/// 1. the share: `ExpAwardWeights` at position `N - 1` (outside the table: bonus 0, weight 1); the killer takes
///    `(KillerExpBonus + ExpWeight) x points`, any other player `ExpWeight x points`;
/// 2. the level difference `d` (player against victim): equal levels or a missing level leave it; otherwise the entry
///    at position `min(|d|, length) - 1` of `ExpScaleByLevelDifference` multiplies by `HigherLevelEnemyExpScale`
///    when the victim is above the player, `LowerLevelEnemyExpScale` below (an empty table leaves it);
/// 3. the combat multiplier [`xp_multiplier`] (1.0 in the first two playthroughs below level 50, else `NotYet`; a
///    missing player level is `NotYet` there too, since the helper reads it);
/// 4. truncated toward zero.
///
/// Who is in the party, the loop order and the `ExpEarn` call are the world's. Status: the first-playthrough,
/// one-player credits of seven live kills are reproduced (COMBAT_KILL_XP.md); more players, a victim above the
/// player, a difference of 5 or more and the second playthrough are UNVERIFIED readings of the record.
pub fn kill_xp(
    cx: &mut EvalCx<'_>,
    globals_definition: ObjRef,
    credit: &KillCredit,
) -> Result<i32, BalanceError> {
    let Some(player_level) = credit.player_level else {
        return Err(NotYet::not_read(
            "the kill credit of a player with no level source: the playthrough multiplier reads the player's level",
            KILL_RECORD,
        )
        .into());
    };
    let m = xp_multiplier(XpKind::Combat, credit.playthrough, player_level)?;

    // 1. The share.
    let weights = required(cx, globals_definition, "ExpAwardWeights")?;
    let (bonus, weight) = {
        let space = cx.world.parts().space;
        let rows = elements("ExpAwardWeights", &weights)?;
        match rows.get(usize::from(credit.players.get()) - 1) {
            Some(row) => (
                number("KillerExpBonus", field(space, row, "KillerExpBonus")?)?,
                number("ExpWeight", field(space, row, "ExpWeight")?)?,
            ),
            None => (0.0, 1.0),
        }
    };
    let mut amount = if credit.is_killer {
        (bonus + weight) * credit.points
    } else {
        weight * credit.points
    };

    // 2. The level difference.
    if let Some(victim) = credit.victim_level {
        let d = i64::from(player_level.get()) - i64::from(victim.get());
        if d != 0 {
            let scales = required(cx, globals_definition, "ExpScaleByLevelDifference")?;
            let space = cx.world.parts().space;
            let rows = elements("ExpScaleByLevelDifference", &scales)?;
            if !rows.is_empty() {
                let at = usize::try_from(d.unsigned_abs())
                    .unwrap_or(usize::MAX)
                    .min(rows.len())
                    - 1;
                let name = if victim.get() > player_level.get() {
                    "HigherLevelEnemyExpScale"
                } else {
                    "LowerLevelEnemyExpScale"
                };
                amount *= number(name, field(space, &rows[at], name)?)?;
            }
        }
    }

    // 3. The combat multiplier, then 4. toward zero.
    amount *= m;
    Ok(trunc_i32(amount))
}

// ---------------------------------------------------------------------------------------------------------------
// Skill points
// ---------------------------------------------------------------------------------------------------------------

/// The general skill points one level-up grants: `int(GlobalsDefinition.GeneralSkillPointsPerLevelUp)` evaluated with
/// the player controller as the source **after** the level was raised (the script `ExpLevelUp`, NATIVE_PROGRESSION.md
/// section 3). The script's `int()` is the VM's float-to-int ([`trunc_i64_wrap_i32`]). In the game's data the value
/// is a conditional: 1 when `PlayerExperienceLevel` is at least 5, else 0, so a player at level `n` has earned
/// `max(0, n - 4)` (verified live at levels 2, 8, 17 and 70). `SpecialistSkillPoints` has no data in this build.
pub fn skill_points_for_level_up(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    globals_definition: ObjRef,
    controller: Option<ObjRef>,
) -> Result<i32, BalanceError> {
    let v = required(cx, globals_definition, "GeneralSkillPointsPerLevelUp")?;
    let data = init_data(cx.world.parts().space, &v)?;
    let x = sys.evaluate_init(&data, controller, None, cx)?;
    Ok(trunc_i64_wrap_i32(f64::from(x)))
}

/// Writes `level` into the replication info's `ExpLevel` as its declared kind (an int, or an int attribute).
pub(crate) fn write_level(
    cx: &mut EvalCx<'_>,
    replication_info: ObjRef,
    level: ExpLevel,
) -> Result<(), BalanceError> {
    let space = cx.world.parts().space;
    let class = space
        .heap
        .get(replication_info)
        .map_err(AttrError::from)?
        .class;
    let Some(p) = space.prop(class, "ExpLevel") else {
        return Err(BalanceError::Attr(AttrError::BadData {
            reason: "the replication info's class declares no ExpLevel".into(),
        }));
    };
    space
        .write(
            replication_info,
            ow_object::ids::PropAddr::scalar(p),
            Value::Int(level.get()),
        )
        .map_err(AttrError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::fixture::Fx;
    use ow_core::notyet::Missing;

    // ---- mission XP ----

    #[test]
    fn the_fire_mission_gives_three_hundred_ninety_five_at_stage_eight() {
        // REALGAME_GROUND_TRUTH.md: GetExperienceReward on the Fire mission for a level-8 Maya (stage 8): 395. The
        // record's table for stages 7..11 (truncated, not rounded): 316, 395, 483, 578, 681.
        for (stage, want) in [(7, 316), (8, 395), (9, 483), (10, 578), (11, 681)] {
            let mut fx = Fx::new();
            let curve = fx.xp_curve();
            let pct = fx.constant_data(0.05);
            let (sys, mut world) = fx.into_world();
            let controller = world.controller();
            let mut cx = EvalCx::new(&mut world);
            let reward = MissionXp::new(
                GameStage(stage),
                pct,
                Some(controller),
                Playthrough::NORMAL,
                ExpLevel::new(8),
            );
            assert_eq!(
                mission_xp(&sys, &mut cx, curve, &reward).unwrap(),
                want,
                "stage {stage}"
            );
        }
    }

    #[test]
    fn the_percentage_is_evaluated_for_the_player() {
        // An invented percentage read from the player's replication info: 0.05 when the PRI's ExpLevel is 8 (a
        // PlayerReplicationInfo context and an ObjectProperty resolver, then x 0.00625).
        let mut fx = Fx::new();
        let curve = fx.xp_curve();
        let pct = fx.player_level_data(0.00625);
        let (sys, mut world) = fx.into_world();
        let controller = world.controller();
        let mut cx = EvalCx::new(&mut world);
        let reward = MissionXp::new(
            GameStage(8),
            pct,
            Some(controller),
            Playthrough::NORMAL,
            ExpLevel::new(8),
        );
        assert_eq!(mission_xp(&sys, &mut cx, curve, &reward).unwrap(), 395);
    }

    #[test]
    fn optional_objectives_add_their_own_truncated_terms() {
        // Record: each optional term truncated on its own. Invented percentages at stage 8 (span 7,918):
        // 0.05 -> 395.9 -> 395; 0.01 -> 79.18 -> 79; 0.003 -> 23.754 -> 23.
        let mut fx = Fx::new();
        let curve = fx.xp_curve();
        let (a, b, c) = (
            fx.constant_data(0.05),
            fx.constant_data(0.01),
            fx.constant_data(0.003),
        );
        let (sys, mut world) = fx.into_world();
        let controller = world.controller();
        let mut cx = EvalCx::new(&mut world);
        let reward = MissionXp::new(
            GameStage(8),
            a,
            Some(controller),
            Playthrough::NORMAL,
            ExpLevel::new(8),
        )
        .with_optional(vec![b, c]);
        assert_eq!(
            mission_xp(&sys, &mut cx, curve, &reward).unwrap(),
            395 + 79 + 23
        );
    }

    #[test]
    fn tvhm_below_fifty_uses_one_and_uvhm_or_fifty_is_not_read() {
        let mut fx = Fx::new();
        let curve = fx.xp_curve();
        let pct = fx.constant_data(0.05);
        let (sys, mut world) = fx.into_world();
        let controller = world.controller();
        let mut cx = EvalCx::new(&mut world);
        let r = |pt, level| {
            MissionXp::new(
                GameStage(8),
                pct,
                Some(controller),
                pt,
                ExpLevel::new(level),
            )
        };
        assert_eq!(
            mission_xp(&sys, &mut cx, curve, &r(Playthrough::TVHM, 8)).unwrap(),
            395
        );
        for (pt, level) in [(Playthrough::UVHM, 8), (Playthrough::NORMAL, 50)] {
            assert!(matches!(
                mission_xp(&sys, &mut cx, curve, &r(pt, level)),
                Err(BalanceError::NotYet(n)) if n.missing == Missing::NotRead
            ));
        }
    }

    // ---- kill XP ----

    /// The live rows of COMBAT_KILL_XP.md, "Results against the live rows": (multiplier, victim level, player level,
    /// award, credit). The multipliers are the record's XPMultiplier tiers (Chump 0.5, Normal 0.75, Tough 1.25).
    const LIVE_KILLS: [(f32, i32, i32, f32, i32); 6] = [
        (0.5, 6, 8, 50.0, 35),
        (1.25, 7, 8, 137.5, 123),
        (0.5, 6, 10, 50.0, 7),
        (0.75, 6, 10, 75.0, 11),
        (1.25, 5, 8, 112.5, 45),
        (0.5, 6, 10, 50.0, 7),
    ];

    #[test]
    fn the_live_kill_rows_are_reproduced() {
        for (mult, victim, player, award, credit) in LIVE_KILLS {
            let mut fx = Fx::new();
            let globals = fx.kill_globals();
            let pawn = fx.enemy(mult, victim);
            let (sys, mut world) = fx.into_world();
            let controller = world.controller();
            let mut cx = EvalCx::new(&mut world);
            let got = kill_xp_award(&sys, &mut cx, globals, pawn, Some(controller), false).unwrap();
            assert_eq!(got, award, "award {mult} L{victim}");
            let c = KillCredit::new(
                got,
                PlayerCount::SINGLE,
                true,
                Some(ExpLevel::new(player)),
                Some(ExpLevel::new(victim)),
                Playthrough::NORMAL,
            );
            assert_eq!(
                kill_xp(&mut cx, globals, &c).unwrap(),
                credit,
                "credit L{victim} p{player}"
            );
        }
    }

    #[test]
    fn the_worked_examples_of_the_record() {
        // NATIVE_COMBAT_GROUNDWORK.md 5.2 "Worked example" (formula only): a Chump at level 8 with Maya at 8: 60;
        // Maya at 9: 54; a level-10 Chump with Maya at 8: 70 x 1.03 = 72.1 -> 72.
        for (victim, player, want) in [(8, 8, 60), (8, 9, 54), (10, 8, 72)] {
            let mut fx = Fx::new();
            let globals = fx.kill_globals();
            let pawn = fx.enemy(0.5, victim);
            let (sys, mut world) = fx.into_world();
            let controller = world.controller();
            let mut cx = EvalCx::new(&mut world);
            let award =
                kill_xp_award(&sys, &mut cx, globals, pawn, Some(controller), false).unwrap();
            let c = KillCredit::new(
                award,
                PlayerCount::SINGLE,
                true,
                Some(ExpLevel::new(player)),
                Some(ExpLevel::new(victim)),
                Playthrough::NORMAL,
            );
            assert_eq!(kill_xp(&mut cx, globals, &c).unwrap(), want);
        }
    }

    #[test]
    fn shares_large_differences_and_missing_levels() {
        let mut fx = Fx::new();
        let globals = fx.kill_globals();
        let (_sys, mut world) = fx.into_world();
        let mut cx = EvalCx::new(&mut world);
        let credit = |players: i32, killer: bool, p: Option<i32>, v: Option<i32>| {
            KillCredit::new(
                100.0,
                PlayerCount::new(players).unwrap(),
                killer,
                p.map(ExpLevel::new),
                v.map(ExpLevel::new),
                Playthrough::NORMAL,
            )
        };
        // Record 5.2: two players, killer 94 percent, partner 90; four: 86 and 80 (UNVERIFIED readings).
        assert_eq!(
            kill_xp(&mut cx, globals, &credit(2, true, Some(8), Some(8))).unwrap(),
            94
        );
        assert_eq!(
            kill_xp(&mut cx, globals, &credit(2, false, Some(8), Some(8))).unwrap(),
            90
        );
        assert_eq!(
            kill_xp(&mut cx, globals, &credit(4, true, Some(8), Some(8))).unwrap(),
            86
        );
        assert_eq!(
            kill_xp(&mut cx, globals, &credit(4, false, Some(8), Some(8))).unwrap(),
            80
        );
        // A difference past the table takes its last entry (lower 0.01, higher 1.15): 100 x 0.01 and 100 x 1.15
        // in single precision are 1.0 and 115.0.
        assert_eq!(
            kill_xp(&mut cx, globals, &credit(1, true, Some(30), Some(8))).unwrap(),
            1
        );
        assert_eq!(
            kill_xp(&mut cx, globals, &credit(1, true, Some(8), Some(30))).unwrap(),
            115
        );
        // No victim level: no scale. No player level: the multiplier cannot be read.
        assert_eq!(
            kill_xp(&mut cx, globals, &credit(1, true, Some(8), None)).unwrap(),
            100
        );
        assert!(matches!(
            kill_xp(&mut cx, globals, &credit(1, true, None, Some(8))),
            Err(BalanceError::NotYet(_))
        ));
    }

    #[test]
    fn critical_kills_and_the_not_yet_branches() {
        let mut fx = Fx::new();
        let globals = fx.kill_globals();
        let pawn = fx.enemy(0.5, 8);
        let vehicle = fx.vehicle();
        let (sys, mut world) = fx.into_world();
        let controller = world.controller();
        crate::offline::fixture::set(
            &mut world,
            controller,
            "ExpScaleForCriticalHitKills",
            Value::Float(2.0),
        );
        let mut cx = EvalCx::new(&mut world);
        // An invented critical factor of 2 on the killer: 60 x 2.
        assert_eq!(
            kill_xp_award(&sys, &mut cx, globals, pawn, Some(controller), true).unwrap(),
            120.0
        );
        // Not critical, or no killer: no factor.
        assert_eq!(
            kill_xp_award(&sys, &mut cx, globals, pawn, None, true).unwrap(),
            60.0
        );
        assert!(matches!(
            kill_xp_award(&sys, &mut cx, globals, vehicle, Some(controller), false),
            Err(BalanceError::NotYet(n)) if n.missing == Missing::NotPorted
        ));
        // UVHM and level 50 credits: the multiplier is not read.
        for (pt, level) in [(Playthrough::UVHM, 8), (Playthrough::NORMAL, 50)] {
            let c = KillCredit::new(
                60.0,
                PlayerCount::SINGLE,
                true,
                Some(ExpLevel::new(level)),
                Some(ExpLevel::new(8)),
                pt,
            );
            assert!(matches!(
                kill_xp(&mut cx, globals, &c),
                Err(BalanceError::NotYet(_))
            ));
        }
    }

    #[test]
    fn damage_type_factors_are_not_ported() {
        let mut fx = Fx::new();
        let globals = fx.kill_globals();
        fx.add_damage_type_modifier(globals);
        let pawn = fx.enemy(0.5, 8);
        let (sys, mut world) = fx.into_world();
        let mut cx = EvalCx::new(&mut world);
        assert!(matches!(
            kill_xp_award(&sys, &mut cx, globals, pawn, None, false),
            Err(BalanceError::NotYet(n)) if n.missing == Missing::NotPorted
        ));
    }

    // ---- skill points ----

    #[test]
    fn skill_points_are_max_zero_level_minus_four() {
        // REALGAME_GROUND_TRUTH.md: max(0, L - 4) spent + unspent: 0 at level 2, 4 at 8, 13 at 17, 66 at 70.
        let mut fx = Fx::new();
        let globals = fx.skill_point_globals();
        let (sys, mut world) = fx.into_world();
        for (level, want) in [(2, 0), (5, 1), (8, 4), (17, 13), (70, 66)] {
            assert_eq!(
                world
                    .skill_points_earned(&sys, globals, ExpLevel::new(level))
                    .unwrap(),
                want
            );
        }
        // The level the world holds is left as it was.
        assert_eq!(world.player_level(), ExpLevel::new(8));
    }
}
