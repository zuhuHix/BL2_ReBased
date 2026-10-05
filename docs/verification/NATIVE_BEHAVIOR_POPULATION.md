# Native glue between missions, behavior sequences and spawning (2026-10-05)

AI-assisted (Claude), analyst lane G2. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Names of game classes, functions, properties and enum values are
quoted as the installed packages declare them; field names come from `tools/ghidra/class_layout.py`. No addresses.

Scope: what the Fire mission ("Rock, Paper, Genocide: Fire Weapons!", `GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire`)
needs between the mission tracker and the target dummy: how the dummy's behavior sequences get enabled and disabled by
mission state (`BehaviorSequenceEnableByMission`), how the dummy is spawned when the `Fire` objective becomes active (the
population den, `MissionPopulationAspect`), how its behavior providers get registered and receive `OnSpawned`, and the
leftovers of `NATIVE_MISSION_DISPATCH.md` ("Not read yet"). Accept, turn-in and objective bookkeeping stay with lane C1
(`NATIVE_MISSION_DISPATCH.md` sections B1 to B6 and the C1 note); loot with G1; stock Engine natives (actor spawn,
controllers, `Possess`) with G3.

Reading aids. Two naming traps cost time and the implementer should know them:

- The interface called `IMission` is the **mission observer** interface (six `MissionReaction*` functions). A class that
  "implements IMission" is an observer of mission changes, not a mission.
- Every `MissionReaction*` and several population "natives" are exec thunks that forward to a C++ virtual of the object;
  the behavior lives in that virtual, not in the thunk. Where several script functions share one body in the executable
  (the compiler folded identical bodies), this note describes the body once.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| MissionTracker.RegisterMissionObserver (and UnregisterMissionObserver) | `native function RegisterMissionObserver(IMission Observer, MissionDefinition Mission)` | Registers every condition and aspect that reacts to the Fire mission | medium | UNVERIFIED |
| (tracker internal) NotifyMissionObservers | not a script function | The six notification kinds and when the tracker fires them | high | UNVERIFIED |
| BehaviorSequenceEnableByMission.MissionReactionLevelLoad | `native function MissionReactionLevelLoad(MissionTracker Tracker, MissionDefinition Mission)` | Initial enable state of the dummy's FireDamage sequence | high | UNVERIFIED |
| BehaviorSequenceEnableByMission.MissionReactionStatusChanged | `native function ...(MissionTracker Tracker, MissionDefinition Mission, EMissionStatus NewStatus)` | Re-evaluation on accept / turn-in | high | UNVERIFIED |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveSetChanged | `native function ...(MissionTracker Tracker, MissionObjectiveSetDefinition NewSet, MissionObjectiveSetDefinition PreviousSet)` | Re-evaluation when `RocksPaper_FinalObj` becomes active | high | UNVERIFIED |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveUpdated | `native function ...(MissionTracker Tracker, MissionObjectiveDefinition Objective)` | Re-evaluation (progress change) | high | UNVERIFIED |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveCleared | `native function ...(MissionTracker Tracker, MissionObjectiveDefinition ClearedObjective)` | Re-evaluation | high | UNVERIFIED |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveComplete | `native function ...(MissionTracker Tracker, MissionObjectiveDefinition CompletedObjective)` | FireDamage turns off when `Fire` completes | high | UNVERIFIED |
| (C++ virtuals of the condition) link, unlink, verdict, before-enable, before-disable | not script functions | When the observer registers, how the verdict is turned into enable/disable | medium | UNVERIFIED |
| BehaviorKernel.ChangeBehaviorSequenceActivationStatus (the enable/disable engine) | `native final function ChangeBehaviorSequenceActivationStatus(BehaviorConsumerHandle, BehaviorProviderDefinition Provider, name SequenceName, EChangeStatus Action)` | Semantics of enable (1), disable (2), toggle (0), the mutex flag | medium | UNVERIFIED |
| BehaviorKernel.IntializeBehaviorProviderForConsumer | `native final function IntializeBehaviorProviderForConsumer(BehaviorConsumerHandle, BehaviorProviderDefinition)` | Order of sequence enabling when the dummy's provider is registered | medium | UNVERIFIED |
| SequenceEventEnableByMission.MissionReaction* (six) | same shapes as above, on `Engine.SequenceEventCustomEnableCondition` | Kismet-side twin: toggles `SequenceEvent.bEnabled` | medium | UNVERIFIED |
| MissionPopulationAspect (script only: Initialize, SetActivationFromMission, MissionReaction*, OnSpawnActor, ...) | script | The den enables itself while `Fire` is active | high | UNVERIFIED |
| PopulationOpportunity.SetEnabledStatus | `native final function SetEnabledStatus(bool bEnabled)` | The switch the aspect flips | high | UNVERIFIED |
| PopulationOpportunity.RespawnKilledActors (Den) | `native function RespawnKilledActors(float Fraction)` | Re-arms a den that already spawned its total | high | UNVERIFIED |
| PopulationOpportunity.DoSpawning (Den) with the master tick | `native function DoSpawning(PopulationMaster Master)` | Spawns one pawn at one spawn point | medium | UNVERIFIED |
| PopulationMaster.SpawnPopulationControlledActor / SpawnActorFromOpportunity | `native function ...` | Pawn creation and registration order | medium | UNVERIFIED |
| PopulationFactory(BalancedAIPawn).CanSpawn / GetSpawnProbabilityAtThisGameStage | `native function ...` | Candidate weights for the one-archetype den | medium | UNVERIFIED |
| WillowAIPawn.InitializeBehaviorProviders | `native function InitializeBehaviorProviders()` | Registers the pawn as consumer and adds the dummy's class provider | medium | UNVERIFIED |
| AIClassDefinition.OnSpawned / AIDefinition.OnSpawned | `native final function OnSpawned(BehaviorConsumerHandle Consumer)` | Delivers `OnSpawned` to the dummy's provider | high | UNVERIFIED |
| BehaviorKernel thread runner, event activation and `FilterObject` (internal) | not script functions | Leftovers from NATIVE_MISSION_DISPATCH | medium | UNVERIFIED |

## Fire mission, predicted order of events (UNVERIFIED, assembled from the sections below)

1. Level start. The den `PopulationOpportunityDen_13` (Description "Rocks, Paper, Genocide: Fire") begins play: it
   connects to the population master, then runs its aspect's `Initialize`. The aspect (`MissionObjective` = the `Fire`
   objective, `Activation` = class default `OPA_ActiveWhenObjectiveActive`) forces the den **disabled**, then registers as
   an observer of the Fire mission. Registration immediately delivers a "level load" notification, which evaluates the
   objective: not active yet, so the den stays disabled.
2. Accept, kickoff dialog, `RockPaper_GoToRange` completes, a behavior advances the mission to the `RocksPaper_FinalObj`
   set. The tracker switches the active set, then tells the observers "objective set changed". The aspect sees that the new
   set contains `Fire` and the previous set did not, so it evaluates with the **respawn flag set**: `Fire` is active, so
   the den is enabled and `RespawnKilledActors(1.0)` forgives earlier spawns.
3. Each population-master tick (every frame the world ticks it): the den checks whether a player is within `SpawnRadius`
   (30000) horizontally and `DenHeight` (class default 1024) vertically of the den, whether the spawn limits and
   `NextSpawnTime` allow a spawn, and calls its spawn routine, which creates one pawn from `PopDef_TargetDummy` at the
   spawn point `WillowPopulationPoint_40`.
4. Creating that pawn (still inside the engine's actor spawn): `WillowPawn.PostBeginPlay` spawns the default controller and
   possesses the pawn; `WillowMind.Possess` calls `InitializeBehaviorProviders`, which registers the pawn as a behavior
   consumer, adds the providers, and for the dummy's `FireDamage` sequence registers its enable condition as a mission
   observer. That registration delivers a "level load" notification, which evaluates the objective (now active), so
   `FireDamage` becomes **enabled**. `Possess` then calls `PostSpawn`, which sends the `OnSpawned` event to the class
   provider (the `Default` sequence, and `FireDamage`'s `OnSpawned` chain that emits `RocksPaper_MoveTargetForward`).
5. After the engine spawn returns: the factory sets the pawn's game stage and level from the balance definition, the master
   registers the spawned actor with the den (aspect `OnSpawnActor`: waypoint creation, nothing else for this den), and the
   den updates its counters and fires the Kismet spawn events (`SeqEvent_PopulatedActor` / `SeqEvent_PopulatedPoint`).
6. A hit with the incendiary damage type runs `FireDamage`'s `OnTakeDamage` chain (not this note). It updates the `Fire`
   objective; the tracker notifies observers "objective updated", then (count reached) "objective complete"; the
   condition re-evaluates, `Fire` is complete, so `FireDamage` is **disabled**.

## Section A: the observer plumbing

### MissionTracker.RegisterMissionObserver / UnregisterMissionObserver
- **Signature:** `RegisterMissionObserver(IMission Observer, MissionDefinition Mission)`; `UnregisterMissionObserver` is
  the inverse (not read separately).
- **Reads:** the tracker's `Role` (must be authority), its `MissionObservers` array (one entry per mission, each holding an
  array of observer interface references), the mission's status record, the mission's `bRepeatable`.
- **Does (in order):**
  1. Does nothing if the observer is null, the tracker is not the authority copy, or the mission is None.
  2. Immediately calls the observer's "level load" reaction (`MissionReactionLevelLoad`), so a new observer evaluates its
     state at once. (Which arguments the call carries was not recovered from the listing; the script signature is
     `(Tracker, Mission)` and that is what the interface thunk passes on.)
  3. Then stores the observer under the mission, **unless** the mission is already Complete and not `bRepeatable`: a
     finished one-shot mission gets the one notification but is never stored, since nothing can change afterwards.
- **Calls into script:** `IMission.MissionReactionLevelLoad` on the observer (through the object's script event path).
- **Edge cases:** the registration is only effective if the mission tracker exists when the observer registers; script
  callers fetch it from the game replication info and a None tracker silently skips the call (see aspect, section D).
- **Implementer checklist:** an observer registered with an Active mission gets exactly one level-load call before the
  registration call returns; registering for a Complete non-repeatable mission still calls it once but retains nothing;
  registration on a non-authority tracker does nothing.
- **Open:** exact argument values of the immediate call; whether registering the same observer twice stores it twice (the
  condition re-registers once per linked sequence record, harmless because reactions are idempotent).

### (tracker internal) notification kinds
The tracker keeps one internal routine that walks the stored observers of a mission and calls one reaction. Kinds, matched
to the call sites that raise them (UNVERIFIED mapping read from caller context and argument counts):

| Kind | Reaction | Raised when |
|---|---|---|
| 0 | `MissionReactionLevelLoad` | after the tracker has applied loaded mission data, for every mission record, after the replay events of NATIVE_MISSION_DISPATCH B5 have been raised (the replay is skipped for the front-end "Loader" map) |
| 1 | `MissionReactionStatusChanged(NewStatus)` | after a status change was accepted, before the directors and script hooks of B4 |
| 2 | `MissionReactionObjectiveSetChanged(NewSet, PreviousSet)` | after the "advance to set" routine has switched the active set (or replaced an active sub-set); **not** raised by clearing the set on Complete/Failed, which relies on the status notification |
| 3 | `MissionReactionObjectiveUpdated` | after progress was written, before the objective's event with id 3 fires |
| 4 | `MissionReactionObjectiveCleared` | after an objective was cleared in an Active mission (progress zeroed) and before the id-5 event |
| 5 | `MissionReactionObjectiveComplete` | when the count equals `ObjectiveCount`, before the set-completion evaluation and before the id-2 event |

- Observers are walked in registration order; a stale (destroyed) observer entry is dropped while walking. The call goes
  through the observer's interface, which invokes the script function by name; for a native-implemented reaction this lands
  in the C++ virtual of section B.
- The ordering that matters for the Fire route: the tracker notifies observers **after** it changed its own state (progress
  written, active set switched, status stored), so a reaction that reads the tracker sees the new state.
- **Implementer checklist:** raise the six kinds at the points above; make the observer list per mission; do not store
  observers of a Complete non-repeatable mission; raise kind 0 after an apply-load, once per mission record.

## Section B: BehaviorSequenceEnableByMission

Class data (all UNVERIFIED as game behavior): `LinkedMission`, `MissionStatesToLinkTo` (bits NotStarted, Active,
RequiredObjectivesComplete, ReadyToTurnIn, Complete, Failed, in status-enum order), `bIsObjectiveSpecific`, `bInstanced`,
`bCreateWaypoint`, `LinkedObjective`, `ObjectiveStatesToLinkTo` (bits NotStarted, Active, Complete),
`ObjectiveSetRestrictions`, and from the base class `LinkedBehaviorSequences` (a list of sequence records currently tied to
this condition). Class defaults: mission states {Active}, objective states {Active}.

The Fire dummy's four conditions (read from `Sanctuary_Dynamic`): FireDamage = mission Fire, objective-specific, objective
`Fire`, `bCreateWaypoint` true, default objective states {Active}. AmpDamage = mission Amp, objective `amp`, same flags.
Slagged = mission Amp, objective `KillCompetitor`, same flags. TargetDummy = mission Amp, **not** objective-specific, mission
states {Complete} only. Each sequence has its own condition object.

### The verdict (shared by all reactions)
Inputs: a tracker (None gives "false") and optionally an object key (see "per-instance objectives" below).

1. **Not objective-specific:** take the mission's status from the tracker (a mission with no record counts as NotStarted).
   The verdict is "that status's bit is set in `MissionStatesToLinkTo`".
2. **Objective-specific:** `LinkedObjective` None gives false. Otherwise classify the objective:
   - **Complete** if the tracker's "objective complete" rule holds: the mission status is Active, RequiredObjectivesComplete,
     ReadyToTurnIn or Complete (not NotStarted, not Failed) and the objective's progress equals its `ObjectiveCount`
     (exact equality; for `bRememberItemsWithinObjective` the bit-mask count is translated first). This is the same rule as
     `MissionTracker.IsMissionObjectiveComplete`.
   - otherwise **Active** if `MissionTracker.IsMissionObjectiveActive` holds. That native is "can the objective be updated
     now": the mission status is Active or RequiredObjectivesComplete, the mission has an active objective set, that set
     contains the objective (a collection set contains its sub-sets' objectives), and progress is still below the count.
   - otherwise **NotStarted** (this includes "mission Failed", "mission not active", "set not yet active").
   The verdict is "the class's bit is set in `ObjectiveStatesToLinkTo`".
3. **Set restrictions (both branches):** if the verdict so far is true and `ObjectiveSetRestrictions` is non-empty, the
   verdict becomes true only if at least one listed set is currently active in the mission (the active set, or one of its
   active sub-sets); an empty list does not restrict.

For per-instance objectives (`LinkedObjective.bRememberItemsWithinObjective`, a bit mask of collected items) the objective
state is additionally computed for the consumer's own item: "Complete" if that consumer's bit is set in the progress mask.
The key comes from the consumer object (not read further). The Fire dummy does not use this; the slice can leave it out.

### Reaction body (all six reactions do the same thing)
The six native reactions are identical in effect and ignore every argument except the tracker: whichever notification
arrives, the condition **re-evaluates the verdict from the tracker's current state and applies it to every sequence record
in `LinkedBehaviorSequences`**.

- Normal case (`LinkedObjective` None or not `bRememberItemsWithinObjective`): one verdict, applied to all linked records:
  true gives action Enable, false gives action Disable.
- Per-instance case: verdict per record, using that record's consumer key; records whose consumer cannot be resolved are
  skipped.
- Applying means asking the kernel to change one sequence record's state (section C): a record whose state record or
  process no longer exists is skipped. Enable of an already enabled sequence and disable of an already disabled one do
  nothing, so the sequence's `OnBehaviorSequenceEnabled` / `OnBehaviorSequenceDisabled` events fire only on a real
  transition.
- **Implementer checklist:** after any of the six notifications, recompute and apply for all linked records (do not
  filter by which objective or set the notification names); a None tracker means "disable".
- **Open:** whether the unused arguments are ever consulted (the body read ignored them); per-instance key derivation.

### How a condition becomes an observer (the lifecycle)
These are C++ virtuals of the condition class, called by the kernel when a provider is registered on a consumer
(section C gives the registration order):

- **Link a sequence record** (virtual 1): skipped when a global run-mode value equals 3 (not identified; the same test guards
  every hook below), and **skipped when `bInstanced`** is true. Otherwise adds (consumer handle, record) to `LinkedBehaviorSequences` if not already there.
- **After linking** (virtual 2): if the game world and its mission tracker exist, calls `RegisterMissionObserver` for the
  condition (as an `IMission`) with `LinkedMission`. That call delivers the immediate level-load notification of section A,
  which is the **initial verdict**: the condition's records get enabled or disabled right there. There is no separate
  "initial state" call; if the tracker does not exist yet the condition never registers and its sequences stay as
  `bEnabledOnSpawn` left them.
- **Unlink a record** (virtual 3): removes the record; if the list became empty, unregisters the observer.
- **Before enable / before disable** (virtuals 5 and 6, both default "no veto"): the mission condition only uses them for
  `bCreateWaypoint` (HUD waypoint on the consumer when the sequence enables for the linked objective, removed when it
  disables); they do not veto. A third hook runs when enable is requested for an already enabled sequence and refreshes the
  waypoint. Waypoints are presentation and out of scope for the slice.
- The multi-condition class `BehaviorSequenceEnableByMultipleConditions` (an array of conditions plus an all/any mode)
  exists and wraps these hooks; the dummy does not use it (not read further).

## Section C: the kernel side of enable/disable

### BehaviorKernel.ChangeBehaviorSequenceActivationStatus and the shared state change
- **Signature:** `ChangeBehaviorSequenceActivationStatus(consumer handle, provider (some callers pass None, presumably meaning any provider; the lookup was not read), sequence name,
  action)` as the script call sites pass it; the action enum is `ITargetable.EChangeStatus` {Toggle 0, Enable 1, Disable 2} (also what `Behavior_ChangeRemoteBehaviorSequenceState`
  passes).
- **Does:**
  1. Looks up the consumer's process; only continues if it exists and is live.
  2. Action 1 (Enable): if the record is already enabled, only the condition's "already enabled" hook runs. Otherwise
     asks the condition's before-enable hook (non-zero vetoes), then **if this sequence has `bSequenceEnabledMutex`**,
     looks for one other enabled sequence of the same provider that also has the mutex flag and disables it first (at most
     one is disabled), then sets the enabled bit and fires the sequence-enabled event (`OnBehaviorSequenceEnabled`) on this
     sequence.
  3. Action 2 (Disable): only if currently enabled: asks the before-disable hook (non-zero vetoes), fires the
     sequence-disabled event, clears the enabled bit and runs a small bookkeeping step (not read). Waiting threads of the
     sequence are not removed here; they end when the thread runner next visits them and finds the sequence disabled
     (section G1).
  4. Action 0 (Toggle): enable if disabled, otherwise disable.
- **Constants:** event names are the engine's fixed `OnBehaviorSequenceEnabled` and `OnBehaviorSequenceDisabled` (their
  numeric FName ids are in the executable; not needed).
- **Implementer checklist:** the "enabled" test in section G1 is the bit this sets; enabling fires its event before any
  other behavior of that sequence can run, and the enabled event is delivered **after** the enabled bit is set while the
  disabled event is delivered **before** the bit is cleared (so a disable event's own chain still runs, once).
- **Open:** whether any of the dummy's sequences carry `bSequenceEnabledMutex` (the package reader's behavior dump does not
  expose the flag; the Fire provider's sequences array is not tag-decoded by `--properties`); the replication branch.

### BehaviorKernel.IntializeBehaviorProviderForConsumer (and the registration it performs)
- **Does (in order), for a provider registered on a consumer:**
  1. Creates one state record per sequence (all initially disabled, with per-event runtime state), and per-process
     bookkeeping.
  2. **Pass 1:** every sequence with `bEnabledOnSpawn` is enabled, through the same enable routine (its event fires).
  3. **Pass 2:** every sequence with a `CustomEnableCondition` has its condition linked and, when linking succeeded, the
     condition's "after linking" step is run, which registers the observer and delivers the initial verdict (section B).
- **Consequences for the dummy:** `Default` and `Idle` are `bEnabledOnSpawn` true and enabled in pass 1; `FireDamage` and the
  other conditional sequences start disabled and are enabled in pass 2 only if their verdict is true; the sequences with no
  condition and `bEnabledOnSpawn` false (`ObjectiveComplete`, `Targetable`, `ResetTarget`) stay disabled until a behavior
  (`ChangeRemoteBehaviorSequenceState`) enables them.
- **A sequence with both `bEnabledOnSpawn` true and a condition** is enabled in pass 1 and can only be disabled in pass 2
  (its sequence-enabled event fires before the condition's verdict is applied). The dummy has none.
- **Implementer checklist:** run the three steps in this order; the verdict must be applied before the provider's first
  event (`OnSpawned`) is fired.

## Section D: the Kismet twin, SequenceEventEnableByMission

- Class `WillowGame.SequenceEventEnableByMission` extends `Engine.SequenceEventCustomEnableCondition` and has the same
  properties as the behavior condition minus `bInstanced`/`bCreateWaypoint`.
- The six `MissionReaction*` natives again share two bodies (LevelLoad, ObjectiveUpdated, ObjectiveCleared, ObjectiveComplete
  share one; StatusChanged and ObjectiveSetChanged share the other) and all do: compute the **same verdict** as section B
  (same mission/objective rules and set restrictions) and write it to the owning `SequenceEvent.bEnabled`; if it changed,
  call the script event `Toggled` on that event (`SequenceEvent.Toggled`, empty in the base class, overridden e.g. by
  `SeqEvent_TakeDamage`).
- One difference from the behavior side: with a None tracker the Kismet reaction returns without changing anything (the
  behavior side applies "disable").
- It registers itself as an observer for `LinkedMission` through a C++ virtual (the same tracker call, when the mission
  tracker exists and the same run-mode test as in section B passes). Who calls that virtual (event registration) was not read.
- Slice relevance: low. The Fire route's Kismet events are `WillowSeqEvent_MissionRemoteEvent`; none was found using
  `SequenceEventEnableByMission` (not searched exhaustively).
- **Implementer checklist:** if a SequenceEvent has this condition, set `bEnabled` from the verdict on every reaction and call
  `Toggled` only on change.

## Section E: population, from the aspect to a spawned pawn

### MissionPopulationAspect (script only; no natives)
Class `WillowGame.MissionPopulationAspect` extends `PopulationAspect`; its owner (`Outer`) is the den. Properties:
`MissionObjective`, `Activation` (`EObjectivePopulationActivation`: 0 OPA_External, 1 OPA_AlwaysActive,
2 OPA_ActiveWhenObjectiveActive, 3 OPA_ActiveWhenObjectiveNotComplete, 4 OPA_ActiveWhenObjectiveNotInactive,
5 OPA_ActiveWhenObjectiveComplete), `ObjectiveUpdateSetting` (`EObjectiveUpdateSetting`: 0 OUS_None, 1 MissionObjectiveOnDeath,
2 OverrideObjectiveOnDeath, 3 MissionAndOverrideObjectiveOnDeath, 4 MissionObjectiveOnStatAdd, 5 MissionObjectiveOnStatAddRemove,
6 OverrideObjectiveOnStatAdd, 7 OverrideObjectiveOnStatAddRemove, 8 MissionObjectiveOnAllDead, 9 OverrideObjectiveOnAllDead,
10 MissionAndOverrideObjectiveOnAllDead), `WaypointSetting` (0 PWS_None, 1 PWS_MissionObjective, 2 PWS_KillOverride, 3 PWS_All),
`OverrideObjective`, `bApplyObjectiveSetRestrictionToActivation`, `WaypointObjectiveSetRestrictions`, `ItemPools`, `bDefendTarget`.
Class default `Activation` is OPA_ActiveWhenObjectiveActive. The Fire den's aspect sets only `MissionObjective` = `Fire`,
`WaypointSetting` = PWS_MissionObjective, `WaypointActorSetting` = spawned actors: so `Activation` = default 2,
`ObjectiveUpdateSetting` = 0 (None). The Amp den's aspect (Den_11) sets `Activation` = OPA_External plus
`ObjectiveUpdateSetting` = OverrideObjectiveOnDeath with `OverrideObjective` = `KillCompetitor`.

- **Initialize** (called by the den's native begin-play, section E2): returns at once if `MissionObjective` is None, or if
  both objectives are set and belong to different missions. If `Activation` is not External: sets the den disabled, finds
  the mission tracker through the den's world game replication info, and registers the aspect (as `IMission`) as an
  observer of `MissionObjective`'s mission. (With External activation the aspect never touches enabled state.)
- **Reactions** (all end in `SetActivationFromMission`; the aspect's `ObjectiveUpdated` does nothing):
  - LevelLoad(Tracker, Mission): `SetActivationFromMission(Tracker, Mission)`.
  - StatusChanged(Tracker, Mission, NewStatus): `SetActivationFromMission(Tracker, Mission, respawn = NewStatus == Active)`.
  - ObjectiveSetChanged(Tracker, NewSet, PreviousSet): respawn flag = (NewSet contains `MissionObjective` and PreviousSet does
    not) or (`ObjectiveUpdateSetting` is 2 or 3, `OverrideObjective` is set, NewSet contains it and PreviousSet does not);
    then `SetActivationFromMission(Tracker, NewSet.Outer, respawn)` (a None NewSet makes the mission match fail: no effect).
  - ObjectiveCleared(Tracker, ClearedObjective): if it is `MissionObjective`, `SetActivationFromMission(..., respawn = true)`.
  - ObjectiveComplete(Tracker, CompletedObjective): `SetActivationFromMission(Tracker, CompletedObjective.Outer)`.
- **SetActivationFromMission(Tracker, Mission, bRespawnIfAlreadyActive = false)**: does nothing unless
  `MissionObjective.Outer` equals `Mission`. Computes `enable` by `Activation`: 1 true; 2 `Tracker.IsMissionObjectiveActive(
  MissionObjective)`; 3 not `IsMissionObjectiveComplete`; 4 active or complete; 5 complete; any other value (including
  External): false, **except** that if `ObjectiveUpdateSetting` is 2 or 3 and `OverrideObjective` is set and active, enable
  becomes true. Then, if `enable` and `bApplyObjectiveSetRestrictionToActivation`, enable is true only when one of
  `WaypointObjectiveSetRestrictions` is an active set. Finally acts on the den:
  - den currently enabled: if `enable` is false, `SetEnabledStatus(false)`; otherwise, if `bRespawnIfAlreadyActive`,
    `RespawnKilledActors(1.0)`;
  - den currently disabled: if `enable`, `SetEnabledStatus(true)` and, if `bRespawnIfAlreadyActive`, `RespawnKilledActors(1.0)`.
- **OnSpawnActor(SpawnedActor)**: grants the aspect's `ItemPools` items to the actor (none for the Fire den), creates the
  mission waypoint per `WaypointSetting` (needs the mission not Complete; attaches a `MissionObjectiveWaypointComponent`
  linked to `MissionObjective` and registers it with the tracker), sets `WillowPawn.MissionObjectiveToUpdateOnDeath` per
  `ObjectiveUpdateSetting` (not for the Fire den), and, if `bDefendTarget` and the mission is NotStarted/Active/Failed,
  adds a defend target. **OnActorDeath / AllActorsRemoved / DenStatAdded / DenStatRemoved / EnabledStatusChanged** update
  objectives or waypoints according to `ObjectiveUpdateSetting`; the Fire den does not use them (EnabledStatusChanged removes
  waypoints when the den is disabled and creates them when it enables with live actors).
- **Implementer checklist:** the Fire den spawns only while `Fire` is active (the set `RocksPaper_FinalObj` active, mission
  Active, progress below the count); it is disabled again at the first reaction after `Fire` completes; enabling it with the
  respawn flag resets its spawn counters.
- **Open:** `bInitialized` handling on the early returns; the rest of the waypoint creation.

### E2. When a den runs its aspect Initialize, and what enabling does
- The den (all `PopulationOpportunity` subclasses) runs this in its native begin-play, in this order: the actor's own
  begin-play, then (server/authority only) connection to the population master (`ConnectOpportunity`), then
  `Initialize` on its `Aspect` and on every entry of `Aspects`, then (if `bUseRandomSpawns`) it builds its random spawn
  order. So the aspect's disabling and observer registration happen **before the first master tick can spawn**.
- **PopulationOpportunity.SetEnabledStatus(bEnabled)**: if the value differs from `IsEnabled`, store it. If the
  opportunity has any aspect, compute `bHasActiveActors` = (number of live spawned actors > 0) and call
  `EnabledStatusChanged(bIsEnabled, bHasActiveActors)` on `Aspect` and on every entry of `Aspects`. Nothing else: **enabling
  does not spawn anything by itself**; the next master tick does.
- **PopulationOpportunityDen.RespawnKilledActors(Fraction)**: `NumTotalActors` (in `SpawnData`) is reduced by
  `trunc((NumTotalActors - NumActiveActors) * Fraction)`; with 1.0 every already dead spawn is forgiven.
- **Implementer checklist:** SetEnabledStatus notifies aspects only on change; RespawnKilledActors(1.0) sets the total
  spawned count equal to the live count.

### E3. The master tick and PopulationOpportunityDen.DoSpawning
- **Master tick (every world tick):** collects the world's player controllers (the controller list, keeping those that are
  `PlayerController`s) and for each registered opportunity calls the opportunity's per-tick routine with that list.
- **Opportunity tick routine:** returns at once unless `IsEnabled`. Then (1) a "refresh viewers" step (the den's version
  recomputes `bPlayerHasBeenDetected`: cleared, then set if **any** player controller passes the test below), (2) if the
  den's spawn gate passes, calls **DoSpawning**; (3) otherwise, if the den has nothing left alive and is not waiting, it
  starts a respawn wait (`bIsWaitingForRespawn`, `RespawnDelayStartTime`; only for opportunities whose master record carries
  its respawn flag, and only if no actor is alive or pending), and when the master's respawn delay has passed it resets the
  den (for the Den: `NumTotalActors` := `NumActiveActors`, `NextSpawnTime` := 0, as `RespawnKilledActors(1.0)` plus clearing
  the delay) and stops waiting.
- **Player test (den):** with `bOpportunityRadius` true (class default; the Fire den keeps it): the player's pawn location
  is within `SpawnRadius` of the den's location in the horizontal plane (squared-distance test) **and** the vertical
  difference is within `DenHeight`. Otherwise, if `DetectionVolumes` is non-empty, the pawn must be inside one of them.
  A controller with no pawn fails. (Fire den: `SpawnRadius` 30000, `DenHeight` default 1024, den at about (9626.4, 9501.7,
  3535.6), so the player must be within about 1024 units of height of that point.)
- **Spawn gate (den), all must hold:** the den's AI component exists; `bPlayerHasBeenDetected`; current time greater than
  `SpawnData.NextSpawnTime`; `NumActiveActors` below `MaxActiveActors` (which is refreshed each check to
  `MaxActiveActorsIsNormal` or `MaxActiveActorsThreatened` depending on the den's AI threat level: below 2 normal, else
  threatened; Fire den 1 and 3); `MaxTotalActors` is 0 (unlimited) or greater than `NumTotalActors` (Fire den: 1); and, if
  `ParentEncounter` is set, the encounter's wave check passes (the Fire den has no encounter).
- **DoSpawning (den), per call:** first reconciles the den's existing member list (dropping members that are gone or not
  pawns). If at least one member was dropped in this call it does not spawn this call. Otherwise, if the gate passes,
  it builds the list of spawn candidates from the den's population definition, **chooses one**, **chooses a spawn point**
  from `SpawnPoints` (in the Fire den the single `WillowPopulationPoint_40`), asks the master to spawn the actor with the
  point's location and rotation, the den's `SystemID` and the opportunity's game-stage values, and then runs the den's
  post-spawn step. At most **one actor per call**.
  - Candidate list: for each entry of the population definition, the entry's factory is asked `GetSpawnFactory` and
    `CanSpawn`; the entry weight (an attribute evaluated at the opportunity's game stage) is multiplied by the factory's
    `GetSpawnProbabilityAtThisGameStage`. For `PopulationFactoryBalancedAIPawn` the factory asks its
    `PawnBalanceDefinition`: `AIPawnBalanceDefinition.CanSpawn` is always true and the probability modifier is always 1.0
    (a factory with no balance definition gives probability 0). Candidates with weight not above 0 are dropped. One
    candidate exists for the Fire den (`PopDef_TargetDummy` -> `PopulationFactoryBalancedAIPawn_0` -> `PawnBalance_TargetDummy`
    -> archetype `GD_TargetDummy.Character.Pawn_TargetDummy`), so the weighted pick is moot.
  - Post-spawn step (den): arms `NextSpawnTime = now + RespawnDelay` when `RespawnDelay > 0`; copies the den's home data
    onto the pawn (home location, radius values, system id, combat volume, critical-actor flag); notifies the spawn point
    (which fires the Kismet `SeqEvent_PopulatedPoint` event) and the den (Kismet `SeqEvent_PopulatedActor`); counters
    (`NumTotalActors`, `NumActiveActors`) are incremented. (Exact call order of these small steps was not read; they happen
    after the pawn is fully created.)
- **Implementer checklist:** a den spawns at most one pawn per master tick; needs a player within the range test; honors
  `MaxTotalActors`; never spawns while disabled; a second spawn after the first dies requires `RespawnKilledActors` (aspect
  respawn flag or the den's own respawn wait).
- **Open:** the exact candidate pick and the "chosen spawn point" rule (single-point den only); capacity
  limits of the master (`HasCapacityToSpawnFromFactory`, `IsPopulationSystemAtCapacity`) were not read; "threat level"
  source for `MaxActiveActors`.
  - Game-stage region: the den's `GameStageRegion` (the Fire den: `GD_GameStages.Zone1.Sanctuary`, min/max 7 to 9) only
    supplies a (min, max) pair that is handed to each factory's `GetSpawnProbabilityAtThisGameStage`; (-1, -1) is used when
    there is no region. For `PopulationFactoryBalancedAIPawn` that call returns the balance definition's modifier, which is
    always 1.0 for `AIPawnBalanceDefinition`, so the region does **not** block the Fire dummy (UNVERIFIED; other factory
    classes may use the pair). The population definition used is the den's `PopulationDef`, or the parent encounter's
    wave definition when an encounter is set; None means no spawn.

### E4. Creating the pawn: PopulationMaster.SpawnPopulationControlledActor and the factory scripts
- The master's spawn call (virtual of `PopulationMaster`, also reached by the natives `SpawnActor` /
  `SpawnActorFromOpportunity`) asks the factory to create the actor: `PopulationFactoryBalancedAIPawn.CreatePopulationActor`
  runs script `SpawnAIPawn`:
  1. Gets the pawn archetype from `PawnBalanceDefinition.GetPawnArchetype`. None means nothing is spawned.
  2. The final location is the point's location raised by the archetype's `CylinderComponent.CollisionHeight`, rotated by
     the spawn rotation (that is, up by the collision height for a level spawn).
  3. If the spawn location context is a player controller or player pawn, the player controller is recorded as the pawn's
     `PlayerMaster` later; for a point context this is skipped.
  4. Calls `Master.SpawnPopulationControlledActor(class, ..., location, rotation, archetype, ...)`, which is the engine's
     actor spawn with the archetype as template (stock Engine; lane G3), then flags the pawn as population-controlled.
  5. **Inside that spawn**, `WillowPawn.PostBeginPlay` runs `SpawnDefaultController` (the AI controller class), which
     `Possess`es the pawn: see section F for what follows. So the pawn's behavior providers are initialized and its
     `OnSpawned` event is sent **before** step 6.
  6. `SetupBalancedPopulationActor`: sets the pawn's game stage and awesome level; computes its exp level (the balance definition's
     `DefaultExpLevel` evaluated at the game stage, which for the dummy is `EnemyLevel_GameStage_Exact`; with
     `bUseInstigatorLevel` the instigator's level is used instead), sets the mind's character class again, applies attribute starting values, initializes
     the balance definition state, builds the item pool list and default inventory, sets flags, records `MySpawnPoint`, and
     calls `SetupInitialDestination` from the point's initial destination (none for the dummy), then the clan.
- After the spawn returns, the master registers the new actor with the opportunity: it first notifies the factory (script
  `PopulationFactory.OnSpawnActor` path), then the opportunity's `OnSpawnActor` hook, which calls `OnSpawnActor(actor)` on
  the opportunity's `Aspect` and each of `Aspects` (section E), then, if the opportunity reports it has spawned everything,
  triggers its all-spawned Kismet event; then the den post-spawn step of E3 runs.
- **Implementer checklist:** order for the dummy: (pawn created, controller possesses, providers registered, `OnSpawned`) then
  (game stage / level set) then (aspect `OnSpawnActor`) then (den counters, Kismet populated events).
- **Open:** engine spawn internals (G3); where the master's spawn-cost accounting enters; the Kismet events' exact timing
  relative to `SetupBalancedPopulationActor`.

## Section F: provider registration and `OnSpawned` on the spawned pawn

### WillowMind.Possess (script) and WillowAIPawn.InitializeBehaviorProviders (native)
- Script order in `WillowMind.Possess(inPawn)` (server, non-vehicle case): remember the pawn, copy `AIClass` from the pawn,
  super `Possess`, set the pawn's `MyWillowMind`, set `CharacterClass`/`HomeLocation`, `InitializeCharacterClass`,
  **`InitializeBehaviorProviders()`**, then **`PostSpawn()`**.
- **WillowAIPawn.InitializeBehaviorProviders** (native; authority copies run it only when the pawn already has a controller):
  1. The `WillowPawn` part: registers the pawn as a behavior consumer if it has no `ConsumerHandle` yet (the kernel hands
     out a handle) and then initializes, in this order, the providers coming from the pawn's `BodyClass` and its parts,
     inventory-independent body/stance/death providers, and so on. (Detail not needed for the dummy; the order is body
     class first.)
  2. Then, if `AIClass` is set: initializes the provider of `AIClass.BehaviorProviderDefinition` (the dummy's
     `CharClass_TargetDummy.BehaviorProviderDefinition_5`), then the provider of `AIClass.AIDef.BehaviorProviderDefinition`.
  Each provider initialization is the kernel registration of section C, so conditions on those sequences register their
  mission observers and get their verdict then.
- **WillowAIPawn.PostSpawn** (script): if the pawn has a mind with an `AIClass`: `UpdatePlayerMaster`,
  `AIClassDefinition.OnSpawned(ConsumerHandle)`, then (if `AIClass.AIDef` is set) `AIDefinition.OnSpawned(ConsumerHandle)`,
  then cloaks if `ShouldCloak`.
- **AIClassDefinition.OnSpawned / AIDefinition.OnSpawned(Consumer)**: each fires the behavior event named `OnSpawned`, link
  id filter -1 (all links), no payload, on that consumer **for that definition's own provider only** (the provider is the
  one named by the definition, so the AIDef's `OnSpawned` reaches the AIDef provider, not the class provider). The event
  goes through the normal event activation (section G): only enabled sequences with an `OnSpawned` event entry react.
  No filter callback is visible in the call (see G3).
- **Consequences for the dummy:** `OnSpawned` reaches the class provider after its sequences were enabled by conditions, so
  with the Fire objective active `FireDamage.OnSpawned` runs (`RocksPaper_MoveTargetForward` per
  `BEHAVIOR_DATA_DECODE.md`) and `Default.OnSpawned` runs (IntMath and instance-data switches). `OnSpawned` is sent exactly once
  per spawn and only by `PostSpawn`; it is **not** re-sent when a sequence is later enabled.
- **Implementer checklist:** register consumer; initialize body providers; initialize the class provider (conditions applied
  immediately); initialize the AIDef provider; then fire `OnSpawned` on the class provider and then on the AIDef provider.
  In `src/slice.*` terms: `spawnDummy()` must apply the enable conditions first, then fire `OnSpawned`.
- **Open:** exact contents of the `WillowPawn` part; whether a client copy repeats any of it.

## Section G: BehaviorKernel leftovers from NATIVE_MISSION_DISPATCH "Not read yet"

### G1. "Sequence still enabled" on running threads
- A thread stores its sequence record. Before and during each run the runner checks the record's enabled bit: if the
  sequence is **not enabled**, the thread is ended without running anything (it is marked finished, its latent state is
  released). The check happens at the start and again before each behavior of a run, so disabling a sequence stops
  its threads at the next behavior boundary; a behavior already executing is not interrupted. The same enabled bit also
  gates event activation (an event on a disabled sequence is counted as skipped and does nothing).
- Additionally a run needs the consumer's process to be live (state "running"); a suspended or destroyed process stops
  threads the same way. The thread's due time must have been reached (`now >= due`); it runs at most 60 behaviors per call
  (the loop ends after the 60th), leaving the thread due immediately so it continues on the next runner pass.
- **Implementer checklist:** when `ChangeRemoteBehaviorSequenceState` / the mission condition disables a sequence, every
  waiting or scheduled thread of that sequence must stop without executing further behaviors.

### G2. Context objects and latent bookkeeping
Per behavior in a thread, the runner (UNVERIFIED; the structure was read only coarsely):
1. **Input variables:** each variable link of type Input on the behavior is copied from the sequence's variable table into the
   behavior's property of that name before the behavior runs, and cleared after (so no variable pointer stays on the shared
   behavior object). Object, vector, int, float, bool, attribute and instance-data variables are all handled by type.
2. **Context objects:** the behavior runs once per context object. If the behavior has a Context-type variable link, the
   context list is built from those variables (an Object variable contributes its object if it passes a class check; an
   AllPlayers variable contributes every player; an InstanceData variable contributes the pawn's instance-data object).
   Otherwise, if the behavior's `Context.ContextObject` is set, that single object; otherwise the single context is the
   consumer's own object. (The `EBehaviorContext` enum — Self, MyInstigator, OtherEventParticipant, EventData,
   UseContextObject — is consumed inside the behavior's own `ApplyBehaviorToContext`/`GetBehaviorContext` path, not by the
   runner; not read.)
3. **Call information given to the behavior:** current time, time since the thread's first run of this behavior, the
   frame's delta, a wait-time slot initialized to -1, and flags (has output links; behavior not yet latent-instanced).
4. **Latent wait:** after the call the behavior's wait-time slot decides: negative means finished; otherwise the wait is
   clamped to at least 1/60 s and the thread's due time becomes `now + wait`. On the **first** latent request the runner
   makes a per-thread **copy** of the behavior (marked as a runtime instance) so its state survives the wait, and stores the
   context list with it; later runs of the thread use the copy. A behavior that stays non-latent is never copied.
5. **After the contexts:** the thread's output links are chosen as in NATIVE_MISSION_DISPATCH A2 (recorded ids, plus -1 if
   `Context.bSupportsDefaultOutputLink`); the first selected link continues on the current thread with its delay, others start
   new threads; no selected link ends the thread. A thread that was cut by the 60-behavior cap keeps its position.
- **Implementer checklist:** latent behaviors need a per-thread instance with their own variable state; context lists are
  re-resolved on each run from variables (not cached across waits, except for the latent copy's stored contexts).
- **Open:** details of the instance-data and named-variable context types; how the stored contexts of a latent copy are used
  on resume; replication.

### G3. FilterObject
- An event definition (`EventData2.UserData.FilterObject`, base class `BehaviorEventFilterBase`) and a per-process runtime
  filter slot both exist. `BehaviorEventFilterBase.ShouldBeInstanced` (bit 0) says whether the definition's filter object
  is used directly or an instanced copy lives in the event's runtime slot.
- Activation applies filters through a **callback supplied by the caller**: the runtime filter slot is consulted when it is
  non-null **and** a callback was supplied; the definition's filter is consulted only when it is non-null, its
  `ShouldBeInstanced` bit is clear, and a callback was supplied; the callback receives the filter object, the consumer
  and two caller values and returns accept or reject. **Without a callback no filter is consulted and the event is not
  rejected by a filter.**
- The callers read in this lane (the tracker's mission events, `OnSpawned`, the script thunks `ActivateBehaviorEventFromScript`
  / `BroadcastBehaviorEventFromScript` through the shared "from script" routine) **pass no callback** (the argument that
  would carry it is zero in every decompiled call site read; but the decompiler dropped some stack arguments of the
  per-event helper thunks, so this is not certain for `OnTakeDamage`).
- Known filter classes: `WillowGame.EventFilter_OnTouch` (script `AllowedToRunThisEvent` on instigator class and allegiance
  flags) and `WillowGame.EventFilter_OnTakeDamage` (`DamageThreshold`, class default 0). The dummy's provider owns three
  `EventFilter_OnTakeDamage` objects with all properties at defaults (threshold 0), so even if they are consulted they
  reject nothing (UNVERIFIED which events carry them).
- **Implementer checklist:** keep ignoring `FilterObject` for mission events and `OnSpawned`; if `OnTakeDamage` filtering is
  added later, a threshold of 0 accepts any damage.

## Not read yet
- `MissionTracker` internals beyond the notification points (C1 owns accept/complete/turn-in); the argument values the
  tracker passes to the observer thunks (new/previous set, objective); `UnregisterMissionObserver` internals.
- Per-instance objective keys (`bRememberItemsWithinObjective`) and the exact way a consumer yields its key.
- `BehaviorSequenceEnableByMultipleConditions`; `bInstanced` conditions (they skip observer registration; what they do
  instead was not found).
- Whether any of the dummy's sequences set `bSequenceEnabledMutex`; the mutex's scope beyond "same provider".
- Master capacity limits, region/game-stage gating of dens, the exact candidate pick and spawn-point pick for dens with
  several points, `HasCapacityToSpawnFromFactory`, saved-actor/memento paths, `SpawnActor` / `SpawnActorFromOpportunity`
  natives other than as the pawn path above.
- The pawn's `WillowPawn.InitializeBehaviorProviders` provider list in full (stance, death, body parts).
- `EventFilter_OnTakeDamage` evaluation, if any.
- Kismet registration of `SequenceEventEnableByMission`.
- Engine-side actor spawn, controllers, `Possess` plumbing (G3).

## Corrections to earlier notes
- `BEHAVIOR_DATA_DECODE.md` section 2 and `src/slice.hpp`: the enable rule is almost right. Differences: (a) an objective is
  "Active" only if the **mission status is Active or RequiredObjectivesComplete**, the **active set contains the objective**
  and progress is **below** `ObjectiveCount`; "in the active set and not complete" omits the status requirement; (b) an
  objective counts as "Complete" only while the mission status is Active, RequiredObjectivesComplete, ReadyToTurnIn or
  Complete: for NotStarted or Failed missions it is NotStarted; (c) the set restrictions are **supported** by the native
  rule (true only if one listed set is active) and apply to both mission-level and objective-level verdicts; (d) every
  notification re-applies Enable/Disable to all linked records (transitions fire the sequence enabled/disabled events);
  (e) the initial verdict arrives when the condition registers (during provider registration), before `OnSpawned`.
- `NATIVE_MISSION_DISPATCH.md` B3: the "objective set changed" observer notification is raised by the advance-to-set
  routine after the active set was switched (not by the set-activation routine itself); clearing the active set on
  Complete/Failed does not raise it.
- `NATIVE_MISSION_DISPATCH.md` A2/Not read yet: the 60-behaviors-per-call cap, the enabled-bit check and the latent copy are
  described in section G above.
- `docs/NATIVE_ANALYSIS.md` / `tools/ghidra/class_layout.py`: for `WillowPawn`-derived classes the executable's offsets are 4
  higher than the computed layout from `ConsumerHandle` onward (`ConsumerHandle` computed 0xe04, used 0xe08; `AIClass`
  computed 0x10c4, used 0x10c8), while `BodyClass` (0x92c) and `MissionTracker` fields match. Something between `BodyClass`
  and `ConsumerHandle` is 4 bytes larger than the layout script assumes. Not investigated; treat WillowPawn offsets with care.
