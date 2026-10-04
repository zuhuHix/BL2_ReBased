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

### Round 4 (2026-10-03, against the matched-distance capture)

The critic scored round 3 5.5/10. Round 4 compares the host with the matched-distance game capture, ignored
`local/realgame/phaselock/matched_650/` (Ice_P, an Adult Bullymong 650 uu away, horizontal FOV 77.55 degrees, frames at
fixed times after `SkillStartTime`). The host shots ran with `-owfov=62.15`, which the host's 4:3 conversion turns into
77.55 degrees at 16:9, with its dummy also 650 uu away. Frames: ignored `local/phaselock/b2/*-20261003-195308.png`.
Side-by-side at 0.25, 0.5, 0.8, 1.5, 3.0, 4.5, 4.8 and 5.0 s: `local/phaselock/compare/phaselock_host_vs_matched_r4_20261003-195308.jpg`.

**FOV check.** In the capture's pre-cast frame the bullymong's feet sit about 252 px below the horizon. The cylinder
bottom is 222 uu below the eye at 650 uu, which gives f of about 738 px (82 degrees horizontal). That is near the
capture's 77.55 degrees (f 797) and far from the 93.9 degrees that the host's maintain-Y reading of `FOVAngle` 77.55
would give. The host's default capture FOV (BL2 setting 90, 106 degrees at 16:9) is much wider than the game's. That
default belongs to the host camera, not this lane, and was not changed.

**Stock data found this round:**
- `Mat_SirenOrbBlackMOD` carries a scalar parameter `DepthBias` (default -20); `_NoBias` (the hand fizzle's) has none.
  `Mat_SirenGlowMOD` has `Bias` -15, and the smoke parent `Mat_Wispy_Smoke` has `DepthBias` -18.
- Both black materials leave `EmissiveColor` unconnected (so they darken toward black) and wire `Opacity` from an
  alpha channel. This supports round 3's reading (darkness from alpha, colour unused).

**Changes (host stand-ins unless said):**

| Critic item | Cause | Change |
|---|---|---|
| Thick bright ring, lit interior | The black disc was fully dark only to half its radius. Measured at the same draw scale, the core sprite's bright magenta band falls at 0.46-0.70 of the black quad's radius and the bubble texture's thin rim at about 0.70 | Black disc fully dark to 0.6 of its radius: the band is hidden and the rim stays as a thin, dim edge |
| Target fully lit inside the bubble | The darkening quad sat at the target's centre, so the target's front half was in front of it | A material's negative `DepthBias` / `Bias` default (read from the template JSON, following parents) moves the sprite that many uu toward the camera and shrinks it by the same ratio: same outline, different depth test (UNVERIFIED reading of the parameter) |
| How dark | The game keeps the target at about a third of its brightness and the interior deep violet (mean about (50, 52, 98)). UE3 blended into an 8-bit target that clamps after each blend; UE5's float target lets the additive bubble layers sum to about 2.5 before the black | Black darkness capped at 0.9 where opaque geometry lies within 40 uu behind the quad (the target: a third of the 8-bit brightness, since (1/3)^2.2 is about 0.09). Capped at 0.96 where the scene is more than 80 uu behind (the interior: 1 - 0.1 / 2.5). Calibrated against the one capture |
| Release ring blue-violet | One reading of `Mat_SirenGlowMOD` cannot give both the game's cobalt cast flashes (needs the brightness-keeping tint) and its cyan-white ring (needs a plain multiply clipped per channel: violet rim x (0.4, 16, 30)). The colours and alpha curves do not separate the two | A labelled per-emitter override in `OpenWillowPhaselockFx.cpp` (`EmitterOverrides`): the end template's `Brighten` uses the plain multiply. The release is now a bright cyan-blue ring |

**Bubble size: not changed, not confirmed.** The script reads `Pawn.Mesh.Bounds.SphereRadius` / `BubbleFXScale`
(local script reading). With full-width sprites that puts this bullymong's rim (0.8 of the `Sphere` quad's half-width)
at 1.18 x 300.8 = 356 uu. The capture measures 167-174 uu, 2.1 times less. Two readings reproduce about 178 uu and
cannot be told apart here, because this bullymong's mesh radius (300.8) is twice its collision radius (150):
- the mesh radius with half-width sprites, or an extra halving somewhere;
- the collision radius with full-width sprites, which contradicts the script reading.

The host hand orb's size relative to the palm favours full-width sprites.

**Also seen in the matched frames, not changed:**
- The release starts at 4.77 s in this capture and the bubble is gone by 5.0 s; the host's outro is at 4.60 s (the
  10-02 timing) and its release ring is still up at 5.0 s.
- A pale blue floor glow under the bubble, against round 1's light-channel stand-in (the light reaches only the
  target).
- The host interior leans magenta where the game's is blue-violet.
- The 0.6 s white starburst.

### Round 5 (2026-10-03): bubble size rule confirmed in game, host calibrated to it

**Size rule: confirmed in game on 2026-10-03.** Source: the real-game lane's SDK reads of the spawned bubble emitters plus
frames, on three bullymong variants (baby, adult, ranged) at 650-668 uu (ignored `local/realgame/phaselock/size_rule/`).
This supersedes round 4's "two readings":
- Each bubble emitter's `DrawScale` = the lifted pawn's `Mesh.Bounds.SphereRadius` at that emitter's own spawn /
  `BubbleFXScale` (66.7). The intro at the lock and the loop 0.2 s later get different values as the pose changes
  (adult 4.28 then 3.90). Intro `DrawScale` x 66.7 is within 1-2 % of the mesh sphere radius read at +0.72 s on all three.
- The collision radius plays no part (150 / 150 / 64, constant).
- `DrawScale3D`, the component's `Scale` and `Scale3D` are 1 and its translation 0, so `DrawScale` is the only size input.
- The loop's visible rim (the blue-minus-red ridge on the frames) is 47.6 / 48.6 / 49.5 uu per `DrawScale` unit
  (mean 48.6, 0.73 x 66.7): a template constant.
- It changes little as the collapse rises (adult 47.8 / 48.6 / 44.7 at 1.5 / 3.0 / 4.5 s).

**Host before:** one draw scale, taken at the lock, for all three templates. The rim ridge measured on host frames
(same detector) fell at 0.88 of the `Sphere` emitter's half-width; `PhaseLockBubble_Dif_Tex`'s own rim peaks at 0.8 and
the surrounding glow moves the ridge out. That is 86.9 uu per unit, about 1.8 times the game's.

**Host now:**
- Each template takes the mesh-bounds draw scale at its own spawn (`AOpenWillowCombatTarget::BubbleDrawScale`).
- All three bubble templates are drawn at 48.6 / (0.44 x the `Sphere` StartSize) of that draw scale, about 0.56. This
  is a host calibration: the cause of the difference was not found (a sprite-size convention, or the stripped
  `Mat_SirenEnemyOrb` graph). The scale is uniform because the game's dark interior and streaks scale with its rim.

**Size check (host frames at the matched FOV, dummy at 668 uu, same detector):**
- The dummy's mesh radius is 109.4, so its stock `DrawScale` is 1.640 and the expected rim is 48.6 x 1.640 = 79.7 uu.
- Measured rim radius: 79.7 / 79.7 / 79.7 uu at 1.5 / 3.0 / 4.5 s (190 px rim to rim), ratio 0.999.

**Other changes (stand-ins, UNVERIFIED):**
- **Floor light restored.** The lock light reaches the floor as its data says. Every 10-03 game frame shows a pale blue
  pool under the bubble; round 1's target-only stand-in came from the 10-02 snow frames, where it was not noticed.
- **Interior colour.** The bubble's black orb (the loop and end `ModulateBlack` emitters only, per emitter) blends toward
  a deep blue-violet, linear (0.022, 0.022, 0.06), instead of black. This stands in for UE3's clamp after every blend,
  which saturated the additive layers to near-white before the orb darkened them. UE5's float target left a magenta
  interior instead.
- **Interior result.** At 3.0 s the host interior measures (38, 37, 84), against the game's (39-48, 38-45, 88-100).
  The hand orb's emitter (same material, nothing additive under it) stays black.

**Timing.** The real-game lane's three first locks put the release at 4.608 s after `SkillStartTime`, which matches the
host's 4.60 s. Its frames show the cyan-blue release ring still up at 5.0 s; matched_650's earlier disappearance was a
second lock of the same pawn (0.175 s longer hold, cause unknown; not modelled). No timing change.

**Still different:**
- The host dummy stays lit in front of the smaller bubble. Its front lies further than the 20 uu depth bias in front
  of the quad; in game the bullymong shows at about a third of its brightness.
- The floor pool is fainter on Sanctuary's dark asphalt than the game's pool on snow and sand.
- The 0.6 s starburst at the hand.

## Round 6 (2026-10-04): the effect materials read from the shader cache

AI-assisted (Claude). The cooked material graphs of the effect materials are stripped (rounds 1-5 guessed what each one does
with its textures), but the game keeps each material's compiled pixel shader and uniform-expression set in
`RefShaderCache-PC-D3D-SM3.upk` (the same cache the weapon paint model was read from, `DECISIONS.md` 2026-10-02). This round
reads those shaders for every material the Phaselock templates use. Listings, shader maps and the working scripts stay under
ignored `local/orch/A/shader/`; nothing below is a listing. Formulas and constants are recorded because the host needs them;
each reading is `UNVERIFIED` against the running game except where a comparison says otherwise.

### How a material is found (own words)

1. A cooked `Material` export ends with the material's compiled-resource block: a 16-byte id (it is also the key of the shader
   map), a count and the referenced textures (negative = import, positive = export number). The id is the window in that tail
   that occurs 3 or more times in the cache, one hit preceded by 16 zero bytes (the map start) and one preceded by the
   material's own name as a length-prefixed string (the copy that is followed by the uniform expressions).
2. The map start is a static parameter set (id, four empty counts for a plain material), then the engine and licensee versions
   (832, 46), a count and a table of 32-byte records (shader type name, shader id, name again), then one table per vertex
   factory. The base-pass pixel shader for the sprite factories is the entry whose type name starts with
   `TBasePassPixelShader`; its code is found by the shader id (a record with the id, 6 bytes, a size, then the code, which
   starts with the version token).
3. The uniform expressions (vectors, scalars, textures; parameters with their defaults, sums of `Time`, rotations of the form
   cos/sin of time x speed, `Periodic` = frac) follow the name copy. The pixel shader's constant table names which register
   holds which expression, so each constant in the code maps to a parameter or a time function.
4. `research/d3d9_bytecode.py` (written from Microsoft's public description of the token format) prints the code; the
   interpretation below was done by hand.

Checks: each material's texture list agrees with UModel's reading of `ReferencedTextures` wherever UModel prints one; the
uniform-expression parser consumes its arrays exactly; the black orb's scalar is `DepthBias` -20, the same value round 4 read
from the tagged data.

### What every pixel shader has in common

* Particle colour (RGB) and alpha arrive as vertex data; most materials use only the alpha or only the colour. The sprite UV
  runs 0..1 over the quad. Time is the material time in seconds.
* **Soft-particle fade.** The scene depth is decoded from the alpha of the scene-colour copy (8192 x sqrt(w) for w > 0); the
  fade is `saturate((scene depth - sprite depth) / D)`, with `D = 1 - DepthBias` for materials that have a `DepthBias` or `Bias`
  parameter and a literal constant otherwise (41 for the bubble sphere and core, 51 for the spikes, 21 for the hand glows and
  the black orb, 16 for the brighten modulate, 19 for the wispy smoke). It is 1 where the scene is far behind the sprite plane
  and 0 where the scene is at or in front of it. **This replaces round 4's reading of `DepthBias` as a camera-ward shift of the
  sprite**, which was wrong: the parameter is the fade distance.
* The shaders write colours up to 4 (`min(c, 4)`); the host clamps every layer to 0..1 before blending, as an 8-bit target
  would (UNVERIFIED reading of the game's target format; see "Host" below).
* The vertex shader is the ordinary camera-facing billboard (rotation about the view axis; velocity alignment is a switch). It
  has no depth bias. The dynamic-parameter vertex factory carries `SphereCollapse` as vertex data, but **no bubble pixel shader
  reads it**, so whatever that parameter does lives in a vertex shader that was not read.

### Material models (formulas and constants)

Notation: `uv` is the sprite UV; `R(a, q)` rotates `q` by `a` radians; `P(x)` = frac(x); `t` = time; `alpha`/`col` = particle
alpha/colour; `fade(D)` as above.

| Material (blend) | Reading |
|---|---|
| `Mat_SirenEnemyOrb` (additive), the bubble sphere | Texture 0 is a smoke map, texture 1 a flat signed normal map (values within +/-0.2 of zero), texture 2 `PhaseLockBubble_Dif_Tex`. Two rotating, panning reads of the smoke map, `R(0.58t, 2uv - 0.5) + (P(0.2t), P(0.1t)) + 0.5` (its red) and `R(-0.75t, 2uv) + (P(-0.4t), P(0.1t))` (its blue), make a displacement `(a, b)`. With `d2 = abs(uv - 0.5)^2` the displacement is scaled by `ring x (1 - 2 d2)`, `ring = 25 d2 - 1.5` for `d2 <= 0.1` and 1 beyond, and is zero where `ring < 0` (inside radius 0.245) or `1 - 2 d2 < 0`. Two reads of the normal map at `R(3t, uv - 0.5) + 0.5` and `R(1.5t, uv - 0.5) + 0.5` (n1, n2) give a direction `(n2.y + n1.x, sqrt(1 - n2.x^2 - n2.y^2) + n1.x)`. The bubble texture is read at `uv + displacement x direction`; result `alpha x texel x fade(41)`. The particle colour is unused. |
| `Mat_EnemyOrbCoreColor` (additive), the violet haze | Colour `col x core`, `core` = `EnergyOrbCoreColor_Dif_Tex` read at `uv + (noise - 0.1) x (tangent-space view direction)` (a small parallax; noise is a scrolling smoke read at `uv + (P(0.4t), P(0.5t))`). Weight `core.r x alpha x mask x d2 x fade(41)`, `mask` = `EnergyOrbCenter_Dif_Mirror` read at `2uv`. The haze is zero in the middle and brightest toward the quad's edge. |
| `Mat_SirenOrbEnergySpikes` (translucent), the blue streaks | Smoke red at `uv + (P(0.25t), 0)` shifts the spike texture's UV by `0.05 x smoke` in both axes; colour `col x spike`; alpha `saturate(spike.r x (1 - 3 m)) x alpha x fade(51)` with `m` the soft mirror mask read at `(2u, 0.5v + 0.76)`, which cuts the middle of the sprite out. |
| `Mat_SirenOrbEnergySpikesMOD` (modulate) | Scene multiplied by `1 - fade(51) x col x spike` per channel (the particle colour is (5, 5, 5)): dark streaks under the blue ones. |
| `Mat_SirenOrbBlackMOD` (modulate), the dark core | Scene multiplied by `1 - darkness`, `darkness = (1 - alpha) x (1 - A^2) x fade(21)`, `A = 1 - saturate(1.5 (1 - saturate(8 d2))^2)` (0 out to 0.15 of the quad, 1 from 0.354). The bubble loop scales the particle alpha to 0, so the core is a hole that is full black in the middle and clear at 0.354 of the quad. `_NoBias` (hand fizzle) has no depth fade. Because of the fade it darkens scenery behind the sprite plane, **not the lifted target's near surface**. |
| `Mat_PowerUpTwirls` (translucent, sub-image) | The twirl atlas is read at a UV bent by two slowly rotating scroll-map reads (0.02 x); colour `(sqrt(t) - t) x col x (0.6, 0, 0.8)`, alpha `saturate(4 t.r^2) x alpha`. The loop template gives it a particle colour of about (0, 0.02, 0.04), so these are dark wisps. |
| `Mat_SirenEnergyRibbons` (additive, mesh) | Ribbon texture read at `(2u - dynamic parameter 0, v)`; colour `saturate(u - 0.05) x b^2 x alpha x col`, `b` its blue channel. |
| `Mat_SirenEnergySwirl` (translucent) | Colour `blue x col`, alpha `texture alpha x alpha`. |
| `Mat_SirenHandGlow` (translucent) | `g` = the soft mirror mask at `2uv`; colour `g x col`, alpha `g x alpha x fade(21)`. |
| `Mat_SirenHandGlowShattered` (translucent) | The star-burst texture times `col`, alpha `texture alpha x alpha x fade(21)`. |
| `Mat_SirenHandInnerOrb` (translucent), the blue palm orb | Four reads of a nebula map in two rotating frames (0.18t about 0.5 and -0.25t about 0.8, each with pans) combine as `(n3 - n4) + (n1 - n2) + 2` and shift a read of the mirrored centre map at `2uv` (m1); with the unshifted read `m2`, `k = sat(2 m2) + 12 (1 - m2)^6 m2 (m1 - sat(2 m2))`. Colour `lerp((0.2508, 0.6524, 0.9323), 1.25 x orb texture, k)`, alpha `saturate(k) x alpha`. The particle colour is unused. |
| `Mat_SirenHandPowerDiffuse` (translucent) | Colour is the particle colour; alpha `35 x nebula(uv + (0, P(0.2t))) x (1 - (1 - 0.1 smoke(uv + (P(0.15t), P(0.2t)))) (1 - mask))^2 x alpha x fade(16)`. |
| `Mati_Wispy_Smoke_Cloud_SubUV` (translucent, sub-image) | Colour `a x col`, alpha `saturate(a x alpha) x fade(19)`, `a` the sub-image alpha (two frames blended by the particle's frame fraction). |
| `Mat_SirenGlowMOD` (modulate) | Scene multiplied by `1 + min(3, fade(16) x alpha x mask(2uv) x col)`: an overbright modulate. |
| `Mat_PhaseLockScreenEffect` (translucent, full-view) | Replaces the view by `saturate(scene^2 x col x mask)` blended by the particle alpha; the mask is `PhaseLockScreenMask02_Dif_Tex` (the second referenced texture; rounds 2-5 used the first, a UV mask). |
| `Mat_SirenSuction` | A refraction: the scene colour read at the screen UV offset by a distortion map, alpha `sat(15 x mask x alpha) x sat((depth - 20) / 512) x fade(26) x square mask`. Not drawn by the host. |

Textures (sampler state from the `Texture2D` exports): wrap addressing and sRGB unless listed. Clamp: `PhaseLockBubble_Dif_Tex`,
`EnergyRibbon_Dif_Tex`, `EnergySwirl_Dif_Tex`, `Square_Mask_Dif`, `Tex_Lens_Flare_Wide_Prime`. Mirror: `EnergyOrbCenter_Dif_Mirror`,
`EnergyOrbSoftMod_Dif_Mirror`. Not sRGB: `Nrm_Test` (a signed two-channel format in the game) and
`EnergyOrbScreenUVDistortion_Nrm_Tex`. `Smoke2_GP_Dif` and `PhaseLockScreenMask02_Dif_Tex` live in `WillowGame.upk`.

### The arms' tattoo glow (Master_Player, read the same way)

`Mati_Siren_Hands` is an instance of `Master_Player` with both static switches off; the cache holds that permutation (id and
switch values match). The power-emissive term of its base-pass pixel shader is, in our words: glow colour `p_PowerEmissiveColor`
(0, 14.55, 20) x `p_EnablePowerEmissive` x `(1 - f^2)` x mask x the diffuse texture, where `f` is the red channel of a fire-tile
texture read at `0.6 x uv + (P(0.00333 t), P(-0.0333 t))`, `mask` is the **B channel of `p_Masks` at (0.5 u, 0.5 v + 0.5)** and
the diffuse is `p_Diffuse` at the UV. A second, always-on term uses `p_EmissiveColor` and the B channel at (0.5 u, 0.5 v).
Rounds 1-5 read the mask at (0.5 u, v), the full-height left half, which also contains the glove silhouettes in B; that
lit the whole sleeve. The tattoo shapes sit in the lower-left quadrant (u < 0.5, v > 0.5) of the B channel.

### Not an effect of the skill: the sigil under the target

The game frames show a magenta/teal eye sigil low in the view from about 0.76 s to the end of the skill. It stays at the same
screen position in two captures with different targets and camera, so it is HUD (the action-skill indicator), not a world
effect. This record's earlier note that it is a ground decal (`matched_650/NOTES.txt`) is wrong; it belongs to the HUD owner.

### Target animation (stock chain)

Read in `NATIVE_PHASELOCK_PRESENTATION.md` (Lane E, `UNVERIFIED`): the lifted pawn plays `PhaseLock_Lift` (blend-in 0.4 s) at the
cast, then `PhaseLock_Loop`, then `PhaseLock_Fall` stretched to 0.5 s on release, then `PhaseLock_Land`; its mesh is re-centred
in Z after 0.35 s (VInterpTo speed 5). Clip lengths for the sets that carry them are in `local/phaselock/census/target_anims.txt`.
The Sanctuary target dummy's AnimSet has none of the four clips, so the stock data gives it no lifted animation. The host
already looks the clips up next to the imported idle clip and plays them when they exist; round 6 added the mesh re-centring.
No enemy with these clips is hosted yet (the slice has Marcus and the dummy), so the animation path has not been exercised.

### Host (round 6)

* `host/ue5/import_phaselock_fx.py` builds the models above as materials with one HLSL node each (`EXACT`), sprite UV, time,
  scene and sprite depth and the particle colour as inputs. The brighten modulate is a UE5 modulate with an unclamped factor.
  Host readings (all `UNVERIFIED`): each additive or translucent layer's colour is divided by its largest channel when that is
  above 1 (keeps the hue; a per-channel clip made the spikes cyan, no clip made them white); the bubble warp is scaled by 0.3
  (full strength frills the rim on every frame, the game's rim is clean at 1.5 and 3.0 s and frilled at 4.5 s); the violet haze
  is multiplied by 3 (the shader's own weights give a haze several times fainter than the frames); the dark streaks under the
  spikes (`Mat_SirenOrbEnergySpikesMOD`) are not drawn, because the shader darkens but no frame shows dark streaks; the
  distortion `Mat_SirenSuction` is not drawn.
* `OpenWillowPhaselockFx.cpp`: the depth-bias shift, the per-emitter colour and `HueOnly` overrides are gone (they belonged to
  the readings above that were wrong). `OpenWillowCombatTarget.cpp`: the bubble's size calibration uses the measured texture
  ring (0.78 of the half-width) so the factor is 0.63; the mesh re-centring; `-owbubbleradius=<uu>` stands in for the pawn's
  mesh bounds radius so that a small test target can be compared with the game's bullymong. `OpenWillowWalker.cpp`: the
  tattoo overlay gets the diffuse texture; more capture times (0.30-0.75 s).
* Capture commands: `tools/run_phaselock_shots.ps1 -Out local/orch/A/roundN -Extra '-owfov=62.15','-owbubbleradius=255'`.

### Round 6 comparison (matched 650 uu frames) and what is still different

Host frames: `-owfov=62.15` (77.55 degrees at 16:9) and `-owbubbleradius=255` (the engine-shape dummy is much smaller than the
adult bullymong, so its bounds radius is replaced for the comparison). Game frames: `local/realgame/phaselock/matched_650/run2`
(0.25-4.5 s) and `size_rule/adult68` (release). Ranked by how visible the difference is, after round 6:

1. **Bubble interior haze colour:** the game's band between the dark core and the rim is blue-violet (mean about (54, 56, 119)
   at 0.6-0.85 of the rim radius); the host's is pink-violet even with the x3 calibration. The core texture is magenta; what
   turns it blue in the game is not found.
2. **The lifted target is not dimmer inside the bubble.** In the game the adult bullymong shows at about a third of its
   brightness. The black orb's depth fade says it should not darken the target's near surface, and no other layer explains
   it. Open.
3. **Streaks are thinner than the game's soft fans** (the game's are about 40-50 px thick at 1.5 s; the shader gives a similar
   figure for the spike texture, but the host's visible part is thinner). Dark streaks under them are not drawn.
4. **The target's animation:** none (no stock clips for the dummy; no enemy with clips is hosted).
5. **Hand:** the swirl arcs are heavier and whiter than the game's thin blue arcs at 0.30-0.45 s; the palm orb is a little
   smaller; at 0.6-0.7 s the host shows the star-burst fan while the game keeps the orb in the palm.
6. **HUD sigil** (not mine): see above.
7. Floor pool on dark asphalt is fainter than on snow; release ring at 5.0 s is brighter in the host.

### Round 7 (2026-10-04, after an independent score of 5/10 for round 6)

The critic's ranked gaps were: an opaque black bubble interior with a thin pink rim; a hand orb lost behind a white star-burst at
0.55-0.75 s; no floor pool or blue scene light; a swirl too heavy and white at 0.30-0.40 s; thin straight release rays. Changes
(all host calibrations are `UNVERIFIED` and are marked as such in `host/ue5/import_phaselock_fx.py`, whose HLSL now uses plain
constants such as `/ 41.0` and prose comments):

* **Interior.** The black orb's darkening is capped at 0.85 and its soft fade has a floor of 0.5, set per emitter for the
  bubble templates only (`EmitterScalars` in `OpenWillowPhaselockFx.cpp`); the hand's dark blob keeps the shader's values. The
  game shows a translucent navy-violet interior with the target at about a third of its brightness, which the shader's own
  fade would not give (it spares the target's near surface); the cause stays open.
* **Rim and haze colour.** The bubble ring is tinted (0.7, 0.9, 1.3) and the haze (0.5, 0.6, 1.5): the texture is pink-violet,
  the game's rim and band are blue. The ring's wobble now grows with the collapse value (0.15 to 0.7 of the shader's push), so the
  rim is clean early and frayed before release, as in the 4.5 s frame.
* **Light.** The lock light's brightness is multiplied by 4 for UE5's unitless units, which gives the pale blue floor pool
  (`PhaselockLightIntensity()` still reports the data's units). 8 gave a hard-edged disc.
* **Hand.** Swirl alpha x0.5 and bluer; star-burst alpha x0.4; the brighten modulate x0.4. The palm orb now reads at 0.45 and
  0.65 s; at 0.55 s the host's hand is still partly hidden by the flash.
* **Dark wisps** (`Mat_PowerUpTwirls`) alpha x0.5.
* Not changed: release shards (host rays are still thin; the game's are angular ribbons), the dummy's pole shape against the
  bullymong, the target's animation (no stock clips for the dummy), the oval look of the bubble (the host dummy's pose/camera).
* A try with the aim point 40 uu higher missed the cast (outside the magnetism radius) and was reverted.

### Round 8 (2026-10-04, after a round 7 score of 6/10)

Frames: `local/orch/A/round15/` (host) against `matched_650/run2` and `size_rule/adult68` (game). Findings and changes; each host
calibration is `UNVERIFIED`:

* **The release "does not collapse" gap was a test-aid error, not an effect rule.** `SphereCollapse` is not read by any bubble
  pixel shader, and the end template's sphere does not shrink until 0.8 of its life, so nothing in the effect data shrinks the
  sphere. What shrinks it in the game is the draw scale, mesh bounds sphere radius at spawn / 66.7, and the pawn's bounds change
  with its pose: the SDK probe read 290 uu at the lock and 193 uu once lifted (`size_rule/adult68`). The host's constant stand-in
  radius gave the intro, loop and end templates the same scale. `-owbubbleradius=` now takes three values, one per template (290,
  260, 185 for the adult bullymong: the loop's value from its measured draw scale, the end's from the 193 lifted pose and the
  game's size at 4.8 s). The host's own dummy does not change pose, so its bubble does not shrink; the stock rule would shrink
  it if its animation changed the bounds. `FParse::Value` stops at commas by default, which made the first try use 290 for all
  three.
* **Hand effect timing and arm pose.** The game's palm orb is opaque by 0.44 s. The orb's alpha scale ramps from 0 at 0.15 to 1
  at 0.35 of its 1.25 s life (alpha 2, so opaque at about 0.25), which only allows that if the effect spawns about 0.1 s before
  the clip's 0.25 s notify; the dark disc is gone by 0.39 s in the game, where the shader's own ramp would keep it to 0.6 s. The
  host now starts the hand effect 0.08 s early (0.12 put the orb ahead of the game's at 0.40 s). Reading the orb's height from
  the frames, the host arm dropped about 0.05-0.07 s before the game's, so the cast clip plays at 0.85 speed (a new rate argument
  of `PlayAction`; the notify time is divided by the same rate). The cause (a blend-in on the game's special move, or a play-rate
  scale from the caller's `SpecialMoveData`) was not found.
* **Palm orb colour:** the shader's 1.25 x orb texture gives a white-blue core; the factor is 0.6 so the orb is the game's
  saturated blue with a highlight. Star-burst alpha x0.2.
* **Interior opacity, measured.** Method: take the annulus 0.45-0.8 of the rim radius (inside the rim, outside the target) and
  the same pixels in the pre-cast frame, convert both to linear luminance, and report the mean ratio, the regression slope (how
  much of the background's contrast gets through) and the correlation. Game (centre 645, 300, radius 205 px): ratio 0.44 / 0.47,
  slope 0.26 / 0.21, correlation 0.45 / 0.37 at 1.5 / 3.0 s; mean colour (28, 32, 86) and (31, 35, 84) of 255. So the game
  interior is neither opaque nor see-through: about a quarter of the background's contrast survives. Host with cap 0.85: slope
  0.06 / 0.15. Host now (cap 0.72, round 15): ratio 0.57 / 0.51, slope 0.11 / 0.26, mean colour (36, 38, 122) / (41, 41, 91).
  The host centre and radius used (640, 190, 195) are approximate, so these figures are rough. The script is
  `local/orch/A/interior.py` (ignored).
* **Pink fringe on the rim:** ring tint (0.55, 0.9, 1.4), haze tint (0.3, 0.5, 1.6).
* **Floor pool:** attenuation radius x1.6, falloff exponent at least 3, light colour pulled halfway to white in RGB, gain 3
  (an HSV blend turned the light pink).
* **End template interior:** darkening cap 0.45 (the game's interior at release is light blue).
* **Open:** the egg-shaped look at 3.0 s (the host dummy's pose and the camera; a re-aim after the lift missed the magnetism
  radius); the angular ice-shard facets of the 0.75-0.8 s flash and its white bleach (the host has a smooth swirl); the release's
  long straight shards (the host's ribbon meshes are curved swooshes, the game's shards are angular and thin; the mesh and
  material that draws them was not identified); the fist clenching around the orb at 0.65 s (the arm clip is the same asset, so a
  different clip or an additive layer is suspected, not checked).

### Round 9 (2026-10-04, after a round 8 score of 5.5/10; the blind A/B preferred round 8 on hand and release and tied the bubble)

Frames: `local/orch/A/round16/` (host, `-owbubbleradius=290,233,167`) against `matched_650/run2` and `size_rule/adult68`. Host
calibrations are `UNVERIFIED`.

* **Bubble too large during the hold: the cause was a calibration mistake, plus a reference mix-up.** (1) Round 6 changed the
  ring factor from the ridge detector's 0.88 of the sprite half-width to the texture ring's 0.78. The 48.6 uu per draw-scale
  unit was measured with the ridge detector (the blue-minus-red ridge, which lies in the rim's outer glow), so the host must use
  the same 0.88: with 0.78 the host's ridge, found by the same detector on the round 15 frame, was 254 px against a calculated
  225 (centre 640, 140; the game's 202-214). `TextureRim` is back at 0.88 (factor 0.56). (2) The reference frames are the
  `matched_650/run2` cast, whose loop scale (bubble radius 165-174 uu / 48.6 = 3.4-3.6, bounds equivalent about 227-233 uu) is
  smaller than the `adult68` cast's (260). The loop stand-in is therefore 233 for these frames. The stand-ins remain per cast, because the pawn's
  bounds at each template's spawn differ from cast to cast.
* **Release size:** with the factor corrected, 167 uu for the end template gives a radius near the game's 70-75 px.
* **Black sphere and swirl at 0.30-0.40 s: the round 8 change caused it.** Starting the whole hand effect 0.08 s early moved the
  swirl (size curve 15x to 1x over a 0.2 s life) and the disc (2.8x to 1x) to later, smaller points on their curves at 0.30 s. Only the
  palm orb ("Center") and the dark disc ("ModulateBlack") need to run ahead (their alpha ramps), so the effect starts at the
  notify again and those two emitters start 0.1 s into their particles' life (`AgeShifts` in `OpenWillowPhaselockFx.cpp`). The
  swirl and sphere sizes at 0.30 s and 0.40 s return to round 7's.
* **Tattoos:** the glow colour is multiplied by 0.3 so the bands stay solid blue.
* **Ray streaks:** the spikes' alpha is halved and each streak is read compressed 1.6x along its length (it ends at 0.31 and 0.69
  of the sprite).
* **Floor glow:** between rounds 7 and 8: gain 5, radius x1.2, falloff exponent at least 3, colour halfway to white. A faint pale
  patch under the bubble that lights nearby ground.
* **Release interior:** the end template's darkening cap is 0.65 (was 0.45), so the interior is less bright.
* Open (unchanged): ice-shard flash, straight release shards, fist clench, target animation, egg shape.

### Round 10 (2026-10-04, after a round 9 score of 5.5/10; the blind A/B kept round 9's hand and preferred round 8's bubble and release)

Frames: `local/orch/A/round18/` (host, `-owbubbleradius=290,233,210`) against `matched_650/run2` and `size_rule/adult68`. Calibrations `UNVERIFIED`.

* **Release size: round 9's "70-75 px radius" was wrong.** Re-measured with the ridge detector (`local/orch/A/ridge.py`: the annulus with
  the highest mean blue-minus-red on a 4x downsampled 1280 px frame): adult68 at 4.80 s centre (648, 228) radius 112 px, at 5.00 s radius 176 px
  (the hold frames of the same cast read 232 at 1.5 and 3.0 s). The critic's reading of the 640 px sheet (shell about 180 px across, radius about 90
  there, 180 px of 1280) is the 4.80 s figure. The end-template stand-in is therefore 210 (round 8 used 185 with factor 0.63; the factor is now
  0.56). The end template's dark cap stays 0.65 (the interior still reads as a void).
* **Ray streaks:** between rounds 8 and 9: full alpha, read 1.15x compressed along their length (round 8 full strength and uncompressed; round 9
  0.5 and 1.6x was invisible; 0.8 and 1.25x was still faint). Translucent blue streaks now show to the left and right of the bubble.
* **Floor glow:** gain 8, radius x1 (the data's 500), falloff exponent at least 1.5, colour 35% toward white: a pale patch on the ground under
  the target. Rounds 8-9 (gain 3-5, x1.2-1.6) were invisible and round 7's (gain 8, falloff 3, full radius) was a hard disc.
* **Black hole at 0.30 s:** yes, the age shift did shrink it: shifting the disc's particle age moved its size curve (2.8x to 1x) too. The disc's
  size curve is now read at the unshifted age (only its alpha runs ahead), and the orb shift is 0.05 s instead of 0.1 s (the orb was about 45 px
  at 0.40 s where the game has none yet). The hole is about 280 px against 300 in the game.
* **Rim:** a white-hot inner edge (the brightest part of the ring pushed toward white-blue).
* Open (unchanged): interior opacity (the host shows the street through the bubble more than the game), ice-shard flash, straight release shards,
  fist clench, target animation, egg shape.

### Round 11 (2026-10-04, consolidation after round 10 scored 5.5/10; the blind A/B kept round 10's bubble and preferred round 8's hand and release)

Frames: `local/orch/A/round22/` (host, `-owbubbleradius=290,233,210`). Calibrations `UNVERIFIED`.

* **Which emitter makes the 0.65 s spikes:** the hand template's `EndBrightness` emitter (material `Mat_SirenHandGlowShattered`, the star-burst
  texture on a rectangular sprite). It has a 0.25 s emitter delay, spawns 2 particles lasting 0.25-0.45 s, and its size multiplier runs from 0 to
  15 over 1.5 life units, so it flashes at 0.5-0.95 s after the effect starts, i.e. at about 0.65 s in the frames. It fires because the stock
  template has it. The game frames show no such rays, only a few sparks, and the cause of that difference was not found (the material may draw
  something else in the game than the star texture). The host now multiplies that emitter's strength by 0.15 (`Gain`, a new per-emitter
  material scalar set in `EmitterScalars`).
* **Hand at 0.40 s:** the palm orb now starts 0.03 s into its life (was 0.05 s): no orb is visible at 0.40 s, the palm is open with wide arcs, and
  the orb is solid by 0.45 s.
* **Ground glow:** gain 6.5, radius 0.85 of the data's, falloff exponent at least 1.75, colour 20% toward white: a soft pale-blue patch rather than
  round 10's hard white pool; a wide, soft streak replaces the thin ground rays (the spike texture is read at 0.55x on the vertical axis, so each
  streak is about 1.8x wider and about two fit on a sprite).
* **Release:** the end template's spikes are at half strength and its three ribbon meshes at 0.4, so fewer straight lines cross the interior.
  The shards remain straight; making them curved ribbons outside the bubble was not done.
* **0.30 s void: not fixed.** The void is about 200 px against the game's 330. Two ways of enlarging it (the quad 1.6x, and widening the core inside
  the shader by 1.5x) both made the dark disc fade out to a faint blue ring instead of growing, although the disc's alpha and the darkening formula
  are unchanged; cause not found. The host keeps the shader's own size. Recorded as open together with the 0.80 s whiteout and the interior opacity.

### Round 12 (2026-10-04, after a round 11 score of 6.3/10)

Frames: `local/orch/A/round25/` (host, normal FOV) and `round25fp/` (host with `-owfpfov=45`), both `-owfov=62.15 -owbubbleradius=290,233,210`.
Calibrations `UNVERIFIED`.

* **Foreground FOV adoption.** With Lane C's `-owfpfov=<n>` the arms and gun use UE 5.8's first-person primitive FOV. The hand effect's pooled
  sprite components (`UOpenWillowFxComponent`) now take the same first-person primitive type when the arms are first-person relevant
  (`bFirstPersonSpace`, set in the hand-effect spawn code in `UpdatePhaselockPresentation`; Lane C's BeginPlay hunk is untouched). The tattoo
  overlay is a material on the arms and follows them already. The bubble, the screen effect and the light stay in world space. Checked by
  capture: with the flag the swirl, disc and orb stay on the hand, and the hand sheet is much closer to the game (`review7/hand_pairs_fp45.jpg`):
  the void is large and black, the hand and orb are the right size. The effect is a separate decision for the maintainer (the flag is still opt-in).
* **The 0.30 s void.** Diagnosis, with a run-time override (`-owfxscalar=Template:Emitter:Parameter:Value[;...]`, no rebuild or import) and a
  dark-pixel measure over the region above the hand (fraction of pixels below 20/255):
  * disabling the disc (`DarkCap` 0) removes the void entirely (0.6% dark): the void is that one emitter's, not another layer's;
  * removing the depth fade (`FadeFloor` 1) with the core 1.5x wider changed little: the soft-particle fade is not the cause;
  * the core 1.5x wider grew the dark area from about 10% to 25-33%. Round 11 had read this as "fading out" because the enlarged core is a gradient
    the street shows through, not a solid black disc as in the game; **the cause was the shader's soft falloff, not the size**;
  * the game's size is also what the narrower foreground FOV gives: with `-owfpfov=45` the unchanged disc is 21% dark and about the game's width.
  Fix for the normal-FOV case: `CoreScale` 1.6 (wider core in the same quad) and `CoreSharp` 2.5 (steeper edge: coverage x2.5, saturated) on the
  hand's black orb only; both are skipped when the hand effect is drawn in first-person space. The bubble's black orb keeps the shader's values.
  The void is now solid and slightly larger than the game's.
* **Rim weight.** Four more reads of the bubble ring, moved radially by +/-4% and +/-8%, are added as a violet-blue halo (x0.3, tint 0.7, 0.6,
  1.3): the rim is a soft glow of roughly 12-15 px with a violet haze inside, against the texture ring's 5 px.
* **Not done:** the wide horizontal blue shafts beside the bubble (150-250 px, lower priority), the frothy white-cyan rim at 4.50 s, the 0.80 s
  whiteout, interior opacity, the other open items.

### Round 13 (2026-10-04, after a round 12 score of 6/10)

The blind A/B put round 12's thick rim worse than round 11's, and the normal-FOV void widening worse than round 11's hand; round 12's
release and first-person support were kept.

* **Reverted:** the bubble rim halo (round 11's rim is back) and the hand void's `CoreScale` / `CoreSharp` (in the shader and the table).
  With first-person space on, the void is unchanged anyway and is the right size. Round 12's note above on the void's diagnosis (the soft falloff,
  not the size) stands.
* **Kept:** first-person space for the hand sprites (`bFirstPersonSpace`), the release changes, and the `-owfxscalar=Template:Emitter:Parameter:Value[;...]`
  diagnostic (below).
* **Hand, with `-owfpfov=45` (all captures this round use it):** the swirl was a pale ring about 170 px where the game has saturated cyan ribbons
  with dark gaps about 280 px across. `Mat_SirenEnergySwirl` is back to alpha 0.8 and a more saturated cyan (0.25, 0.75, 1.3); the palm orb's blue
  is deeper (x0.7) and its texture factor 0.9 (veins). With the first-person field of view these give the right ribbon size and a deep-blue orb. A
  1.3x orb size table entry made the orb too large (about 90 px against 70) and was removed. The hand and arm remain smaller than the game's (arm
  mesh and foreground FOV are not this lane's) and the forward fist (a different clip pose) was not attempted.
* **Quest suite with the flag on:** run through a local copy of `tools/test_quest.ps1` that appends `-owfpfov=45` to both launches
  (`local/orch/A/test_quest_fp.ps1`, ignored).

`-owfxscalar` (diagnostic, host only): on the command line, `-owfxscalar=Part_SirenASHandOrb:ModulateBlack:DarkCap:0` sets that material scalar on
every sprite of the named emitter of the named template at creation time, so a material or emitter hypothesis can be tested without a rebuild or an
asset import. Several overrides are separated by `;`. It is read once and does nothing when absent.
