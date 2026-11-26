## 2026-11-26: The spawn's attribute starting values, and the links past them

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SPAWN-SETBASEVALUE, the analyst half of the clean room: another lane
implements it. Note: `docs/verification/NATIVE_SPAWN_SETBASEVALUE.md`, on main d118d2bb (probed on #807's head 0a9cfdaf). The native code was not read
by this lane; every native rule comes from existing notes. **Every rule is UNVERIFIED in the running game.** No code
changes.

- **What was found:**
  - the spawns' new stop is `AttributeInitializationDefinition.SetBaseValue` (NATIVE_ATTRIBUTES.md section 1, ported in
    #572), called by `WillowPawn.GetAttributeStartingValuesFromCharacterClass` from the AI mind's class step. Its
    preflight refuses because the starting value's attribute is an unloaded handle (the first is
    `GD_Balance_HealthAndDamage.AIParameters.Attribute_EnemyNonWeaponDamageMultiplier`);
  - it appears now because #803 gives the mind its `CharacterClass`: with that write removed, a local probe made no
    `SetBaseValue` call at all;
  - with the starting values in, the probe's link 3 enemy has 58.88 health instead of 117.76, and each melee swing deals
    69.73 instead of 13.95 (the port's numbers, with the shield and protection-timer stages stubbed).
- **Proposed (for the hub):** a set-up preload in the combat world, Off only and logged, of each spawnable AI class
  chain's starting-value attributes, their base-value definitions, and each attribute's context resolvers and value
  resolvers. It has the threshold-attribute preload's shape. The native is unchanged. `AttributeStartingValuesNew` is
  left `NotRead`.
- **Where the links go next** (the probe, each earlier stop stubbed and reverted):
  - both spawns: `ApplyGradeCustomizations`, then `UpdatePlayThroughMaterial` (IMPL-COMBAT-MUSIC), then complete;
  - both links: section 4.2's five plain reads, then `IsFriendlyFire`'s argument, which arrives as an object (the VM's
    interface cast);
  - link 3: `AdjustDamage`'s hit-region cap (`NotPorted`, the rule only outlined), then the unbound
    `GetDisplayNameAtGrade` and `NotifyKilledEnemy`;
  - link 4: the shield and protection-timer stages (`NotPorted`), Maya's class-level modifiers (the deferred player
    side), and three residency stops (the force-feedback waveform, the globals definition, `CombatMusicProperties`);
    then it passes;
  - link 13: the third swing reads `AttackInjuredMeleeDamageModifier` on the AI mind, and the packages declare it only
    on `WillowPlayerController`;
  - links 5 and 14 are blocked behind 3 and 13.
- **Found by the way:** #805's `NothingTouched` stand-in applies only with no collision service. With #807's `NoLevel`
  installed, turning `PlayerPawnDuringSetUp` on stops link 1 at `SetCollisionSize`'s touch rule.
- **Open questions for the hub:** the preload; the category lists; the interface value (VM cast or the native's check);
  `NothingTouched` under `NoLevel`; the combat world's globals preload and its two subobjects; the melee-injured
  attribute for an AI instigator.
- **Verified:** nothing in the game. The predicted stops come from the probe.
- **Review of #811 (fix round):**
  - the preload now has a discovery phase (the run's spawn kinds, a set-up copy of each archetype, its `AIClass` and
    `AttributeParentClass` chain loaded before their fields are read);
  - it walks the transitive closure `SetBaseValue`'s preflight walks (nested formulas and reached conditionals), with a
    bounded alternative whose remaining refusal is named as a predicted stop;
  - it says which loads the guarantee covers;
  - section 8's stop is a prediction for the set-up form (the probe loaded at the refusal);
  - Maya's down is a projection;
  - the index cell states the Rust Off binding and Reference;
  - the refusal kinds are written in prose.
