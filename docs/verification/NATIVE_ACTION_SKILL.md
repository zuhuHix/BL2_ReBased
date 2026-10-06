# Native action skill runtime: key press to activation, lifetime, early end, cooldown, event order (2026-10-06)

AI-assisted (Claude), analyst lane G17. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script behaviour was read with the local listing of `WillowGame`
(`research/script_disasm.py`); data values were read from the installed packages with `ow-package --properties`
(`GD_Siren_Streaming_SF.upk`, `Startup.upk`, `WillowGame.upk`). Raw output stays under the ignored analysis folders.

Scope: how the action-skill key becomes an activation of Maya's Phaselock, the checks that refuse it, the skill's
lifetime, every way it ends early, when the cooldown starts and how it is read and shown, and the order of script events
and behavior events. This note **extends and does not repeat**: [NATIVE_SKILLS.md](NATIVE_SKILLS.md) (grades, modifier
stack, the resource-pool drain formula, the per-frame skill refresh), [NATIVE_PHASELOCK_TARGETING.md](NATIVE_PHASELOCK_TARGETING.md)
(target score, the three skill constraints, lift bob), [NATIVE_PHASELOCK_PRESENTATION.md](NATIVE_PHASELOCK_PRESENTATION.md)
(clips, hand effect, bubble), [PHASELOCK_STOCK_DATA.md](PHASELOCK_STOCK_DATA.md) (identities, behavior chain),
[NATIVE_WEAPON_FIRING.md](NATIVE_WEAPON_FIRING.md) (the reload abort), [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md).
Overlap: lane G7 (skills), G5 (HUD movie), G4 (controller helpers), G3 (resource pools).

## Summary
| Native / rule | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| Input action `ActionSkill` then console command `StartActionSkill` (data plus script) | exec function `StartActionSkill()` | Key press entry point | high | UNVERIFIED |
| `WillowPlayerController.StartActionSkill` / `ServerStartActionSkill` (script) | `exec function StartActionSkill()`, `server function ServerStartActionSkill(WillowPawn SkillTarget)` | Every refusal rule, toggle, active-ability branches | high (script) | UNVERIFIED |
| `SkillEffectManager.IsSkillActive` | `native function bool IsSkillActive(Controller SkillInstigator, SkillDefinition Definition)` | "Already active" test | high | UNVERIFIED |
| `Skill.NotifySkillEvent`, `SkillEffectManager.NotifySkillEvent` | `native function NotifySkillEvent(ESkillEventType Event, ...)` | Skill-level behavior events (OnActivated, OnDeactivated) | medium | UNVERIFIED |
| Skill per-frame refresh, end-of-duration handshake with the action skill (internal, extends NATIVE_SKILLS 4.5) | no script signature | How a timed action skill may expire | medium | UNVERIFIED |
| `ActionSkill` per-frame update (internal) | no script signature | Tick event, nearing-completion event, timers | medium | UNVERIFIED |
| `ActionSkill.OnActionSkillActivated` / `OnActionSkillDeactivated` | `native final function ...(BehaviorConsumerHandle, WillowPawn, ...)` | Provider events at start and end | high | UNVERIFIED |
| `ActionSkill.OnActionSkillActiveAbilityActivated` / `...Notified`, `OnOwnerAcquiredAutoAimTarget` / `OnOwnerLostAutoAimTarget`, `OnActionSkillNearingCompletion` | same family | Provider events (none linked for Phaselock) | high | UNVERIFIED |
| `SkillDefinition.OnActivated` / `OnDeactivated` / `OnActionSkillActiveAbilityActivated` / `OnActionSkillCooldownAbilityActivated` | `native final function ...` | Skill-provider events (tattoo glow) | high | UNVERIFIED |
| `WillowPawn.IsActionSkillRunning` and `ActionSkillStateExpressionEvaluator.Evaluate` | `native function bool IsActionSkillRunning()`, `native function bool Evaluate(Object)` | Condition "action skill running" in skill data | medium (body of the first not located) | UNVERIFIED |
| `WillowPlayerController.SetActionSkillTime` and the `ActionSkillTime` field | `native function SetActionSkillTime()` | Progress of the active ability for the HUD bar | high for the formula, low for the caller | UNVERIFIED |
| `WillowPlayerController.GetActionSkillDuration` (extends NATIVE_SKILLS 5.1) | `native final function float GetActionSkillDuration()` | Duration passed to active-ability events | high | UNVERIFIED |
| `WillowHUDGFxMovie.UpdateActionSkill` (cooldown icon and bar) | `native function UpdateActionSkill()` | How cooldown is shown | low | UNVERIFIED |
| Cast special move blocks weapon actions (script, explains the reload abort) | `SpecialMove_WeaponAction.ClientStarted`, `WillowPlayerController.PerformSharedWeaponActions` | Reload abort, weapon busy during the cast | high (script) | UNVERIFIED |

Script functions (not native) that carry most of the behaviour here: `WillowPlayerController.StartActionSkill`,
`ServerStartActionSkill`, `ActionSkillCallback`, `StartActiveSkillCooldown`, `IsActionSkillOnCooldown`,
`IsActionSkillCoolingDown`, `ServerDeactivateSkill`, `Behavior_ActivateSkill`, `Behavior_DeactivateSkill`, `ResetSkillCooldown`,
`PerformSharedWeaponActions`, `WeaponActionComplete`; `Skill.Activate`, `Deactivate`, `Initialize`, `UpdateGrade`;
`SkillEffectManager.ActivateSkill`, `DeactivateSkill`; `WillowPawn.ActionSkillStarted`, `ActionSkillEnded`, `EndActionSkill`,
`EnableActionSkill`; `ActionSkill.OnActionSkillStarted`, `OnActionSkillEnded`, `TearOff`, `IsDeactivateBlocked`,
`OnActionSkillWantsToDeactivate`; `LiftActionSkill.*`.

Enums used (from the package): `SkillDefinition.ESkillEventType` (0 SkillActivated, 1 SkillDeactivated, 2 SkillPaused,
3 SkillResumed, 4 to 9 damaged enemy/friendly/neutral and damaged by those, 10 ShieldDepletedAfterBeingFull, 11 to 16 killed
and killed by, 17 WeaponZoomed, 18 WeaponUnzoomed, 19 WeaponShotMissed, 20 WeaponFired, 21 WeaponReloaded, 22
PlayerDeathAverted, 23 ActionSkillCooldownAbilityActivated, 24 ActionSkillActiveAbilityActivated, 25 to 28 unaware / behind
/ melee override / grenade override, 29 WeaponSwapped, 30 PlayerRecoveredFromDownState, 31 ShieldDepleted, 32 ShieldFull,
33 WeaponStartReload, 34 PlayerResurrected, 35 WeaponManuallyReloaded, 36 AppliedStatusEffectToEnemy, 37 DownStateBegin,
38 DownStateEnd, 39 BledOut, 40 and 41 status effect type begin and end, 42 MeleeAttack, 43 MAX). `ESkillState` Deactivated 0,
Active 1, Paused 2. `ESkillType` Action is 1, ActionAugment 2. The physics enum value 9 is Ladder (stock UE3 order; 14 is
Custom, which agrees with NATIVE_PHASELOCK_PRESENTATION).

## 1. From key press to a request (data plus script)

- **Input action.** `GD_Input.Actions.InputAction_ActionSkill` (`InputActionDefinition`, `ActionName` = `ActionSkill`) has one
  `OnBegin` behavior, a `Behavior_ClientConsoleCommand` whose command is the text `StartActionSkill`. So a press (begin, not
  release) runs the console command on the local controller; `StartActionSkill` is an exec function. The player class
  strings (`GBA_UseActionSkill`, `<StringAliasMap:Action.ActionSkill>`) are only the menu label of the same action.
- **`WillowPlayerController.StartActionSkill` (local controller only).** If an auto-aim strategy exists, ask it for its preferred
  target with the instantaneous flag (details in NATIVE_PHASELOCK_TARGETING sections 1 to 3); keep the result as
  `CurrentActionSkillTarget` only when it is a `WillowPawn`, otherwise clear it. Then call `ServerStartActionSkill(target)`.
  Without an auto-aim strategy the target is none. The key does **not** check cooldown, grade, weapon or health on the client;
  all of that is decided in the server function and in the skill's constraints.

**Implementer checklist.** 1. Key down maps to "start action skill" exactly once per press. 2. The target is the auto-aim
preferred target if it is a pawn, else none. 3. No client-side refusal.

## 2. `ServerStartActionSkill(SkillTarget)`: the refusals, in order

Runs on the authority. Read as a decision list (the jump structure of the listing was read with the usual offset drift
and the reading below is the consistent one, UNVERIFIED):

1. Take the controller's pawn as a `WillowPawn`. If the controller's pawn is a vehicle, use the vehicle's driver instead.
2. **Refuse (return) when that pawn is on a ladder** (physics value 9). Nothing else changes.
3. Store `CurrentActionSkillTarget := SkillTarget`.
4. Look up the tree's action skill and its state. **If the tree does not know a skill (no action skill), do nothing** (the
   target is still cleared at the end).
5. Compute three flags: `onCooldown` = the controller's cooldown pool is valid and its current value is above 0;
   `coolingDown` = `onCooldown` and the current value is below the pool maximum; `active` = the skill manager reports this
   definition active for this controller (`IsSkillActive`, section 3). Only for the skill named `Skill_Gunzerking`,
   `coolingDown` is recomputed as `coolingDown and not active`. (That is the whole Gunzerking special case; it does not
   waive the cooldown refusal, see Corrections.)
6. **Not on cooldown and not active: start.** If the controller is in a vehicle and that vehicle seat is not an attached-rider
   seat, return without starting. Otherwise ask the manager to activate the skill with instigator = this controller, no
   additional target, **grade = the grade stored in the tree**, and the state-change delegate `ActionSkillCallback`
   (section 4). The manager does nothing, silently, if activation is currently not allowed (it queues the request instead),
   if the definition fails the grade test (a `bSubjectToGradeRules` skill at grade 0, which is Phaselock untrained) or if the
   constraints fail inside `Skill.Activate` (section 4).
7. Otherwise, if the definition's `bCanBeToggledOff` is set: deactivate the skill (a second press ends it). Phaselock leaves
   the flag at its default (false), so a press while the lock is running does not end it.
8. Otherwise, if `active` and not `coolingDown`: the "active ability" path: when the pawn's action skill object allows
   automatic active-ability activation **and** its limits allow another one (allowed when: the count restriction is off or the
   count is below the maximum per cycle, and the frequency restriction is off or the next-allowed time has passed; on success
   the count goes up and the next-allowed time is now plus the configured frequency), call
   `ServerStartActionSkillActiveAbility(target, notify client = true)`. The class default is "not allowed" and Phaselock's
   archetype does not set it, so for Phaselock this branch does nothing. While the lock runs the pool is full (section 5),
   so `coolingDown` is false and this is the branch a second press falls into.
9. Otherwise, if `coolingDown`: the "cooldown ability" path with the same shape (`AllowAutomaticCooldownAbilityActivation`,
   `AllowNewCooldownAbilityActivation`, then `ServerStartActionSkillCooldownAbility(target)`); not allowed for Phaselock, so a
   press during the cooldown does nothing visible.
10. Finally `CurrentActionSkillTarget := none` (always, after any branch).

`ServerStartActionSkillActiveAbility` (when it runs): optionally tells the owning client (`ClientStartActionSkillActiveAbility`)
and runs the action skill's `StartActionSkillActiveAbility(pawn, controller, GetActionSkillDuration, ActionSkillTime, target)`,
then raises skill event 24 on the manager; the cooldown twin raises event 23 (via the skill's own event natives, section 8).
`LiftActionSkill.StartActionSkillActiveAbility` calls the base and then Ruin if `CanRuin` (an upgrade, not in the slice).

**Where "injured", "weapon state" and "no target" are decided.** Not here. Injured or dead, weapon putting down / restricted /
shared action, and vehicle seat are the skill constraints evaluated inside `Skill.Activate` (NATIVE_PHASELOCK_TARGETING
section 4; this note confirms the caller). "No target" is not a refusal: the skill starts and fizzles (section 7).

**Implementer checklist.** The order above; the ladder check first; the cooldown test is `pool current > 0`; the active test is
section 3; grade is passed through; the target variable is cleared at the end; pressing during an active or cooling Phaselock
changes nothing.

**Open.** Whether the ladder test really precedes the vehicle replacement (the two statements were read as: replace the pawn
by the driver if the pawn is a vehicle, then test physics).

## 3. `SkillEffectManager.IsSkillActive(Controller, SkillDefinition)` (native)

- **Reads:** the manager's list of active skill objects; each skill's definition, instigator and state byte.
- **Does:** true if any skill in the list has this definition **and** this instigator **and** a state other than Deactivated
  (so Paused also counts as "active"). Otherwise false. Linear search; no side effects.
- **Calls into script / natives:** none.
- **Edge cases:** a freshly created skill whose constraints failed has state Deactivated and does not count; the manager removes
  such skills on its next tick (NATIVE_SKILLS 4.5).
- **Implementer checklist:** `exists(skill where definition and instigator match and state != Deactivated)`.
- **Open:** `GetActiveSkillForInstigator` / `...ByDefinition` are virtual natives whose bodies were not located; the manager's
  callers use them to fetch the skill whose duration is read (NATIVE_SKILLS 5.1).

## 4. Activation and deactivation order (script), and when the delegate runs

`SkillEffectManager.ActivateSkill` (needs activation allowed, a definition, an instigator and the grade test) creates the
`Skill`, calls `Skill.Initialize` (copies definition, instigator, extra target; grade := max(grade, 1); Duration base :=
`InitialDuration`; state Deactivated; **stores the delegate**), appends it to the active list and calls `Skill.Activate`.

**`Skill.Activate`, in order** (extends NATIVE_SKILLS 4.1; this is the exact order, which the earlier note listed without
order):
1. State := the constraint check for activation. If the result is Deactivated, **stop here**: nothing below runs, **the delegate
   is not called**, and the skill disappears at the next manager tick. (That is why a blocked cast, for example during a
   weapon swap put-away, uses no cooldown.)
2. Record the activation time. Register the skill as a behavior consumer and load its provider (`BeginNondeterministicProviderRegistration`).
3. Build the effect records. If the state is Active: add every modifier (mode 0) and, for a Timed definition, `StartTime := now`.
4. If the definition has an `ActionSkillArchetype`: **`WillowPawn.ActionSkillStarted(definition)`** (section 6), which spawns
   the action skill actor and runs `OnActionSkillStarted`; for Phaselock this is the whole lift setup.
5. `NotifySkillEvent(SkillActivated)` (section 8): the skill's own provider event `OnActivated` (Phaselock: tattoo glow).
6. Run the console commands in `SkillActivationActions` on the controller (none for Phaselock).
7. **Call the state-changed delegate with `true`** (`ActionSkillCallback`, section 5). Then add the vision-mode effect if any.

**`Skill.Deactivate`, in order:** (only if the state is not already Deactivated) `NotifySkillEvent(SkillDeactivated)`;
deactivation console commands; remove modifiers (mode 2) and clear the applied effects; **state := Deactivated; call the
delegate with `false`**; if the definition has an action skill archetype: **`WillowPawn.ActionSkillEnded`**; remove vision
mode; unregister the consumer.

So at the instant the delegate runs with `true`, the action skill actor already exists, its `OnActionSkillStarted` has run,
the lift/target selection and its behavior events have happened (section 10), and the skill's own `OnActivated` has fired.
At the instant it runs with `false`, the skill state is already Deactivated but the action skill actor has not yet been told.

**Implementer checklist.** Keep this order; the delegate is the only thing that starts the cooldown; a failed activation
check leaves no trace.

## 5. `ActionSkillCallback` and the cooldown start

`WillowPlayerController.ActionSkillCallback(skill, bActivated)` (script, runs on the authority):
- asserts the skill is the tree's action skill; reads `active` from the manager;
- **bActivated true:** `StartActiveSkillCooldown` (refill the cooldown pool to 100 percent: `RefillPercentage(1.0)`), increment the
  stat `STAT_PLAYER_ACTION_SKILL_USES`, then for every tree skill of type ActionAugment (2) that has a state: if the action skill
  is active, activate it with its own grade (NATIVE_SKILLS 4.1);
- **bActivated false:** deactivate every ActionAugment skill. Nothing touches the pool.

`StartActiveSkillCooldown` does nothing if the pool reference is invalid. **Cooldown = pool current value; "ready" = current is
0** (`IsActionSkillOnCooldown` is `current > 0`). The cooldown is read by script as: `GetSkillCooldownTime` = pool maximum,
`GetSkillCooldownTimeRemaining` = pool current, `ResetSkillCooldown` sets current to 0, all returning 0 / doing nothing for an
invalid pool. Because the drain is 1 per second the current value is the remaining seconds.

**Resolution of an open question in PHASELOCK_STOCK_DATA ("when exactly the refill happens relative to `OnSelectedTarget`").**
The refill is at the **end** of `Skill.Activate`, after the whole lift setup. `OnSelectedTarget` (inside the lift setup) has
already activated `Skill_Phaselock_CooldownManager` (consumption PreAdd -1, net drain 0), so the pool is refilled to its
maximum while not draining, and starts draining only when `OnReleasedTarget` deactivates the manager. For a target that
cannot be lifted, or no target, the manager is never activated and the drain runs from the cast (section 7).

Data (installed packages): Maya's pool is `D_Resourcepools.PlayerPools.ActiveSkillCooldownPool_Siren`: resource
`ActiveSkillCooldown`, `BaseMaxValue` = the designer attribute `GD_Siren_Skills.Misc.Cooldown_Phaselock` (13, from
PHASELOCK_STOCK_DATA), `BaseConsumptionRate` 1, no regeneration rates, no `OnResource*` behaviors, `StartingValue` and the
`StartWith*` flags left at their defaults (so the pool should start at 0, ready; the pool creation native was not read,
UNVERIFIED). `bUpdateCurrentValueOnExtremaChange` is false (a maximum change does not move the current value). The class
`CharClass_Siren` points to it as `SkillCooldownPoolDefinition`.

**Implementer checklist.** cast: current := max after the cooldown manager is on; each frame current -= (consumption -
active regen) x dt (NATIVE_SKILLS 5.2); ready when current <= 0; fizzle: current := 0.

**Open.** `ActionSkillCooldownComplete` (script, plays the class's "action skill available" sound when the grade is above 0, an
event-style function): its native caller was not found. The pool has no depleted behavior in the data, so the call comes from
native code; the HUD update (section 13) plays its own "active ability ready" cue at the moment the displayed cooldown reaches 0.

## 6. The action skill actor: `WillowPawn.ActionSkillStarted`, `EnableActionSkill`, end, `TearOff` (script)

- `ActionSkillStarted(skill)`: finds the controller (the pawn's, or the driven vehicle's) and calls
  `EnableActionSkill(true, skill, controller.CurrentActionSkillTarget)`. This is the only place the stored target is consumed.
- `EnableActionSkill(true, ...)`: only if no action skill is running on this pawn and the role is authority, **spawn** an actor of
  the archetype's class using the archetype as its template (owner = the pawn) and store it in `MyActionSkill`; if the new
  actor is not yet initialised: reset the active-ability and cooldown-ability counters to 0 and their "next time" to now,
  copy the debug flag, call **`OnActionSkillStarted(pawn, controller, target)`** on it, then tell the HUD
  (`NotifyHUDOfEnableActionSkill(skill, true)`).
- `ActionSkill.OnActionSkillStarted` (base): stores pawn, controller; on the authority also the target, the player controller /
  player pawn / mind casts and `TimeStarted` (game elapsed time); calls the native `OnActionSkillActivated` (section 9);
  sets `bInitialized`. `LiftActionSkill.OnActionSkillStarted` runs the base first (section 10).
- The actor registers itself as a behavior consumer with its own provider in native begin-play code (the archetype's
  `BehaviorProviderDefinition`); the event natives of section 9 use that handle.
- **End.** `Skill.Deactivate` calls `WillowPawn.ActionSkillEnded`, which on the authority calls `EndActionSkill`:
  `EnableActionSkill(false)` (tell the HUD `NotifyHUDOfEnableActionSkill(skill, false)`; on the authority call the actor's
  **`TearOff`**), then `ClientEndActionSkill`. `ActionSkill.TearOff` (authority only): sets the tear-off flag, calls
  **`OnActionSkillEnded`**, sets the actor `LifeSpan` to 1 second and clears the pawn's `MyActionSkill`. `OnActionSkillEnded`
  (Lift: `EndSkill` first, then the base): native `OnActionSkillDeactivated` event, `NotifyActionSkillRunTime(now - TimeStarted)`
  to the controller (a stat), clear the cached references, force-unregister the consumer. On clients the torn-off actor runs the
  same `OnActionSkillEnded` and marks itself for destruction.

**Implementer checklist.** One action-skill actor per pawn, created by the activation, destroyed one second after the end;
`IsActionSkillRunning` is true from the spawn until the pawn's reference is cleared.

## 7. Phaselock lifetime: stages, durations, and every way it ends

Data (installed packages): `Skill_Phaselock`: `bSubjectToGradeRules` true, `SkillType` Action, `DurationType` Timed,
**`InitialDuration` 120**, `MaxGrade` 1, `PlayerLevelRequirement` 1, `ActionSkillArchetype` = `ActionSkill_Phaselock`
(`LiftActionSkill`), no `bCanBeToggledOff`, three constraints (weapon action, healthy, vehicle), skill provider with
`OnActivated`, `OnDeactivated` and a killed-enemy response. `ActionSkill_Phaselock`: `LiftDuration` 0.7, `LockDurationFormula`
= attribute `Att_Phaselock_Duration` (5), `LockDurationScaleFormula` = attribute `PhaselockTimeScale` of the target (1, or 0.6 for
25 s after a lock, PHASELOCK_STOCK_DATA), `LockFadeOutTime` 1.1; class default `ReleaseBufferTime` 1,
`ActionSkillNearingCompletionTime` 2, `TickRate` left at 0, `bAllow...AbilityActivation` flags left false.

Two clocks exist and must not be confused:
- the **skill object's** `Duration` (base 120, a safety limit; also what `GetActionSkillDuration` returns), and
- the **action skill actor's** own schedule: `SkillDuration = LiftDuration + LockDuration(Maya) x TimeScale(target)` (5.7 s first
  lock), computed once in `OnActionSkillStarted` only when the role is authority and a target exists.

Timeline of a normal cast (script timers; seconds after the cast; first lock, no upgrades):

| t | What | Source |
|---|---|---|
| 0 | `Skill.Activate` steps 1 to 7 (section 4); `PhaseLockTarget`: state 1 (lifting), `OnSelectedTarget`, attribute effects, timers set (LockTarget at 0.7; StartOutro so that it fires at SkillDuration - 1.1 = 4.6; EndSkill at SkillDuration + ReleaseBufferTime = 6.7), unstagger, `LiftTarget`; then the delegate: cooldown pool refilled | script |
| 0.7 | `LockTarget`: state 3, bubble fade-in, `OnTargetBecomesLocked` | script |
| 4.6 | `StartOutro`: state 2, `OnTargetIsAboutToBecomeUnlocked`, release timer 1.1 s | script |
| 5.7 | `ReleaseTarget`: finish effects, mark the target usable again (`bCanUse` := true), **`OnReleasedTarget`** (deactivates the cooldown manager: the drain starts), remove the Phaselock attribute modifiers, state 0, drop (NATIVE_PHASELOCK_PRESENTATION 2.2 steps 6 and 7) | script |
| 6.7 | `EndSkill` timer: cleanup (below), then `ServerDeactivateSkill(action skill)`: `Skill.Deactivate`, delegate(false), `ActionSkillEnded`, `TearOff`, `OnActionSkillEnded`, `OnActionSkillDeactivated` event | script |
| 7.7 | the action skill actor is destroyed (LifeSpan 1) | script |

The cooldown therefore reaches 0 at 5.7 + 13 = **18.7 s** after the cast for a base first lock (matches PHASELOCK_STOCK_DATA).
The HUD bar value (`GetDeferredActionSkillTime`, section 13) runs 0 to 1 over SkillDuration - LiftDuration, measured from the end
of the lift, while the state is not 0 (negative, i.e. hidden, during the first 0.7 s), and is -1 once released.

**`LiftActionSkill.EndSkill`** (the timer target, also run from `OnActionSkillEnded` and `Destroyed`): remove the first-person
hand effect; on a client stop here; if the state is 4 (Ruin) apply it; release the lifted pawn if still held (full release as
above); finish a pending landing immediately; detonate a subsequence projectile; state := 0; clear the timers Fizzled,
EndSkill, LockTarget, StartOutro, ReleaseTarget, TransitionToBubbleFXLoop; then, if the controller and its tree exist,
`ServerDeactivateSkill(action skill)`. It is idempotent: the second call (from `OnActionSkillEnded`) finds nothing to do and
the manager finds no active skill to deactivate.

**Every way the skill ends**
1. **Normal:** EndSkill timer (above).
2. **No target or a target that cannot be locked:** `FizzleOut`: `bFizzled`, event `OnLiftFailed`, a `Fizzled` timer of
   `ReleaseBufferTime` (1 s), miss impact effect; `Fizzled` then **sets the cooldown pool to 0** and deactivates the skill. The
   cooldown pool was refilled by the delegate at t = 0, so a miss costs a 1 s cooldown that is then wiped. A target that is a
   valid pawn but fails `CanLiftTargetIf` (flag `Flag_Skills_CanPhaseLock`) takes the **blocked** path: event `OnTargetBlocked`
   (damage, effects, then the provider's delays 0.95 / 1.45 / 3.2 s lead to behavior `DeactivateSkill` of `Skill_Phaselock`,
   PHASELOCK_STOCK_DATA); no cooldown manager, no reset, so the 13 s drain runs from the cast.
3. **Target dies, is staggered (not charmed) or otherwise stops being alive-and-well while held** (checked every tick,
   `IsLiftedPawnIncapacitated`): `InterruptPhaseLock`: event `OnKilledTarget`; if a subsequence projectile can be spawned
   (upgrade, states 1 and 3 only) spawn it, otherwise **re-arm the EndSkill timer to `ReleaseBufferTime` (1 s) and release the
   target now**. So an early kill ends the skill about 1 s later and starts the cooldown drain at the release.
4. **Maya's skill constraints fail while active** (injured, dead; weapon-action check applies at activation only; vehicle): the
   manager's per-frame refresh sees the state change and deactivates the skill (NATIVE_SKILLS 4.5, NATIVE_PHASELOCK_TARGETING 4);
   `Deactivate` then ends the action skill as in section 6 and `EndSkill` releases the target at once (so `OnReleasedTarget`
   runs and the drain resumes). The pawn's own `OnActionSkillOwnerDied` and `OnActionSkillOwnerInjured` calls are empty for
   `LiftActionSkill`.
5. **Duration limit of the skill object (120 s):** see section 12a; not reachable in normal play (the schedule above ends by 6.7 s).
6. **Script resets:** `Behavior_ResetActionSkillCooldown` (and `ConditionalResetInterruptedActionSkill` after an interrupted grenade
   throw when the action skill's `ShouldResetOnInterruptedGrenadeThrow` is true; default false): if the pawn's action skill
   `CanResetActionSkill` (default true) set the pool to 0 and `ServerDeactivateSkill`. `ResetActionSkill`: pool to 0 and
   deactivate. Not used by Phaselock's data.
7. **Pressing the key again** does not end it (section 2, step 7).

**Implementer checklist.** The skill's end is always `Skill.Deactivate` followed by the action skill teardown; implement the
timers as absolute times from the cast; an early target kill shortens the end to release + 1 s; a miss gives cooldown 0 after
1 s; a blocked target gives the plain 13 s drain.

## 8. `Skill.NotifySkillEvent` and `SkillEffectManager.NotifySkillEvent` (native)

- **Signature:** `(ESkillEventType event, Controller instigator, ...)` and the skill's own variant without the instigator
  filter; up to five trailing optional arguments (damage data, an object, an extra object).
- **Manager version does:** clear a "dirty" bit, then for every active skill whose instigator is the given controller and whose
  definition exists, call the skill's own `NotifySkillEvent` with the same event; finally a trailing bookkeeping call.
- **Skill version does:** ignore the event unless the skill has a definition and is not Deactivated; **a Paused skill reacts
  only to events 1 (deactivated) and 2 (paused)**. When a damage-data argument accompanies the event it also derives an amount
  (the damage amount minus a second value of the same record, floored at 0) and stores it in a field of the instigator controller (use not read).
  Then it dispatches by event number to a small handler that fires a **behavior event on the skill's own consumer** (the skill
  object registered in `Skill.Activate`, section 4). Names read: 0 -> `OnActivated`, 1 -> `OnDeactivated`, 2 -> `OnPaused`,
  3 -> `OnResumed`, 23 -> `OnActionSkillCooldownAbilityActivated`, 24 -> `OnActionSkillActiveAbilityActivated` (the last two pass
  the controller and the controller's stored action-skill target). Events 11 (killed enemy) and 14 (killed by enemy) instead walk
  the definition's `EventResponses` list and fire one event per response of the matching kind that passes its target-criteria
  filter. Events 4 to 9, 10, 12, 13, 15 to 22, 25 to 42 each have their own handler (or are ignored); only the shapes were
  seen, not each name.
- **Calls into script:** none directly; the behavior kernel runs the linked behaviors.
- **Constants:** none.
- **Edge cases:** an unknown number does nothing.
- **Implementer checklist:** skill-level events 0 and 1 are delivered to that skill's provider with the instigator as the one
  parameter; events 23 and 24 carry (instigator, action-skill target).
- **Open:** events 4 to 9, 11 to 16 response filters (only the shape was read), and which cases of 10 to 42 beyond those named
  have handlers.

## 9. The `ActionSkill` event natives (native, behavior-event senders)

All of these are small wrappers: they read their arguments from the script call, build a behavior event with the **same name as
the function**, attach the parameters in order, and deliver it to the action skill's provider through the behavior kernel
for the given consumer handle (`ActivateEventForConsumer`). They have no other effect and return nothing.

| Function (script) | Event name | Parameters (in order) |
|---|---|---|
| `OnActionSkillActivated(handle, pawn, target)` | `OnActionSkillActivated` | pawn, target pawn |
| `OnActionSkillDeactivated(handle, pawn)` | `OnActionSkillDeactivated` | pawn |
| `OnActionSkillActiveAbilityActivated(handle, pawn, timeRemaining, target)` | same name | pawn, float, target |
| `OnActionSkillActiveAbilityNotified(handle, pawn, timeRemaining, target)` | same name | pawn, float, target |
| `OnOwnerAcquiredAutoAimTarget(handle, pawn, target)` (and the same-shape `OnOwnerLostAutoAimTarget`, read from its thunk only) | same name | pawn, target |
| `OnActionSkillNearingCompletion(handle, pawn, target)` | same name | two objects |
| `OnTimerEvent(handle, enum)` | built from the enum value | none |

Phaselock's provider has **no behaviors linked** to `OnActionSkillActivated`; its linked events are the script-built ones
(`OnSelectedTarget`, `OnTargetBecomesLocked`, `OnTargetIsAboutToBecomeUnlocked`, `OnReleasedTarget`, `OnKilledTarget`,
`OnLiftFailed`, `OnTargetBlocked`, `OnActionSkillDeactivated`, and the upgrade-only ones; PHASELOCK_STOCK_DATA). So of this table only
`OnActionSkillDeactivated` has behaviors for Phaselock (deactivate `Skill_DetonateAvailable`, screen particle, sound). The
`LiftActionSkill.On*` functions (`OnSelectedTarget` etc.) are script wrappers that do the same thing by storing the two object
parameters as behavior variables and calling the kernel with the function's own name.

**Implementer checklist.** Event name = function name; parameters positional as above; the consumer is the action skill actor,
registered when it spawns.

## 10. `LiftActionSkill.OnActionSkillStarted` and the order of script events at cast (script, with the natives above)

In order, inside step 4 of `Skill.Activate`: base `OnActionSkillStarted` (fields, `OnActionSkillActivated` event, `bInitialized`);
`UpdateTargetPawn` (if the target is hard-attached to a base pawn, the target becomes that base, repeated); **if authority and a
target exists:** `SkillDuration` computed, the cast special move played on Maya (hit or miss move, NATIVE_PHASELOCK_PRESENTATION 2.2),
`SkillStartTime` := now; always `SelectTarget(target)`:
- target valid (not friendly, alive and well, not already phaselocked) **and** (no driven vehicle) **and** `CanLiftTargetIf` passes:
  `PhaseLockTarget`;
- valid but `CanLiftTargetIf` fails, or target is driving: `TargetBlocked` (event `OnTargetBlocked`);
- not valid: if `CanResurrectTarget` (upgrade) resurrect, else `FizzleOut`.

`PhaseLockTarget`, in order: state 1, `StateDuration` = LiftDuration, `LiftedPawn` := target, **`OnSelectedTarget` event**,
apply `PhaselockedAttributeEffects` to the target (IsPhaselocked PostAdd +1; saved for removal), set the three timers (section 7),
decide charm (`CanCharmTarget`: needs the Thoughtlock upgrade active), unstagger a staggered target, `LiftTarget`
(NATIVE_PHASELOCK_PRESENTATION 2.2), spawn the Helios burst if that upgrade is active (`ShouldSpawnHelios`).

**Event order for one normal cast** (script event, then provider behaviors in link order; only the base-game ones):
`OnActionSkillActivated` (no behaviors) -> `OnSelectedTarget` (dialog, cooldown manager on, sound, screen particle, AI provoke,
AI hold) -> skill `OnActivated` (tattoo glow) -> delegate true -> pool refill -> [0.7 s] `OnTargetBecomesLocked` (AI flag
IsPhaselocked true, custom event `Phaselocked`) -> [4.6 s] `OnTargetIsAboutToBecomeUnlocked` -> [5.7 s] `OnReleasedTarget`
(AI flag cleared, cooldown manager off, diminishing returns on the target) -> [6.7 s] skill `OnDeactivated`, delegate false,
`OnActionSkillDeactivated`.
Reload abort and the weapon-busy state are not in this list; they come from the cast move (section 12b).

## 11. Damage, state and release of the lifted target (what is script, what is data)

- **Native:** nothing special. The target's position is set by script every tick (NATIVE_PHASELOCK_TARGETING 5); special-move
  play is the stock native (NATIVE_PHASELOCK_PRESENTATION 2.3); `WillowPawn.IsStaggered`, `IsAliveAndWell`, `SetDefaultPhysics`
  are read-only helpers here.
- **Held state:** `Attributes.IsPhaselocked` PostAdd +1 on the target (read by `IsPhaselocked` so a second lock is refused; the
  attribute is removed on release); AI flag `Flag_Skills_IsPhaseLocked` true at lock, false at release; AI hold; forced
  uncloak; collision rules; all as in the existing notes.
- **Damage while lifted:** none from the base skill (PHASELOCK_STOCK_DATA); the target takes ordinary damage from weapons. A target
  that dies, or is not alive-and-well, ends the lock via `InterruptPhaseLock` (section 7).
- **Release:** `ReleaseTarget` as in section 7; the diminishing-returns skill (25 s, time scale -0.4 on the target) is activated
  by `OnReleasedTarget`; an incapacitated or flying target is not dropped with the fall clip but left (its world collision is
  restored and the loop clip stopped).

## 12. Native internals that govern the lifetime

### 12a. Skill per-frame refresh, expiry of an action skill (extends NATIVE_SKILLS 4.5)
- **Reads:** the skill's `Duration`, `StartTime`, state, the definition's `DurationType`, `bDoNotShiftPastCurrentTime`,
  `ActionSkillArchetype`, and the controller's pawn's `MyActionSkill`.
- **Does:** when a Timed skill has `StartTime + max(Duration, 0)` in the past: if the definition has **no** action skill
  archetype, the state becomes Deactivated at once (as NATIVE_SKILLS says). If it has one: fetch (and cache in the skill) the pawn's
  action skill actor; **once** (a per-skill flag) call its script `OnActionSkillWantsToDeactivate` (sets the actor's
  `bWantsDeactivate` and tells the owning client); then every frame call the script function `IsDeactivateBlocked` (returns
  `bBlockDeactivate`, default false). While blocked the skill stays in its current state; when not blocked the state becomes
  Deactivated and the cache is cleared. If there is no action skill actor the skill expires normally.
- **Calls into script:** `OnActionSkillWantsToDeactivate`, `IsDeactivateBlocked`, then `Deactivate` / `Resume` / `Pause` per
  the new state (as before).
- **Constants:** none (Phaselock: Duration 120).
- **Implementer checklist:** the 120 s limit exists but is shadowed by the actor's schedule (ends at 6.7 s); keep it as a
  backstop, with the want-to-deactivate / blocked handshake, so a blocking subclass would be honoured.
- **Open:** per-frame reset of the "wants to deactivate" flag when the skill is re-activated.

### 12b. Cast special move blocks weapon actions (script; resolves "why the reload aborts")
`Phaselock_1st3rd_SM` and `Phaselock_Fizzle_1st3rd_SM` are `SpecialMove_FirstAndThirdPersonAnimation` data with
**`bBlocksWeaponActions` true** (first-person moves `Anim_Phaselock` / `Anim_Phaselock_Fizzle`, `AnimName` `AA_PhaseLock` /
`AA_PhaseLock_Fail`). When such a move starts on a player pawn with a controller and a positive duration (the duration is what
the animation definition's start returns, presumably the clip length; not read),
`SpecialMove_WeaponAction.ClientStarted` calls `WillowPlayerController.PerformSharedWeaponActions(duration)`: **stop reloading on
the main and the off-hand weapon (`StopReloading`, which calls the native `OnAbortReload`), force unzoom, put the weapon into
the `WeaponBusy` state, set `bPerformingSharedWeaponAction`, start a timer `WeaponActionComplete` of `duration`** (1 s when the
duration is not positive). `WeaponActionComplete` clears the flag, tells both weapons (`FinishedWeaponAction`) and calls
`CheckReload`. Effects for Maya: (1) a reload in progress is aborted at the cast, without refill (confirmed in game 2026-10-02:
`OnAbortReload` observed); (2) for the length of the cast move the weapons are busy and `CanPerformWeaponAction` is false (the
weapon-action constraint is evaluated before the move plays, so the cast does not refuse itself). Because the cast move is played
only when an authority-side target exists (section 10, as read), a cast with no target at all may not block weapons (UNVERIFIED).

### 12c. `ActionSkill` per-frame update (native)
- **Does, every frame the world is running:** (1) if `NextTick` is in the past: raise the script event `OnActionSkillTick(DeltaTime)`
  and set `NextTick := now + TickRate` (`TickRate` 0 means every frame; `LiftActionSkill.OnActionSkillTick` is where the lift
  follow, light, landing, effects and the incapacitation check run, section 10). (2) The three behavior timers
  (`ActionSkillTimers`): any enabled timer whose time has passed is disabled and its event raised through script (not used by
  Phaselock). (3) **Nearing completion:** only when the actor's role byte reads as authority and it has a player controller: compute remaining =
  (1 - `ActionSkillTime`) x `GetActionSkillDuration`; if remaining is **above** `ActionSkillNearingCompletionTime` (default
  2 s) clear the "event fired" flag; else if not fired yet, fire `OnActionSkillNearingCompletion` once and set the flag.
  (4) If the actor is flagged for destruction, destroy it.
- **Edge cases:** for Phaselock `ActionSkillTime` is the fraction of the skill object's 120 s, so nearing completion would fire
  at 118 s after the cast; it never gets there. The event has no linked behavior in the Phaselock data.
- **Implementer checklist:** call the actor's tick every frame; nothing else is needed for the slice.

### 12d. `WillowPawn.IsActionSkillRunning` and `ActionSkillStateExpressionEvaluator.Evaluate`
- `IsActionSkillRunning`: a virtual native; its body was not located. Every reader found treats it as "the pawn has a
  `MyActionSkill` actor" (the spawn happens in `EnableActionSkill`, the reference is cleared in `TearOff`); the likely
  definition is `MyActionSkill` exists and is initialised. UNVERIFIED.
- `ActionSkillStateExpressionEvaluator.Evaluate(object)`: resolve the object to a pawn (the object itself if it is one; if it is a
  game controller class its pawn; a further fallback through another owner link was not identified), ask `IsActionSkillRunning`, and return true if (the evaluator's
  "must be running" flag is set and it is running) or (its "must not be running" flag is set and it is not), else false; no pawn
  gives false. Usable as a skill-constraint evaluator. A skill event-response filter elsewhere in the manager applies the same
  meaning with a one-byte kind (1: only while the action skill runs, 2: only while it does not). Not used by Phaselock's own
  constraints.

## 13. Cooldown and progress shown to the player (native HUD, low confidence)

- **`WillowPlayerController.SetActionSkillTime`** (native, virtual): remembers the old value, sets the field `ActionSkillTime` to
  -1, then **if the skill manager has the tree's action skill active, with Duration above 0 and state Active**, sets it to
  `clamp((now - Skill.StartTime) / Skill.Duration, 0, 1)`; if the value changed it marks the controller's replication state
  dirty. So `ActionSkillTime` is -1 when nothing timed is running and otherwise the fraction elapsed of the **skill object's
  duration (120 s for Phaselock)**. Its caller was not found.
- **`WillowPlayerController.GetActionSkillDuration`** (extends NATIVE_SKILLS 5.1): the lookup is: the game must be running, the
  local player's skill manager exists, the controller has a skill tree whose action skill has an active instance; the result is
  that instance's effective `Duration`, else 0. Phaselock: 120 plus modifiers.
- **`WillowHUDGFxMovie.UpdateActionSkill`** (per HUD tick): (a) cooldown icon: from the pool's current value c and maximum m, when
  either changed by more than 1e-8, show frame `(1 - c / m) x 98 + 2` (2 just after a cast, 100 when ready) on the character
  cooldown clip, and when c reaches 0 after having been above 0 raise the "ActiveAbilityReady" cue (rate limited by a time test; its exact
  conditions were not resolved); (b) active-skill bar: value v = the controller's `ActionSkillTime`, **unless the
  player-name definition says to defer to the skill**, in which case v = the action skill actor's `GetDeferredActionSkillTime`
  (script; base returns 0; Lift returns (now - SkillStartTime - LiftDuration) / (SkillDuration - LiftDuration) while its state is
  not 0, else -1). Negative v hides the bar; otherwise the bar shows frame `v x 99 + 1`. Maya's `GD_PlayerNameId.Siren` has
  `GFxActionSkillHasBar` true and `GFxActionSkillDeferTimeToSkill` true, so her bar shows the lock progress, not 120 s progress.
- **Implementer checklist for the slice HUD:** cooldown icon fraction = 1 - remaining / 13 (frames 2 to 100); lock bar = Lift's
  deferred time; hide when negative.

## Edge cases collected
- Untrained (grade 0): pressing the key does nothing and no cooldown starts.
- Cast refused by a constraint (weapon putting down, injured, vehicle seat): no skill, no delegate, no cooldown, no message.
- Second press during the lock or the cooldown: nothing (flags default false).
- Ladder: the press is dropped before anything else.
- Fizzle: pool refilled then zeroed after 1 s; skill ends at 1 s.
- Early kill: release at once, skill ends 1 s later; the cooldown drain is already running from the release.
- Maya downed during the hold: the skill ends within a frame, the target is released at once.

## Open
- Native caller of `SetActionSkillTime` and of `ActionSkillCooldownComplete`.
- The body of `IsActionSkillRunning` and the two `GetActiveSkillForInstigator*` natives (virtual, class vtable not resolvable by
  the tool).
- Pool start value (pool creation native) and whether `ActiveSkillCooldownPool_Siren` is created refilled or empty.
- Whether a cast with no target plays any first-person clip (no script branch found that plays one).
- `Pawn.IsAliveAndWell` body (still), response-filter details of skill damage events.

## Not read yet
`SkillEffectManager.DeferActivateSkill` consumer and the `bAllowSkillActivation` switch owner; the HUD tracked-skill update
called from the manager tick; `ActionSkill.HandleTimerEvent` / `ITimerBehavior` timers; `Behavior_AIProvoke`, `Behavior_SetAIFlag`,
`Behavior_AttributeEffect` (other lanes); `LiftActionSkill.Ruin` and the upgrade paths; the replication of the action skill actor
(`ReplicatedEvent`, custom event replication) beyond what NATIVE_PHASELOCK_PRESENTATION covers.

## Corrections to earlier notes
- **NATIVE_SKILLS 5.2** says `ServerStartActionSkill` "refuses to start while on cooldown (except the special-cased
  `Skill_Gunzerking`)". The script refuses on cooldown for every skill; the Gunzerking name only changes the `coolingDown` flag
  used by the active-ability / cooldown-ability branches (`coolingDown and not active`). Section 2 lists the complete branch order.
- **NATIVE_SKILLS Open** ("`SkillDefinition.PlayerLevelRequirement` no use found"): the value for `Skill_Phaselock` is 1 (data); still
  no reader found.
- **NATIVE_SKILLS 4.5** (expiry waits for the action skill "to report the ability finished"): resolved in 12a: the skill calls the
  action skill's `OnActionSkillWantsToDeactivate` once and polls `IsDeactivateBlocked`.
- **NATIVE_SKILLS 5.2** ("wiring of `ActionSkillCooldownComplete` to the pool event not read"): the pool definition has no
  behaviors; the caller is native and still not found (Open).
- **PHASELOCK_STOCK_DATA UNVERIFIED** ("when exactly `ActionSkillCallback` refills the pool relative to `OnSelectedTarget`"): after it
  (section 5). Also "what `Skill_Phaselock.InitialDuration` 120 does": it is the skill object's backstop duration, the value
  `GetActionSkillDuration` returns, and the base of `ActionSkillTime` (sections 7, 12a, 13).
- **NATIVE_WEAPON_FIRING** (reload abort callers: "the start of an `ExecuteActionSkill` (Phaselock is one)"): for Phaselock the abort comes
  from the cast special move with `bBlocksWeaponActions` through `PerformSharedWeaponActions` (12b); `ExecuteActionSkill` is a
  different action skill class that calls `StopReloading` itself.
- **NATIVE_PHASELOCK_TARGETING sections 1 and 4**: confirmed that `Skill.Activate` makes the constraint check (its first
  statement) and that the cast aborts a reload; added that a failed check leaves no cooldown (section 4).
- **NATIVE_PHASELOCK_PRESENTATION 2.2 step 1**: the cast move is played only when the role is authority and a target exists; with
  no target nothing in `OnActionSkillStarted` plays a clip (still open who, if anyone, does).
