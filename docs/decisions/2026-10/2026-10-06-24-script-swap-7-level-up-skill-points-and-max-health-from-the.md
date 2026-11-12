## 2026-10-06: script swap 7, level-up skill points and max health from the script path (UNVERIFIED natives, one stand-in)

AI-assisted (Claude), implementer lane I4. The VM's `ExpLevelUp` now awards skill points through its own script: `EvaluateInitializationData`
is bound to the attribute evaluator (from NATIVE_ATTRIBUTES / NATIVE_PROGRESSION, UNVERIFIED), which now resolves the player's replication info
and the `PlayerExperienceLevel` condition, so the installed `INI_SkillPointsPerLevelUp` gives 1 point per level from level 5 and 0 below.
`RecalculateAttributeInitializedState` (called by `OnExpLevelChange`) is a labelled host-boundary stand-in: its body was not read, so it
evaluates the health pool's base maximum (the data's `Init_PlayerHealth`, 80 x 1.13^L, minimum 20) for the new level. The slice hands the host
`skill_points` and `max_health` events; the host compares them with its own numbers (both agree on real data, for example 8 to 9: 1 point,
240.323) but does not yet adopt them, because `UOpenWillowSkills` has no points counter or health setter. Not modelled: the class's `OnLevelUp`
health refill, respec, playthroughs above 1, DLC skill-point definitions. Checks: ctest 11/11, verify_packages 9/9, UE build succeeded, quest
suite 98/98 + resume 11/11 (was 96 + 11), door suite 16/16, no new stubs. Verified: automated suites only.

