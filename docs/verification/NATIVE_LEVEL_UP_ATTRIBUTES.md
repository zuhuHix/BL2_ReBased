# Native level-up attributes: what RecalculateAttributeInitializedState rebuilds, how a pool reacts, and the level-up refill (2026-10-07)

AI-assisted (Claude), analyst lane G23. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script behaviour was read with `research/script_disasm.py`
(the local WillowGame listing and a local Engine listing); data was read with `ow-package` through the existing
`tools/` decoders; field offsets were named with `tools/ghidra/class_layout.py` and are given as field names. The one
class that tool could not size (it stops at a map property in `WillowPlayerController`) was sized with a local copy that
gives a map property 60 bytes; the class size that comes out agrees with the size the executable registers for the class,
which is the oracle.

The question this note answers is the one left open by NATIVE_ATTRIBUTES section 8 and by the "Script swap 7" section of
[SANCTUARY_RPG_MISSION.md](SANCTUARY_RPG_MISSION.md): `WillowPlayerController.RecalculateAttributeInitializedState`, which
`OnExpLevelChange` calls after the level rises, was a thunk whose body was not read, and the level-up's health refill was
not modelled. Both are now read. Confirmation: part of this note was checked against two local real-game captures of a
level-up made on 2026-10-07 by lane L1 (`local/realgame/L1/levelup_run1.json` and `levelup_run2.json`, game-derived, local
only). They are **not yet recorded** in [REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md); the orchestrator should record
them. Where a statement says "observed", that is the source.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| WillowPlayerController.RecalculateAttributeInitializedState (thunk plus the controller-class and player-controller virtuals) | `native function RecalculateAttributeInitializedState()` (no arguments, no result) | Fire: the level-up's new maximum health, and the class-level attributes | high for what it touches, medium for the context of each read | UNVERIFIED (effect on the health pool observed) |
| ResourcePoolManager.RecalculateBaseValues (the pool-rebase helper the virtual uses) | `native final function RecalculateBaseValues(ResourcePoolManager Mgr)` | Fire: rebases every pool, health included | high | UNVERIFIED (observed through the health pool) |
| ResourcePool.CalculateBaseValues (script, Engine) with ApplyUpgrades and UpdateCurrentValueOnExtremaChange | `function CalculateBaseValues(optional bool bOnlyCalculateAttributeInitializedState)` | Fire: the pool-side rule: new base maximum, current value follows only a full pool | high (script) | UNVERIFIED (observed) |
| ResourcePool.GetMaxValue | `native final function float GetMaxValue(optional bool bBase)` (parameter name from usage) | Fire: heal amount, load-time fill | high | UNVERIFIED |
| ResourcePool.SetCurrentValue | `native final function SetCurrentValue(float Value)` | Fire: clamp rule for fills | high | UNVERIFIED |
| ResourcePool.UpdateLastValues, ResourcePool.PoolIsNowFull | `native final function UpdateLastValues()`, `native final function PoolIsNowFull()` | Fire: the "was full" memory behind the follow rule | high | UNVERIFIED |
| WillowPlayerController.OnExpLevelChange (script, WillowGame) | `function OnExpLevelChange(bool bFeedback, bool bNaturalLevelup)` | Fire: the level-up order, the guard, the behavior runs | high (script) | UNVERIFIED (order observed) |
| PlayerClassDefinition.OnLevelUp collection (data: GD_PlayerShared.Behaviors.PlayerBehavior_LevelUp) through Behavior_AttributeEffect | data plus script | Fire: the refill of health, the cooldown reset, the shield recharge boost | medium-high | UNVERIFIED (health and shield effects observed) |
| WillowPawn.SetSecondWindReason | `native final function SetSecondWindReason(byte Reason)` | none (a reason code stored for the injured state) | medium | UNVERIFIED |

## The result in one paragraph

A level-up does three separate things to health, in this order. (1) `RecalculateAttributeInitializedState` rebuilds the
**base** of every pool's maximum from its data (health: `80 x 1.13^level`, minimum 20, evaluated for the new level) and
keeps whatever modifiers are on the maximum's stack; it does **not** touch a pool's current value unless the pool was
already full, in which case current follows the new maximum. (2) The player class's `OnLevelUp` behaviors then activate a
timed skill whose first effect adds the **effective maximum health** to current health once; the pool clamps the sum, so
the player ends at **full health**. The same skill resets the action-skill cooldown and a second skill boosts shield
recharge for four seconds. (3) Nothing rescales current health proportionally. Observed on 2026-10-07: from half health
and from a quarter of health the player was at the new maximum after the level-up, and the maximum after step 1 alone was
the new maximum with the old current value.

## 1. WillowPlayerController.RecalculateAttributeInitializedState

- **Signature:** no arguments, no result. Called from script by `OnExpLevelChange` (every level change), by
  `ServerSetSaveGameData`, `ServerItemSaveGameDataCompleted`, `ServerSkillSaveGameDataCompleted` and
  `ApplyCharacterClassDefaults` (load and class change, section 8), and for AI pawns and minds from their own setup.
- **The thunk:** the script-callable entry steps over its (empty) argument list and calls a **virtual method of the object's
  class** (slot 282 of the class's method table, counted from 0). One shared thunk serves `Controller`,
  `WillowPlayerController` and `WillowMind`, each reaching its own override; `WillowVehicle` has its own thunk and its own
  slot (not read). This confirms the NATIVE_ATTRIBUTES section 8 statement that it is a thin thunk and answers why the body
  was not found earlier: the class's method table was not located by name. It is located here through the class's
  registration (the constructor that the registration names stores the table address), and slot 282 of the player
  controller's table is the override described next.
- **Reads:** the controller's `Pawn`, `CharacterClass` (a `CharacterClassDefinition`; script sets it to the `PlayerClass`
  when the class is announced, `ClientNotifyClassChanged`, so for the player it is the same class definition), its
  `ResourcePoolManager`, and its `MyWillowPawn` with that pawn's `StatusEffectComp`.
- **Does (in order).** The override runs the controller-class behaviour first, then its own part.
  1. **Class-level pawn attributes (controller-class part).** Only when the controller has a pawn that is not a vehicle and
     has a `CharacterClass`:
     - `Pawn.EncumbranceResistance`'s **stored value** is overwritten with the evaluation of
       `CharacterClass.BaseEncumbranceResistance`. Unlike the 12 below, only the value is written: the base companion and the
       modifier stack are not touched and nothing is recomputed (medium confidence that this asymmetry is real; for Maya the
       data value is 0, so nothing visible changes).
     - twelve pawn damage-modifier attributes are re-initialised: for each, the **base** is set to the evaluation of the
       class's matching init data and the value is recomputed from the existing modifier stack with the stack formula of
       NATIVE_WEAPON_RULES section 1 (no change notification is sent). In order: Normal impact, Normal status effect,
       Explosive impact, Explosive status effect, Shock impact, Shock status effect, Corrosive impact, Corrosive status
       effect, Incendiary impact, Incendiary status effect, Amp impact, Amp status effect, fed from
       `BaseNormalDamageModifiers`, `BaseExplosiveDamageModifiers`, `BaseShockDamageModifiers`,
       `BaseCorrosiveDamageModifiers`, `BaseIncendiaryDamageModifiers` and `BaseAmpDamageModifiers` (each a pair:
       `ResistanceToImpact`, then `ResistanceToStatusEffect`).
  2. **Pool rebase (controller-class part, always, even with no pawn).** Every pool in the controller's
     `ResourcePoolManager` is rebased (section 2): by design the pools the controller owns (health, shield, accuracy, experience,
     both skill-cooldown pools and the ammo pools; that the health pool is one of them is observed, the rest follows from the
     class data). The health pool is therefore recalculated by this call, not by a separate native.
  3. **Status-effect resistances (player-controller part).** Only when `MyWillowPawn` exists, has a `StatusEffectComp`, and
     the controller's `CharacterClass` is a `WillowCharacterClassDefinition`: ten attributes on the pawn's status-effects
     component are re-initialised exactly as the damage modifiers (base set, value recomputed from the stack, no
     notification), in the order Ignite, Shock, Corrosive, Slow, Amp chance resistance, then Ignite, Shock, Corrosive, Slow,
     Amp duration resistance, from the class's `Base<Kind>ChanceResistanceModifier` and `Base<Kind>DurationResistanceModifier`.
- **Evaluation context:** every class-level read above evaluates its `AttributeInitializationData` with **the controller as
  the context source and no override source** (NATIVE_PROGRESSION section 1 evaluator).
- **Does not:** touch the player experience pool's current value, the inventory, the skills, the weapons or the controller's
  own attributes; send an attribute-changed notification; fire a script event.
- **Calls other natives:** the pool rebase of section 2 (which calls the script function of section 3 on each pool); the
  attribute-initialization evaluator (NATIVE_PROGRESSION section 1); the stack recompute (NATIVE_WEAPON_RULES section 1).
- **Constants / formulas:** none of its own. For Maya's class the data are: encumbrance resistance 0; all twelve damage
  modifiers 1; all ten status-effect resistances 1 (the defaults of `WillowCharacterClassDefinition`; her class sets none of
  them). So for Maya the class-level part writes the same numbers every call.
- **Edge cases:** no pawn: only the pool rebase runs. Pawn is a vehicle: the first part is skipped. No `MyWillowPawn`: the
  third part is skipped, the others still run. `CharacterClass` unset: the first and third parts are skipped.
- **Implementer checklist:**
  1. Calling it on a controller with a health pool whose data is `Init_PlayerHealth` changes the pool's **base** maximum to
     `max(20, 80 x 1.13^level)` at the controller's current `ExpLevel` and nothing else in that pool except by section 3.
  2. Modifiers already on the maximum's stack survive: the effective maximum is the new base through the same stack formula
     (observed: the ratio of effective to base maximum was 2.01747 before and after, from a Badass Rank modifier).
  3. Current health is unchanged by the call alone unless the pool was full (section 3).
  4. With no pawn the pool rebase still happens.
  5. It sends no notification and fires no script event.
- **Open:** the `WillowMind` and `WillowVehicle` versions (not read); the vehicle pools.

## 2. ResourcePoolManager.RecalculateBaseValues (the rebase helper)

- **Signature:** `native final function RecalculateBaseValues(ResourcePoolManager Mgr)`.
- **Does:** walks the manager's 16 pool slots in index order; for every slot that holds a pool it calls the pool's **script
  function `CalculateBaseValues` with its argument set to true** (the name is looked up as a function name, so a script
  override on a pool subclass would run; none exists in the shipped script of Engine or WillowGame). An empty slot or a
  None manager is skipped. This is the helper `RecalculateAttributeInitializedState` uses (section 1 part 2) and the script
  entry of this native is the same code with the manager taken from the argument.
- **Corrects** NATIVE_ATTRIBUTES section 8: the function whose name was "not decoded" is `CalculateBaseValues`, the
  argument is true, and the 16 slots are the manager's `ResourcePools` array.
- **Calls into script:** `ResourcePool.CalculateBaseValues(true)` on each pool.
- **Implementer checklist:** iterate pools in slot order; call the rebase with "only the attribute-initialised state" true.
- **Open:** the slot order that `CreatePool` produces (it does not matter for the rules read, no pool reads another).

## 3. ResourcePool.CalculateBaseValues (script), ApplyUpgrades and UpdateCurrentValueOnExtremaChange

Read from the Engine package script (`research/script_disasm.py`); this is script, so it runs on our VM unchanged once the
natives below exist.

- **Signature:** `CalculateBaseValues(optional bool bOnlyCalculateAttributeInitializedState)`.
- **Does, with the argument true (the level-up call), in order:**
  1. Nothing if the pool has no `Definition`.
  2. Sets the **base** of `MaxValue` to the evaluation of the definition's `BaseMaxValue` with the pool's
     `AssociatedProvider` as context. This is the "assign to an attribute" form: it stores the base and recomputes the value
     from the stack, with **no notification** (NATIVE_BYTECODE_OPCODES, opcode for "let attribute"), so the current value is
     **not clamped** by this step.
  3. Skips the resets of the minimum, consumption rate, regeneration rates and idle delay that the argument-false form does
     (those keep their stacks and bases as they are).
  4. Resets `RecentImpulseCount` and the **base** of `RegenerationDisabled` to their defaults (the stack is kept).
  5. `ApplyUpgrades`: if the pool's upgrade level (an attribute named by the definition, read from the provider) is above 0,
     the base maximum becomes the current base maximum plus the evaluation of the definition's `MaxValueUpgrade`. Health and
     shield have no upgrade data.
  6. Unless the pool is being initialised, `UpdateCurrentValueOnExtremaChange` (below).
  With the argument false it additionally resets the minimum, consumption, active and passive and idle regeneration rates
  and the idle delay to their definition bases (used at pool creation and reinitialisation, not on level-up).
- **UpdateCurrentValueOnExtremaChange:** if the definition's `bUpdateCurrentValueOnExtremaChange` is false, nothing. If true:
  - when current value is **at or above the maximum recorded at the last `UpdateLastValues` call**, current becomes the new
    maximum and `PoolIsNowFull` runs ("a full pool stays full");
  - otherwise, when current value is at or below the recorded minimum, current becomes the minimum;
  - in every case it ends by calling `UpdateLastValues`.
  So the test is against the **recorded** maximum, not the live one, and it is a follow rule, not a proportional rescale.
- **Which pools follow (read from data):** the default of the definition class is **true**; Maya's health pool and
  melee-cooldown pool inherit it, her experience pool and her active-skill cooldown pool set it to false. The shield pool
  inherits true (its base maximum is 0 from data; capacity comes from the equipped shield's modifiers, which this call does
  not touch).
- **Constants / formulas:** health base maximum from the data formula (NATIVE_PROGRESSION section 4); the follow rule above.
- **Edge cases:** a pool at 0 with a recorded minimum of 0 is "at or below the minimum" and is set to the minimum (no change).
  A stale recorded maximum (a pool whose maximum changed through a modifier since the last `UpdateLastValues`) changes which
  branch is taken; in the captures the recorded maximum equalled the live one.
- **Implementer checklist:**
  1. Level-up call on a pool at 50 percent: maximum changes, current does not.
  2. Level-up call on a pool whose current equals its recorded maximum: current becomes the new maximum.
  3. A pool with the follow flag false never moves its current value in this call.
  4. After the call the recorded minimum, maximum and current equal the live ones when the follow flag is true.
  5. Base maximum is assigned without a notification, so no clamp happens in that step.

## 4. ResourcePool.GetMaxValue, SetCurrentValue, UpdateLastValues and PoolIsNowFull

- **GetMaxValue(optional bool bBase):** returns the maximum's **base companion** when the argument is true, otherwise the
  effective maximum (value after the stack). No side effects.
- **SetCurrentValue(Value):** does nothing unless the pool is authoritative. Otherwise: if the pool's resource is an
  integer-valued resource, the value and the maximum are truncated toward zero first and the fractional part of the value is
  kept in the pool's remainder field; then current becomes the value clamped to the range minimum..maximum, with the
  **minimum winning when the minimum exceeds the maximum**; the idle-delay start time is set to the current world time (or 0
  with no world time); then a virtual "current value changed" hook runs (the base pool class's hook is empty; the subclass
  hooks, experience and ammo pools, were not read). Used by script for fills (`SetCurrentValue(GetMaxValue())` on health and
  shield at load) and called by `AddCurrentValueImpulse`.
- **UpdateLastValues():** clears the "created and not modified" flag if current differs from the recorded current by more than
  0.0001, then records current, minimum and maximum.
- **PoolIsNowFull():** sets the "has been full since last depleted" flag when the minimum is below the maximum.
- **Implementer checklist:** the clamp order of `SetCurrentValue` (minimum first), the integer-resource truncation, the
  authoritative gate, and that the recorded values change only through `UpdateLastValues`.
- **Open:** the "current value changed" hooks of subclasses; the periodic per-frame pool update that also calls
  `UpdateLastValues` (not read).

## 5. The attribute-change clamp (re-used, exercised by the heal)

NATIVE_ATTRIBUTES section 4 describes the pool's reaction to a change of `CurrentValue` made through the attribute
machinery with notification on: current is clamped (below minimum becomes the minimum, at or above the maximum becomes the
maximum). Not re-read here. It is what makes the heal of section 7 end exactly at the maximum: observed, the health after
the level-up equalled the effective maximum to the last digit although the added amount was a full maximum.

## 6. WillowPlayerController.OnExpLevelChange (script) and its place in the level-up

`ExpLevelUp(bCheated)` raises `ExpLevel` by 1, adds the skill points, broadcasts the message, sets a pawn reason code
(section 9) and calls `OnExpLevelChange(true, not bCheated)`. `OnExpLevelChange(bFeedback, bNaturalLevelup)` then, in order:

1. `ExpPointsNextLevelAt` base := the experience required for `ExpLevel + 1`.
2. `RecalculateAttributeInitializedState()` (section 1): the new maximum health.
3. If the controller's pawn implements the body interface and a `WillowPawn` comes out of it: the pawn's game stage is set to
   the new level; the pawn's `IntrinsicArmor` base is set from `PlayerClass.BaseArmor`; then, **only if `bFeedback` and more than
   1.0 second of game time has passed since `LastLevelUpTime`**: `LastLevelUpTime` is set to now and the class's `OnLevelUp`
   behaviors run on the pawn (instigator: the pawn's instigator); then, if `bNaturalLevelup`, `OnLevelUpNaturally` runs the
   same way.
4. If `bFeedback`: the client call `ClientOnExpLevelChange(level)` (HUD, achievements, telemetry).
5. Always: the online game settings are flagged for update.

Notes: the 1.0 second guard is a strict "greater than" and sits before **both** behavior sets; with `LastLevelUpTime` starting
at 0 a level-up in the first second of game time gets no behaviors. With no pawn (the VM controller) steps 1, 2, 4 and 5 run
and the behaviors, the game stage and the armor do not. The load path calls it with `bFeedback` false, so a load never runs the
behaviors (section 8). Order observed on 2026-10-07 by function hooks: level raised, `OnExpLevelChange`, then the native
recalculation (maximum changes, current unchanged), then current equals the maximum at the start of `ClientOnExpLevelChange`.

## 7. The level-up behaviors (data) and the refill

`CharClass_Siren.OnLevelUp` is one `Behavior_RunBehaviorCollection` naming `GD_PlayerShared.Behaviors.PlayerBehavior_LevelUp`;
`OnLevelUpNaturally` names `PlayerBehavior_LevelUpNaturally`. The first collection holds four behaviors in data order: a
particle spawn, a dialog event trigger (both cosmetic) and two `Behavior_AttributeEffect` entries. Each attribute-effect
behavior hands its `SkillDefinition` to the pawn's skill activation (script, `Behavior_ActivateSkill`, reaching
`SkillEffectManager.ActivateSkill`; NATIVE_SKILLS section 4.1) and, separately, applies its (empty) plain effect array.
The skills carry a default starting grade of 1 and the heal effect needs a grade of at least 1; it was observed to apply, so the skills run at grade 1 or more (the grade argument is not passed by the behavior).

| Skill (data) | Duration | Effect | Reading |
|---|---|---|---|
| first `OnLevelUp` skill | timed, duration not set in the data | `HealthCurrentValue`, PostAdd, value = effective `HealthMaxValue` x 1 | **the refill**: current += maximum, then clamp (section 5) gives current = maximum |
| same skill, second effect | same | `ActiveSkillCooldownCurrentValue`, Scale, value = `ActiveSkillCooldownMinValue` | the action-skill cooldown current value is multiplied by the pool's minimum (0 for Maya), a **cooldown reset** (not observed) |
| second `OnLevelUp` skill | timed, 4 seconds | `ShieldActiveRegenerationRate`, PostAdd, value = effective `ShieldMaxValue` x 0.5 | shield recharge boost of half the maximum per second for 4 s; target kind "none" with "include self" set |
| `OnLevelUpNaturally` skill | timed, 30 seconds | `WeaponDamage`, Scale, value 1 | weapon damage scale up by 1 for 30 s (doubles damage by the stack formula) after a natural level-up |

Health and cooldown values are plain pool attributes, so (NATIVE_ATTRIBUTES section 3) a PostAdd or Scale changes the stored
value in place and permanently, and a timed skill's expiry does not undo it (plain-attribute removal is a no-op). The
modifier value is computed once when the skill's effects are built, which is after the pool rebase, so the heal amount is the
**new** effective maximum. The shield regeneration rate is a stack attribute: the boost is removed when the 4 seconds end.

- **Observed (2026-10-07, local captures, lane L1; level-up by the real `ExpLevelUp(false)` with the player at 8 and then 9):**
  - run 1: level 8 to 9, health 214.5329 of 429.0658 (exactly half) and shield 90.29 of 180.59. After the native
    recalculation: maximum 484.8444 (pool base maximum 212.6755 to 240.3233), current still 214.5329. At the start of
    `ClientOnExpLevelChange`: current 484.8444 = maximum. Shield maximum 180.5892 unchanged, base 0; shield current then rose
    to full in about 1.2 s of sampled frames at roughly 90 per second, which is 0.5 x 180.59.
  - run 2: level 9 to 10, health 121.2111 of 484.8444 (a quarter) and shield 18.06 of 180.59. After the recalculation:
    maximum 547.8741 (base 240.3233 to 271.5654), current 121.2111. At the start of `ClientOnExpLevelChange`: 547.8741 =
    maximum. The shield again filled at about 90 per second.
  - the effective maximum divided by the base maximum was 2.01747 both before and after (a profile Badass Rank modifier on
    the stack), so the rebase replaced the base and kept the stack.
  - the pool's recorded maximum became the new maximum; the player controller's last-level-up time was set; the pawn's game stage
    went from 8 to 9.
  The new base maxima equal `80 x 1.13^level` to float precision (240.3233, 271.5654), as NATIVE_PROGRESSION section 4 says.

## 8. The same native at load time and at class change

The save-load path does not call a different native; it calls this one from script, and does the filling itself.
- `ServerSetSaveGameData`: sets `ExpLevel` to `max(saved level, 1)`, **calls the native**, writes the saved experience into the
  experience pool, then `OnExpLevelChange(false, false)`, which calls the native a **second time** (no behaviors because
  `bFeedback` is false; game stage and intrinsic armor are set).
- `ServerSkillSaveGameDataCompleted`: calls the native once, nothing else.
- `ServerItemSaveGameDataCompleted`: refreshes the skills that affect the player, **calls the native**, then, if the pawn and
  its shield pool exist, calls `CalculateBaseValues(true)` on the shield pool and `SetCurrentValue(GetMaxValue())` on it (a
  fill to the effective maximum); then, if the pawn's health pool exists, tells the pawn the maximum changed
  (`OnHealthPoolMaxValueModified`, not read), calls `CalculateBaseValues(true)` on the health pool and
  `SetCurrentValue(GetMaxValue())` (a fill). So a **load fills health and shield explicitly**, and does not use the level-up
  heal.
- `ApplyCharacterClassDefaults` (class set or switched; also the AI mind's version): after setting the shield pool definition on
  the manager and applying the pawn's starting attribute values, it re-applies the playthrough effects and **calls the native
  last**.
- `AttemptPreSaveGameLoadFixup` (NATIVE_SAVE_LOAD) is a separate native: it rewrites the loaded save object (level clamp,
  skill points) before any of this and never calls the native described here.
So the native itself behaves the same on load and level-up (rebase every pool, keep stacks, current follows only a full pool);
what differs is what surrounds it: load fills explicitly and runs no behaviors, a level-up runs the behaviors.

## 9. WillowPawn.SetSecondWindReason(Reason)

`ExpLevelUp` calls it with the value 2 when the controller has a pawn, before `OnExpLevelChange`. The native stores the byte in
a pawn field only when a pawn virtual (not read) reports a condition and the field is still 0. It reads as a reason code for
the injured ("second wind") state and does not touch health. Confidence medium; not needed by the slice.

## Maya on real data (installed packages, `ow-package`)

| Item | Value | Source |
|---|---|---|
| Health pool definition | `D_Resourcepools.PlayerPools.HealthPool`: base maximum from `Init_PlayerHealth`, starts at maximum, follow flag inherited true, idle regeneration delay 0.5 | package data |
| Shield pool definition | `D_Resourcepools.PlayerPools.ShieldPool`: no base maximum (base 0), follow flag inherited true | package data |
| Experience pool, active-skill cooldown pool | follow flag false | package data |
| Base maximum health | level 8 212.6755 (confirmed in game 2026-10-02), level 9 240.3233, level 10 271.5654 (both observed 2026-10-07) | NATIVE_PROGRESSION section 4, captures |
| Attributes re-initialised by the native for her class | encumbrance resistance 0, twelve damage modifiers 1, ten status-effect resistances 1, plus the base maximum and the pool rules of every pool | data defaults, section 1 |
| `OnLevelUp` behaviors | the four-behavior collection of section 7 | package data |

**Level 8 to 9 starting from half health (the quest suite's case), with no gear or skills modifying the pool (the slice's pool
state):**

| Moment | Maximum | Current |
|---|---|---|
| before | 212.6755 | 106.3378 |
| after the native recalculation alone | 240.3233 | 106.3378 (unchanged: the pool was not full) |
| after `OnLevelUp` (the refill) | 240.3233 | **240.3233 (full)** |
| what proportional keeping would give (NOT the game's rule) | 240.3233 | 120.1617 |

The real game refills to the maximum; it does not keep the fraction. If the behaviors do not run (no pawn, `bFeedback` false, or
inside the 1.0 second guard) the player is left at the old current value, 106.3378. The host's `RefreshHealthForLevel` (current
health becomes the new maximum) therefore agrees with the observed outcome in the normal case.
Confirmed in game: the refill from half and quarter health (local captures above, with a Badass Rank modifier so the numbers
are 429/484/547 rather than 212/240/271); not confirmed: the numbers for a modifier-free pool, the guard, the cooldown reset.

## Implementer checklist for the VM (replacing the stand-in)

1. `RecalculateAttributeInitializedState` on the VM controller: for every pool the controller owns, set the base maximum from
   the pool definition's `BaseMaxValue` evaluated at the controller's `ExpLevel`; keep the stack; then apply the follow rule
   (section 3) with the pool's recorded maximum; then update the recorded values. Do not change current otherwise. The stand-in
   that only re-evaluates the health base maximum is the right value but not the right rule: it must also apply the follow rule
   and keep the recorded maximum so that a full pool follows.
2. Model the level-up heal in `OnExpLevelChange`'s pawn branch (or an equivalent host call): when `bFeedback` is true and more
   than 1.0 second of game time has passed since the previous level-up, run the data's first effect: current += effective
   maximum, then clamp to the maximum. Add the cooldown reset and the four-second shield recharge boost if the slice has those
   pools; the natural-level-up damage doubling is optional.
3. A load must not run the heal; it fills explicitly (section 8).
4. Tests: half-health 8 to 9 ends at the new maximum; quarter-health ends at the new maximum; a level-up inside one second of the
   previous one leaves current at the old value; a full pool ends at the new maximum even with the behaviors skipped; the native
   alone leaves a half-health pool's current unchanged.

## Not read yet

- The `WillowMind` and `WillowVehicle` overrides of the same virtual (AI level scaling and vehicles).
- The pool "current value changed" hooks of the experience, ammo and other pool subclasses, and the per-frame pool update that
  also records the last values.
- The pawn's `OnHealthPoolMaxValueModified` (load path) and the pawn-side replicated health variables (they follow the pool in the
  captures).
- The pawn-side route from a behavior's skill hand-off to the controller's activation (the script is read to the controller; the
  captures show it works).
- What sets a pool's `AssociatedProvider` (the context of the base-maximum evaluation); either the controller or the pawn
  resolves `PlayerExperienceLevel` through the replication info, so the health value is the same.
- A separate pawn routine (not on the level-up path) that rebuilds a health pool and **does** keep the current/maximum fraction
  (found by its call of `CalculateBaseValues`; probably the AI player-count scaling): shows the engine has a proportional path,
  but the player level-up does not use it.
- The default duration of a timed skill whose data sets none (the heal skill).

## Corrections to earlier notes (listed, not applied)

1. NATIVE_ATTRIBUTES section 8 and its summary row: `RecalculateAttributeInitializedState` bodies are now read (sections 1 and 2
   here); `ResourcePoolManager.RecalculateBaseValues` calls `CalculateBaseValues` with true on each of the 16 slots (name no longer
   "not decoded", confidence no longer low); `WillowVehicle` has its own thunk and slot, only Controller, WillowPlayerController and
   WillowMind share one.
2. NATIVE_PROGRESSION section 4 ("Not checked: whether a level-up refills current health (the host does)") and its "What was not
   read" line ("health refill on level-up"): answered in sections 3 and 7: base rebased, current unchanged by the native, refilled to the
   maximum by the `OnLevelUp` behaviors; observed on 2026-10-07.
3. SANCTUARY_RPG_MISSION "Script swap 7": the notes **do** say now which native recomputes the health pool's maximum
   (`RecalculateAttributeInitializedState`, through the pool rebase, section 1), so the stand-in is a stand-in for a known rule;
   "what the real native recomputes ... is not established" and "whether a level-up refills current health" are answered; the
   stand-in should also apply the follow rule of section 3.
4. DECISIONS.md 2026-10-01 entry ("progression in the quest save; health follows level"): its UNVERIFIED items "that the pool caps the sum
   at the maximum" and "that the timed effect acts once as a heal" are observed to hold (two captures); the "1 s guard" is a strict greater-
   than on game time since the previous level-up and covers both behavior sets; the same skill also resets the action-skill cooldown (the
   entry says it scales the cooldown) and a second skill boosts shield recharge. The host comment in `OpenWillowWalker.cpp` is consistent.
5. NATIVE_SAVE_LOAD (`ServerItemSaveGameDataCompleted` description): add that the load fills shield and health with an explicit
   `CalculateBaseValues(true)` then `SetCurrentValue(GetMaxValue())`, and that the level-up heal is not part of load; the
   native is called up to five times in a load (set-save-data, its `OnExpLevelChange`, skills completed, items completed, class defaults).
6. REALGAME_GROUND_TRUTH.md: record the two level-up captures (section 7) with the caveats above.
