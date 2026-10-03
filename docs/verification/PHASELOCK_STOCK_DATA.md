# Phaselock and one Motion upgrade path from stock data (2026-10-01)

AI-assisted (Claude). This record covers extraction and structural checks only. **No
original-game observation, no UE run and no VM execution of the Phaselock script was made for it.**
Every behavioural statement below comes from installed data or from reading installed bytecode with
`research/script_disasm.py`. The script is summarised in our own words and no listing is in the
repository. Anything fitted is marked `UNVERIFIED`. Wiki values were not used.

## What was recovered

### Identity (all from `GD_Siren_Streaming_SF.upk` unless noted)

| Piece | Object |
|---|---|
| Skill tree root | `GD_Siren_Streaming.SkillTree.SkillTree_Siren` -> `GD_Siren_Skills.SkillTree.Branch_ActionSkill_Phaselock` (one tier, `PointsToUnlockNextTier` 1, children Motion, Harmony, Cataclysm) |
| Action skill | `GD_Siren_Skills.Phaselock.Skill_Phaselock` (`SkillDefinition`, MaxGrade 1, `ActionSkillArchetype` below) |
| Archetype | `GD_Siren_Skills.Phaselock.ActionSkill_Phaselock` (`WillowGame.LiftActionSkill`) and its `BehaviorProviderDefinition_0` (1 sequence, 13 events, 89 behaviors, 95 links) |
| Helper skills it switches | `Skill_Phaselock_CooldownManager` and `Skill_Phaselock_DiminishingReturns`; other skills it switches belong to upgrades (`Motion.ThoughtLock_Feedback`, `Motion.Subsequence_Feedback`, `Cataclysm.Ruin_Feedback`, `Cataclysm.Skill_DetonateAvailable`) |
| Cooldown | `Startup.upk`: `GD_Siren.Character.CharClass_Siren.SkillCooldownPoolDefinition` = `D_Resourcepools.PlayerPools.ActiveSkillCooldownPool_Siren` (BaseMaxValue = `GD_Siren_Skills.Misc.Cooldown_Phaselock`, BaseConsumptionRate 1) |
| Lift shape | `PhaseLockDef_Default` (+ Loader, Probe, Rakk, Gyrocopter by body tag); `HeightFromGround`/`DropTime` from `Default__PhaseLockDefinition` (`WillowGame.upk`) |
| Skill points | `Startup.upk`: `GD_Globals.Skills.INI_SkillPointsPerLevelUp`, `INI_TotalSkillPointsForCurrentLevel(Formula)` |

### Phaselock numbers

| Value | Number | Source |
|---|---|---|
| Lift time | 0.7 s | archetype `LiftDuration` |
| Lock duration attribute | 5 | `Misc.Att_Phaselock_Duration` (DesignerAttribute BaseValue), read through `LockDurationFormula` on Maya |
| Lock time scale on the target | 1 by default | `Attributes.PhaselockTimeScale` resolver DefaultValue, read through `LockDurationScaleFormula` on the target |
| Fade-out (outro) | 1.1 s | archetype `LockFadeOutTime` |
| Release buffer / Ruin hold | 1 s / 1 s | `Default__LiftActionSkill` (not overridden) |
| Lift height | 200 uu above the ground under the target | `Default__PhaseLockDefinition.HeightFromGround` (not overridden by `PhaseLockDef_Default`) |
| Lift snap | half the height in the first half of the lift | `LiftSnapHeightPct` / `LiftSnapTimePct` 0.5 |
| Hover bob | 30 uu amplitude, sin((t - start) * 0.5 * pi) | `LiftBobAmplitude` 30, `LiftBobFrequency` 0.5 (formula from script) |
| Drop | 0.5 s | `DropTime` |
| Miss impact trace | 1024 uu | `MissTraceDistance` (for the miss effect only, not a targeting range) |
| Cooldown | 13 s pool, drains at 1/s | `Cooldown_Phaselock` constant resolver 13; pool BaseConsumptionRate 1 |
| Cooldown pause | consumption rate PreAdd -1 while `CooldownManager` is active | `Skill_Phaselock_CooldownManager` (TARGET_Self, MT_PreAdd -1); it is switched on by `OnSelectedTarget` and off by `OnReleasedTarget` |
| Diminishing returns | target's `PhaselockTimeScale` MT_Scale -0.4 for 25 s | `Skill_Phaselock_DiminishingReturns` (InitialDuration 25), activated on the target by `OnReleasedTarget` |
| State applied to the target | `Attributes.IsPhaselocked` MT_PostAdd +1 while held; AI flag `Flag_Skills_IsPhaseLocked` true on lock, cleared on release | archetype `PhaselockedAttributeEffects`; `Behavior_SetAIFlag_3`/`_2` |
| Who can be lifted | `CanLiftTargetIf` = AI flag `Flag_Skills_CanPhaseLock` | otherwise `OnTargetBlocked` (no lift) |
| Constraints to cast | weapon action available; on foot or riding in back; healthy | `Skill_Phaselock.SkillConstraints` |

Derived timeline (script formulas, data numbers; `UNVERIFIED` until a game capture), seconds after the cast:

| Case | Skill duration | Locked state | Outro starts | Target released | Skill ends |
|---|---|---|---|---|---|
| Base, first lock | 5.7 | 0.7 - 4.6 | 4.6 | 5.7 | 6.7 |
| Base, same target again within 25 s | 3.7 | 0.7 - 2.6 | 2.6 | 3.7 | 4.7 |
| Suspension grade g, first lock | 5.7 + 0.5g | 0.7 - (4.6 + 0.5g) | 4.6 + 0.5g | 5.7 + 0.5g | 6.7 + 0.5g |

Formulas: SkillDuration = LiftDuration + LockDuration(Maya) * TimeScale(target); LockTarget at
LiftDuration; StartOutro at SkillDuration - LockFadeOutTime; release at SkillDuration; EndSkill at
SkillDuration + ReleaseBufferTime.

Damage: base Phaselock lifts a liftable target without damaging it. Every `Behavior_CauseDamage` reached
from `OnSelectedTarget`, `OnTargetBecomesLocked` and `OnKilledTarget` sits behind a `Behavior_CompareFloat`.
Judging by what each one gates (Converge melee definitions, `Init_Ablaze_DamageCalc` for Helios,
`Ruin_Feedback`, the Sweet Release projectile, `Subsequence_Feedback`), those compares test upgrade
grades. That attribution is an inference (`UNVERIFIED`): the compared values sit in data the reader does
not decode (see "Dead ends"). A target that cannot be lifted (`OnTargetBlocked`) takes
`Behavior_CauseDamage_11` (or `_16` when it is a player) with `Phaselock_Impact`. The damage formula comes
from a behavior variable whose value is not decoded, so **the blocked-target damage amount is not recovered**.

### Event chain (the base-game part of the provider, in link order)

Event names are the names of the `LiftActionSkill`/`ActionSkill` script functions that fire them (oracle
below). Upgrade-gated branches are listed but would not run for a Maya without those skills (`UNVERIFIED`).

| Event (fired by) | Behaviors, in link order |
|---|---|
| `OnActionSkillActivated` (`ActionSkill.OnActionSkillStarted`) | none linked |
| `OnSelectedTarget` (`PhaseLockTarget`) | dialog `DialogEvent_HUM_React_ASkill_Siren` -> activate `CooldownManager` -> sound -> screen particle -> `AIProvoke` (target provoked by Maya) -> [CompareFloat gates: +1.25 s Converge pull, +0.75 s Helios burst]; second link: `Conditional` on Thoughtlock -> particle, `AIHold` |
| `OnTargetBecomesLocked` (`LockTarget`) | deactivate `Subsequence_Feedback` -> activate `Skill_DetonateAvailable` -> sound -> AI flag IsPhaseLocked = true -> custom event `Phaselocked` -> [CompareFloat gates: particle; Ruin chain] |
| `OnTargetIsAboutToBecomeUnlocked` (`StartOutro`) | sound -> +0.5 s particle -> deactivate `ThoughtLock_Feedback` |
| `OnReleasedTarget` (`ReleaseTarget`) | AI flag IsPhaseLocked cleared -> deactivate `CooldownManager` -> activate `DiminishingReturns` on the target -> remove instance data; charm-end chain (threat -0.5, `ThoughtLockEnd`) |
| `OnActionSkillDeactivated` (`ActionSkill.OnActionSkillEnded`) | deactivate `Skill_DetonateAvailable` -> screen particle -> sound |
| `OnKilledTarget` (`InterruptPhaseLock`) | deactivate `ThoughtLock_Feedback`; [CompareFloat gates: Sweet Release projectile; `Subsequence_Feedback`] |
| `OnLiftFailed` (`FizzleOut`) | sound |
| `OnTargetBlocked` (`TargetBlocked`) | `IsObjectPlayer` -> damage with `Phaselock_Impact` -> particles, sound -> [gates] -> delays 0.95/1.45/3.2 s -> deactivate `Skill_Phaselock` |
| `OnHealedTarget`, `OnCharmTarget`, `OnRuinImpact` | Res, Thoughtlock and Ruin only |

`Skill_Phaselock`'s own provider (`BehaviorProviderDefinition_2`): `OnActivated` -> tattoo glow
coordinated effect; `OnDeactivated` -> deactivate `Subsequence_Feedback`; `Killed Enemy` -> kill dialog.

Six behaviors are not reached from any event (`Explode_3/4/5`, `CauseDamage_13`, `ActivateSkill_11`,
`DeactivateSkill_12`); they are listed in the manifest's `check.unreachedBehaviors`.

### Script versus native

Script (readable, summarised in the manifest's `scriptFlow`): `WillowPlayerController.StartActionSkill`,
`ServerStartActionSkill`, `ActionSkillCallback`, `StartActiveSkillCooldown`, `IsActionSkillOnCooldown`;
`ActionSkill.OnActionSkillStarted/Ended`; all of `LiftActionSkill`'s state machine (`OnActionSkillStarted`,
`SelectTarget`, `CanPhaseLockTarget`, `PhaseLockTarget`, `LiftTarget`, `BeginLifting`, `LockTarget`,
`StartOutro`, `ReleaseTarget`, `DropTarget`, `EndSkill`, `InterruptPhaseLock`, `FizzleOut`, `Fizzled`,
`GetLiftLocation`, `GetBobLocation`, `UpdateLiftedPawn`, `GetPhaseLockDefinition`) and the 13 `On*` event
wrappers; `SkillTreeGFxObject.CanUpgradeSkill` (the level-5 training gate).

Native: target choice (`WillowAutoAimStrategy.GetPreferredTarget`, so the **range**); skill
activation and effects (`SkillEffectManager`); the behavior kernel
(`BehaviorKernel.ActivateBehaviorEventFromScript`); the tier unlock (`PlayerSkillTree.GetSkillState`'s
`bIsUnlocked`, `UpgradeSkill`); resource-pool consumption; special-move playback. Operator natives in the
listings were named from `iNative` in `Core.upk`/`Engine.upk` (for example 174 `Add_FloatFloat`,
171 `Multiply_FloatFloat`, 280 `Actor.SetTimer`, 187 `Sin`), not from memory.

### Upgrade path: Motion, tier 1 (5 points) then Suspension

Chosen because Suspension changes Phaselock observably (longer lock) through attribute data alone, with no
behavior or script change, and sits in Motion's second tier.

| Step | Skill | Max grade | Per grade (data) |
|---|---|---|---|
| 0 | `Skill_Phaselock` | 1 | the action skill costs 1 point; the root tier needs 1 point before any tree opens |
| 1 | `Motion.Ward` (or `Motion.Accelerate`) | 5 | Ward: ShieldMaxValue MT_Scale +0.05/grade, ShieldOnIdleRegenerationDelay MT_Scale -0.08/grade. Accelerate: WeaponDamage +0.03, WeaponProjectileSpeedMultiplier +0.04 per grade (MT_Scale) |
| 2 | `Motion.Suspension` | 5 | `Att_Phaselock_Duration` MT_PostAdd +0.5 at grade 1, +0.5 per grade (0.5, 1.0, 1.5, 2.0, 2.5) |

Rules: Motion tiers each ask `PointsToUnlockNextTier` 5 (data); a tier opens when the branch holds the
sum of the lower tiers' points (host rule; the game computes it natively, so this is `UNVERIFIED`, though
it matched 55 traced spends on 2026-09-27). Training needs player level 5 (script constant in
`CanUpgradeSkill`). Skill points: total = 1 * Level^1 - 4 when Level >= 5, one per level-up from level 5
(`GD_Globals.Skills`; the value below level 5 is the conditional's unlisted default, `UNVERIFIED` as 0). So
Suspension grade g needs 1 + 5 + g points: level 11 for grade 1, level 15 for grade 5.

Lock time with Suspension: 5 + 0.5g, assuming `(base + PreAdd) * (1 + Scale) + PostAdd` (`UNVERIFIED`;
it is the rule fitted for weapons in `tools/weapon_stats.py`). The manifest has every grade's timeline.

## Structural oracles that passed

- Packed links: the 95 links of the archetype's provider and the 4 of `Skill_Phaselock`'s are covered
  exactly once by the event and behavior ranges (`linksTiledExactly`), every range is inside the link
  array and every link names an existing behavior (same check as `src/behavior.cpp`).
- All 12 distinct event names in the provider are `LiftActionSkill`/`ActionSkill` function exports in
  `WillowGame.upk`, as the script's `GetFuncName()` dispatch requires (`eventNamesAreScriptFunctions`).
- Suspension's own info-box line (its `AttributePresentationDefinition` on `Att_Phaselock_Duration`,
  rendered by `tools/skill_stats.py`: +0.5 ... +2.5 seconds) equals the computed change of the lock duration
  at every grade (`presentationAgreesWithDuration`). This is self-consistency within the install, not a
  game observation; Suspension is not among the skills in the local UI traces.
- Cooldown 13 s agrees with the `Cooldown: 13 seconds` line already produced by
  `tools/prepare_skill_tree.py` and with the earlier host reading (DECISIONS 2026-09-25).
- Skill points: the data formula (level - 4 from level 5) equals the host rule fitted to the traced
  level-45 character.
- Every attribute reference in the manifest resolved (`unresolved` is empty).

## UNVERIFIED

- The modifier combination rule (above) for skills and designer attributes.
- The timeline as a whole: script read, never executed or compared with the game.
- Cooldown semantics: that the pool's current value drains at the consumption rate, so PreAdd -1 pauses the
  cooldown from target selection to release (cooldown ready about 18.7 s after a base cast). When exactly
  `ActionSkillCallback` refills the pool relative to `OnSelectedTarget` is not known.
- The meaning of a link's id byte (0, 1, 2 on CompareFloat/Conditional/IsObjectPlayer; 255 elsewhere).
  `Behavior_Conditional`'s condition is `ConditionId` 1, which fits id 1 = condition true.
- Which upgrade each `CompareFloat` tests (inferred from what it gates), and that all of them stay closed
  without upgrades.
- Blocked-target damage amount; the targeting range; what `Skill_Phaselock.InitialDuration` 120 does;
  that the `Skill_Phaselock` effect on `Cooldown_Phaselock` (TARGET_None) is display-only.
- That an omitted `BaseValueScaleConstant` means 1 (an omitted value of 0 would make the lock duration 0).

## Dead ends and findings for other owners

- **VM object dump loses enum values inside structs.** `ow-package --object-dump` (VM instantiation)
  reports `ModifierType` 0 and `EffectTarget` 0 inside `SkillEffectDefinitions`, where the tagged reader
  (`--properties`) reads `MT_PostAdd` / `TARGET_Self` from the same bytes. It also shows omitted
  `BaseValueScaleConstant` as 0. The tool therefore uses only the tagged reader. Anyone reading
  `AttributeEffects` or similar structs through `vm::Runtime` (for example a `Behavior_AttributeEffect`
  handler) should check this in `src/vm.cpp` (not changed here).
- **Behavior variable values are not decoded.** `VariableData` decodes only Name and Type
  (`BVAR_Attribute`, `BVAR_Float`, `BVAR_Object`...). The archetype's provider export ends with 392 bytes
  after its tagged properties that no reader consumes. Some 4-byte values in them equal the export indices
  of `SkillGradeModifiers.Converge/Ruin/SubSequence`, and some read as plausible floats. That is a lead
  for the CompareFloat inputs and the blocked-target damage, **not** a decoded layout. No offsets were
  fitted.
- **No VM execution.** Running `LiftActionSkill` through the VM needs, at least: a world time, `Actor.SetTimer`
  timers, and `BehaviorKernel.ActivateBehaviorEventFromScript` bridged to `vm::BehaviorProvider::fireEvent`
  with the calling function's name. That bridge lives in files owned by the behavior/VM work and was not
  attempted.

## Host prototype versus stock data

| Topic | Host now (`OpenWillowWalker`/`OpenWillowCombatTarget`) | Stock data / script |
|---|---|---|
| Lock length | `BeginPhaselock(Now, 5.f)`: released 5.0 s after the cast | released at 0.7 + 5 * scale = 5.7 s; locked state 0.7-4.6 s |
| Fade | shell fades over the last 1.1 s | outro 1.1 s from 4.6 s (agrees in length) |
| Lift height | 170 cm estimate, ease-out over 0.7 s | 200 uu above ground (+ collision height, ceiling-clamped); quadratic snap to 50% height in 0.35 s, then ease-out |
| Hover | 6 cm sine at 2.4 rad/s plus rotation wobble | 30 uu at sin(0.5 * pi * t), smoothed; no rotation in the script |
| Cooldown | 13 s counted from the cast | 13 s pool, paused while the target is held: ready about 13 s after release (`UNVERIFIED` semantics) |
| Miss | fail animation, no cooldown | `OnLiftFailed`, miss impact; cooldown reset and skill end after 1 s |
| Targeting | 30 cm sphere sweep to 2500 cm | native auto-aim preferred target; range not in Phaselock data |
| Valid target | any host dummy not locked or dead | not friendly, alive and well, not already phaselocked, not driving; `Flag_Skills_CanPhaseLock` or else blocked (damage, no lift) |
| Re-lock same target | same 5 s | 25 s diminishing returns: 3.7 s total |
| Cast gate | action grade >= 1 | also weapon action available, on foot / back seat, healthy (skill constraints) |
| Upgrades | none applied | Suspension +0.5 s per grade (manifest `phaselockByModifierGrade`) |
| Skill points | level - 4 from level 5 | same, from `GD_Globals.Skills` |
| Target state | `bPhaselocked` | `IsPhaselocked` +1 and AI flag `Flag_Skills_IsPhaseLocked`; `AIProvoke` on selection |

The "Host now" column describes the host before 2026-10-01's player-side pass. Which rows the host now takes from
this manifest, and which remain open, is listed in `docs/verification/SANCTUARY_RPG_MISSION.md`, "Player side with
stock data".

## Manifest

`python tools/prepare_action_skill.py --reader build/Release/ow-package.exe --game "$env:OPENWILLOW_BL2"`
writes `local/character/action_skill_siren.json` (about 20 s). Format `openwillow.action_skill/2` since 2026-10-01 (adds `skill.constraintEvaluators`, `actionSkill.canLiftTargetIfChain` and `autoAim`; a /1 manifest must be regenerated). The fields shared with /1:

- `skill`: id, name, maxGrade, playerLevelRequirement, initialDuration, constraints, `effects` (rows as below),
  `behaviorProvider` (as below).
- `cooldown`: pool, attribute, seconds, baseConsumptionRate.
- `actionSkill`: id, class, `settings` (LiftDuration, LockFadeOutTime, ReleaseBufferTime, RuinHoldTime,
  LiftSnapTimePct, LiftSnapHeightPct, LiftBobAmplitude, LiftBobFrequency, MissTraceDistance, BubbleFX times),
  `lockDuration` {attribute, base}, `lockDurationScale` {attribute, default}, `phaselockedAttributeEffects`,
  `canLiftTargetIf`, `phaseLockDefinitions` {default, byBodyTag, classDefaults}, `linkedSkills`, `animations`.
- `helperSkills`: {skill path: {name, initialDuration, effects}} for the Phaselock helpers.
- `behaviorProvider`: {id, sequences: [{name, enabledOnSpawn, events: [{name, enabled, outputs, links}],
  behaviors: [{index, id, class, properties, undecodedProperties, variables, links}], check}]}. A link is
  {behavior (index), outputId, delay}; `outputs`/`variables` map a property name to
  [{variable, name, type}].
- `eventNamesAreScriptFunctions`, `scriptFlow` [{function, events, summary}], `native` [text].
- `upgradePath`: branch {id, name, tiers: [{tier, pointsToUnlockNext, skills: [{id, name, maxGrade, onPath,
  effects}]}]}, spendOrder, actionSkillPointsToUnlockTrees, branchPointsBeforeTier,
  minimumPlayerLevelToTrainFromScript, `phaselockByModifierGrade` [{grade, lockDurationAttribute,
  firstLock, relockWithinDiminishingReturns}] (each a timeline: skillDuration, lockedAt, lockedFor,
  outroAt, releasedAt, endSkillAt), presentationAgreesWithDuration.
- `skillPoints`: perLevelUp / totalForLevel {id, conditions: [{when, constant, formula}]}.
- `unresolved`: attribute paths that did not resolve.

An effect row is {attribute, modifierType (MT_Scale/MT_PreAdd/MT_PostAdd), target (TARGET_*), startGrade,
values[grade 0..max]} (`tools/skill_stats.py effect_rows`). The skill-tree UI manifest
(`tools/prepare_skill_tree.py`) is unchanged; this one adds gameplay numbers and does not repeat UI text.

## Reproduce

```powershell
cmake --build build --config Release
python tools/prepare_action_skill.py --reader build/Release/ow-package.exe --game "$env:OPENWILLOW_BL2"
python research/script_disasm.py WillowGame.upk LiftActionSkill.PhaseLockTarget   # output stays local
python tests/prepare_action_skill_test.py
python tests/skill_stats_test.py
```

## Stock presentation census (2026-10-02)

AI-assisted (Claude). Read-only census of what the installed data and script say Phaselock *shows*: the bubble,
the effects, the lifted target's animation and Maya's first-person cast. **No game capture, no UE run.** Sources:
`ow-package --properties` (tagged reader, with a local array schema), `--behavior-dump`, and `research/script_disasm.py`
on `LiftActionSkill`, `SpecialMove_PhaseLock` and `SpecialMove_FirstPerson`. Listings, dumps and exports stay under
ignored `local/phaselock/census/` and `local/phaselock/umodel-test/`. Script behaviour is summarised in our own words.
Every reading below is `UNVERIFIED` until a game capture confirms it. All objects are in `GD_Siren_Streaming_SF.upk`
unless noted; a few FX assets are imports exported by `Startup.upk`.

### What the archetype names

`ActionSkill_Phaselock` (with `Default__LiftActionSkill` in `WillowGame.upk` for anything it does not override)
names every presentation piece:

| Piece | Object / value |
|---|---|
| First-person hand effect | `FX_CHAR_Siren.Particles.Part_SirenASHandOrb` (miss: `Part_SirenASHandFizzle`), socket `LilithMeleeFX` on the arms, small offset, scale 0.35 (class defaults) |
| Third-person hand effect | the same templates at bone `Bip01_L_Hand` |
| Bubble | `Part_SirenASEnemyOrbBegin` -> `Part_SirenASEnemyOrb` (loop) -> `Part_SirenASEnemyOrbEnd`; `BubbleFXScale` 66.7, intro 0.2 s, outro overlap 0.2 s |
| Bubble parameters | particle parameters `PhaselockLifeTime` (loop lifetime) and `SphereCollapse` (0 -> `MaxCollapseValue` 0.75 over `CollapseDuration` 2 s) |
| Light | `PhaselockLight` (a `PointLightComponent`; class default radius 500, brightness 4, blue-violet, no shadows) |
| Maya's animation | `PhaselockSMD_Hit` `Misc.Phaselock_1st3rd_SM` (third person `AA_PhaseLock`, first person `Misc.Anim_Phaselock` = arms sequence `Phase_Lock_Lift`); miss: `Phaselock_Fizzle_1st3rd_SM` (`AA_PhaseLock_Fail` / `Phase_Lock_Fail`); both block weapon actions |
| Target animation | `PhaseLockDef_Default` (`HeightFromGround` 200, `DropTime` 0.5) with `SpecialMove_PhaseLock` names `PhaseLock_Lift` (blend-in 0.4 s), `PhaseLock_Loop` (loops), `PhaseLock_Fall` (stops on last frame), `PhaseLock_Land` |

The provider's own effects (see the event table above) add `Part_PhaseLockScreenEffect` (a screen particle shown on
`OnSelectedTarget`, hidden on `OnActionSkillDeactivated`), `Part_PhaseLock_Miss_Impact` on a miss,
`Part_PhaseLock_EnemyCannotBeLocked` on a blocked target, and sounds (names only):
`Ak_Play_FX_Player_Phaselock_Activate`, `_Loop`, `_Collapse`, `Ak_Play_FX_Maya_Phaselock_False_Start`.

### Bubble chain (script, our summary)

- Nothing is drawn around the target during the 0.7 s lift except the light. `LockTarget` spawns the bubble.
- `SpawnBubbleFX` spawns an emitter at the lift end point with the intro template. Its draw scale is the lifted
  pawn's mesh bounds sphere radius divided by `BubbleFXScale`, so the bubble follows the body size. After
  `BubbleFXIntroTime` it changes to the loop template.
- The loop emitter's life span and its `PhaselockLifeTime` parameter are the locked state's duration plus the
  overlap. Each tick `UpdateEffects` raises `SphereCollapse` toward 0.75. The rise is timed to end with the lock, over
  the last `CollapseDuration`.
- `StartOutro` destroys the loop and spawns the end template, which holds the twirls, smoke and a closing flash.
- `UpdatePhaselockLight` attaches the light to the lifted pawn. It ramps the light up over the lift, holds it while
  locked and ramps it down over the outro.
- Loop template emitters: two 4-frame sprite "spikey" sheets (additive and modulated), a core-colour sprite, a
  sprite named `Sphere` using `Mat_SirenEnemyOrb` (texture `PhaseLockBubble_Dif_Tex`, its dynamic parameter driven by
  `SphereCollapse`), a modulate-black sprite, and an orbiting twirl sprite (`FX_CHAR_Lilith.Materials.Mat_PowerUpTwirls`).
  So the stock bubble is **camera-facing sprites with a bubble texture, not a mesh sphere**. Sizes read from the baked
  tables are in the 200-500 range before the draw scale (layout fitted, below).

### Target-side animation chain (script, our summary)

- `LiftTarget` picks the `PhaseLockDefinition`: the body class's own definition, then the `LiftBodyMap` body tag
  (Loader, Probe, Rakk, Gyrocopter), else `PhaseLockDef_Default`. It plays the lift special move, queues the loop and
  switches the pawn to a flying physics mode with world collision off.
- `UpdateLiftedPawnMeshOffset` runs after the snap part of the lift. It eases the mesh translation (VInterp speed 5)
  so that the mesh's bounds centre moves onto the pawn's location, which centres the body in the bubble.
- `DropTarget` restores default physics, zeroes velocity and plays the drop move stretched to `DropTime`. If the
  target cannot play drop animations, it stops the loop instead. `CheckLandTarget` plays the land move once the pawn
  walks again.
- These names resolve per enemy AnimSet. Seen with `ow-package`: `Anim_Psycho.Base_Pyscho` (Lift 0.87 s, Loop 3.03 s,
  Fall 0.2 s, Land 0.83 s), `Anim_Nomad.Base_Nomad`, `Anim_Goliath.Base_Goliath` and `Anim_Spiderant_NEW.Shared_Spiderant`
  (`SouthernShelf_Dynamic.upk`, `IceCanyon_Combat.upk`). 77 packages carry the names.
- **The Sanctuary target dummy has none of them.** `GD_TargetDummy.Character.Pawn_TargetDummy`'s mesh component uses
  only `Anim_Sanctuary.Anim_Fink`, and no sequence in `Sanctuary_Dynamic.upk` is named `PhaseLock_*`. Its body class
  (`BodyTag_Psycho`) has no own `PhaseLockDef`. So the stock data would ask for the default names and find no clip on
  the slice's dummy. What the engine then shows (probably the idle pose kept), and whether the dummy is liftable at
  all (`Flag_Skills_CanPhaseLock`), is `UNVERIFIED`. The Psycho-shaped `GD_PsychoShared.Anims.PhaseLockAnim_PsychoShared_*`
  moves exist in the same package but nothing found here references them.

### Maya's first-person cast: what is attached to the arms

- `Phase_Lock_Lift` (arms AnimSet `Anim_Siren.Siren_1st`, 26 frames, 0.87 s) has one notify at 0.25 s. It is an
  `AnimNotify_UseBehavior` that fires the custom event `PlayPhaselockHandFXFirstPerson` on the action skill.
  `Phase_Lock_Fail` fires the same event at 0.05 s.
- `LiftActionSkill.RunCustomEvent` handles that event. It creates a particle component on the arms at socket
  `LilithMeleeFX`, with the first-person offset and scale. It sets the component to foreground depth and owner-only
  visibility, and activates `Part_SirenASHandOrb` (fizzle template on a miss). The component is removed when the
  system finishes.
- The socket is on `Char_Siren.Hands_Siren`, bone `L_Weapon_Bone`, a few units out.
- Hand-orb emitters: three mesh-particle ribbons (`FX_CHAR_Siren.Meshes.Smesh_Twirly_01/02/03` with
  `Mat_SirenEnergyRibbons`), an energy swirl, a suction sprite, a modulate-black sprite, an inner-orb sprite
  (`Mat_SirenHandInnerOrb`), a short sub-UV "after smoke" burst and three glow/flash sprites.
- `Skill_Phaselock`'s `OnActivated` enables `GD_Siren_Skills.CoordinatedEffects.Phaselock_TatooGlow`. That effect
  drives the material scalar `p_EnablePowerEmissive` over 1 s (rises to about 0.8 near the middle, back to 0). The
  four curve keys are stored out of time order and are read as stored.
- `Char_Siren.Mati_Siren_Hands` (parent `Common_Materials.Player.Master_Player`) carries that parameter at 0 and an
  HDR cyan `p_PowerEmissiveColor`. So the arms' tattoo is expected to flash cyan during the cast. That the coordinated
  effect reaches the first-person arms mesh is native and `UNVERIFIED`. Which mask channel limits it to the tattoo was
  not read.
- **Likely "missing something":** the hand orb at `LilithMeleeFX` from 0.25 s into the arm clip, plus the 1 s cyan
  tattoo glow on the arm material. The full-screen `Part_PhaseLockScreenEffect` also starts when the target is
  selected. The host draws a violet beam from `L_Hand` instead, and has no tattoo glow and no screen effect.

### Online references (local only)

On 2026-10-02 the maintainer allowed looking online. The images and their sources are under ignored
`local/phaselock/ref_online/` (`sources.txt`). Promotional renders show the tattooed left arm glowing cyan with a
violet orb and orbiting ribbons in the hand, which fits `Part_SirenASHandOrb` plus `Phaselock_TatooGlow`. YouTube
auto-frames show, in first person, an enemy inside a violet sphere with a dark core (through a scope) and a lifted
Bullymong with limbs spread while the frame is washed out pale. These are third-party, low-resolution frames. They
support which effects are visible but are not a capture and verify no timing or value.

### Data layout lead: baked distributions (fitted, UNVERIFIED)

Cooked emitter modules keep no distribution objects (`Distribution` is None). The values are only in each
`RawDistributionFloat/Vector.LookupTable` (read as a float array). The tables fit this pattern: two leading values
(the minimum and maximum over all entries), then samples of 1 (float) or 3 (vector) values, or min/max pairs for
uniform distributions, spaced by `LookupTableTimeScale`. Example: a 23-entry alpha table with time scale 20 is 2
range values plus 21 samples over the particle's life. The tagged reader shows no `Op`, entry count or chunk size,
so constant, curve and uniform tables cannot be told apart from the data alone yet. Spawn rates are 0 in the bubble
emitters, so the particles come from `BurstList`, which the reader does not decode. `ParticleModuleParameterDynamic.DynamicParams`
and `ParticleSystem.LODSettings` are not decoded either.

*Update 2026-10-02:* the "Particle template reader" section below supersedes this lead: the header bytes are ordinary delta-serialized tags and `BurstList` and `DynamicParams` are decoded by `research/particle_system.py`.

### What UModel build 1590 exports here (timed, 2026-10-02)

| Command | Time | Result |
|---|---|---|
| `-export -game=border -uncook -groups -png -gltf GD_Siren_Streaming_SF.upk` | 3.3 s | 158/158 supported objects: 107 textures, 33 materials and 8 material instances (`.mat` heuristic), 7 static meshes, 3 skeletal meshes. `ParticleSystem` (36 in the package) is an unknown class and is not exported; AnimSets are skipped in glTF mode. 12 import warnings (textures and materials outside the package, for example `PhaseLockScreenMask02_Dif_Tex`, `EnergyOrbReflect_Cube`) |
| Targeted from `Startup.upk`: `Mat_EnemyOrbCoreColor`, `Mat_SirenSuction`, `Mat_SirenGlowMOD`, `Mat_SirenHandGlow`, `Smesh_Twirly_01` | 0.1-0.2 s each | all exported with their textures |
| `Mat_SirenOrbBlackMOD` | 0.1 s | skipped ("empty parameters": a material with no texture parameter) |
| `Part_SirenASEnemyOrb ParticleSystem` | 0.1 s | "no supported objects" |
| `-md5 ... Siren_1st AnimSet`; `-md5 SouthernShelf_Dynamic.upk Base_Pyscho AnimSet` | 0.1 s; 1.3 s | exported, including `Phase_Lock_Lift`/`_Fail` and the four Psycho `PhaseLock_*` clips |

So UModel provides the bubble's and hand orb's textures, the twirl meshes, the arms and Psycho clips, and heuristic
material slots. It provides no emitter structure, module values or material graphs. The material `.mat` files name
textures only (for example `Mat_SirenEnemyOrb`: `PhaseLockBubble_Dif_Tex`). They do not show blend modes, the
`SphereCollapse` dynamic-parameter wiring or depth bias.

### Bounded host plan (not started)

1. **Can be imported now (UModel plus the existing glTF/md5 tools):** the bubble and hand-orb textures, the
   `Smesh_Twirly_01-03` meshes, the Psycho `PhaseLock_*` clips (for a Psycho-shaped target; the slice dummy has none),
   and the arms are already imported.
2. **Arm effect, smallest step:** play a host stand-in for the hand orb on `LilithMeleeFX` at 0.25 s into
   `Phase_Lock_Lift`. It would be a Niagara or mesh assembly with the imported textures and twirl meshes, foreground
   and owner-only. Separately, drive a cyan emissive pulse on the arms material with the decoded 1 s curve.
   Both use only numbers read above. The look of the emitters stays a host approximation until step 4.
3. **Bubble:** replace the 220 cm `M_OW_FxAdditive` sphere with camera-facing sprites using
   `PhaseLockBubble_Dif_Tex` and the spike sheets. Scale them by mesh bounds radius / 66.7. Spawn them at the lock
   (not at the cast), with an intro of 0.2 s, an end burst at the outro and the collapse parameter over the last 2 s.
   The point light (radius 500, brightness 4) was removed earlier for pooling under Lumen; re-adding it needs a
   Lumen-aware choice.
4. **Needs decoding first, for anything closer:** the `RawDistribution` table header (op, element count, chunk size)
   and `BurstList` (spawn counts), `ParticleModuleParameterDynamic.DynamicParams`, the remaining module properties
   (`Required` sub-UV and alignment are already read), and the material graphs of the FX materials
   (blend mode, how `SphereCollapse` and the dynamic parameter are used). A `research/` reader for these was
   the bounded next step and now exists (`research/particle_system.py`, section below); the FX material graphs remain stripped. A converter to Niagara is a separate, larger task.
5. **Target animation:** use the `PhaseLockDefinition` chain (lift with blend-in, queued loop, drop stretched to
   0.5 s, land on walking) and the mesh-centring rule. For the slice dummy, decide with the maintainer whether a
   Psycho-shaped dummy borrows the Psycho clips: stock data gives the dummy no clips.

## Particle template reader (2026-10-02)

AI-assisted (Claude). `research/particle_system.py` (synthetic tests: `tests/particle_system_test.py`, 16 pass) reads
cooked `ParticleSystem` templates with the pure-Python package loader of `research/behavior_census.py` and writes one
JSON per template under ignored `local/phaselock/emitters/` (`python research/particle_system.py --oracle`). Each
JSON holds every emitter, every LOD, every module's properties, the particle parameters, the effect materials'
tagged properties and a per-emitter digest of LOD 0. No values are recorded here; they stay in the local JSON.
Nothing was run in the game or in UE.

**How values are resolved.** Cooked templates are delta-serialized: a property equal to the archetype's is not
written. The reader merges own tags over the archetype chain. That is the export's `ArchetypeIndex`, else the same-named
child of its outer's archetype, else `Engine.Default__<Class>`, whose own chain is followed to `Core`. Structs merge
field by field and arrays are replaced whole. Without the class defaults the baked tables look headerless (the
2026-10-02 census above): `Op`, `LookupTableNumElements` and `LookupTableChunkSize` are ordinary byte tags that are
only written when they differ from the class default.

**Layouts and what checks them.**

| Item | Reading | Status |
|---|---|---|
| Tag streams of all particle and distribution exports | Every tag's value uses exactly its declared size, and the stream ends exactly at the export end. Prefix: 4 bytes, or 8 or 16 for distribution subobjects. Engine 347, GD_Siren_Streaming_SF 2437, Startup 15608, WillowGame 1244 exports; 0 failures | proven structurally; the extra prefix words are not understood |
| `RawDistribution` table | `[range_a, range_b]` then entries of `ChunkSize` floats, `ChunkSize = NumElements x width` (width 1 float, 3 vector; `NumElements` 2 = low/high pair). Constants and pairs are stored as exactly 2 entries with `TimeScale` 0. Curves store entries at `StartTime + k / TimeScale` | layout: all 17,506 non-empty tables in the four packages pass. Constants and pairs: proven against 171 Engine tables whose uncooked distribution objects are kept, all matching. **Curve sampling: FITTED** (435 of 535 curve tables in the Phaselock package end exactly at time 1.0; no kept curve object has a curve table to compare with) |
| Range header | Pair tables: (min of the lows, max of the highs), or the range over both halves when `Type` has bit 0x80 (19 Startup tables). Single-value tables: bounds the entries, sometimes wider than the samples (a key between samples) | fitted; `Type` meaning UNVERIFIED |
| `Op` | 1 for a single value, 2 for a uniform pair; 3 also occurs on pair tables (19 in the Phaselock package). A uniform with low = high is baked as `Op` 1 (15 Engine cases) | values observed; meaning of 2 vs 3 UNVERIFIED |
| `BurstList` | Tagged structs: `Count`, `CountLow`, `Time`, `CountDistribution`. All four fields are written in every element (192 + 1355 + 155 lists) | layout proven. Whether `Time` is a fraction of the emitter duration is UNVERIFIED |
| `DynamicParams` | Tagged structs: `ParamName`, `ValueMethod`, `bUseEmitterTime`, `bSpawnTimeOnly`, `bScaleVelocityByParamValue`, `ParamValue` (a raw distribution). All fields are always written | layout proven; the slot-to-material wiring is not in the tags |
| Particle parameters | `Distribution*ParticleParameter` objects are kept and their table is empty (45 here). They carry `ParameterName`, input and output ranges, and `Constant` (defaults from the class chain) | decoded. The mapping (clamp input, map to output, `Constant` when the parameter is unset) is UE3 general knowledge and UNVERIFIED |
| Enums | An enum export is a count followed by that many FNames, ending exactly at the export end. Used to name the default (first) value of absent enum properties | fitted on Engine enums |
| Materials | `BlendMode`, `LightingModel`, `TwoSided` and usage flags read from tags. The expression list survives only as mostly empty object slots plus parameter expressions. Graphs, dynamic-parameter use and texture wiring are not in the tags | as stored |

Names of the template parameters: the bubble loop reads `PhaselockLifeTime` (all six emitters' lifetimes) and
`SphereCollapse` (the `Sphere` emitter's dynamic parameter 0, input range 0 to 0.75). The intro and the miss effects
read `PhaseLockIntroLifetime`, which the script summarised above never sets, so they would fall back to the
parameter's `Constant` (UNVERIFIED engine behaviour). Material blend modes: the bubble's `Sphere`, the core colour and
the ribbons are additive and unlit. The spike sheets come as one translucent and one modulate copy. The black
sprites and the `GlowMOD` flashes are modulate. The inner orb, suction, swirl, hand glow and screen effect are
translucent and unlit.

**Not decoded:** semantic units (rotation in turns, sizes in UU, colour as HDR multipliers) are standard UE3
knowledge and are not checked here. Orbit chaining, the location primitives and the mesh type-data defaults
(alignment) are read but not interpreted. The `ow-package` C++ reader needs no change for these structs:
`BurstList=StructProperty:ParticleBurst` and `DynamicParams=StructProperty:EmitterDynamicParameter` in an
`--array-schema` file would let it list them too, since their elements are ordinary tag streams.

## Host presentation pass, second round (2026-10-03)

AI-assisted (Claude). Host and tooling only; no parser change. Compared against the 2026-10-02 game captures
(`REALGAME_GROUND_TRUTH.md`, frames under ignored `local/realgame/phaselock/`, a bullymong in Three Horns) on the
host's Sanctuary dummy: the effect is compared, not the scene. Frames, logs and the side-by-side stay under ignored
`local/phaselock/`.

**Capture clock.** The `cast_close` SDK samples put the lift start 0.21 s after the key press. Times below are from the
lift start, which is the host's cast time.

**What the defects turned out to be.**

| Seen in the 2026-10-03 host frames | Cause | Status |
|---|---|---|
| No bubble from 0.98 s to about 4 s, then a bubble appearing | The host materials are recreated by every import, and the import runs with `-nullrhi`, so the first game run compiled their shaders on first draw; translucent quads are not drawn until then. An unchanged rerun showed the bubble from the loop start | fixed: `FOwFxTemplate::Preload` loads every template's materials and meshes when the manifest loads and compiles the parents' shaders synchronously (editor builds). The first run after an import now draws everything on time |
| Hand orb "size 0x0", arm swinging down by 0.35 s | The mesh-particle sizes are mesh scales (0.12-0.7), which the log printed with no decimals. The early arm came from the capture clock: the first screenshot stalls a frame by 0.36 s and later ones by about 0.13 s, so the "0.25 s" frame was taken at +0.52 s | fixed in the log (`mesh scale`, actual time per shot) and in the capture (game time advances at most 1/60 s per frame in `-owphaselockshots`). With correct times the arm stays raised to about 0.6 s, as in the game |
| No dark core during the hold; pink-white sphere | UE5 applies modulate materials apart from the additive layer, even in the before-DOF pass, so `Mat_SirenOrbBlackMOD` never darkened the additive core sprites drawn before it | host stand-in: the darkening modulates are drawn as translucent black with opacity equal to the modulate weight (exact for a black target), in emitter order. The black orb has no texture parameter, so its mask is a host disc (full to two thirds of the radius). The hold now shows a near-black core with the violet rim of `PhaseLockBubble_Dif_Tex` |
| Black wedge at the collapse | The end template's smoke ran its sub-images the wrong way (the host ignored the SubUV module's `SubImageIndex`, which runs 15 -> 0) | fixed: `SubImageIndex` drives linear sub-image layouts; spawn-time distributions are read at the emitter's time (the smoke's `StartSize` curve) |
| Violet pool on the floor | `PhaselockLight` data (radius 500, brightness 4, falloff 0.5, `LAC_DYNAMIC_AND_STATIC_AFFECTING`) drawn as a UE5 unitless light | host stand-in: the light reaches only the lifted target (lighting channel 1). The data says it should light the floor too; the capture shows no pool. The UE3 -> UE5 brightness mapping stays open |
| White full-screen wash at 1.2-2.0 s | The screen material drew its mask texture's brightest channel x colour (4, 6, 30) | host stand-in: a modulate of the view by the colour's hue, weighted by alpha and the mask's green streaks. It reads as the game's strong blue tint |

**Cast animation data (no host change).** `Phase_Lock_Lift` has no `RateScale`, and `Anim_Phaselock`
(`SpecialMove_FirstPerson`) names only the clip and a behavior provider. `Default__GearboxAnimDefinition` has
`PlayRate` 1, `BlendInTime` and `BlendOutTime` 0.1 and `EC_OnBlendOut`. `SpecialMove_FirstPerson.PlayAnim` (script,
our summary) takes the play rate from the caller's `SpecialMoveData` (`PlayRateScale`, or clip length / `Duration` when
a duration is given); those values were not resolved. With the corrected capture clock the host's arm timing
matches the game's to the eye.

**Burst `Time` convention (open).** Only `Part_PhaseLockScreenEffect` has a burst whose time depends on the convention
(`Time` 0.7 in a 1.5 s emitter). In game the blue tint starts about 0.82 s after the lift starts. That fits 0.7 s
better than 0.7 x 1.5 = 1.05 s (the host's convention), if `OnSelectedTarget` fires at the lift start. Not changed;
UNVERIFIED either way.

**Still different from the game (UNVERIFIED stand-ins or open):** the real dark blob around the raised hand at about
0.27 s (none of the decoded hand emitters explains it under the host's modulate reading); the solid blue palm orb at
0.45-0.6 s (the host shows white-blue flashes and swirls there); the intro's 0.8 s burst, which the host draws as a
white band (the `Mat_SirenHandGlow` rectangle, colour (0.5, 0.8, 20), tone-maps to white in UE5); the screen tint
starting at 1.05 s instead of about 0.82 s; what `SphereCollapse` does in the stripped graph (the host draws nothing
from it). The modulate readings in `MODULATE_READINGS` and `DARKEN_AS_TRANSLUCENT`, the black orb's disc and the light
channel are host choices.

### Round 3 (2026-10-03, later)

An independent critic scored the round-2 side-by-side 4.5/10 (it was 2.5 before). Round 3 worked on its three items
and checked first whether one host-side cause explains several of them. Frames: ignored `local/phaselock/b2/*-20261003-184336.png`.
Side-by-sides: `local/phaselock/compare/phaselock_host_vs_real_r3_20261003-184336.jpg` (0.25, 0.5, 1.5, 3.0 and 4.8 s)
and `..._r3_extra_...` (0.35, 0.8, 1.2, 4.5 and 5.0 s).

**One common cause: the tone curve, not exposure.** Auto-exposure is already off project-wide, so exposure was not
adapting. What differs is the tone curve. UE3 shows each channel of the final colour clipped at 1, so an HDR particle
colour such as (0.5, 0.8, 20) at opacity 0.25 reads as saturated cobalt (0.125, 0.2, 1). UE5's filmic curve maps the
same value to near white. Host stand-in (UNVERIFIED; BL2's PC tone pipeline was not checked): the additive parent caps
each channel of a layer's contribution at 1, which is exact for one additive layer over the scene. The translucent
parent caps it at 1 / opacity. The global tone mapper is unchanged.

| Critic item | Cause found | Change (host stand-ins unless said) |
|---|---|---|
| Screen grade grey-brown at 1.5 and 3 s | Round 2's modulate divided the particle colour by its largest channel, which darkens the scene | Divide by its luminance (Rec. 709) instead: the tint keeps the scene's brightness and pushes it to blue. Burst `Time` is now read as seconds of emitter time (UNVERIFIED); only the screen burst moves (1.05 s -> 0.70 s), which fits the game's tint onset of about 0.82 s after the lift starts |
| Cast flashes white | The `Mat_SirenGlowMOD` flashes multiply the view by (3, 6, 12). Even UE3's clip would give near white there, while the game shows cobalt at 0.8 s (f018-f020) | `Mat_SirenGlowMOD` reads its colour as a brightness-keeping tint (colour / luminance, `HueOnly`). The 0.8 s frame is now a cobalt flash with radial streaks |
| No dark burst at +0.27 s | Under the colour-only reading the hand orb's `ModulateBlack` (colour 1) did nothing | One reading of `Mat_SirenOrbBlackMOD` fits all three of its emitters: darkness = mask x (1 - alpha), towards black, colour unused. The bubble emitters have alpha scaled to 0 (dark core, as before). The hand emitter has alpha 0 at spawn (0.25 s) and 1 by 0.2 of its life: a large dark blob around the raised hand at 0.25-0.35 s, gone by 0.5 s, as in game f007-f009 |
| Faint palm orb | The 0.5 s flash washed out the orb sprite (`Mat_SirenHandInnerOrb`, blue on `EnergyOrbCenter2_Dif_Tex`, 15 x size-over-life about 2 x hand scale 0.35, about 11 uu) | No size change. With the flash and clip changes a blue orb shows in the palm at 0.5 s. It looks smaller than the game's because the whole hand is about 2.5x smaller on screen in the host (arms placement or FOV, not the effect). Relative to the palm, the host orb is close to the game's |
| Hard blue rim | Round 2's black disc (full to two thirds of the radius) also darkened the core sprite's magenta edge | Black disc full to half the radius (`RadialSharpness` 2): the hold shows a near-black core with a soft violet-magenta rim |

**Bubble size: not changed.** The draw scale is the stock rule: the pawn's mesh bounds radius / `BubbleFXScale` 66.7,
which is 104-109 uu for the host dummy. The game's bullymong stands much further from the camera, so apparent size
cannot be compared, and its bounds radius is not known here. Checking the size needs a game capture at a matched
distance (about 650 uu), or the bullymong's bounds radius read from the game.

**Still different:**
- The release ring is blue-violet. The game's ring is cyan-white at 4.9-5.0 s. The same `HueOnly` reading that fixes
  the cast flashes reduces the end template's `Brighten` (0.4, 16, 30) to about x2 blue. Round 2's plain multiply
  gave cyan-white there, but white cast flashes. One material reading does not fit both emitters, so neither was
  tuned per emitter.
- The 0.6 s white starburst (`Mat_SirenHandGlowShattered`, colour 1).
- The lifted target's pose: the dummy has no `PhaseLock_*` clips in the stock data.
- What `SphereCollapse` drives.
