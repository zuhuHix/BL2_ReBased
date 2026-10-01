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

## Manifest

`python tools/prepare_action_skill.py --reader build/Release/ow-package.exe --game "$env:OPENWILLOW_BL2"`
writes `local/character/action_skill_siren.json` (about 20 s). Format `openwillow.action_skill/1`:

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
