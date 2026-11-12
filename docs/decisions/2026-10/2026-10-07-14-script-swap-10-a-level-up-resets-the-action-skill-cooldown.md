## 2026-10-07: script swap 10, a level-up resets the action-skill cooldown

AI-assisted (Claude), implementer lane I7 (work queue item 2). Observed in game (lane L3, `COMBAT_GOLDEN_TRACE.md` section E): `ExpLevelUp(false)`
set the cooldown to 0 within the same call, both at the 13.0 maximum during a Phaselock hold and at 7.99 during the drain.

Read from the data: the effect is on `OnLevelUp`, not `OnLevelUpNaturally`. It is the second effect of the first level-up skill: Scale of the
cooldown pool's `CurrentValue` by the pool's minimum, which is 0 for Maya.

What changed:
- **VM.** The VM builds a minimal `ResourcePool` over the class's `SkillCooldownPoolDefinition`, with the minimum read from the data, so the
  effect's value comes from the data and is not assumed. The pool is kept out of the manager, whose rebase would evaluate a designer-attribute
  maximum. The effect is handed to the host as the new event `action_skill_cooldown` (modifier type, value), after `health`.
- **Host.** `ModifyPhaselockCooldown` applies the modifier to the Phaselock cooldown it keeps, as an offset on its drain model that the next
  cast clears. A running cast is not touched. The host has no handler for the game's `ActionSkillCooldownComplete`, so it only logs completion.

UNVERIFIED:
- that the reset also follows a not-natural level-up (the script runs `OnLevelUp` for both; the trace has natural ones only);
- the VM pool construction;
- a reset during a held cast in the host (only logged, not tested).

Tests: synthetic scenario S10 (an invented minimum of 0.25, so the value is visibly the data's) and the quest checks
`level_up_resets_action_skill_cooldown` and `action_skill_cooldown_modifier_scales_then_resets_host_cooldown`.

Also today (docs only): NATIVE_DAMAGE_DEATH section 10.4 now records the live station-path fee timing (11.8 s after arrival, lane L3) in place of
"about 2 s".

Checks (rerun by the orchestrator): ctest 11/11, verify_packages 9/9, event order self-test and 18/18, UE build succeeded, quest 105/105 + 11/11,
mover 16/16, inventory 49 PASS / 0 FAIL.

