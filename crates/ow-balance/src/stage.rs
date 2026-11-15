//! Owner: lane W3-BALANCE. The region game stage (`WillowRegionDefinition.GetRegionGameStage`, an internal routine;
//! NATIVE_PROGRESSION.md section 2, "Region game stage"), the level of a mission and of the dens in a region.
//!
//! As read, in order:
//!
//! 1. Defaults: stage 1.
//! 2. Playthrough 3 (UVHM): a player-level-based value of at least 50, overpower-aware, **not read in detail**:
//!    `NotYet { NeedsRealGame(2) }` (architecture section 12, question 2).
//! 3. The table: the region's `DlcExpansion` table if it has one, else `GlobalsDefinition.RegionBalanceData`, entry by
//!    playthrough index clamped to the table, whose `BalanceDefinitions` are searched in order for the region. A
//!    region with a DLC expansion is `NotYet { NotRead }` here: the field of the expansion that holds its table is not
//!    recorded. A region no table entry names keeps the default (stage 1) and stores nothing (UNVERIFIED: the record
//!    gives the defaults, not whether they are stored).
//! 4. **Stored stage first.** A stage the player controller already holds for this region and this playthrough is
//!    returned unchanged ([`RegionStages`]).
//! 5. Otherwise, in the region's entry: `boost` = `GlobalsDefinition.GameStageIncreaseAbovePlayer`, or the entry's
//!    when `bSpecifyBoostAbovePlayer`; `clampToPlayer(min, max)` = `clamp(player level + boost, min, max)`, the player
//!    level being the local player's pawn's experience level, or `min` with no local player pawn; each
//!    `MissionOverrides` entry whose mission is **Complete** gives `clampToPlayer(MinGameStage, MaxGameStage)` and the
//!    largest wins; with none, `clampToPlayer(MinDefaultGameStage, MaxDefaultGameStage)`. The result is stored.
//!
//! Choices where the record is silent, each UNVERIFIED: the bounds and boosts are `AttributeInitializationData`
//! evaluated with the player controller as the source and truncated toward zero ([`trunc_i32`], the attribute path);
//! the clamp is UE3's `Clamp` (below `min` is `min`, else at most `max`), so bounds out of order give `min`. The
//! awesome level the game stores with the stage is not read here (the record names `DefaultAwesomeLevel` and an
//! override's `AwesomeLevel`, not their types). The record's playthrough index chooses the table entry by position;
//! `tools/slice_values.py` matches the entry's `PlayThroughNumber` instead, which agrees on a table in playthrough
//! order.
//!
//! **Where the game stores it:** on the player controller, per region and playthrough. The controller field is not
//! read, so the store is this crate's [`RegionStages`], which the world keeps with its player controller
//! (`OfflineWorld` holds one). When the game calls `AWillowPlayerController.ResetGameStageForRegion` is not read
//! (architecture section 12, question 14): nothing here resets a stored stage. **Status: UNVERIFIED** in the running
//! game; the Sanctuary first-playthrough numbers (7..9, 8..11 once WelcomeToSanctuary is complete) are the record's
//! reading of the data.

use ow_attr::AttributeSystem;
use ow_attr::api::EvalCx;
use ow_core::num::trunc_i32;
use ow_core::{DetMap, NotYet};
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use crate::offline::data::{
    class_path, elements, field, field_opt, flag, init_data, object, property, required,
};
use crate::types::{BalanceError, ExpLevel, GameStage, Playthrough};

/// The record the rule comes from.
pub const RECORD: &str = "NATIVE_PROGRESSION.md section 2, \"Region game stage\"";

/// The stage the routine starts from and returns when no table entry names the region.
pub const DEFAULT_STAGE: GameStage = GameStage(1);

/// The region stages a player controller holds: one per (region, playthrough), set the first time the region's stage
/// is computed in that playthrough and returned unchanged afterwards. Ordered by key (`DetMap`), so a listing is
/// deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegionStages {
    stored: DetMap<(ObjRef, u8), GameStage>,
}

impl RegionStages {
    /// No stage stored.
    pub fn new() -> Self {
        Self::default()
    }

    /// The stage stored for `region` in `pt`, if any.
    pub fn get(&self, region: ObjRef, pt: Playthrough) -> Option<GameStage> {
        self.stored.get(&(region, pt.index())).copied()
    }

    /// Stores `stage` for `region` in `pt`, as a saved game restores it. Answers the stage it replaced.
    pub fn set(&mut self, region: ObjRef, pt: Playthrough, stage: GameStage) -> Option<GameStage> {
        self.stored.insert((region, pt.index()), stage)
    }

    /// Every stored stage in key order: `(region, playthrough index, stage)`.
    pub fn iter(&self) -> impl Iterator<Item = (ObjRef, u8, GameStage)> + '_ {
        self.stored.iter().map(|(&(r, p), &s)| (r, p, s))
    }

    /// How many stages are stored.
    pub fn len(&self) -> usize {
        self.stored.len()
    }

    /// True when nothing is stored.
    pub fn is_empty(&self) -> bool {
        self.stored.is_empty()
    }
}

/// One region-stage question: which region, in which playthrough, for which player.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct RegionQuery<'a> {
    /// The `WillowRegionDefinition` (a mission's `GameStageRegion`).
    pub region: ObjRef,
    /// The current playthrough.
    pub playthrough: Playthrough,
    /// The local player's pawn's experience level; `None` with no local player pawn (the bounds' minimum is used).
    pub player_level: Option<ExpLevel>,
    /// The missions whose status is Complete (4) for this player, as the mission tracker reports them; an override
    /// applies when its `Mission` is one of these.
    pub completed_missions: &'a [ObjRef],
    /// The player controller: the source the bounds are evaluated with (UNVERIFIED choice) and the owner of the
    /// stored stages.
    pub controller: Option<ObjRef>,
}

impl<'a> RegionQuery<'a> {
    pub fn new(
        region: ObjRef,
        playthrough: Playthrough,
        player_level: Option<ExpLevel>,
        completed_missions: &'a [ObjRef],
        controller: Option<ObjRef>,
    ) -> Self {
        RegionQuery {
            region,
            playthrough,
            player_level,
            completed_missions,
            controller,
        }
    }
}

/// The region's game stage (module documentation): the stored stage if `stages` holds one for this region and
/// playthrough, else the stage computed from `globals_definition`'s `RegionBalanceData`, which is then stored.
///
/// `globals_definition` is the `GlobalsDefinition` the game's globals name (`GD_Globals.General.Globals` in the
/// game's data).
pub fn region_game_stage(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    globals_definition: ObjRef,
    stages: &mut RegionStages,
    q: &RegionQuery<'_>,
) -> Result<GameStage, BalanceError> {
    // Step 2: UVHM.
    if q.playthrough.is_uvhm() {
        return Err(NotYet::needs_real_game(
            "the UVHM region game stage (a player-level-based value of at least 50, overpower-aware)",
            2,
            RECORD,
        )
        .into());
    }
    // Step 3: the table.
    if let Some(v) = property(cx, q.region, "DlcExpansion")?
        && object("DlcExpansion", &v)?.is_some()
    {
        return Err(NotYet::not_read(
            format!(
                "the region game stage of {}, which has a DLC expansion: the expansion's region table is not recorded",
                class_path(cx.world.parts().space, q.region)
            ),
            RECORD,
        )
        .into());
    }
    let Some(entry) = region_entry(cx, globals_definition, q)? else {
        return Ok(DEFAULT_STAGE);
    };
    // Step 4: the stored stage.
    if let Some(stage) = stages.get(q.region, q.playthrough) {
        return Ok(stage);
    }
    // Step 5: compute and store.
    let stage = computed_stage(sys, cx, globals_definition, &entry, q)?;
    stages.set(q.region, q.playthrough, stage);
    Ok(stage)
}

/// The `RegionBalanceData` entry naming the region in the playthrough's table, if any.
fn region_entry(
    cx: &mut EvalCx<'_>,
    globals_definition: ObjRef,
    q: &RegionQuery<'_>,
) -> Result<Option<Value>, BalanceError> {
    let table = required(cx, globals_definition, "RegionBalanceData")?;
    let rows = elements("RegionBalanceData", &table)?;
    let Some(last) = rows.len().checked_sub(1) else {
        return Ok(None);
    };
    // "entry chosen by playthrough index, clamped to the table".
    let row = rows[usize::from(q.playthrough.index()).min(last)].clone();
    let definitions = field(cx.world.parts().space, &row, "BalanceDefinitions")?.clone();
    for d in elements("BalanceDefinitions", &definitions)? {
        let Some(definition) = object("BalanceDefinitions", d)? else {
            continue;
        };
        let by_region = required(cx, definition, "BalanceByRegion")?;
        for entry in elements("BalanceByRegion", &by_region)? {
            let space = cx.world.parts().space;
            if object("Region", field(space, entry, "Region")?)? == Some(q.region) {
                return Ok(Some(entry.clone()));
            }
        }
    }
    Ok(None)
}

/// An `AttributeInitializationData` bound or boost, evaluated with the controller and truncated toward zero.
fn bound(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    v: &Value,
    controller: Option<ObjRef>,
) -> Result<i32, BalanceError> {
    let data = init_data(cx.world.parts().space, v)?;
    let x = sys.evaluate_init(&data, controller, None, cx)?;
    Ok(trunc_i32(x))
}

/// UE3 `Clamp(x, min, max)`: `min` below `min`, else `x` below `max`, else `max`.
fn clamp(x: i32, min: i32, max: i32) -> i32 {
    if x < min {
        min
    } else if x < max {
        x
    } else {
        max
    }
}

fn computed_stage(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    globals_definition: ObjRef,
    entry: &Value,
    q: &RegionQuery<'_>,
) -> Result<GameStage, BalanceError> {
    let specify = {
        let space = cx.world.parts().space;
        flag(
            "bSpecifyBoostAbovePlayer",
            field(space, entry, "bSpecifyBoostAbovePlayer")?,
        )?
    };
    let boost_value = if specify {
        let space = cx.world.parts().space;
        field(space, entry, "GameStageIncreaseAbovePlayer")?.clone()
    } else {
        required(cx, globals_definition, "GameStageIncreaseAbovePlayer")?
    };
    let boost = bound(sys, cx, &boost_value, q.controller)?;
    let to_player = |min: i32, max: i32| -> Result<i32, BalanceError> {
        match q.player_level {
            None => Ok(min),
            Some(level) => {
                let x = level
                    .get()
                    .checked_add(boost)
                    .ok_or(BalanceError::OutOfRange {
                        what: "region stage (player level + boost overflows)",
                        value: i64::from(level.get()),
                    })?;
                Ok(clamp(x, min, max))
            }
        }
    };
    // The completed overrides: the largest wins.
    let overrides = {
        let space = cx.world.parts().space;
        field_opt(space, entry, "MissionOverrides")?.cloned()
    };
    let mut best: Option<i32> = None;
    if let Some(overrides) = overrides {
        for o in elements("MissionOverrides", &overrides)? {
            let (mission, min_v, max_v) = {
                let space = cx.world.parts().space;
                (
                    object("Mission", field(space, o, "Mission")?)?,
                    field(space, o, "MinGameStage")?.clone(),
                    field(space, o, "MaxGameStage")?.clone(),
                )
            };
            let Some(mission) = mission else { continue };
            if !q.completed_missions.contains(&mission) {
                continue;
            }
            let min = bound(sys, cx, &min_v, q.controller)?;
            let max = bound(sys, cx, &max_v, q.controller)?;
            let stage = to_player(min, max)?;
            best = Some(best.map_or(stage, |b| b.max(stage)));
        }
    }
    let stage = match best {
        Some(s) => s,
        None => {
            let (min_v, max_v) = {
                let space = cx.world.parts().space;
                (
                    field(space, entry, "MinDefaultGameStage")?.clone(),
                    field(space, entry, "MaxDefaultGameStage")?.clone(),
                )
            };
            let min = bound(sys, cx, &min_v, q.controller)?;
            let max = bound(sys, cx, &max_v, q.controller)?;
            to_player(min, max)?
        }
    };
    Ok(GameStage(stage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::fixture::{Fx, RegionData};
    use ow_core::notyet::Missing;

    /// Sanctuary's first-playthrough entry as the record reads it (NATIVE_PROGRESSION.md section 2): default 7..9;
    /// overrides after WelcomeToSanctuary 8..11 and after ThePhoenix 9..13; no boost.
    fn sanctuary(fx: &mut Fx) -> RegionData {
        fx.region_table(7, 9, &[(8, 11), (9, 13)], 0)
    }

    fn ask(
        fx_parts: &mut (AttributeSystem, crate::offline::OfflineWorld),
        r: &RegionData,
        stages: &mut RegionStages,
        pt: Playthrough,
        level: Option<i32>,
        completed: &[ObjRef],
    ) -> Result<GameStage, BalanceError> {
        let (sys, world) = fx_parts;
        let controller = world.controller();
        let q = RegionQuery::new(
            r.region,
            pt,
            level.map(ExpLevel::new),
            completed,
            Some(controller),
        );
        let mut cx = EvalCx::new(world);
        region_game_stage(sys, &mut cx, r.globals, stages, &q)
    }

    #[test]
    fn sanctuary_first_playthrough_clamps_the_level_to_seven_to_nine() {
        // Record: L = clamp(level at first query, 7, 9).
        for (level, want) in [(1, 7), (6, 7), (7, 7), (8, 8), (9, 9), (10, 9), (50, 9)] {
            let mut fx = Fx::new();
            let r = sanctuary(&mut fx);
            let mut w = fx.into_world();
            let mut stages = RegionStages::new();
            let got = ask(
                &mut w,
                &r,
                &mut stages,
                Playthrough::NORMAL,
                Some(level),
                &[],
            );
            assert_eq!(got.unwrap(), GameStage(want), "level {level}");
            assert_eq!(
                stages.get(r.region, Playthrough::NORMAL),
                Some(GameStage(want))
            );
        }
    }

    #[test]
    fn welcome_to_sanctuary_complete_gives_eight_to_eleven_and_the_largest_override_wins() {
        for (level, want) in [(5, 8), (8, 8), (10, 10), (11, 11), (15, 11)] {
            let mut fx = Fx::new();
            let r = sanctuary(&mut fx);
            let mut w = fx.into_world();
            let mut stages = RegionStages::new();
            let done = [r.missions[0]];
            let got = ask(
                &mut w,
                &r,
                &mut stages,
                Playthrough::NORMAL,
                Some(level),
                &done,
            );
            assert_eq!(got.unwrap(), GameStage(want), "level {level}");
        }
        // Both complete: 8..11 and 9..13; the larger clamp wins.
        let mut fx = Fx::new();
        let r = sanctuary(&mut fx);
        let mut w = fx.into_world();
        let mut stages = RegionStages::new();
        let done = [r.missions[0], r.missions[1]];
        let got = ask(
            &mut w,
            &r,
            &mut stages,
            Playthrough::NORMAL,
            Some(12),
            &done,
        );
        assert_eq!(got.unwrap(), GameStage(12));
    }

    #[test]
    fn the_first_query_wins() {
        // Record: "fixed per player and playthrough by the first time it is asked".
        let mut fx = Fx::new();
        let r = sanctuary(&mut fx);
        let mut w = fx.into_world();
        let mut stages = RegionStages::new();
        assert_eq!(
            ask(&mut w, &r, &mut stages, Playthrough::NORMAL, Some(7), &[]).unwrap(),
            GameStage(7)
        );
        // A later level and a completed override do not change the stored stage.
        let done = [r.missions[0]];
        assert_eq!(
            ask(
                &mut w,
                &r,
                &mut stages,
                Playthrough::NORMAL,
                Some(10),
                &done
            )
            .unwrap(),
            GameStage(7)
        );
        assert_eq!(stages.len(), 1);
        // Another playthrough is stored apart (TVHM uses the table's second row, here the same bounds).
        assert_eq!(
            ask(&mut w, &r, &mut stages, Playthrough::TVHM, Some(10), &[]).unwrap(),
            GameStage(9)
        );
        assert_eq!(stages.len(), 2);
        let listed: Vec<_> = stages.iter().map(|(_, p, s)| (p, s)).collect();
        assert_eq!(listed, [(0, GameStage(7)), (1, GameStage(9))]);
    }

    #[test]
    fn no_player_pawn_takes_the_minimum_and_a_boost_is_added() {
        let mut fx = Fx::new();
        let r = sanctuary(&mut fx);
        let mut w = fx.into_world();
        let mut stages = RegionStages::new();
        assert_eq!(
            ask(&mut w, &r, &mut stages, Playthrough::NORMAL, None, &[]).unwrap(),
            GameStage(7)
        );
        // An invented boost of 2 on the globals: level 6 + 2 = 8.
        let mut fx = Fx::new();
        let r = fx.region_table(7, 9, &[], 2);
        let mut w = fx.into_world();
        let mut stages = RegionStages::new();
        assert_eq!(
            ask(&mut w, &r, &mut stages, Playthrough::NORMAL, Some(6), &[]).unwrap(),
            GameStage(8)
        );
    }

    #[test]
    fn the_playthrough_row_is_clamped_to_the_table() {
        // A table with one row serves TVHM from that row ("clamped to the table").
        let mut fx = Fx::new();
        let r = fx.region_table_rows(&[(7, 9)]);
        let mut w = fx.into_world();
        let mut stages = RegionStages::new();
        assert_eq!(
            ask(&mut w, &r, &mut stages, Playthrough::TVHM, Some(20), &[]).unwrap(),
            GameStage(9)
        );
        // Two rows: TVHM reads the second (invented bounds 20..25).
        let mut fx = Fx::new();
        let r = fx.region_table_rows(&[(7, 9), (20, 25)]);
        let mut w = fx.into_world();
        let mut stages = RegionStages::new();
        assert_eq!(
            ask(&mut w, &r, &mut stages, Playthrough::TVHM, Some(8), &[]).unwrap(),
            GameStage(20)
        );
    }

    #[test]
    fn a_region_no_entry_names_keeps_the_default_and_stores_nothing() {
        let mut fx = Fx::new();
        let mut r = sanctuary(&mut fx);
        r.region = fx.other_region();
        let mut w = fx.into_world();
        let mut stages = RegionStages::new();
        assert_eq!(
            ask(&mut w, &r, &mut stages, Playthrough::NORMAL, Some(30), &[]).unwrap(),
            DEFAULT_STAGE
        );
        assert!(stages.is_empty());
    }

    #[test]
    fn uvhm_and_dlc_regions_are_not_yet() {
        let mut fx = Fx::new();
        let r = sanctuary(&mut fx);
        let dlc = fx.dlc_region();
        let mut w = fx.into_world();
        let mut stages = RegionStages::new();
        match ask(&mut w, &r, &mut stages, Playthrough::UVHM, Some(60), &[]) {
            Err(BalanceError::NotYet(n)) => assert_eq!(n.missing, Missing::NeedsRealGame(2)),
            other => panic!("expected NotYet, got {other:?}"),
        }
        let mut d = r.clone();
        d.region = dlc;
        match ask(&mut w, &d, &mut stages, Playthrough::NORMAL, Some(8), &[]) {
            Err(BalanceError::NotYet(n)) => assert_eq!(n.missing, Missing::NotRead),
            other => panic!("expected NotYet, got {other:?}"),
        }
        assert!(stages.is_empty());
    }

    #[test]
    fn ue3_clamp() {
        assert_eq!(clamp(5, 7, 9), 7);
        assert_eq!(clamp(8, 7, 9), 8);
        assert_eq!(clamp(10, 7, 9), 9);
        // Bounds out of order: min.
        assert_eq!(clamp(8, 9, 7), 9);
    }
}
