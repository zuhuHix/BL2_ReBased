## 2026-10-02: progression rules from the native notes in the tools and the host (UNVERIFIED in game)

AI-assisted. Implements `docs/verification/NATIVE_PROGRESSION.md` sections 1-4. Those rules were read from native code,
and none has been confirmed by running the game. Package parsing is unchanged: `src/` and `CMakeLists.txt` were not
touched.

- **Formula order.** `tools/weapon_recipe.py` (`formula_value`), `tools/weapon_stats.py` and `tools/loot_pools.py` now
  evaluate `Multiplier x (Level^Power + Offset)`. Before, they added the offset outside the multiplier.
  - A census of the base-game packages (local only) found 228 distinct enabled formulas. 19 of them have both a
    non-zero Offset and a Multiplier other than 1: enemy health and damage, enemy and world-discovery XP, melee damage,
    class-mod bonuses, three Soldier/Mercenary skill formulas, vehicle damage and the XP curve. DLC packages were not
    included.
  - No slice number changes. The slice gear recipes and manifest are byte-identical before and after. The loot display
    check is unchanged (2,505 of 2,554 agree). Maya's health formula has no offset, and the XP curve's offset cancels
    in the reward span.
- **Level curve and cap.** `UOpenWillowSkills::ExperienceForLevel` is now `max(0, trunc(60 x (n^2.8 + 7.33)) - 499)`,
  evaluated in single precision. It was `floor(60 n^2.8 - 60)`.
  - Changed thresholds: level 2 357 -> 358, level 5 5,375 -> 5,376, level 8 20,207 -> 20,208, level 9 28,125 -> 28,126.
    Level 46 stays 2,715,586, the one real-game threshold on record.
  - The level cap of 50 applies in `AddExperience`, `SetLevel` and save restore. DLC cap increments are a TODO.
  - Float and double evaluation differ by one point at levels 17, 22, 33, 42, 45, 47 and 49. Which value the game
    gives at those levels is not known.
- **Mission XP.** `MissionXp` is now `trunc(span x percentage)` on the integer curve. Before, it rounded a span computed
  on unrounded doubles.
  - Changed amounts: stage 8 396 -> 395, 9 484 -> 483, 10 579 -> 578, 11 682 -> 681. Stage 7 stays 316.
  - The mission level is now Sanctuary's region game stage, not the slice gear level. `tools/slice_values.py` decodes
    the playthrough-1 `RegionBalanceData` entry into `world.json` `values.xp.region_stage` (default 7..9,
    WelcomeToSanctuary 8..11, later overrides).
  - The host fixes the stage as `clamp(level + boost, min, max)`, using the largest completed override or else the
    default. It does this when the walker sets Maya's level at session start and keeps the value in the quest save.
  - A manifest without the block gets a logged STAND-IN with the note's bounds.
  - The suite's Maya starts at level 8, so the stage is 8 and the reward is 395.
- **Max health** already matched the note. `SLICE_WORLD_PLACEMENT.md` 2b now records that the 94 constant is never
  used (native reading).
- **Checks.**
  - CTest 10/10 and packages 9/9 pass.
  - Synthetic Python tests pass: weapon_recipe 8, weapon_stats 19, loot_pools 9, slice_world 17.
  - The `OpenWillow.Skills` automation test passes (curve values, synthetic order and truncation cases, cap).
  - Quest suite: 75/75 first run and 11/11 resume. New checks are `mission_level_is_region_stage_fixed_at_start`,
    `level_curve_matches_tool_integer_curve` and `resume_region_stage_from_save` (the stored stage 8 is kept where a
    fresh computation at the resumed level 11 would give 9). `xp_amount_is_candidate_formula_at_mission_level` is
    renamed `xp_amount_is_truncated_rule_at_region_stage`.
  - Door suite 16/16.
  - Every rule above is still unverified in game. The note lists the confirmations: turn-in XP 395 at stage 8,
    "next level at" 358 at level 2, and the first skill point at level 5.

