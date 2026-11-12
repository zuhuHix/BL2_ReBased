## 2026-10-05: script swap 2: Fire mission experience through the script path (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, from NATIVE_PROGRESSION.md and NATIVE_MISSION_SCRIPT_BRIDGE.md only. The host's
own XP computation is gone: `UpdateMissionStatus(Complete)` → `ServerGrantMissionRewards` → `GetExperienceReward` +
`ExpEarn(amount, 4)` on the VM, then the pool update runs the script `ExpLevelUp` → `OnExpLevelChange`.

- New `src/progression.*`: a bounded attribute evaluator (constant, simple-math and global-level resolvers, value formula,
  conditional on `PlayThroughCount`, range and rounding; any other shape throws) and the experience curve.
  `GetExperienceReward` = `trunc(float(span × percentage × m))` at the mission's locked game stage (m = 1 first playthrough
  below 50; other cases throw "not implemented"). `ExpEarn` scales (taken as 1: not decoded), clamps to the maximum level's
  XP and never decreases a VM-side pool. Also from the bridge note: `GetGameStage`, `GetMaxExpLevel`,
  `GetExpPointsRequiredForLevel`, `GetCurrencyRewardType`, `GetCurrencyReward` (multiplier 0 only),
  `ShouldGrantAlternateReward`, `GetItemRewardsForPlayer` (empty rewards only; pool rolls logged as not implemented).
- Design: the VM owns the pool and level; the host keeps Maya's display, skill points, health and save, pushes its inputs
  (region stage, level, experience) before accept and turn-in, applies the `Experience` event and compares the `Level`
  event with its own level. The host's earlier formula remains only as the suite's oracle.
- VM fix: struct default tags fill a static array (`ArrayDim` > 1) element by element (before, the last tag replaced the
  whole field). `ProviderDefinitionPathName.PathComponentNames` became an array as a result; `providerPathLeaf`
  (`src/behavior.*`) reads its last name for the two users. The slice's 22 host events are unchanged.
- Real data: 395 XP at stage 8 (the amount confirmed in game on 2026-10-02), 316 at stage 7; level 8 → 9 by the script from
  27,900 XP. Turn-in stubs 19 → 14.
- Checks: CTest 11/11 (synthetic invented curve, locked stage, level-up, clamps; static-array struct case), packages 9/9,
  UE build Succeeded, quest suite first run PASS 81/81 (new check `script_pool_update_levels_up_to_host_level`) and resume
  PASS 11/11, door suite PASS 16/16, inventory suite PASS 49 / FAIL 0 / NOT_RUN 0 / KNOWN_DIVERGENCE 0. Sensitive file:
  `CMakeLists.txt` (`src/progression.cpp` added). UNVERIFIED: XP scales = 1, the conditional "all expressions hold",
  the BaseValueMode numbering, everything but the 395 amount.

