# Native skills: skill points, the skill tree, skill grades and effects, action-skill cooldown (2026-10-05)

AI-assisted (Claude), analyst lane G7. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script behaviour was read with `research/script_disasm.py`
(local listing of `WillowGame`); enum values come from the package enums; field names were fixed with
`tools/ghidra/class_layout.py` (the in-memory tree structure of `PlayerSkillTree` is private to the native class and is
described here by what it holds, not by offsets).

Already covered elsewhere and not repeated: skill points `max(0, L - 4)` and the level curve (confirmed in game),
`ExpEarn` / `ApplyExpPointsToExpLevel` ([NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md),
[NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md)), Phaselock constraints
([NATIVE_PHASELOCK_TARGETING.md](NATIVE_PHASELOCK_TARGETING.md)), the attribute modifier stack formula
([NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) section 1), the attribute-initialization evaluator
([NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) section 1), Maya's data ([PHASELOCK_STOCK_DATA.md](PHASELOCK_STOCK_DATA.md)).
Overlap: lane G5 (GFx: `SkillTreeGFxObject`, HUD skill icons), lane G4 (controller helpers), lane G3 (engine core:
resource pools are only read here as far as the cooldown needs).

## Summary
| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| PlayerSkillTree.UpgradeSkill | `native function bool UpgradeSkill(SkillDefinition Skill)` | Spend one point on a skill | high | UNVERIFIED |
| PlayerSkillTree.SetSkillGrade | `native function bool SetSkillGrade(SkillDefinition Skill, int SkillGrade)` | Load / client mirror; unlock bookkeeping | high | UNVERIFIED |
| PlayerSkillTree.GetSkillState | `native function bool GetSkillState(SkillDefinition SkillDef, out SkillTreeSkillStateData OutSkillState)` | Grade, unlocked flag for UI and activation | high | UNVERIFIED |
| PlayerSkillTree.GetBranchState | `native function bool GetBranchState(SkillTreeBranchDefinition BranchDef, out SkillTreeBranchStateData OutBranchState)` | Branch counters in the UI | medium | UNVERIFIED |
| PlayerSkillTree.GetTierState | `native function bool GetTierState(SkillTreeBranchDefinition BranchDef, int TierNumber, out SkillTreeTierStateData OutTierState)` | Tier unlocked flag, points in tier | medium | UNVERIFIED |
| PlayerSkillTree.GetSkillPointsSpentInTree | `native function int GetSkillPointsSpentInTree()` | Respec refund, "earned any points" | high | UNVERIFIED |
| PlayerSkillTree.GetActionSkill / HasTrainedASkillOfType / AllSkills / AllSkillsOfType | lookups | Phaselock lookup, activation loop | high | UNVERIFIED |
| PlayerSkillTree.Initialize | `native function Initialize(SkillTreeDefinition SkillTreeDef)` | Builds the runtime tree | medium | UNVERIFIED |
| PlayerSkillTree.SaveSkillSaveGameData / ApplySkillSaveGameData | `native function ...(PlayerSaveGame SaveGame)` | Quest save of grades | medium | UNVERIFIED |
| WillowPlayerController.InitPlayerSkillTree | `native final function InitPlayerSkillTree()` | Creates the tree for the class | medium | UNVERIFIED |
| WillowPlayerController.ResetSkillTree | `native final function int ResetSkillTree(bool bIgnoreProficiencies, optional bool bIsCharacterLoad)` | Respec, load | high | UNVERIFIED |
| WillowPlayerController.HasPlayerEarnedAnySkillPoints | `native function bool HasPlayerEarnedAnySkillPoints()` | UI gating | high | UNVERIFIED |
| WillowPlayerController.GetActionSkillDuration | `native final function float GetActionSkillDuration()` | Phaselock lock time | medium | UNVERIFIED |
| SkillDefinition.DoesSkillPassMinGradeTest | `native function bool DoesSkillPassMinGradeTest(int SkillGrade)` | Passive skills need grade 1 | high | UNVERIFIED |
| Skill.CalculateModifierValue / CalculateModifierValueFromDefinitionEffectArray | `native function float CalculateModifierValue(out SkillEffectData EffectData, int SkillGrade, Object ContextSource)` and `...FromDefinitionEffectArray(SkillDefinition InDefinition, int EffectIndex, int SkillGrade, Object ContextSource)` | How a grade scales an effect | high | UNVERIFIED |
| Skill.AddSkillEffect | `native final function AddSkillEffect(Controller EffectInstigator, out SkillEffectData InEffect)` | Effect becomes a modifier | high | UNVERIFIED |
| Skill.AdjustModifiers | `native final function AdjustModifiers(EAdjustModifierMode AdjustMode, optional bool bSuppressNotify)` | Apply / remove modifiers | high | UNVERIFIED |
| Skill.ForceRefresh | `native function ForceRefresh()` | Re-evaluate after a grade change | high | UNVERIFIED |
| SkillEffectManager tick, RefreshSkillsForInstigator / RefreshSkillsAffectingInstigator | `native function ...(Controller SkillInstigator)` | When effects are re-applied | medium | UNVERIFIED |
| ResourcePool per-frame update (internal, via the cooldown pool) | no script signature | Phaselock cooldown drain | medium | UNVERIFIED |

Script (not native) functions that carry most of the behaviour: `WillowPlayerController.ExpLevelUp`, `OnExpLevelChange`,
`ServerUpgradeSkill`, `ClientSetSkillGrade`, `GetSkillUpgradeCost`, `CheckSkillActivation`, `ServerSetSaveGameData`,
`ServerSetSkillSaveGameData`, `ServerSkillSaveGameDataCompleted`, `ServerPurchaseSkillTreeReset`, `ClientResetSkillTree`,
`UpdateSkillsAfterTreeReset`, `StartActionSkill` (see NATIVE_PHASELOCK_TARGETING), `ServerStartActionSkill`,
`ActionSkillCallback`, `StartActiveSkillCooldown`, `IsActionSkillOnCooldown`; `SkillEffectManager.ActivateSkill`,
`DeactivateSkill`, `UpdateSkillGrade`, `DeactivateAllSkillTreeSkillsForPlayer`; `Skill.Initialize`, `Activate`,
`Deactivate`, `Pause`, `Resume`, `UpdateGrade`, `BuildSkillEffects`; `SkillTreeGFxObject.CanUpgradeSkill`,
`RequestSkillUpgrade` (UI side, lane G5).

Enums (package): `ESkillType` Passive 0, Action 1, ActionAugment 2, Kill 3, Proficiency 4. `ESkillState` Deactivated 0,
Active 1, Paused 2. `EEffectDurationType` Infinite 0, Timed 1. `EEffectTarget` None 0, Self 1, Allies 2, Enemies 3, All 4,
Pets 5. `AttributeModifier.EModifierType` Scale 0, PreAdd 1, PostAdd 2. `Skill.EAdjustModifierMode` InitialAddModifer 0, AddModifer 1, RemoveModifer 2. `ESkillTreeFailureReason` (UI) NoSkillPoints 0,
SkillLocked 1, SkillMaxed 2, DataIssue 3, NoFailure 4.

## 1. Where skill points come from, where they live, who is told

**Storage.** Unspent points are one integer on the player replication info: `WillowPlayerReplicationInfo.GeneralSkillPoints`
(a second, unused counter `SpecialistSkillPoints` sits beside it; the data gives it no points). Grades are not stored on
the controller: they live inside the controller's `PlayerSkillTree` object (field `WillowPlayerController.PlayerSkillTree`)
as one grade per skill, plus per-tier and per-branch bookkeeping. `ExpLevel` and `ExpPointsNextLevelAt` are on the same
replication info. A save stores the unspent points and one (skill, grade) pair per skill; spent points are never stored,
they are implied by the grades (section 3, load).

**Award (script `WillowPlayerController.ExpLevelUp`, confirmed amount, see NATIVE_PROGRESSION).** In order, when the level
is below `GetMaxExpLevel`: `ExpLevel` goes up by one; the old unspent count is remembered; `GeneralSkillPoints` increases
by the integer from evaluating `GlobalsDefinition.GeneralSkillPointsPerLevelUp` with the controller as context (the new
level is already in place, so the "level >= 5" condition sees the new level); a telemetry call records points earned;
`SpecialistSkillPoints` increases by its own definition's value (none in this data); then **the controller's
`FireSkillPointsChangedDelegates(GeneralSkillPoints)` runs**, which calls every registered `OnSkillPointsChanged` delegate
with the new total (the Status menu and HUD register such delegates). If the old count was 0 and the new count is above 0,
two "first skill point" stats are incremented. Then the level-up feedback message is broadcast, the pawn's second-wind
reason is set, `OnExpLevelChange(true, not bCheated)` runs and a "first level up" stat is incremented.
`OnExpLevelChange` sets the base of `ExpPointsNextLevelAt` to the requirement for level + 1, recomputes the attribute
initialized state, sets the pawn's game stage to the new level, re-evaluates the class's intrinsic armour, and (when feedback is
wanted and at least one second has passed since the last one) runs the class's `OnLevelUp` behaviors and, for a natural
level-up, `OnLevelUpNaturally`; with feedback it also calls the owning client's `ClientOnExpLevelChange(level)`, which queues
the HUD level-up display and the achievement check. On a remote client the replicated change of `GeneralSkillPoints` fires
the same delegates from the replication-info's replicated-event handler.

**Who is told when.** The skill-points delegates fire at the moment the integer changes (level-up, spend, respec, load);
the HUD movie method `WillowHUDGFxMovie.UpdateSkillPoints` (native, lane G5) and the skill tree UI's
`HandleSkillPointsChange` are the listeners. The first-time contextual prompt for the skill tree appears in
`DoLevelUpNotifications` when the shown level is 5 or more (script, plays the level-up sound at most once per second).

**Implementer checklist.**
1. A level-up adds `int(PointsPerLevelUp)` (1 from level 5, else 0) to the unspent counter after the level has increased.
2. Fire the skill-points-changed delegates with the new total right after the add.
3. The first-skill-point stat only when the counter goes from 0 to above 0.
4. Nothing re-awards points for levels already passed: loading a save restores the stored unspent counter, not a formula.

**Open.** The delegate bodies registered by the menus (lane G5); the telemetry calls.

## 2. Spending a point

### 2.1 Script entry and what it does (not native; read from the listing)
The Status menu's skill tree calls `SkillTreeGFxObject.RequestSkillUpgrade`, which first asks `CanUpgradeSkill`; a
"no failure" answer calls the server function `WillowPlayerController.ServerUpgradeSkill(Skill)` and then asks the tree to
update its branch progression for the movie; any other answer plays the failure sound.

`CanUpgradeSkill` (UI-side checks, in this order): the tree has no state for that skill: data issue; the skill's tier is
locked **or the player's `ExpLevel` is below 5** (constant in the script): locked; the grade is already `MaxGrade`: maxed;
the UI's copy of the unspent points is 0 or less: no skill points; otherwise no failure. The level-5 check lives only
here; the native upgrade and the server function do not test the level. `SkillDefinition.PlayerLevelRequirement` was not
found in any script read for this note.

`ServerUpgradeSkill(Skill)` (runs on the authority), in this order, and does nothing at all if `Skill` is None:
1. cost = `GetSkillUpgradeCost(Skill)`: **0 for a Proficiency skill, otherwise 1**;
2. if `GeneralSkillPoints >= cost`: call the native `PlayerSkillTree.UpgradeSkill(Skill)`; only if that returns true:
3. subtract `cost` from `GeneralSkillPoints`; record a telemetry event;
4. read the skill's new state from the tree; ask `SkillEffectManager.UpdateSkillGrade(Self, Skill, newGrade)`; **if no
   active skill instance existed for it (returns false), run `CheckSkillActivation(Skill)`** so a passive is activated
   for the first time (see 5.1);
5. call the owning client's `ClientSetSkillGrade(Skill, GeneralSkillPoints, newGrade)`, which sets the same grade in the
   client's tree copy (native `SetSkillGrade`) and fires the `OnSkillGradeChanged(skill, points, grade)` delegate (UI);
6. for a Proficiency skill a weapon-proficiency feedback message is broadcast.
(The exact jump structure of step 4 could not be read reliably from the listing offsets; the order above is the
straight-line reading. UNVERIFIED.)

### 2.2 PlayerSkillTree.UpgradeSkill(Skill)
- **Reads:** the runtime tree (below): the skill's entry, its tier's unlocked flag, the skill's grade, `SkillDefinition.MaxGrade`.
- **Does:** returns false (no change) if the skill is not in this tree, if its tier is not unlocked, or if its grade is
  already `MaxGrade`. Otherwise sets the grade to grade + 1 through the same routine as `SetSkillGrade` (clamped to
  `0..MaxGrade`), which recomputes tier unlocks (section 2.3), and returns the result of that routine (true when the grade
  changed). If the new grade is exactly 1, the tree is owned by a `WillowPlayerController` and the skill is of type Action
  or Kill, a one-time presentation hook is called on an object held by the controller (a hint/prompt call with two
  name constants; not identified; no gameplay effect read). Proficiency skills are in the tree's skill list but belong to
  no tier of a branch (see "tree structure").
- **Calls into script:** none. **Calls other natives:** none by name.
- **Edge cases:** returns false for a skill with grade 0 whose tier is locked; there is no point or level check here.
- **Implementer checklist:** (1) refuse unknown skill / locked tier / maxed skill, in that sense, without touching state;
  (2) grade + 1; (3) recompute tier unlocks of that skill's branch; (4) the caller subtracts the point only on true.
- **Open:** the identity of the first-train hook.

### 2.3 Tier and branch unlock rule (internal routine run after any grade change)
The runtime tree is built by `PlayerSkillTree.Initialize` from the `SkillTreeDefinition`: the root
`SkillTreeBranchDefinition` and, recursively, its `Children` (the three class trees are the root's children). For every
branch, each `Tiers` entry becomes a tier record that stores two numbers computed at build time: its position, and the
**cumulative requirement = the sum of `PointsToUnlockNextTier` of all lower tiers in that branch** (0 for the first tier).
Each branch also stores `MaxPointsForBranch`-style totals: the sum of all its tiers' `PointsToUnlockNextTier` (the points
that open the child branches) and, for display, the sum of the maximum grades of its skills.

After a grade changes (UpgradeSkill, SetSkillGrade, save/load replay), for the **branch that contains the skill** only:
1. Walk the branch's tiers from the first. Skip tiers already unlocked.
2. At the first locked tier: if the **total points currently spent in the branch** (sum of grades of every skill in all its
   tiers) is at least that tier's cumulative requirement, mark that one tier unlocked. Either way, stop (at most one tier
   opens per call).
3. Only when every tier of the branch is unlocked: if the branch's total points are at least the sum of all its tiers'
   `PointsToUnlockNextTier`, unlock the **first tier of every child branch**.
A grade decrease never re-locks anything; only a full reset clears unlock flags.
The branch's own first tier is unlocked only by the reset (root) or by its parent as above.

For the slice's data this is the host's existing rule (tier opens when the branch holds the sum of the lower tiers'
`PointsToUnlockNextTier`) and also explains the action skill gate: the root branch has one tier of 1 point
(the action skill), so spending that point opens the first tier of the Motion, Harmony and Cataclysm trees; spending
points in a tree then opens its higher tiers.

**Per-tier point count.** Points in a tier = sum of the grades of that tier's skills, counting only as many skills as the
tier's layout has cells (3 when a branch has no layout object). So proficiency and any skill not placed in a layout cell
do not count.

### 2.4 Implementer checklist (spend)
1. `ServerUpgradeSkill` order above; the unspent counter, not a derived number, is the authority for "can afford".
2. Action skill: grade 0 to 1 costs one point; its tier is unlocked from the start (root tier 0).
3. Tree skills: locked until the cumulative requirement is met by points already in the branch.
4. Spending fires `OnSkillGradeChanged` and `OnSkillPointsChanged`.

## 3. The runtime tree natives

### PlayerSkillTree.SetSkillGrade(Skill, Grade)
Clamps the grade to `0..SkillDefinition.MaxGrade`, stores it and, if it changed, runs the unlock rule of 2.3 for the skill's
branch and returns true; returns false when the skill is unknown or the grade is unchanged. **No check that the tier is
unlocked** (it is the load/replay path). Used by `ClientSetSkillGrade` and `ServerSetSkillSaveGameData`.

### PlayerSkillTree.GetSkillState / GetBranchState / GetTierState / GetSkillPointsSpentInTree
- `GetSkillState(Skill, out State)`: false for an unknown skill; otherwise fills `SkillDefinition`, `ParentBranchDefinition`
  (the branch of the skill's tier), `TierNumber`, `SkillGrade` and `bIsUnlocked` (the skill's tier flag).
- `GetBranchState(Branch, out State)`: `BranchDefinition`, `PointsSpentInBranch` (2.3), `MaxPointsForBranch` (the sum of the
  maximum grades of the branch's skills) and `bIsUnlocked` = the first tier's flag.
- `GetTierState(Branch, Tier, out State)`: `ParentBranchDefinition`, `TierNumber`, `PointsSpentInTier`, `bIsUnlocked`.
- `GetSkillPointsSpentInTree()`: the sum of `PointsSpentInBranch` over every branch (proficiencies excluded).
- `UpdateBranchProgression(Movie)` only computes three 0..1 progress values (points in each child branch divided by that
  branch's unlock total) times a movie field for the skill tree's progress bars; presentation, lane G5.
- `GetActionSkill()`: the tree's recorded action skill (the first skill of type Action found at build time).
  `HasTrainedASkillOfType(type)`: true if any skill of that type has grade above 0 and its tier unlocked.
  `AllSkills()` / `AllSkillsOfType(type)` are the iterators `UpdateSkillsAfterTreeReset` and `ActionSkillCallback` use;
  they include proficiency skills.

### PlayerSkillTree.Initialize(Definition)
Does nothing if already initialised. Builds the branch / tier / skill records from the definition (2.3), records the
action skill and the first skill of each skill type, appends the globals' proficiency skills to the skill list, and
then performs the same reset as 3.1 without proficiency preservation (so every grade starts at its
`DefaultStartingGrade`, all tiers locked except the root's first tier).

### 3.1 Reset and respec (`WillowPlayerController.ResetSkillTree(bIgnoreProficiencies, bIsCharacterLoad)`, internal tree reset)
1. Count points spent before. Set every skill's grade to its `DefaultStartingGrade`, except that proficiency skills keep
   their grade when `bIgnoreProficiencies` is true. Clear every tier's unlocked flag, then unlock the first tier of the root
   branch. Count points spent after. The result (returned to script) is **before minus after**, i.e. the refund.
2. Tree listeners are notified with that number (`ISkillTreeListener.HandleSkillTreeReset`, implemented natively on the
   controller; the HUD/skill-tree UI register for it).
3. Back in the controller routine, when running with authority and `bIsCharacterLoad` is false, a one-bool event is sent to the
   owning client (by name; most likely `ClientResetSkillTree`, which resets the client's copy when it is not the authority).
Script users: `ServerPurchaseSkillTreeReset` (pawn must be alive; cost = `EvaluateInitializationData(GlobalsDefinition.
CostToResetSkillPoints, Self)` credits; needs credits >= cost) adds the returned refund to `GeneralSkillPoints`, records
telemetry, removes the credits and marks the replication info dirty; otherwise `ClientPurchaseSkillTreeResetFailed`
(not-enough-money dialog). `UpdateSkillsAfterTreeReset` (script; the caller was not found in script, probably the native
listener) deactivates every tree skill for the player, zeroes the kill-skill timer, runs `UpdateKillSkills(false)`, re-checks
activation of every skill (so grade-0 skills with no grade rule come back) and runs `HandleBadassSkillActivation`.

### 3.2 Save and load
- Save: `SaveSkillSaveGameData(SaveGame)` writes one (SkillDefinition, grade) entry per skill in the tree, including grade 0.
- Native `ApplySkillSaveGameData` sets each entry's grade (clamped) and runs the unlock rule; **but script does not use it
  for loading**. `ApplyPlayerSaveGameData` -> `ServerSetSaveGameData` sets level, experience and **the stored unspent
  `GeneralSkillPoints` directly**, then calls `ResetSkillTree(false, true)`; the controller's script `ApplySkillSaveGameData`
  (called by the save-loading code, caller not traced here) iterates the save's skill list calling `ServerSetSkillSaveGameData(skill, grade)` (server) or `ClientSetSkillGrade` (client); each server
  call does native `SetSkillGrade`, then `SkillEffectManager.UpdateSkillGrade`, then `CheckSkillActivation`; finally
  `ServerSkillSaveGameDataCompleted` recomputes the attribute initialized state. Points are not deducted on load.
- Because `SetSkillGrade` replays one skill at a time and opens at most one tier per call, the tier flags after a load depend
  on how many calls follow the point at which a threshold is met; in the usual case (a save lists every skill, grade 0
  included, and there are more calls than tiers) all due tiers end up open. UNVERIFIED; the host should recompute all tier
  flags from the loaded grades instead of replaying one-at-a-time.

### 3.3 Tree creation: `WillowPlayerController.InitPlayerSkillTree()`
Called by script `OnPlayerClassChange` right after it clears `PlayerSkillTree`. If the controller has no tree yet it creates
a `PlayerSkillTree` object (outer = the controller, stored in `PlayerSkillTree`), requests the class's skill tree
definition from the player pawn data manager (`LoadSkillTreeDefinitionAsync`) with the completion event name
`SkillTreeDefinitionLoaded`, and then performs a reset of the empty tree. When the load completes, script
`RunStreamingDataEvent` handles `SkillTreeDefinitionLoaded`: it calls `PlayerSkillTree.Initialize(definition)`,
`RegisterListener(Self as ISkillTreeListener)`, and schedules `NotifyReadyToLoadPendingSavegame`. Saved grades are therefore
applied only after the definition has loaded.

### 3.4 `WillowPlayerController.HasPlayerEarnedAnySkillPoints()`
True when the replication info's `GeneralSkillPoints` is above 0 or the tree exists and has spent points. False when
there is no replication info. Used to decide whether skill UI hints may show.

## 4. How a grade becomes an effect

### 4.1 Which skills become active, and when
- `CheckSkillActivation(Skill)` (script): if the tree knows the skill, **Passive (0) and Proficiency (4) skills** are given to
  `SkillEffectManager.ActivateSkill(controller, skill, none, grade)`. Action (1), ActionAugment (2) and Kill (3) skills are
  not activated here: the action skill is activated by `ServerStartActionSkill`, augments by `ActionSkillCallback` (activated
  while the action skill is running, deactivated when it ends, for every ActionAugment skill in the tree), and kill skills by
  `UpdateKillSkills` on kills.
- `ActivateSkill` (script): does nothing if `bAllowSkillActivation` is false (then `DeferActivateSkill` queues the request);
  needs a definition, an instigator and `SkillDefinition.DoesSkillPassMinGradeTest(grade)`; creates a `Skill` object of the
  definition's `SkillClass`, initialises it (grade, state Deactivated), appends it to `ActiveSkills`, for Kill skills first
  refreshes all skills of that instigator, then runs `Skill.Activate`.
- `SkillDefinition.DoesSkillPassMinGradeTest(Grade)`: **true unless `bSubjectToGradeRules` is set and `Grade` is 0 or less.**
  A skill with the flag therefore only exists while the player holds at least one point in it; one without it can be
  activated at grade 0.
- `Skill.Initialize`: copies definition, instigator, extra target; grade = `UpdateGrade(InGrade)`; `Duration` base :=
  `InitialDuration`, `Range` base := `BaseRange`; state Deactivated.
- `Skill.Activate` (script): state := result of the constraint check (activation) ; if Deactivated, stop; record the
  activation time; register a behavior consumer and the definition's behavior provider with the behavior kernel; **build the
  effects** (4.2); if state Active, apply all modifiers (`AdjustModifiers` mode 0); timed skills record `StartTime`; Action
  skills notify the pawn (`ActionSkillStarted`); `NotifySkillEvent(SkillActivated)`; run the console commands in
  `SkillActivationActions` on the instigator; call the state-changed delegate; add the vision-mode effect if any.
- `Skill.Deactivate` / `Pause` / `Resume`: Deactivate notifies, runs `SkillDeactivationActions`, removes modifiers
  (mode 2), drops the applied-effect records, sets state Deactivated, informs the pawn (`ActionSkillEnded`), and
  unregisters the behavior consumer. Pause removes modifiers (mode 2), sets Paused and forces a refresh; Resume re-applies
  (mode 1), sets Active and forces a refresh.
- State changes caused by constraints are described in NATIVE_PHASELOCK_TARGETING section 4.

### 4.2 Skill.AddSkillEffect(Instigator, EffectData)
Called once per entry of `SkillDefinition.SkillEffectDefinitions` by `BuildSkillEffects`. It appends an applied-effect
record to `Skill.SkillEffects`: a copy of the effect data, the list of attribute **contexts** (the objects whose attribute
will be modified, section 4.4), and a freshly created `Core.AttributeModifier` object whose `Type` is the effect's
`ModifierType` and whose `Value` is the effect value for the skill's **current grade** (4.3). The modifier is not yet on any
attribute; `AdjustModifiers` does that.

### 4.3 Skill.CalculateModifierValue(EffectData, Grade, Context) and ...FromDefinitionEffectArray(Definition, Index, Grade, Context)
The second picks entry `Index` of `Definition.SkillEffectDefinitions` (0.0 when the definition is None or the index is out
of range) and uses the first. **Formula**, with `g` = the skill's current grade (the effective `Skill.Grade` attribute, see
4.5):
- if `g < GradeToStartApplyingEffect`: the value is **0**;
- otherwise
  `value = BaseModifierValue + PerGradeUpgrade x ((g - GradeToStartApplyingEffect) div step) + bonus`
  where `BaseModifierValue` and `PerGradeUpgrade` are `AttributeInitializationData` evaluated with the effect instigator as
  context (NATIVE_PROGRESSION section 1), `step = max(PerGradeUpgradeInterval, 1)` (`PerGradeUpgradeInterval` of 0 or 1 means
  every grade) and `div` is **integer** division;
- `bonus` is `Modifier` of the `BonusUpgradeList` entry with the **largest `GradeToApplyAt` that is <= g** (0 if none or
  if the nearest is 9999 or more grades below); the entries are not summed.
Single precision result. `tools/skill_stats.py` already uses `base + per x floor((g - start)/interval)` and a start default
of 1 for an omitted field; two differences to check: the bonus list is not modelled there, and an omitted
`GradeToStartApplyingEffect` is the struct default (0 unless the package states otherwise), not 1.

Examples (read from data, not from the game): Phaselock's Suspension, base 0.5 and +0.5 per grade, start 1, gives 0.5 x g
for g = 1..5; a Scale effect of +0.05 per grade gives 0.05 g.

### 4.4 Skill.AdjustModifiers(Mode, ...)
Walks every applied effect of the skill and, for each of its contexts, either adds or removes the effect's modifier object
on the attribute named by `EffectData.AttributeToModify`:
- **Mode 0 (InitialAddModifer, activate):** add for every effect, whatever the attribute kind.
- **Mode 1 (AddModifer, resume / re-apply):** add only for attributes whose definition has `bIsSimpleAttribute` false.
- **Mode 2 (RemoveModifer, pause / remove / deactivate):** remove only for attributes whose definition has `bIsSimpleAttribute` false.
Adding or removing goes through the attribute definition's `ValueResolverChain`: each resolver in order is asked to add (or
remove) the modifier on the context and the walk stops at the first resolver that reports failure. The usual resolver
(`ObjectPropertyAttributeValueResolver`) finds the named attribute property on the context object (caching the lookup per
class) and adds the modifier to that property's modifier stack, or removes it. **Adding refuses a modifier object already
on that stack** (no duplicates); either operation recomputes the property value from base and stack with the formula of
NATIVE_WEAPON_RULES section 1 and then notifies the owner of the attribute change unless the optional `bSuppressNotify` argument is true (the per-frame refresh
removes with notification suppressed and re-adds with it on). How a "simple" attribute's modifier is
removed on deactivation (modes 1 and 2 skip it) was not resolved; see Open.

### 4.5 When effects are (re)applied and how a grade change reaches them
- `Skill.UpdateGrade(NewGrade)` (script) sets the skill's `Grade` attribute base to **max(NewGrade, 1)** and sets
  `bForceRefreshModifiersNextTick`. `Skill.Grade` is an int attribute: effective grade = base + the modifier stack
  (class mods that raise skill levels act here; not in the slice).
- `SkillEffectManager.UpdateSkillGrade(instigator, definition, grade)` (script) finds an **active** (not Deactivated) skill
  of that definition for that instigator, calls its `UpdateGrade`, and returns true; false when none is active.
- **Per-frame refresh** (native, run for every skill in `ActiveSkills` from the manager's tick, before anything else
  about the skill): nothing for a Deactivated skill. Otherwise: take the new state from the constraint check; for timed
  skills (`DurationType` Timed), if the clock is before the start time and `bDoNotShiftPastCurrentTime` is set pull the
  start time back to now, and when `StartTime + max(Duration, 0) <= now` the skill expires (state Deactivated), except that a
  skill with an `ActionSkillArchetype` waits for that action skill object to report the ability finished (not read in detail);
  dispatch Deactivate / Resume / Pause when the state differs from the current one; **then, if the skill is not Deactivated
  and (the force flag is set, or the definition has `bAutoUpdateContexts` and the next context-update time has passed)**:
  if Active, remove the non-simple modifiers (mode 2); re-gather the contexts and recompute each effect's modifier
  `Value` from the skill's current grade (4.3); if Active, add the non-simple modifiers back (mode 1); clear the force flag
  and set the next context-update time to `now + SkillEffectUpdateIterval`.
  So a grade change takes effect on the **next manager tick** after `UpdateGrade`, by editing the existing modifier objects
  in place, re-applying them, and recomputing every affected attribute value.
- After the refresh the manager's tick removes skills that are Deactivated from `ActiveSkills`, and for Active skills with a
  tracked-skill type updates the HUD tracked-skill state.
- `Skill.ForceRefresh()` sets the force flag and runs the refresh for that skill immediately.
- `SkillEffectManager.RefreshSkillsForInstigator(Instigator)` does the same for every active skill whose instigator is that
  controller (guarded against re-entry). `RefreshSkillsAffectingInstigator(Instigator)` additionally refreshes skills of
  **other** instigators whose effects' resolved targets include this instigator (allies / auras).
- `SkillEffectManager.DeactivateAllSkillsForInstigator` and the script `DeactivateAllSkillTreeSkillsForPlayer` deactivate (remove
  modifiers, drop effects) all of a player's skills; used by respec and class changes.

### 4.6 Contexts (which objects get modified), as far as read
For each effect the contexts are re-collected on activation and on every refresh. For `TARGET_Self` effects they are the
objects the attribute's `ContextResolverChain` yields for the effect instigator (typically the player pawn, controller or
replication info, depending on the attribute: for example Maya's health and shield attributes live on the pawn, her
cooldown on the controller's pool). `EffectSourceInstanceDataName` on the definition redirects the instigator to a named
instance-data object. For Allies / Enemies / All / Pets the search uses the effect's range, target criteria and the dueling /
self / non-player flags. Not needed for the slice's self effects.
**Confidence low** for the target-search part; **medium** for the self case.

### 4.7 Implementer checklist (effects)
1. Passive tree skill with grade >= 1 -> create a skill instance at that grade, build one modifier per effect with the
   formula of 4.3, apply to the attribute named on the resolved context object (modifier stack formula as in weapons).
2. Spend -> grade + 1 -> if the skill is already active set its grade (>= 1) and re-evaluate every effect on the next tick:
   remove old values, recompute, re-apply; otherwise activate it.
3. Respec / deactivate -> remove every modifier the skill added.
4. Do not apply an effect (value 0) while the grade is below `GradeToStartApplyingEffect`.
5. Pause/resume (constraint changes) remove/re-add the modifiers; the modifier objects survive pausing.

## 5. Action skill (Phaselock): duration and cooldown, beyond NATIVE_PHASELOCK_TARGETING

### 5.1 WillowPlayerController.GetActionSkillDuration()
Returns 0 when the game is not running, there is no `PlayerSkillTree`, no action skill, or the skill manager has no active
instance of the action skill for this controller; otherwise the **current effective `Duration` of the running action-skill
instance** (`Skill.Duration`, base `SkillDefinition.InitialDuration` plus modifiers). Used by script for the active-ability
timing handed to the pawn's `ActionSkill` (`StartActionSkillActiveAbility(..., duration, ActionSkillTime, target)`). The
Phaselock lock time itself is a different number (the designer attribute `Att_Phaselock_Duration`, see PHASELOCK_STOCK_DATA).

### 5.2 Cooldown (script + resource pool)
- The cast (`ActionSkillCallback` with `bActivated` true) calls `StartActiveSkillCooldown`, which refills the controller's
  `SkillCooldownPool` to 100 percent (`RefillPercentage(1.0)`), counts the use stat, and activates / deactivates ActionAugment
  skills to match.
- `IsActionSkillOnCooldown`: the pool is valid and its current value is above 0. `IsActionSkillCoolingDown`: on cooldown and
  current below max (the exact second comparison is on the pool's max; read as "partly drained"). `GetSkillCooldownTime` =
  pool `MaxValue`; `GetSkillCooldownTimeRemaining` = pool current value; `ResetSkillCooldown` sets current to 0.
  `ServerStartActionSkill` refuses to start while on cooldown (except the special-cased `Skill_Gunzerking`), while the skill
  is already active it toggles it off only when `bCanBeToggledOff`.
- **Pool drain (native, `ResourcePool` per-frame update, base class; the experience pool overrides it).** Net rate per
  second = `ActiveRegenerationRate - ConsumptionRate` (+ `PassiveRegenerationRate`); if `OnIdleRegenerationRate` is non-zero it
  is added once the idle delay has passed since the value last changed. The cooldown pool's base consumption rate is 1 and
  regeneration 0 (data), so the current value falls by 1 per second from the refilled maximum toward the minimum. Per frame,
  `current = clamp(current + rate x dt + remainder, min, max)`; integer-valued resources keep the fractional remainder.
  If a regeneration pool is attached its value limits and is charged for the change. Nothing happens while
  `RegenerationDisabled` is non-zero. When the value reaches the minimum the definition's `OnResourceDepleted` behaviors
  run; leaving the minimum runs `OnResourceNotDepleted`; reaching the maximum runs `OnResourceRegenerated`, leaving it
  `OnResourceNotRegenerated` (mapping of the four events to slots read from definition field order; UNVERIFIED).
- **How skills modify cooldown.** Any skill effect whose attribute is `ConsumptionRate` (or `MaxValue`, or the regeneration
  rates) of the cooldown pool does it through the same modifier stack, so `ConsumptionRate` PreAdd -1 sets the net rate
  to 0 and pauses the drain, which confirms the semantics assumed for `Skill_Phaselock_CooldownManager`
  (PHASELOCK_STOCK_DATA). A Scale modifier on `ConsumptionRate` changes the drain speed (faster drain = shorter
  cooldown); on `MaxValue` it changes the cooldown length, and with `bUpdateCurrentValueOnExtremaChange` the current value
  follows a maximum change. The cooldown-completion cue is `ActionSkillCooldownComplete` (script; plays the class's
  "action skill available" sound when the action skill's grade is above 0; its wiring to the pool event was not read).
- **Implementer checklist:** pool current := max at cast; each frame current -= (consumption - activeRegen) x dt with
  consumption = (1 + sum PreAdd) x (1 + up)/(1 - down) + PostAdd evaluated per NATIVE_WEAPON_RULES section 1; clamp to
  [min, max]; ready when current <= 0 (0.0001 tolerance used by the engine for extrema tests).

## Edge cases collected
- `UpgradeSkill` on a skill from another class's tree: false (not in this tree). `SetSkillGrade` the same.
- A passive with `bSubjectToGradeRules` and grade 0 never activates; after a respec its modifiers are removed by
  `DeactivateAllSkillTreeSkillsForPlayer` and not re-created until a point is spent again.
- `Skill.UpdateGrade` never goes below 1; a skill whose spend returns grade 0 is not active in the first place.
- `ActivateSkill` while `bAllowSkillActivation` is false (loading): the request is queued, not lost.

## Open
- The identity and effect of the first-train hook in `UpgradeSkill` (a presentation call, not gameplay).
- What `bIsSimpleAttribute` means for removal: modes 1 and 2 of `AdjustModifiers` skip such attributes. If an effect's attribute
  is simple, the deactivation path as read does not remove its modifier; either simple attributes are not stack based or
  the modifier is dropped another way (object release). Check which of Maya's skill attributes are simple before trusting 4.4
  for them (data question, `AttributeDefinitionBase.bIsSimpleAttribute`).
- Context collection for non-self targets (4.6), the action-skill wait in the expiry test (4.5), `HandleSkillTreeReset` native
  body on the controller, `DeferActivateSkill` queue consumer, the exact event name sent after a reset.
- Whether the native tier unlock really opens at most one tier per call in normal play (data with `PointsToUnlockNextTier` 0
  tiers would matter; the three Maya trees use 5 per tier, the root 1).
- `SkillDefinition.PlayerLevelRequirement` (no use found); the level-5 rule exists only in the UI check.

## Not read yet
`PlayerSkillTree.GetTierLayout` (UI layout rows), `InitializeGFxHelper*`, `DumpTree`, `RegisterListener` / `UnRegisterListener`
bodies, `SkillEffectManager.GetActiveSkillForInstigator*`, `IsSkillActive`, `IsSkillPaused`, `NotifySkillEvent` /
`NotifySkillDamagedEvent` / `TriggerTakeHitEvents` (the event-response machinery for skills with `EventResponses`),
`Skill.GetAttributeContexts` body in detail, `SkillTreeGFxObject.*` natives (lane G5), `WillowHUDGFxMovie.UpdateSkillPoints` and
the other HUD skill natives (lane G5), `WillowClassMod.IsModifyingSkill`, `SkillExpressionEvaluator.Evaluate`,
`ActionSkill.*` natives other than their script call sites, `ExperienceResourcePool` update (covered by the XP notes).

## Corrections to earlier notes
- NATIVE_PHASELOCK_TARGETING (section 4 and "cooldown pool's native update was not read"): the pool update is now read
  (5.2). Drain = `ActiveRegenerationRate - ConsumptionRate`; the CooldownManager's PreAdd -1 on the consumption rate gives
  zero net change, as assumed.
- `tools/skill_stats.py` (grade formula): the form matches the native; add the `BonusUpgradeList` rule (4.3) and check the
  default of an omitted `GradeToStartApplyingEffect`.
- Host `OpenWillowSkills` (`TrySpend`): rules agree with 2.1 to 2.3 (point check, max grade, locked tier by cumulative
  requirement, action skill first). Differences: the host checks `ActionGrade >= ActionPointsToUnlockTrees` per spend instead
  of the root branch's total points opening the child trees (equivalent for the shipped data); the native does not test the
  level, the UI does (level 5, equivalent because points start at level 5); the host skips `DefaultStartingGrade` and proficiency
  skills (not in the slice).
