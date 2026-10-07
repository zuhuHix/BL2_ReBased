# Native progression rules: mission XP, level curve, skill points, player health (2026-10-02)

AI-assisted. Behaviour notes written from a local reading of `Borderlands2.exe` in Ghidra, under the policy in
[LEGAL.md](../LEGAL.md) ("Analysing the executable") and [NATIVE_ANALYSIS.md](../NATIVE_ANALYSIS.md). Nothing below
is listing, pseudo-code or an address; functions are named by their registered native name or, for internal
routines, by what they do and which registered native reaches them. Script behaviour was read with
`ow-package --disasm`. Data values (constants of the installed `Startup.upk`) were decoded locally with the project
reader and are quoted only where a rule needs them. **Every rule here was `UNVERIFIED`** when written: it was read
from native code or script. Each section ends with the in-game observation that would confirm it. On 2026-10-02 the
mission XP (section 2), level curve and skill points (3) and maximum health base (4) were confirmed in game; the rest
stays `UNVERIFIED` ([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md)).

Field offsets were named with `tools/ghidra/class_layout.py`. That tool now also sizes Gearbox's
`FloatAttributeProperty` / `IntAttributeProperty` (4 bytes, value in place) and `ByteAttributeProperty` (1 byte);
before, it stopped at the first such field, which made `Controller`, `WillowPawn`, `WillowPlayerReplicationInfo` and
`BalanceModifierDefinition` unreadable. Every offset used below landed on a field of the expected type.

Host code cited is the working tree on top of commit `17a7664`.

## 1. How an `AttributeInitializationData` is evaluated

Everything below (XP curve, reward percentage, health, game stages) goes through one native evaluator, reached from
`AttributeInitializationDefinition.EvaluateInitializationData` and from every native that reads balance data. Input:
the 16-byte struct `{BaseValueConstant, BaseValueAttribute, InitializationDefinition, BaseValueScaleConstant}` and a
context object (plus an optional second context tried first).

1. **Base.** Start from `BaseValueConstant`. If `BaseValueAttribute` is set **and** the attribute's
   `ContextResolverChain` yields a context (every resolver in the chain must succeed; an empty chain yields none), the
   base becomes the attribute's value: its `ValueResolverChain` run in order, each resolver receiving the previous
   value. `NoContextNeededAttributeContextResolver` always succeeds (it returns the given context, or the engine
   globals when there is none), so attributes using it always win over the constant.
2. **Definition.** If `InitializationDefinition` is set:
   - if `ValueFormula.bEnabled`: `f = Multiplier × (Level ^ Power + Offset)` (each term is itself an
     `AttributeInitializationData`, evaluated recursively; the power is skipped when `Power` = 1). **The offset is
     added before the multiplier**, not after it;
   - else if `ConditionalInitialization.bEnabled`: `f` = the `BaseValueIfTrue` of the first `ConditionalExpressionList`
     entry whose expressions hold, else `DefaultBaseValue`;
   - `BaseValueMode` combines `f` with the base: value 0 (`BASEVALUE_InitializationDefSetsBaseValue`, the cooked
     default) replaces the base with `f`; values 1, 2 and 3 give base + f, base × f and f − base (which enum name
     belongs to 1..3 was not checked);
   - if `RandomVariance.bEnabled`, a random amount between `LowerBound` and `UpperBound` is added (integer-uniform when
     `bUseIntegerRandomization`).
3. **Scale.** Multiply by `BaseValueScaleConstant` (the struct default is 1).
4. **Clamp and round** (only when a definition is set): `RangeRestriction` min, then max (each only when enabled);
   then `RoundingMode`: 0 none, 1 nearest, 2 floor, 3 ceiling.
5. Arithmetic is single-precision `float`.

Host and tool difference: `tools/weapon_recipe.py:166` and `tools/weapon_stats.py:167` compute
`Multiplier × Level ^ Power + Offset` (offset outside the multiplier). The native reading puts it inside. This changes
every formula with a non-zero `Offset` and a `Multiplier` other than 1; the XP curve below is one, and the Sanctuary
slice values are unaffected only because the XP reward uses a difference of two curve points (the offset term cancels)
and the health formula has no offset. Worth a census of formulas with both terms before trusting other derived
numbers.

**Implemented 2026-10-02.** `tools/weapon_recipe.py` (`formula_value`), `tools/weapon_stats.py` and
`tools/loot_pools.py` now add the offset inside the multiplier (Python floats, not single precision). Census of the
base-game packages (local, `local/a2impl/formula_census.json`): 228 distinct enabled `ValueFormula` definitions, 33 with
a non-zero or attribute-valued `Offset`, 19 of those with a `Multiplier` that is not the constant 1 (enemy health and
damage per player or level, enemy and world-discovery XP, player and Roid melee damage, class-mod stat bonuses, two
Soldier and one Mercenary skill formulas, vehicle damage, and the XP curve). None of them feeds a slice number: the slice
gear (`tools/weapon_slice_gear.py`, all five recipes and the manifest), the loot display check (2,505 of 2,554 entries
agree, 1 differs, 48 unresolved, identical before and after) and Maya's health are unchanged; the XP curve's offset
cancels in the reward span. DLC packages were not included.

Confirmation: any formula with both a non-zero `Offset` and a non-unit `Multiplier` whose result is visible in the game
(for example a weapon card stat). A synthetic oracle with invented values: `{Multiplier 2, Level 3, Power 2, Offset 1}`
gives 20 under this reading and 19 under the tools' reading.

## 2. Mission XP reward: `MissionDefinition.GetExperienceReward`

Script signature: `GetExperienceReward(WillowPlayerController PC, bool bAlternate)` (second argument as read by the
native: a bool that selects `AlternativeReward` instead of `Reward`).

**Amount.** With `L` = the mission's level (below), `R(n)` = experience required for level `n` (section 3):

```
span   = R(L + 1) - R(L)                                  (integers)
amount = trunc( float( span × pct × m ) )
```

- `pct` is `Reward.ExperienceRewardPercentage` (or `AlternativeReward`'s) evaluated with the player controller as
  context (section 1). For the Fire mission it is `XPReward_02_Small`: 0.05 on playthrough 1 (the
  `tools/slice_values.py` decoding stands).
- `m` is a playthrough multiplier, read from `WillowGlobals.PlayThroughBasedBalanceModifiers` (one
  `BalanceModifierDefinition` per `PlaythroughToBalance`), with the player level taken from
  `WillowPlayerReplicationInfo.ExpLevel`:
  - playthrough index 2 or more (UVHM): the last `BalanceModifiers` entry whose `MinEffectiveLevel` ≤ the player's
    level, its `XPGainedFromQuestsMultiplier`;
  - otherwise, if the player's level is ≥ 50 and the playthrough-3 modifier exists:
    `ModifierToXPGainedTowardsNewLevelsInEarlierPlaythroughs[playthrough index]`;
  - otherwise **1.0**. The slice (playthrough 1, level < 50) is in this case.
- The product is computed in extended precision, stored as `float`, then **truncated** toward zero (not rounded).
- Optional objectives: for every entry of `ObjectiveDefs` with `bObjectiveIsOptional` that the mission tracker reports
  complete, the same formula with that objective's `OptionalExperienceRewardPercentage` is added (each term truncated
  on its own). If the mission has objectives but no mission tracker can be found, the whole reward is 0 (an edge
  case, not expected in play).

**Mission level `L`.** The level is the mission's **game stage** (`IIBalancedActor` game-stage slot), not its
`ExpLevel`. That getter, unless `bGameStageLocked` is set, recomputes the stage from `GameStageRegion` each time it is
asked and stores it (and the region's awesome level) on the mission; with no region it returns 0; with the lock set it
returns the stored `GameStage`. The Fire mission has `GameStageRegion = GD_GameStages.Zone1.Sanctuary` and no lock.

**Region game stage** (`WillowRegionDefinition.GetRegionGameStage`, internal routine):

1. Defaults: stage 1, awesome level 1.
2. Playthrough 3 (UVHM): the stage is a player-level-based value of at least 50 (overpower-aware; not read in detail).
3. Otherwise the table is the region's `DlcExpansion` table if it has one, else `GlobalsDefinition.RegionBalanceData`
   (entry chosen by playthrough index, clamped to the table), whose `BalanceDefinitions` are searched in order for the
   region.
4. **Stored stage first.** If the local player controller already holds a stage for this region and this playthrough,
   that stage is returned unchanged. A computed stage is stored on the player controller, so a region's stage is
   **fixed per player and playthrough by the first time it is asked** (`AWillowPlayerController.ResetGameStageForRegion`
   exists; when it is called was not read).
5. Otherwise, in the region's `RegionBalanceData`:
   - `boost` = `GlobalsDefinition.GameStageIncreaseAbovePlayer` (0 in this data), or the entry's
     `GameStageIncreaseAbovePlayer` when `bSpecifyBoostAbovePlayer`;
   - `clampToPlayer(min, max)` = `clamp(playerLevel + boost, min, max)`, where `playerLevel` is the local player's
     pawn's experience level; with no local player pawn it is `min`;
   - each `MissionOverrides` entry whose mission's status is **Complete** (4) gives
     `clampToPlayer(MinGameStage, MaxGameStage)`; the largest wins (and its `AwesomeLevel`);
   - with no completed override, `clampToPlayer(MinDefaultGameStage, MaxDefaultGameStage)` and `DefaultAwesomeLevel`.

Sanctuary, playthrough 1 (`Balance_P1_Zone1`): default 7..9; overrides after `M_Ep4_WelcomeToSanctuary` 8..11,
`M_Ep5_ThePhoenix` 9..13, then later episodes up to 28..30. So for the slice, `L` = `clamp(level at first query, 7, 9)`,
or `clamp(level, 8, 11)` if WelcomeToSanctuary was already complete then.

**Numbers** (playthrough 1, `pct` 0.05, `m` 1):

| stage L | span R(L+1) − R(L) | native amount (truncated) | host candidate (rounded) |
|---|---|---|---|
| 7 | 6,322 | 316 | 316 |
| 8 | 7,918 | **395** | 396 |
| 9 | 9,672 | 483 | 484 |
| 10 | 11,579 | 578 | 579 |
| 11 | 13,639 | 681 | 682 |

**Host today.** `OpenWillowSliceData.cpp:99-103` (`MissionXp`) rounds `pct × (required(L+1) − required(L))` computed
on unrounded doubles; `OpenWillowQuest.cpp:113` takes `L` from the gear manifest's mission-weapon `level` (8), and
`OpenWillowQuest.cpp:506` (`GrantExperience`) grants that. Differences: truncation instead of rounding (395, not 396,
at stage 8); integer curve points; `L` should be the Sanctuary region stage from the player's level when the region
was first queried (7..9), not the weapon's level. **Implement:** a region-stage function in the host (data:
`RegionBalanceData` for Sanctuary, stored once per playthrough in the save) and `MissionXp` = truncation of
`span × pct` on the integer curve.

**Implemented 2026-10-02** in the host: `FOpenWillowSliceData::MissionXp` (truncation on the integer curve from the
manifest's formula) and `RegionStage` (the playthrough-1 table decoded by `tools/slice_values.py` into
`values.xp.region_stage`; a labelled stand-in with the bounds above when an older manifest lacks it), fixed by
`UOpenWillowQuest::FixRegionStage` when the walker sets Maya's level at session start and kept in the quest save. The
quest suite's player starts at level 8, so the stage is 8 and the reward 395. Completed missions come from the quest's
completed set (the dependency fixture holds `M_Ep3_CatchARide` only, so the WelcomeToSanctuary override never applies
in the suite). Not modelled: playthrough multipliers, optional objectives, UVHM, `ResetGameStageForRegion`.

**Confirmation:** the XP number shown on the Fire mission's turn-in (or the experience bar delta in an sdk trace) for a
character whose Sanctuary stage is known; at stage 8 it should read 395. A level-7 and a level-10 character
entering Sanctuary for the first time should see the reward for stage 7 and stage 9 respectively.

**Confirmed in game on 2026-10-02** by calling `MissionDefinition.GetExperienceReward` on the Fire mission for a
level-8 Maya (mission game stage 8): 395 ([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md)). The stage at levels 7
and 10 and a real turn-in were not observed.

## 3. Experience curve and level-up

**Required experience** (internal routine used by the reward and the level-up code):

```
f(n)  = 60 × (n ^ 2.8 + 7.33)                       GlobalsDefinition.ExpPointsRequiredForLevel
R(n)  = max(0, trunc(f(n)) - trunc(f(1)))           trunc(f(1)) = trunc(499.8) = 499
```

The level is passed to the formula through a global attribute slot (`Attribute_ExperienceLevelForExpPointCalculation`
is a `GlobalAttributeValueResolver` reading slot 0, which the routine sets to `n`); the level-1 value is cached and
refreshed when the globals change. Values: R(2) 358, R(3) 1,241, R(5) 5,376, R(8) 20,208, R(9) 28,126, R(46) 2,715,586
(the one real-game threshold the host already cites; it matches).

**Level cap.** `WillowPlayerController.GetMaxExpLevel` = 50, plus each licensed level-cap DLC's increment, clamped to
50 + the sum of all increments.

**Level-up** (`ExperienceResourcePool.ApplyExpPointsToExpLevel`): while the experience pool's current value (a
`float`) is ≥ `WillowPlayerReplicationInfo.ExpPointsNextLevelAt` (> 0) and the level is below the cap, the script
event `WillowPlayerController.ExpLevelUp(bFeedback)` runs; several levels can be gained from one grant. If any level
was gained, `LevelUpCount` is incremented. How `ExpPointsNextLevelAt` follows the new level is script (not read here).

`ExpLevelUp` (script): if `ExpLevel` < `GetMaxExpLevel`: `ExpLevel` + 1; `GeneralSkillPoints` +=
`int(GlobalsDefinition.GeneralSkillPointsPerLevelUp)` evaluated after the increment; `SpecialistSkillPoints` likewise
(no data in this build); first-skill-point stat and messages; `OnExpLevelChange`. `GeneralSkillPointsPerLevelUp` is
`INI_SkillPointsPerLevelUp`: a conditional, **1 when `PlayerExperienceLevel` ≥ 5, else 0**. So the points earned at
level `n` are `max(0, n − 4)`.

**Host today.** `OpenWillowSkills.cpp:50-55` uses `floor(60 n^2.8 − 60)`: one point lower than the native curve at
most levels (357 vs 358 at level 2, 28,125 vs 28,126 at level 9; equal at 4 and 46). `OpenWillowSkills.cpp:63`
(`AddExperience`) levels up with no cap (comment says so). `OpenWillowSkills.h:38` (`EarnedPointsAt` = max(0, L − 4))
**matches**. **Implement:** `ExperienceForLevel(n) = max(0, trunc(f32(60 × (n^2.8 + 7.33))) − 499)` evaluated in
`float`, and the level-50 cap.

**Implemented 2026-10-02:** `UOpenWillowSkills::RequiredExperience` / `ExperienceForLevel` (single precision), the
cap `MaxLevel` = 50 in `AddExperience` / `SetLevel` / `RestoreProgression` (DLC increments left as a TODO). Float and
double evaluation of the curve differ by one point at levels 17, 22, 33, 42, 45, 47 and 49 (and most levels above 50);
which one the game's pow gives there is not known.

**Confirmation:** the experience bar's "next level at" value for a level-2 character (358 if this reading holds,
357 under the host's) or any low level in an sdk trace of `ExpPointsNextLevelAt`; first skill point on reaching
level 5.

**Confirmed in game on 2026-10-02** by calling `GetExpPointsRequiredForLevel` for levels 1-80: the single-precision
form reproduces every level from 1 to 59 (double is one point high at 17, 22, 33, 42, 45, 47, 49); four levels above
the base cap (60, 68, 74, 79) differ by one. `ExpPointsNextLevelAt` and skill points (`max(0, L − 4)`, spent +
unspent) were read at levels 2, 8, 17 and 70 and agree ([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md)).

## 4. Player maximum health

`CharClass_Siren.HealthPoolDefinition` → `HealthPool.BaseMaxValue` → `Init_PlayerHealth`: a `ValueFormula` with
`Multiplier` = {constant 94, attribute `Att_UniversalBalanceMultiplier_HealthShields`}, `Level` = attribute
`Att_UniversalBalanceScaler`, `Power` = {constant 1, attribute `PlayerExperienceLevel`}, no offset, minimum 20, no
rounding.

**The constant question is settled by section 1:** both balance attributes use `NoContextNeededAttributeContextResolver`,
whose resolution always succeeds, so the attribute value (a constant resolver: **80**, scaler **1.13**) replaces the
94. `PlayerExperienceLevel` resolves through the player's replication info; it falls back to the constant 1 only when
that context cannot be found.

```
maxHealth(L) = max(20, 80 × 1.13 ^ L)        (float)
```

L1 90.4, L5 147.4, L7 188.2, L8 212.7, L10 271.6. Skills, class mods, relics and other attribute modifiers on the
health pool are applied on top of this base and were not read.

**Host today.** `OpenWillowSliceData.cpp:94` (`HealthForLevel`) computes `max(min, multiplier × scaler^L)` with the
manifest's 80 / 1.13 / 20, and `OpenWillowWalker.cpp:107` (`RefreshHealthForLevel`) applies it on level change.
**Matches**; the "94 vs other" note in `SLICE_WORLD_PLACEMENT.md` 2b can be resolved to 80 (still `UNVERIFIED`
in-game). Not checked: whether a level-up refills current health (the host does).

**Confirmation:** Maya's maximum health on the HUD with no health-modifying gear or skills: 90 at level 1, 147 at
level 5, 212 at level 8 (displayed rounding unknown).

**Confirmed in game on 2026-10-02** by reading the health pool's `MaxValueBaseValue` through the SDK: 102.152 (L2),
212.6755 (L8), 638.886 (L17), 415,509.44 (L70, UVHM), each `80 × 1.13^L` to float precision. The HUD shows more
(429 at L8) because the profile's Badass Rank skill modifies the pool ([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md)).

**Confirmed in game 2026-10-07 (lane L1):** a level-up **refills current health to the new maximum** (214.5 of 429.1 became 484.8 of 484.8, and two more level-ups
the same way), it does not keep the fraction or the absolute value; the new maximum (base `80 x 1.13^L`, times the profile's Badass modifier) is written inside
`RecalculateAttributeInitializedState`, and the refill follows inside `OnExpLevelChange`; skill points +1 at 8 to 9, 9 to 10, 10 to 11; the shield's current and maximum
are not set, its recharge rate is raised by half the maximum per second for 4 s; a natural level-up also adds a 30 s weapon-damage scale of +1.0; observed by hooking the
level-up functions and reading the pools around three `ExpEarn` level-ups ([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md), "lane L1").

## What was not read

> Update (2026-10-05): the script side of the turn-in, `ExpEarn` and the level-up trigger are now read in [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) (it also corrects the `ExpLevelUp` parameter name to `bCheated`).

The script side of the turn-in (who calls `GetExperienceReward` and adds the amount to the experience pool); how
`ExpPointsNextLevelAt` is refreshed; the UVHM stage function; the enum names of `BaseValueMode` 1..3; health refill on
level-up; attribute modifiers on pools.
