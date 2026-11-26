# The spawn's attribute starting values, and where links 3, 4, 5, 13 and 14 go next (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SPAWN-SETBASEVALUE (decision 2026-11-26-68), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/sbv/` and
`local/analysis/l1314/probe/`. The note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED in the running game.**

**Why.** With #807's collision stand-ins (decision 2026-11-26-64), links 3, 4, 5, 13 and 14 get past `SetPhysics`'
floor trace. They then stop inside both spawns at `AttributeInitializationDefinition.SetBaseValue`: "a definition it
names is not loaded (a load is a write)". This note says which call that is, why it appears now, what the spawn does
after it, and where each link goes next with every earlier stop stubbed.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **Existing notes reused, not re-read:**
  - NATIVE_ATTRIBUTES.md section 1 (`EvaluateInitializationData` and `SetBaseValue`; ported in #572,
    `crates/ow-world/src/world/set_base_value.rs`);
  - NATIVE_NOUN_ATTRIBUTES.md section 1 (the two starting-value scripts);
  - NATIVE_COMBAT_MUSIC_AND_AFTER.md sections 3 to 7 (#801);
  - NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md section 4.2;
  - NATIVE_DAMAGE_DEATH.md section 1;
  - NATIVE_LINK34_AFTER_MIND.md section 4;
  - NATIVE_PAWN_INTERACTION_PROXY.md (#802 and #805);
  - NATIVE_WILLOW_GLOBALS.md section 6.1.
- **The script** (`research/script_disasm.py`): `Engine.Pawn.TakeDamage`, for the argument it passes to
  `GameInfo.IsFriendlyFire`.
- **The native code:** not read by this lane. Every native rule below comes from the notes listed.
- **Package tables** (`ow-package <package> --names`, `--exports`): which class declares
  `AttackInjuredMeleeDamageModifier`.
- **A local probe** (temporary code, reverted, never committed):
  - an installed Off strict `--slice-gate` under the machine lock, on #807's head 0a9cfdaf;
  - with IMPL-COMBAT-MUSIC's commit (43ad5ef7) and #805's two code commits (f0413d0f, ab2fc1d3) laid over it;
  - printing the script stack and the handles at the refusal;
  - then stubbing each stop in turn (section 6 lists the stubs).

## 2. Which `SetBaseValue` this is

It is `AttributeInitializationDefinition.SetBaseValue(DestAttribute, BaseValue, ValueContextSource,
AttributeContextSource)`: the attribute starting value of NATIVE_NOUN_ATTRIBUTES.md section 1. It is not a resource
pool's. The port's native (#572) evaluates `BaseValue` with the value context, sets the attribute's base on its context,
and returns the setter's bool. The setter runs the change notification (no `bSuppressNotify`, so the script default).

**The call chain** (the probe's script stack at the refusal, on both spawns):
1. `WillowMind.InitializeCharacterClass`;
2. `WillowMind.ApplyCharacterClassDefaults`;
3. `WillowPawn.ApplyAttributeStartingValues`;
4. `WillowPawn.GetAttributeStartingValuesFromCharacterClass`;
5. then `SetBaseValue`, once per starting value, with the AI pawn as both context sources.

**The refusal** is the port's preflight (NATIVE_ATTRIBUTES.md section 1, #572):
- the call names an unloaded definition handle, and a load is a write, so the preflight refuses before anything changes;
- here the `DestAttribute` is `GD_Balance_HealthAndDamage.AIParameters.Attribute_EnemyNonWeaponDamageMultiplier`, an
  `AttributeDefinition` the store holds only as a handle.

**Why it appears only now.** The script walks the controller's character class (NATIVE_NOUN_ATTRIBUTES.md section 1).
Until #803, the adopt left the mind's `CharacterClass` None, so the walk found no class and made no call. #803 writes
the pawn's `AIClass` there (the hub's ruling on #796), so the AI class's starting values now run. The probe confirms
it: with that one write removed, the spawns made no `SetBaseValue` call at all.

**What it changes.** In the probe, with the starting values in (and #803's class-level part), the link 3 enemy's
health is 58.88 instead of 117.76. Each of link 4's and link 13's melee swings deals 69.73 instead of 13.95. These are
the port's numbers with the stubs of section 6, not observations of the game.

## 3. Off answer: a set-up preload of the starting values' definitions

**Proposed** (the same shape as `equip_run::preload_threshold_attributes`, whose destinations are also
`SetBaseValue`'s; hub ruling needed, open question 1). At the combat world's set-up, in Off, before any step writes,
for each AI pawn archetype the run can spawn:
1. take the archetype's `AIClass` and walk it, then its `AttributeParentClass` chain, as the script does;
2. on each class, read its `AttributeStartingValues` entries. For each, read the entry's `Attribute`, and the
   `BaseValueAttribute` and `InitializationDefinition` of its `BaseValue`;
3. also read the archetype's own `AttributeStartingValues` the same way;
4. for each attribute found, also read:
   - its `ContextResolverChain` entries;
   - its `ValueResolverChain` entries up to the first None;
5. load each unloaded one through the store, and log it as a harness input: "preload <path> (a SetBaseValue definition
   the game keeps resident; UNVERIFIED)";
6. a failed load is logged and left, so the native's own refusal shows it later.

**Declarations:**
- the fields must be declared: `AIClass` an object, `AttributeParentClass` an object, `AttributeStartingValues` an array
  of `AttributeStartingValue` structs, and `Attribute` an object on the entry;
- `BaseValue` must be an `AttributeInitializationData`, read with the evaluator's own data reader;
- an undeclared field or a wrong kind is a `VmError::Script`, and nothing is loaded.

**What is not covered:**
- `AttributeStartingValuesNew` (the category lists). The script walks them after the plain list, but their entry layout
  is not read here. A class whose `AttributeStartingValuesNew` is declared and non-empty gives the preload's `NotYet {
  NotRead }`, "the starting-value categories' layout", before anything loads. In the probe the plain lists sufficed;
  whether the slice's classes have category entries was not checked (open question 2).

**The native is unchanged:** `SetBaseValue`'s preflight keeps refusing an unloaded handle, so a definition the preload
missed still shows. **Reference:** nothing changes; the C++ does not implement `SetBaseValue`.

**In the probe** the two spawns between them named three starting-value attributes, and nine objects were loaded:
- `Attribute_EnemyNonWeaponDamageMultiplier`, with a `WillowMindAttributeContextResolver` and an
  `AIParameterValueResolver`;
- `GD_Balance_Experience.Attributes.Attribute_ExperienceMultiplier`, with a `PawnAttributeContextResolver` and an
  `ObjectPropertyAttributeValueResolver`;
- `Attribute_DamageToVehiclesOnCollisionMultiplier`, with the same two resolver classes as the first.

## 4. What the spawn does after it

Past the starting values each spawn goes on with the steps the existing notes read:
1. the class step's recalculation;
2. the balance step of #788, which reaches `BaseBalanceDefinition.ApplyGradeCustomizations` and then
   `WillowAIPawn.UpdatePlayThroughMaterial` (NATIVE_COMBAT_MUSIC_AND_AFTER.md sections 3 and 4). IMPL-COMBAT-MUSIC
   implements both (43ad5ef7). Until it lands, both spawns stop at the first as an unbound native;
3. with both in, **both spawns complete** in the probe.

## 5. Past the spawns: the hit and the swing

Both links then pass through `Pawn.TakeDamage`. Its first new needs:
- **the five plain reads of NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md section 4.2:**
  - `GetBarrelSourceTime`;
  - `GetPlantSourceTime`;
  - `GetCanInflictFriendlyFire`;
  - `IsUsingWalkingPhysics`;
  - `IsPlayingAny`.

  They are read there but not ported. `DamagePipeline.GetBarrelSourceTime()` is the first unbound one on both links.
- **`GameInfo.IsFriendlyFire`'s second argument.** The script fills `InstigatingTargetable` with a dynamic cast of the
  instigator's pawn to the `ITargetable` interface. The VM's cast keeps the value an object when it succeeds, so the
  interface-typed local holds a plain object. #798's argument check (review finding 6) accepts only an interface value
  or None, so it is a `VmError::Script`, "InstigatingTargetable holds Object, not an interface", on both links. The
  argument is the player's pawn on link 3 and the AI pawn on link 4. The fix is a VM or native question (open
  question 3).

**Link 3** (its `hit`), past those, meets in order:
1. **`AdjustDamage`'s hit-region cap** for `GD_PrimalBeastShared.HitRegions.HitRegion_Body`, whose `bTrackDamage` is
   set. This is the port's `NotPorted`. The rule (the region consumer's `AddDamageToHitRegion`) is only outlined in
   NATIVE_DAMAGE_DEATH.md section 1, stage 13. **A read for lane F.** It stops after partial writes, and because the
   run's stop is sticky it also masks link 4's spawn;
2. **`BaseBalanceDefinition.GetDisplayNameAtGrade(int)`**, unbound and not read (#801 section 7 predicted it);
3. **`WillowPlayerController.NotifyKilledEnemy(string)`**, unbound and not read (#799's note names it).

**Link 5** runs on link 3's kill, so it is blocked by link 3 throughout.

**Link 4** (its `swing`), past section 5's first two items, meets in order:
1. `AdjustDamage`'s shield stage for a victim with a shield pool (stage 10, Maya has one) and the protection timer
   (stage 15, a player hit by an AI). Both are read in NATIVE_DAMAGE_DEATH.md section 1 but `NotPorted`;
2. **Maya's class-level modifiers.** The resistance reads her `NormalImpactDamageModifier` as 0 with a base of 0. That is
   #803's deferred player side: the class-level part needs the controller's `Pawn` during the set-up, behind
   `PlayerPawnDuringSetUp`, which is off. Turning it on needs #805's three natives, and then:
   - #805's `SetCollisionSize` stand-in (`NothingTouched`) applies only in a world with **no** collision service;
   - since #807 installs `NoLevel`, link 1 then stops at "the touch update (its overlap rule is not read)" (open
     question 4). In the probe, a `NoLevel` world taking the stand-in let link 1 pass again;
3. **`DoForceFeedbackWaveform`'s waveform**, `GD_Globals.General.Globals.ForceFeedbackWaveform_0`, not loaded. It is a
   subobject of the globals definition;
4. **the globals definition's first load** ("which numbers the challenge list"). The combat world does not preload the
   globals definition the way the Fire driver does (`slice::unlock_setup`, NATIVE_WILLOW_GLOBALS.md section 6.1);
5. **`WillowGame.CombatMusicParameters`**, the globals definition's `CombatMusicProperties`, not loaded (read by
   IMPL-COMBAT-MUSIC's manager creation).

With 3 to 5 loaded (open question 5), **link 4 passes** in the probe.

**Link 13** (three swings, on link 4's world):
- at 69.73 a swing, Maya goes from 212.68 to 142.94 (link 4), to 73.21, to 3.48. These numbers are with stage 10 and
  stage 15 stubbed out, so her shield and the protection timer are not applied. They are the port's, not the game's;
- on the third swing she is below 30% of her maximum, so `AdjustDamage` reads the instigator's
  `AttackInjuredMeleeDamageModifier` (NATIVE_DAMAGE_DEATH.md section 1, stage 2);
- the port reads it on the AI's `WillowMind`. The packages declare it only on `WillowPlayerController`, so the port's
  `NotYet { NotRead }` follows: "the attribute WillowMind.AttackInjuredMeleeDamageModifier: the VM object's class does
  not declare it" (open question 6);
- past it, that swing would take Maya below 0, the first time a probe has seen her go down on link 13.

**Link 14** needs link 13's down, so it is blocked by link 13.

## 6. The probe's stubs, in order

All were temporary and reverted:
1. the three attributes and their resolvers, loaded at the refusal (section 3's set);
2. IMPL-COMBAT-MUSIC's implementation of `ApplyGradeCustomizations` and `UpdatePlayThroughMaterial` (43ad5ef7);
3. section 4.2's five reads, bound by that section's rules;
4. `IsFriendlyFire` accepting an object as its second argument;
5. every `NotPorted` stage of `AdjustDamage` skipped (the hit-region cap, the shield stage, the protection timer);
6. `GetDisplayNameAtGrade` answering an empty name;
7. #805's two code commits, `PlayerPawnDuringSetUp` turned on, and `NothingTouched` taking a `NoLevel` world;
8. the waveform loaded at its call;
9. the globals definition preloaded as the Fire driver does, and its `CombatMusicProperties` loaded.

With all of them, the probe scored 9 ok / 14 (X = 10.75/14 = 76.8%): link 4 passes, link 3 stops at
`NotifyKilledEnemy`, and link 13 at the attribute.

## 7. Tests (synthetic fixtures, for the implementing lane)

1. **The preload walk:** a toy AI class with a parent class, each with one starting value whose attribute, base-value
   attribute and initialization definition are unloaded handles, and an attribute with a context resolver and two value
   resolvers, the second followed by None and a third. Expected:
   - every named object loaded, the third value resolver not;
   - one harness line each, in walk order (parents first);
   - a second run loads and logs nothing.
2. **The archetype's own list:** a starting value on the archetype alone is preloaded too.
3. **Declarations:** an undeclared `AttributeStartingValues`, or one of another kind, is a `VmError::Script` with
   nothing loaded.
4. **Categories:** a declared, non-empty `AttributeStartingValuesNew` is the `NotRead`, decided before any load.
5. **A failed load** is logged, left, and the run goes on; the native's later call refuses with its own text.
6. **Reference:** the preload does nothing and logs nothing.
7. **The effect:** after the preload, an Off spawn of the toy pawn writes the starting value's base on its context,
   and without it the native refuses with `UNLOADED` (a test of the existing native's preflight through the spawn).

## 8. Predicted stop per link

The installed Off strict `--slice-gate`, after section 3's preload alone (nothing else of section 6; measured by the probe
on #807's head 0a9cfdaf, the base of this note):
- **links 3, 4 (and 5, 13, 14 blocked behind them):** both spawns stop at the unbound
  `BaseBalanceDefinition.ApplyGradeCustomizations` (until IMPL-COMBAT-MUSIC lands);
- with IMPL-COMBAT-MUSIC in too, both at the unbound `DamagePipeline.GetBarrelSourceTime()`;
- then the `IsFriendlyFire` argument (open question 3), and then each link as section 5 lists.

The score stays 8 ok / 14 until link 4's section 5 items are in.

## Open questions

1. **The preload:** may the combat world preload the starting values' definitions at set-up, as section 3 proposes (the
   threshold-attribute preload's shape)? Or is a load inside the spawn acceptable as a counted, logged write?
2. **`AttributeStartingValuesNew`:** read the category entries' layout (lane F), or keep section 3's `NotRead`?
3. **The interface value:** should the VM's dynamic cast and interface cast to an interface class yield an interface
   value? Or should the natives that declare an interface parameter (here `IsFriendlyFire`) accept an object whose class
   implements the interface? The first is closer to UnrealScript; the second is local to #798's check.
4. **#805 under #807:** should `NothingTouched` also apply to a collision service with no geometry (`NoLevel`)? It needs
   a ruling before `PlayerPawnDuringSetUp` is turned on in the combat world.
5. **The combat world's globals:** preload the globals definition as the Fire driver does, plus its two subobjects the
   swing reaches (the force-feedback waveform and `CombatMusicProperties`)?
6. **`AttackInjuredMeleeDamageModifier` for an AI instigator:** the attribute exists only on `WillowPlayerController`.
   Does the native skip it unless the instigator is a player controller? It needs a native read (lane F), or a ruling
   that the port skips it for a controller class that does not declare it.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

- After a PrimalBeast spawn, read its mind's `Attribute_EnemyNonWeaponDamageMultiplier` and the pawn's health, against
  the probe's 58.88 for the baby.
- Read one melee swing's `FinalDamage` on Maya from a level-7 adult, against the probe's 69.73. That probe number has no
  shield applied, so it is an upper bound for the health loss.
- Read `ITargetable` values in `Pawn.TakeDamage`'s call to `IsFriendlyFire`, to see the argument's interface part.
