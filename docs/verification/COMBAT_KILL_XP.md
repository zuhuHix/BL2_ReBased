# Kill experience on the VM: the award and the credit of a bullymong kill (plan step S-d, 2026-10-08)

AI-assisted (Claude), implementer lane Sc. Worked from the own-words note [NATIVE_COMBAT_GROUNDWORK.md](NATIVE_COMBAT_GROUNDWORK.md) (section 5, G24), the live rows of
[COMBAT_GOLDEN_TRACE.md](COMBAT_GOLDEN_TRACE.md) ("Open-rule checks", lanes L5/L5b, and lane L3) and the installed data and script. No decompiler output was read. Every rule below is
`UNVERIFIED` as a statement about the native code unless a row says which live number it reproduces. It builds on the death chain of [COMBAT_DEATH.md](COMBAT_DEATH.md): the installed
`WillowGameInfo.Killed` script calls the two natives.

## What runs where

`WillowGameInfo.Killed` (script) calls `WillowExperiencePipeline.CalculateExperiencePointsForKill(Killer, KilledPawn, bKilledByCritical, DamageSource, DamageType)` and, through
`AwardCombatExperience` (script, once per victim), `WillowExperiencePipeline.AwardCombatExperienceToParty(points, Killer, KilledActor)`. Both are now bound (`src/combat_xp.cpp`):

- **The award** (what `CalculateExperiencePointsForKill` returns, for the whole party) is **read from the data**: the game's globals (`GD_Globals.General.Globals`) hold
  `BaseEnemyExperienceFormula`, an attribute initialisation whose base value is the killed pawn's `ExperiencePointMultiplier` attribute and whose definition (`Init_BaseEnemyExperience`) scales it
  by `Multiplier x (Level + 4)`, the multiplier being `Init_EnemyExperience_PerPlaythrough` (a condition on the playthrough count: 10 on the first, 11 on the second, else 10) and the level the pawn's
  `ExpLevel`. The existing evaluator (`src/progression.*`) evaluates it with the killed pawn as context; the closed form `mult x 10 x (level + 4)` of the plan is therefore **not coded**, it is what
  the data gives. The critical factor is the killer's `ExpScaleForCriticalHitKills` attribute (default 1), and the lists of damage-source / damage-type factors (`DamageTypeExpModifiers`) are empty in the
  data; a non-empty list, a vehicle victim and a killer in a vehicle are reported as not implemented.
- **The credit** (`AwardCombatExperienceToParty`): for each replication info in `WorldInfo.GRI.PRIArray` whose owner is a player controller, in list order, the share is `(KillerExpBonus + ExpWeight) x points`
  for the killer and `ExpWeight x points` for the others (the `ExpAwardWeights` row at position N - 1, N = the game's `EffectiveNumPlayers`; outside the table - a game at its default 0 - bonus 0 and weight 1),
  then multiplied by the `ExpScaleByLevelDifference` entry at position `min(|d|, length) - 1` (`HigherLevelEnemyExpScale` when the victim is above the player, `LowerLevelEnemyExpScale` below, nothing when
  equal; d is the player controller's `ExpLevel` against the victim's), the combat multiplier (1.0: first playthrough, below the cap, not coded), **truncated toward zero** in single precision, and handed to the
  controller's script/native `ExpEarn(amount, 1, 1)`. All three tables are read from the globals object at the time of the call.
- **The pawn's multiplier:** the stock spawn writes the AI class's starting values into the pawn's attributes (`ApplyAttributeStartingValues`, unread). `CombatSystem::adopt` now does the one the award
  needs: the class's `Attribute_ExperienceMultiplier` entry (a `GD_AI_Balance.XP.XPMultiplier_0N_*` definition: Chump 0.5, Normal 0.75, NormalHigh 1.0, Tough 1.25, ...) is evaluated and written to the pawn's
  `ExperiencePointMultiplier` and its base value. UNVERIFIED reading; the live pawns read the same values (COMBAT_GOLDEN_TRACE.md).

## The player and the pool: the mission bridge

The credit goes through the **existing** player experience pool and level-up chain (swaps 2, 7, 7b: `src/mission_script.*`). A `--combat-run` with a `maya:<level>[:<experience>|:short<n>]` step is built over the
bridge: the player is the bridge's controller (`ExpEarn` is its native, the pool a C++ float, the level-up the installed `ExpLevelUp` script run by `tick`), the game's replication info lists its
`PlayerReplicationInfo` (new: the bridge adds it to `GRI.PRIArray` with the controller as its `Owner`; the slice's output is unchanged by it, see the gates in `local/p2/Sc/handoff.md`). The player's level is an input
(`maya:10` between two kills changes it); `tick:<s>` is the host's tick (time passes and the pool's per-frame update runs). `players:<n>` sets `EffectiveNumPlayers` (the VM has one local player; it is for the
synthetic tables). `killtag:<text>` appends a tag to the ids of the following kill rows. A run without `maya` keeps the earlier `player:<level>` controller, whose `ExpEarn` is not bound: the award is still
computed and the event list shows the amount passed, but there is no credit to measure.

Rows: each kill writes `kill.<class>.L<victim level>.p<player level>[.<tag>]` = `{"award": n, "credit": n}` into the run's `rows` (`credit` = what the pool gained, the delta of the experience pool around the hit;
the same id twice in a run becomes `<id>.a`, `<id>.b`), which `tools/verify_combat_pawn.py --vm` reads.

## Results against the live rows

`local/p2/Sc/final_rows.sh` (one run, 1 min 40 s: the first spawn loads the code packages) and `python tools/verify_combat_pawn.py --vm`:

| Row | Live (award / credit) | VM |
|---|---|---|
| baby bullymong L6, player 8 | 50.0 / +35 | 50 / 35 |
| adult L7, player 8 | 137.5 / +123 | 137.5 / 123 |
| baby L6, player 10 (twice) | 50.0 / +7 | 50 / 7, 50 / 7 |
| regular L6, player 10 | 75.0 / +11 | 75 / 11 |
| adult L5, player 8 (lane L3) | 112.5 / +45 | 112.5 / 45 |
| baby L6, player 10 (lane L3) | 50.0 / +7 | 50 / 7 |

All seven kill rows PASS (relative tolerance 1e-3; credits are integers), together with the four health rows and three per-level bases of the same run (14 PASS, 0 FAIL, 15 MISSING: the
arrival and ratio rows belong to plan step S-e). These are fits to the live numbers through the data chain, not a reading of the native; the lower-level table entries d = 1 to 4 are the only ones the
live record covers.

**A kill that crosses a level boundary** (`local/p2/Sc/xp_levelup.sh`): Maya at level 8 with 10 points to go (`maya:8:short10`, experience 28116 of 28126), a baby L6 killed: the credit is 35, the pool 28151;
the next `tick` runs the installed `ExpLevelUp`: level 9, one skill point, and the health pool is **rebased by the existing swap 7b chain**: maximum 212.676 (80 x 1.13^8) -> 240.323 (80 x 1.13^9), the full pool follows
(health 240.323 of 240.323), the next requirement 37798. The live L5b kills never crossed a boundary, so this is the VM's chain (observed live for a level-up on its own in lanes L3/L5), not a kill row.

## Not verified, and things to know

- Not covered by any live row: equal levels, a victim above the player (`HigherLevelEnemyExpScale`), a difference of 5 or more, more than one player (the weights and the killer bonus), playthrough 2 and above, the
  critical factor. The first four are checked only by the synthetic test (`tests/combat_test.py`, a toy globals object with invented tables and a toy `ExpEarn`).
- **A disagreement with the note, not resolved:** NATIVE_COMBAT_GROUNDWORK 5.1 and the plan write the award as `max(10, ...)` ("range restriction minimum 10"). In the installed `Init_BaseEnemyExperience` the
  minimum restriction is **not enabled** (`bEnableMinValueRestriction` false, the minimum value 10 is present). The evaluator follows the data, so there is no floor. No real class reaches it (the smallest multiplier is 0.5,
  and 0.5 x 10 x (level + 4) is at least 25 from level 1), so no row distinguishes the two readings.
- The party loop's replication list is the VM's `PRIArray` (the bridge's one player, or the `player` steps' controllers); a controller that is not a `WillowPlayerController` is skipped, as the note says.
- `WillowGameInfo.EffectiveNumPlayers` is 0 at the class default; "outside the table" gives the single-player row, so the result is the same as N = 1 (and `players:1` gives the identical credits).
- The floating "+N" notification and `ExpEarn`'s own side effects are the bridge's (swap 2 notes); `ExpEarn`'s scales (`ExpCombatPointsScale`, `ExpAllPointsScale`) are taken as 1 there (UNVERIFIED, unchanged).
- The `bAwardedExperienceForKill` guard and the "no killer" case (`bAwardKillsToAllPlayers`) are the installed script's; the VM's runs have a killer.

## Reproduce

```
cooked="$OPENWILLOW_BL2/WillowGame/CookedPCConsole"
local/p2/Sc/final_rows.sh      # needs the game; writes local/p2/Sc/final_rows.json
python tools/verify_combat_pawn.py --vm local/p2/Sc/final_rows.json
python tests/combat_test.py build/Release/ow-package.exe     # synthetic: the toy tables (45, 33 / 7, 60, 30) and the toy death
```
