## 2026-10-07: script swap 7b, level-up pool rebase and refill on the VM; the host adopts the VM's health

AI-assisted (Claude), implementer lane I4, from `docs/verification/NATIVE_LEVEL_UP_ATTRIBUTES.md`. The VM controller now has a resource pool
manager with one health pool built from Maya's stock `HealthPoolDefinition`, seeded with the host's current health. `RecalculateAttributeInitializedState`
rebases every pool through the installed script `ResourcePool.CalculateBaseValues(true)` (base maximum from the data; current follows only a full pool),
replacing swap 7's stand-in. `OnExpLevelChange`'s pawn branch runs as script, so the more-than-1-second guard is the script's own. The refill is a labelled
stand-in for `BehaviorBase.RunBehaviors` over the class's `OnLevelUp` collection: the `Behavior_AttributeEffect` skills' plain-attribute effects are
applied to the VM's pools (the heal adds the effective maximum, then the pool clamps). `GetAWillowPawn` and `SetGameStage` are small stand-ins with no
note. The host adopts the VM's `health` event, so a level-up from half health ends full, as observed in game (lane L1); its own formula stays as the
oracle. Not modelled: the class-level damage modifiers and status resistances (0/1 constants for Maya), the action-skill cooldown reset, the shield
recharge boost, the natural-level-up weapon-damage scale. Checks: ctest 11/11, verify_packages 9/9, UE build succeeded, quest suite 98/98 + resume
11/11 (new `script_level_up_refills_health`), door suite 16/16, inventory suite PASS 49 FAIL 0. The refill rule is confirmed in game; the rest
is UNVERIFIED.

