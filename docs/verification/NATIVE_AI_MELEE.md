# Native AI melee: target choice, action-tree selection and the melee hit (draft, 2026-10-08)

AI-assisted (Claude), analyst lane G25. Read from the game executable with Ghidra (tools/ghidra/, driven by `tools/ghidra/run.ps1`), written in
our own words; no decompiler output, pseudo-code or listing structure is reproduced here. Script facts come from the cooked packages
(`research/script_disasm.py`) and from the live dumps already recorded in `COMBAT_GOLDEN_TRACE.md` (`local/realgame/L5b/`).
**Every rule is UNVERIFIED in the running game** unless a line says how it was confirmed; each section ends with the live observation that would
confirm it. Purpose: plan step S-h groundwork (how a bullymong picks a target and starts and lands a melee attack).

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| AIComponent tick (target update, then state update; no script name) | called from the mind's per-frame tick | S-h: cadence of every AI decision | medium | UNVERIFIED |
| AIComponent.FindTargets / GetTargets | `native function FindTargets()` / `GetTargets()` | S-h: who is a candidate (search radius) | medium | UNVERIFIED |
| AIComponent.ValidTarget | `native function bool ValidTarget(Actor)` | S-h: candidate filter | low (partly read) | UNVERIFIED |
| AIComponent.AddTarget / InitializeTargetRecord | `native function MindTargetInfo AddTarget(Actor)` | S-h: record creation | medium | UNVERIFIED |
| AIComponent.ShouldRemoveTarget / ForceRemoveTarget / TargetRemoved | `native function bool ShouldRemoveTarget(MindTargetInfo)` | S-h: when a target is dropped | medium | UNVERIFIED |
| AIComponent.CalcTargetInfo | `native function CalcTargetInfo(MindTargetInfo)` | S-h: distance, exposure, threat per record | medium | UNVERIFIED |
| AIComponent.PrioritizeTarget | `native function PrioritizeTarget(MindTargetInfo)` | S-h: priority formula | high (formula), medium (iterator meaning) | UNVERIFIED |
| AIComponent.ChooseBestTarget / SetTarget / TargetChanged | `native function ChooseBestTarget()`, `SetTarget(Actor)`, `TargetChanged(Actor,Actor)` | S-h: current target, Target_Acquired / Target_Lost | medium | UNVERIFIED |
| AIComponent.ValidCurrentTarget (CanTargetIf / StopTargetingIf) | `native function bool ValidCurrentTarget(MindTargetInfo)` | S-h: target hysteresis | high | UNVERIFIED |
| Targeting iterators of Targeting_ShortRange (WillowTI_*, TargetIterator*, TI_Prioritize*) | no script names (C++ virtuals) | S-h: what the numbers mean | medium | UNVERIFIED |
| AI action-tree selection (node types 1 to 4, eligibility, timed start, resources) | no script name (AIComponent virtuals) | S-h: which Action runs | medium | UNVERIFIED |
| ActionSequence.TriggerBehavior | `native final function TriggerBehavior(name)` | S-h: the "Attack" behavior event | medium | UNVERIFIED |
| MeleeDefinition.MeleeAttack | `native final function MeleeAttack(Object ContextObject)` | S-h/S-e: the melee hit | medium (radius path), low (cone path) | UNVERIFIED |
| Melee hit application (internal to MeleeAttack) | none | S-h/S-e: allegiance filter, damage, knockback, behavior lists | medium | UNVERIFIED |
| WillowPawn.BeginMeleeOverTime / EndMeleeOverTime, IMeleeAttacker interface | `native` | S-h: charge and grab melee | low (only the entry points) | UNVERIFIED |

## How the bullymong chain fits together (reading order, not a mirror of any listing)

1. Every frame the mind ticks its `AIComponent`. Unless the AI is on hold or throttled, the component first refreshes its target list, then runs
   the action-tree state update. Both are gated by one interval, `AIComponent.UpdateRate`.
2. The target refresh finds pawns within a search radius, builds one `MindTargetInfo` (here `WillowMindTargetInfo`) per candidate, runs the
   definition's "Knowledge" iterators to fill the numbers, runs its "Prioritization" iterators to score each record, sorts and sets the best valid
   one as `CurrentTarget`. Setting it raises the AI events `Target_Acquired` / `Target_Lost` on the running actions.
3. The state update walks the AI definition's node tree (`AIDef_PrimalBeastAdult.NodeList`, node types: 1 action, 2 random, 3 priority, 4 sequence) and
   picks the action to run. `Action_AnimAttack` is the melee action. Its `Attack` state raises the behavior event `Attack` on the pawn.
4. The animation drives a `Behavior_MeleeAttack`, whose script calls `MeleeDefinition.MeleeAttack(pawn)`. That native finds the victims in a
   volume in front of the attacker and applies damage to each through the normal damage chain (already described in `NATIVE_DAMAGE_DEATH.md`).

## AIComponent: cadence

- **Reads:** `UpdateRate`, `LastStateUpdateTime`, `LastTargetUpdateTime`, the `Holds` array, `AIDef`, `TargetingDef`, `Targets`, `CurrentTarget`, `ScriptedTarget`.
- **Does (in order), once per mind tick when the mind may tick at all (`WillowMind.CanTickAI` true):**
  - ensure the AI interface is bound and the component is initialised (first run: reset the tree state, see "State selection");
  - if the component is on hold (any entry in `Holds`): no target update, and the current action is stopped (forced); otherwise
  - update targets (not forced): skipped while `now < LastStateUpdateTime + UpdateRate`; when it runs it stores `LastTargetUpdateTime = now`;
  - give the running action its per-frame update (delta time) and check the timed-failure timers of the tree (see below);
  - update the state (not forced): same interval test against `LastStateUpdateTime`, stores `LastStateUpdateTime = now` when it runs.
- **Edge cases:** the target update tests `LastStateUpdateTime` (not `LastTargetUpdateTime`), so in steady state the two updates run in the same tick,
  at most once per `UpdateRate`. `ForceUpdateTargets` skips the interval test. A separate helper that receives a (hit) report forces a full target update when the
  component has exactly one record; its caller was not identified (see "Not read yet").
- **Implementer checklist:** one `tickAI(dt)` that does target update then state update; both guarded by `now >= LastStateUpdateTime + UpdateRate`; a hold stops the running action.
- **Live confirmation:** read `AIComponent.UpdateRate` of a bullymong; poll `LastTargetUpdateTime` and `LastStateUpdateTime` every frame and check the
  spacing of changes equals `UpdateRate` (hook `GearboxFramework.AIComponent:Tick` if script-visible, else poll from a `Engine.Actor:Tick` hook on the pawn).

## Target list: finding, filtering, adding, removing

- **Search radius.** `TargetingDefinition.SearchRadius` when `bOverrideSearchRadius` is set, otherwise `AIDefinition.TargetSearchRadius` (4096 for the adult bullymong, whose
  `Targeting_ShortRange` is read as the definition; whether it sets the override flag is not recorded). The test compares **horizontal** distance
  (X and Y only) squared with the radius squared; height is ignored.
- **FindTargets:** if the component has a parent AI (`bGetTargetsFromParents`), targets come from the parent; otherwise every pawn the world lists
  is a candidate: it must be within the radius and pass `ValidTarget`, then `AddTarget`. A candidate is skipped when the owner is flagged in a
  way I did not decode (a flag of the owning mind combined with a per-pawn state test).
- **ValidTarget(actor) (partly read):** false when the AI or its interface is missing or the actor is the owner itself. The actor is first mapped to
  its driver/controller-side pawn when it has one. If the owner's allegiance test says the actor is not an enemy, it is only valid when
  `TargetingDefinition.bCanTargetFriendlies` is set and, when `CanTargetFriendliesIf` exists, that evaluator returns true. Additional tests that
  were not decoded: a visibility/known-targetable list lookup and a vehicle test (a pawn riding something is not a direct target).
- **AddTarget(actor):** if a record for the actor exists, return it. Otherwise, when `ValidTarget` holds: create a record of class
  `MindTargetInfoClass` (`WillowMindTargetInfo` for Willow AI), store the actor in `Target`, call `InitializeTargetRecord`, append it to `Targets`, run
  `CalcTargetInfo` on it immediately, and forward the add to the parent AI. The record's `Targetable` interface and `LastKnownLocation` are filled by the Willow iterators.
- **ShouldRemoveTarget(record):** true when `ValidTarget(record.Target)` fails now, or when a parent AI exists and does not know the target.
- **Remove stale (start of every target update):** every record with `ShouldRemoveTarget` true is marked, except that if there is exactly one
  record, an extra owner test passes and the target is still valid, the record is kept. Marked records are removed from the end of the array
  towards the front (`TargetRemoved` runs for each).
- **Implementer checklist:** radius test is XY only; one record per actor; records appended in discovery order; removal pass before the find pass.
- **Live confirmation:** a bullymong with a player at 3000 uu horizontal and 1500 uu above: expect it to be a target (height ignored). Read `AIComponent.Targets`
  (array of `WillowMindTargetInfo`: `Target`, `Distance`, `bShouldRemove`) while the player crosses 4096 uu.

## CalcTargetInfo and PrioritizeTarget

- **CalcTargetInfo(record):** for each entry in `TargetingDefinition.Knowledge` (class `TI_Calc` subclasses) it first calls the iterator's mind-based
  evaluation (if the owner is a `GearboxMind`), then its interface-based evaluation. Each iterator type fills only its own fields (below).
- **PrioritizeTarget(record):** for each entry in `TargetingDefinition.Prioritization` (class `TI_Prioritize` subclasses, each with a `Weight`):
  set `record.OnePriority = 0`, run the iterator (mind-based, then interface-based), then accumulate. `record.Priority = sum(OnePriority * Weight) / sum(Weight)`;
  the record is left unchanged when the weight sum is not positive. Each iterator writes `OnePriority`, normally in 0..1.
- **Implementer checklist:** weighted mean; weights come from data; an iterator that does not write leaves OnePriority at 0.
- **Live confirmation:** read `Targeting_ShortRange.Prioritization[i].Weight` and the live `MindTargetInfo.Priority`, `OnePriority` is overwritten each iterator, so
  verify by hooking each iterator's script-visible entry if any, otherwise compare `Priority` with the predicted mean using the per-iterator formulas below.

## The iterators of GD_AI_Targeting.Targeting_ShortRange

Knowledge: `WillowTI_CalcCanFire`, `WillowTI_CalcCombat`, `WillowTI_CalcDistances`, `WillowTI_CalcExposure`, `WillowTI_CalcThreat`.
Prioritization: `TargetIteratorPrioritizeExposedTargets`, `TargetIteratorPrioritizeNearbyTarget`, `TargetIteratorPrioritizePreviousTarget`, `TI_PrioritizeBonus`,
`WillowTI_PrioritizeAttacking`, `WillowTI_PrioritizeIdle`. Weights and the iterator fields (`MaxDistance`, `DecayRate`, `FullyEngagedPct`, `MaxAICurrentlyTargeting`) are data and
were not read (live read needed).

- **WillowTI_CalcDistances:** `Distance` = 3D distance from the owner pawn to the target; `Distance2D` = horizontal distance; `DistanceVertical` = absolute height difference;
  `DotToTarget` = dot of the owner's facing (yaw, pitch) with the unit vector to the target; `DotFromTarget` = the same for the target's facing back at the owner;
  `LeftDotToTarget` and `DotTargetFacing` = sideways and facing dot products (read as the cross term and the facing-vs-facing dot).
- **WillowTI_CalcExposure:** `TargetExposureToMe` = a 0..1 exposure fraction obtained from the engine's visibility service for the pair of pawns; the 3-level
  `Exposure` byte is 0 when below 0.4, 1 from 0.4 to below 0.9, 2 from 0.9. When exposure changes between none and some, `ExposureChangeTime` is set to now. If the Willow
  mind is a local player's pawn the exposure comes from a different source (not decoded). A "forgotten" flag (`bTargetLost`) is kept from the mind.
- **WillowTI_CalcCanFire:** `WeaponDotToTarget` = dot of the owner's weapon aim direction (pawn eye height) with the direction to the target (or to the target's `Targetable`
  location when it has one); `bCanFireAt` and `bCanSeeOrHearTarget` are set when `Exposure` is not zero.
- **WillowTI_CalcThreat:** sets `ThreatLevel` from the engine (0 = none), and may raise it to 2 (high) when the target can see the owner within a distance
  and view-angle test against the owner's weapon range data; it then reports the new level to the mind. The thresholds were not tabulated.
- **WillowTI_CalcCombat:** not resolved (no vtable found); open.
- **TargetIteratorPrioritizeExposedTargets:** `OnePriority = TargetExposureToMe`.
- **TargetIteratorPrioritizeNearbyTarget:** with no awareness-zone info for the target: `OnePriority = clamp(1 - Distance / MaxDistance, 0, 1)`; with awareness-zone info:
  the zone record's own priority clamped to 0..1.
- **TargetIteratorPrioritizePreviousTarget:** `OnePriority = 1` when the target is the mind's remembered previous/current target, else 0.
- **TI_PrioritizeBonus:** `OnePriority = record.BonusPriority` plus a bonus asked from the AI interface (`record.BonusPriority` is also added by
  script, `GearboxMind.OnTargetPriority`).
- **WillowTI_PrioritizeAttacking:** keeps a decaying damage-share meter per record: it adds each recorded hit's damage fraction that occurred since the last update
  (from `HitTargetHistory`, divided by the game's time dilation), subtracts `DecayRate * dt`, clamps the meter (`TotalDamagePct`) to 0..1, and sets
  `OnePriority = clamp(TotalDamagePct / FullyEngagedPct, 0, 1)`. Without time dilation or with non-positive fields nothing is written.
- **WillowTI_PrioritizeIdle:** takes how many AIs are already targeting the target (`WillowPawn.NumAICurrentlyTargeting` for a pawn target, an equivalent counter on
  the other target kinds; 1 when no counter is readable) and sets `OnePriority = clamp(1 - count / MaxAICurrentlyTargeting, 0, 1)`; when the owner itself is the target's current
  attacker it is subtracted first. Only runs when the owner has an AI component.
- **Live confirmation (all):** dump `Targeting_ShortRange` iterator objects (`Weight`, `MaxDistance`, `DecayRate`, `FullyEngagedPct`, `MaxAICurrentlyTargeting`) and, for a bullymong with a
  player in sight, the `WillowMindTargetInfo` fields `Distance`, `Distance2D`, `DistanceVertical`, `DotToTarget`, `TargetExposureToMe`, `Exposure`, `ThreatLevel`, `Priority`.

## ChooseBestTarget, SetTarget, TargetChanged, ValidCurrentTarget

- **ChooseBestTarget:** `PrioritizeTarget` on every record; sort the array by `Priority`, highest first (inferred from the comparator; the sort is not stable);
  take the first record in that order for which `ValidCurrentTarget` is true; its actor is the best target (none if no record qualifies). If it differs
  from `CurrentTarget`, call `SetTarget`.
- **ValidCurrentTarget(record):** requires the AI, `TargetingDef` and `AIDef`. If the record's actor is the `CurrentTarget`, it is invalid when `StopTargetingIf` exists and is true.
  Otherwise it is invalid when `CanTargetIf` exists and is false. Then, if a `ScriptedTarget` is set, only that actor is valid.
  So `CanTargetIf` gates acquiring and `StopTargetingIf` gates losing (hysteresis).
- **SetTarget(actor):** if different from `CurrentTarget`: raise the AI event `Target_Lost` (when there was a previous target), store the new `CurrentTarget`, raise `Target_Acquired`
  (when there is one), then call `TargetChanged(old, new)`. The event goes to the running action and to every other action in the tree (`Action.ActivateEvent(name)`). In
  `WillowActionSequencePawn` script: `Target_Acquired` re-reads `Target`, `TargetRec` and `ParentTargetRec` from the component; `Target_Lost` clears all three.
  The order of the two events is read from code structure (lost first), not from names; UNVERIFIED.
- **TargetChanged(old, new) (base):** when there was no target and now there is one, the component re-arms the timed start of every tree node (see "Timed fields").
- **Implementer checklist:** acquire needs CanTargetIf, keep needs not StopTargetingIf; highest priority first; at most one SetTarget per update.
- **Live confirmation:** hook `GearboxFramework.AIDefinition:OnTargetAcquired` and `:OnTargetLost` and `WillowGame.WillowActionSequencePawn:ActivateEvent` and log the order and arguments.

## State selection: the AI action tree

The tree is `AIDefinition.NodeList` (struct `AIComponent.AITreeData`). Node 0 is the root. Node `Type`: 1 action, 2 random, 3 priority, 4 sequence.
`AIDef_PrimalBeastAdult` (live dump): root is a priority-type node `Tree` with children GoToScriptedDestination, `Priority`, Patrol (indices 1, 2, 13); `Priority` (node 2) has the
children PickUpObject (combat perch), ThrowIceSpike, CloseMelee, `Random`, BackUp. `Random` (node 6) has Melee (weight 1.0), `Sequence` Roar then LeapAttack (0.25), ThrowRock (0.25).

- **Selection pass (every state update):** clear every node's "next active child"; evaluate the root. A node is only considered when it is eligible (below). An action node
  selects itself. A priority node tries its children in order and selects the first child whose own condition (the per-child `Conditions` entry, when not None) is true and whose subtree selects an action;
  a priority node that already has an active child and is not interruptible (`bInterrupt` false) re-tries that child first. A sequence node selects its
  current sequential child (advancing after each finish). A random node builds the list of its children, picks one by weight (a uniform draw in 0..1 times the weight sum), tries it, and when
  that subtree selects nothing removes it from the list and draws again until the list is empty.
- **Node eligibility:** all of: not before `NextTimedRun` (when set); not within 1.0 s after `LastFailTime` (only when a global debug/option flag is on; unresolved); the node's
  `CanRunIf` evaluator (when set) is true; its `Resource` (when set) is available from the AI resource registry; for an action node the action object exists and its script `CanRun`
  returns the name None (any other name is the failure reason, stored in `ActionFailName`).
- **Timed fields.** `TimedStart` (min, max): when the first target is acquired every node with a valid range (both positive, min <= max) gets `NextTimedRun = now + uniform(min, max)`.
  For the adult bullymong only the combat-perch node has one: 10 s, so it cannot start before 10 s after the first target. `TimedReset` (adult perch node: 20 to 30 s) is read by
  the code that finishes a node; its use is UNVERIFIED (probably the next `NextTimedRun`). `TimedFail` is checked every frame: a node whose `NextTimedFail` time has passed
  and which lies on the running path is stopped (forced).
- **State update (every UpdateRate):** pick the new state with the selection pass; if the running action has an uninterruptible flag pair set, nothing changes; if the running action
  is still allowed to run and no different state was found, it continues; if a different state was selected and the change is permitted (an interruption test), stop the old action (forced) and
  start the new one.
- **Script side that gates attacks (public in the package):** `Action_GenericAttack.CanRun` returns, in order, the parent's reason, `NoTarget` (no `Target`), `Flying`,
  `PathingFailed` (flag), `InvalidLocation` (attack type 2 and the attack-location check invalid), `NotReachable`, and `TargetIsDownedPlayer` (when the game's
  `bEnemiesCannotDoCloseAttacksOnDownedPlayers` is set and the target is an injured player). `Action_LeapAtTarget.CanRun` adds `TooFar` (target farther than the game
  globals' `AIMinAttackDistance` / `AIMinVertAttackDistance` from the nav mesh, or outside the combat volume). `Action_AnimAttack` GetDesiredState: movement on hold means stop; otherwise
  attack location reached or movement not allowed means `RotateToTarget` (aim), else `MoveToTarget`.
- **Implementer checklist:** an explicit tree evaluator with the four node types; timed start armed on first target; eligibility as above; random draw uses a uniform value in 0..1
  (the engine's integer random divided by 32768).
- **Live confirmation:** per `AIComponent` read `CurrentState` (the running node), the node array fields `NextTimedRun`, `ActiveChild`, `ActionFailName`; log a bullymong from
  first sight of the player for 30 s: the perch node must not run before the 10 s mark; the random node's outcome frequencies should approach 1.0 : 0.25 : 0.25 among eligible children
  (over 100 samples); `Action.CanRun` reasons via a hook on `Action_GenericAttack:CanRun`.

## ActionSequence.TriggerBehavior

- **Does:** resolves the name to an event of the pawn's behavior provider and activates it for the pawn's behavior consumer (`BehaviorKernel.ActivateEventForConsumer`, see
  `NATIVE_BEHAVIOR_POPULATION.md`). Nothing happens when the AI interface or the consumer is missing or the event name is not found.
- **Used by:** `Action_AnimAttack.Attack.BeginState` raises the event `Attack` (and `Action_AttackLoop` raises `StartAnim`). How the event reaches the `Behavior_MeleeAttack` of a bullymong is
  data in the character's behavior provider and was not read.
- **Live confirmation:** hook `GearboxFramework.ActionSequence:TriggerBehavior` (name argument) and `WillowGame.Behavior_MeleeAttack:ApplyBehaviorToContext` and log which `MeleeDefinition`
  is in `Behavior_MeleeAttack.MeleeDefinition`, and the time from `Attack` to the melee call (the animation offset).

## MeleeDefinition.MeleeAttack

- **Signature:** `native final function MeleeAttack(Object ContextObject)`. Called from `Behavior_MeleeAttack.ApplyBehaviorToContext` (only when the world is not a network client) and from
  `ExecuteActionSkill.InitiateMeleeAttackDamage` (player action skills).
- **Reads:** the definition's flags and numbers: `bUseDamageRadius`, `bUseTargetCone`, `bMeleeOverPeriodOfTime`, `bHitEachTargetOnlyOnce`, `bExpandRadiusOverTime`, `bIgnoreWorldGeometry`,
  `DamageRadius`, `DamageHeight`, `DamageHeightOffset`, `DamageConeAngle`, `ConeAngle`, `TraceDistance`, `MaxMeleeDuration`, `HitStartSocketOrBone`, `HitStartOffset`; the attacker through the
  `IMeleeAttacker` interface of the context object.
- **Does (in order):**
  - the context object is converted to `IMeleeAttacker`; when it does not implement it, nothing happens;
  - if `bUseDamageRadius` is set the **radius volume** below runs; otherwise if `bUseTargetCone` is set the **cone trace** runs; if neither is set, nothing happens
    (all five bullymong definitions use the radius volume, `bUseTargetCone` false);
  - each victim found is handed to the hit application below.
- **Radius volume.**
  - The origin and facing come from the attacker's trace source (`IMeleeAttacker.GetMeleeTraceSourceLocationAndRotation`, for an AI pawn the
    `WillowAIPawn` override), shifted by `HitStartOffset` (and `HitStartSocketOrBone` when set), in the attacker's rotated frame. The vertical centre is the origin height plus
    `DamageHeightOffset`.
  - The radius is `DamageRadius`. If `bMeleeOverPeriodOfTime` and `bExpandRadiusOverTime` are both set and `MaxMeleeDuration` is positive, the outer radius grows with the elapsed
    fraction of the melee (elapsed / duration, times `DamageRadius`), and the inner radius is what the previous interval reached, so only the newly covered ring is hit.
  - Candidates are the actors the world lists that are alive, not hidden or deleted, carry a collision-related actor flag (not decoded), and are not the attacker itself.
  - A candidate's vertical extent (centre height plus or minus its collision half-height) must overlap the band centre plus or minus `DamageHeight` (so `DamageHeight` is a half-height).
  - The angle test: the dot product of the unit vector from origin to candidate with the attacker's horizontal forward vector must be at least cos(`DamageConeAngle` / 2).
    For 360 degrees every direction passes. `DamageConeAngle` is a full angle in degrees (Melee 128 degrees means plus or minus 64).
  - The distance test: 3D distance from the origin (with the vertical centre) must be at most the radius. A candidate beyond the radius is still accepted when a per-actor extra
    test passes (not decoded; probably a collision-extent allowance), otherwise rejected.
  - Unless `bIgnoreWorldGeometry`, a line trace from the attacker to the candidate must not hit another blocking object first.
  - Qualifying candidates are applied in the order the world lists them.
- **Cone trace (low confidence; the bullymong does not use it).** A trace of `TraceDistance` forward from the offset origin; a direct hit on an actor applies the hit to it. When
  the trace hits nothing, the candidate within `TraceDistance` whose direction has the greatest dot with the forward vector, at least cos(`ConeAngle`) (here `ConeAngle` is used
  as the angle itself, not halved), and with a clear line, is hit instead.
- **Hit application (per victim).**
  - The victim is first replaced by its controller-side owner when it is a proxy (a pawn component maps to its pawn).
  - If `bMeleeOverPeriodOfTime` and `bHitEachTargetOnlyOnce` are both set: skip when the attacker's melee state already lists the victim; else record it.
  - Allegiance: using the attacker's mind, the victim must be an enemy (`bAffectEnemy`), a friend (`bAffectFriendly`) or neutral (`bAffectNeutral`). A victim without a controller skips this test.
  - Damage amount: the `Damage` attribute-initialisation data evaluated for the attacker pawn (for the bullymong this reproduces the live closed form: 8 x 1.13^level x class
    `EnemyNonWeaponDamageMultiplier` x `BaseValueScaleConstant`, adult melee 73.697 at level 5; see `COMBAT_GOLDEN_TRACE.md`). `Knockback` is evaluated the same way and the momentum
    is the horizontal unit direction from the attacker to the victim (height zero) times Knockback times 100000 (reading of the second evaluation; confidence medium-low).
  - The damage is delivered through the engine damage call with the definition's `DamageSource` class, damage type (`DamageTypeDefinition`, or the name override), impact definition,
    status effect damage and chance, the hit location, the momentum and the attacker's controller as instigator. From there `NATIVE_DAMAGE_DEATH.md` applies (shield first, `AdjustDamage`
    stages, `WillowPlayerPawn.TakeDamage`).
  - After the damage: if the victim was an enemy run the `OnHitEnemy` behaviors; then `OnHitEnemyShield` when the victim has a positive shield value, else `OnHitEnemyHealth`;
    then `OnKilledEnemy` when the victim is no longer alive; if the victim was a friend run `OnHitFriendly`; if enemy or friend run `OnHitEnemyOrFriendly`. The behaviors run
    with the attacker pawn as instigator and the victim as target.
- **Calls into script:** the behavior lists (script `Behavior_*` objects); `Pawn.TakeDamage` chain via the engine damage call.
- **Calls other natives:** `IMeleeAttacker` interface methods (`GetMeleeTraceSourceLocationAndRotation`, `HasActorAlreadyBeenHitByMelee`, `GetMeleeAttackerMass`, begin/end over time), the attribute
  evaluator, the damage call, the line trace.
- **Implementer checklist:** (1) no `IMeleeAttacker` or neither volume flag means no hit; (2) radius volume test as above with the three conditions (vertical band, half-angle, 3D radius);
  (3) allegiance flags; (4) damage from the data, momentum on the horizontal plane; (5) behavior lists in the stated order; (6) one hit per victim per call (once per melee when over time).
- **Constants of the five live bullymong definitions** (from the live dump in `local/realgame/L5b/`): `MeleeDef_PrimalBeast_Melee`: radius 640, half-height 256, cone 128, offset -100 forward;
  `_RadiusLeap`: 320, 192, 360, offset +200; `_RadiusBadass`: 1024, 128, 360, scale 1.5; `_MeleeGrabBaby` (over time 0.75 s, interval 0.1, once per target): 580, 320, 128; `MeleeDef_Melee_Charge` (over
  time 5 s, interval 0.0667, once per target): 512, 80, 64, trace 340.
- **Live confirmation:** hook `WillowGame.MeleeDefinition:MeleeAttack`; place Maya at set distances and angles from an adult bullymong and record hit or miss (and
  `Engine.Actor:TakeDamage` arguments: `DamageAmount`, `Momentum`, `HitInfo`). Expect hits out to about 640 uu, not behind the bullymong (plus or minus 64 degrees), and not above 256 uu of vertical
  difference. Compare the momentum vector with the horizontal direction times Knockback times 100000.

## Melee over time

- `WillowPawn.BeginMeleeOverTime(def)` / `EndMeleeOverTime()` are thin calls into the pawn's `IMeleeAttacker` implementation (the pawn keeps a `MeleeState` struct: active flag, definition,
  hit list). `WillowPawn.Landed` ends the melee when the definition has `bStopMeleeOnLanded`. Who calls `MeleeAttack` repeatedly every `AttackInterval` until `MaxMeleeDuration` ends was **not read**.

## Not read yet
- `WillowAIComponent` overrides (`GetTargets`, `ValidTarget`, `ValidCurrentTarget`, `TargetChanged`, `InitializeTargetRecord`, `NotifyAttackedBy`, combat zones, threat level setters): no vtable
  could be resolved by the tooling for the Willow classes; `WillowTI_CalcCombat` likewise.
- `WillowMind.NotifyAttackedBy`, `AddTargetable`, `IsAwareOfTarget`, `NotifyTakeHit` (awareness zones and the `CheckAwareness` iterator).
- The `IMeleeAttacker` implementations on `WillowPawn` / `WillowAIPawn` (trace source, hit list, over-time timer).
- Full `ValidTarget` and the hidden-owner condition in `FindTargets`; the node-completion code (loop counts, `TimedReset`); the interruption test between actions; `Action_LeapAtTarget` and
  movement natives (`FollowPath`, nav mesh).
- Weights and field values of `Targeting_ShortRange` iterators and the `AIResource` registry (live reads).

## Corrections to earlier notes
- None. `COMBAT_GOLDEN_TRACE.md` section on the bullymong ("AI: `AIDef_PrimalBeastAdult` ...") stays as written; this note adds how the AI works. The plan text in `local/p2/S2/bullymong_plan.md`
  ("`MeleeDefinition.MeleeAttack` (native, unread)") is now partially read.
