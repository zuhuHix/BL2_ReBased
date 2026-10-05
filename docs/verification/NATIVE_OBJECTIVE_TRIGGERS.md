# Native objective triggers: how the Fire mission's objectives get progressed (2026-10-05)

AI-assisted (Claude), analyst lane G9. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Nothing in this note was confirmed in the running game. Names of game
classes, functions, properties and enum values are quoted as the installed packages declare them; field names come from
`tools/ghidra/class_layout.py`. No addresses. Script facts (the `WillowWaypoint` functions, `Pawn.TakeDamage`, the script
behavior `Behavior_UpdateMissionObjective`, the controller hook `UpdateMissionObjective`) were read with
`research/script_disasm.py` and are described by what they do.

Scope: the two objectives of `GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire`, so the host's stand-ins can be replaced:
`RockPaper_GoToRange` (host today: player capsule vs the `WillowWaypoint_9` cylinder) and `Fire` (host today: the dummy's
provider run by a host damage-type class). Accept/turn-in and B1 to B6 bookkeeping stay with lane C1
(`NATIVE_MISSION_DISPATCH.md`, `NATIVE_MISSION_SCRIPT_BRIDGE.md`); behavior kernel, observers and population with G2
(`NATIVE_BEHAVIOR_POPULATION.md`); stock Engine natives with G3 (`NATIVE_ENGINE_CORE.md`); use/interaction with G8.

## The picture in nine sentences

1. `RockPaper_GoToRange` is completed by a placed `WillowWaypoint` actor (`WillowWaypoint_9`, class `WillowGame.WillowWaypoint`,
   a `Trigger` subclass), not by a Kismet event, a trigger volume or a behavior. Its own script reacts to the engine's
   `Touch` event.
2. The engine raises `Touch` on both actors when the player pawn's collision overlaps the waypoint's `CylinderComponent`
   (radius 357.81, half height 145.31); the script keeps only pawns for which `IsPlayerOwned()` is true (so Marcus, who walks into
   the same cylinder, is ignored).
3. The script then asks the tracker whether the linked objective is currently updatable and, if so, calls
   `MissionTracker.UpdateObjective(LinkedObjective)` with no bit: **no player argument exists anywhere**; mission progress is one
   record on the world's tracker.
4. The cylinder is not switched on and off by the objective. If the player is already standing inside when
   `GoToRange_ObjSet` becomes active, the waypoint (registered as a mission observer) re-runs its check for every actor in its
   `Touching` list in the "objective set changed" reaction.
5. `Fire` is progressed by the dummy's `OnTakeDamage` behavior event. That event is raised natively from the AI controller's
   `NotifyTakeHit`, which `Pawn.TakeDamage` calls after the health change; its `DamageType` output is the **damage pipeline's
   `DamageTypeDef` object** and its `DamageSource` output is the damage-source class passed to `TakeDamage`.
6. The dummy's `Behavior_UpdateMissionObjective` is script. It calls the same `MissionTracker.UpdateObjective(Fire, bit)` through
   the world's tracker; `bit` is 0 unless the context object implements `IMissionObjective` (placed interactive objects and
   usable items do; a pawn does not).
7. `UpdateObjective` then tells every **local** `WillowPlayerController` `UpdateMissionObjective(Objective, Bit)` (HUD text and
   fanfare) and every non-local one `ClientReceiveMissionObjective`; no "instigating player" is passed.
8. The native mission behaviors (`AdvanceObjectiveSet`, `MissionRemoteEvent`, `ClearObjective`) act only when the behavior runs
   with **the `MissionTracker` as its own consumer** (that is how the mission's own provider runs them); a pawn's provider cannot
   advance a set.
9. The marker on the HUD is a `MissionObjectiveWaypointComponent` attached to the target actor (the stock `WillowWaypoint`, or the
   spawned dummy via the den's aspect, G2) and registered with the tracker; it is shown only for the **tracked** mission and only
   while its objective is updatable.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| Engine touch dispatch (actor overlap update, begin/end touch; no script name) | raises `event Touch(Actor Other, PrimitiveComponent OtherComp, vector HitLocation, vector HitNormal)` / `event UnTouch(Actor Other)` | GoToRange | medium (dispatch), low (shape test) | UNVERIFIED |
| WillowWaypoint.PostBeginPlay / Touch / ProcessPlayerTouch / MissionReactionObjectiveSetChanged (script) | script, `WillowGame.WillowWaypoint` | GoToRange | high (script read) | UNVERIFIED |
| Actor.IsPlayerOwned | `native simulated function bool IsPlayerOwned()` (virtual) | GoToRange | medium | UNVERIFIED |
| MissionTracker.IsMissionObjectiveActive | `native final function bool IsMissionObjectiveActive(MissionObjectiveDefinition MissionObjective)` | GoToRange, Fire (enable condition) | high | UNVERIFIED |
| MissionTracker.IsObjectiveSetActive | `native final function bool IsObjectiveSetActive(MissionObjectiveSetDefinition ObjectiveSet)` | waypoint restrictions | high | UNVERIFIED |
| MissionTracker.IsMissionObjectiveComplete | `native final function bool IsMissionObjectiveComplete(MissionObjectiveDefinition MissionObjective)` | enable conditions, HUD | high | UNVERIFIED |
| MissionTracker.UpdateObjective (fan-out, additions to NATIVE_MISSION_DISPATCH B1) | `native final function UpdateObjective(MissionObjectiveDefinition MissionObjective, optional int ObjectiveBit)` | GoToRange, Fire | high | UNVERIFIED |
| Behavior_UpdateMissionObjective.ApplyBehaviorToContext (script) | `WillowGame.Behavior_UpdateMissionObjective`, property `MissionObjective` | Fire | high | UNVERIFIED |
| Behavior_AdvanceObjectiveSet.ApplyBehaviorToContext (addition to B3) | virtual native, property `ObjectiveSetToAdvanceTo` | both transitions | high | UNVERIFIED |
| Behavior_MissionRemoteEvent.ApplyBehaviorToContext | virtual native, property `EventName` | Marcus walk, dummy moves | high | UNVERIFIED |
| Behavior_ClearObjective / Behavior_DecrementObjective / MissionTracker.DecrementObjective | virtual natives, `ObjectiveToClear` / `ObjectiveToDecrement`; `native final function DecrementObjective(MissionObjectiveDefinition Objective)` | not on the Fire route | medium | UNVERIFIED |
| MissionTracker.RegisterWaypoint / UnregisterWaypoint, MissionObjectiveWaypointComponent refresh and RemoveWaypoint | `native final function RegisterWaypoint(WaypointComponent Waypoint, MissionDefinition InMission)`; `RemoveWaypoint()` | marker (presentation) | medium | UNVERIFIED |
| WillowPlayerController.UpdateMissionObjective (script hook) | `simulated function UpdateMissionObjective(MissionObjectiveDefinition MissionObjective, int ObjectiveBit)` | HUD/presentation | high | UNVERIFIED |
| Pawn.TakeDamage to Pawn.NotifyTakeHit to Controller.NotifyTakeHit (WillowMind) to AIClassDefinition.OnTakeDamage | `native function OnTakeDamage(out BehaviorConsumerHandle ConsumerHandle, Object Instigator, float Damage, float ShieldDamage, Object DamageSource, Object DamageType)` | Fire | medium | UNVERIFIED |

## Engine touch dispatch (no script name)

- **What it is:** the engine routine that keeps an actor's `Touching` array current and sends script `Touch`/`UnTouch`. It is
  one actor-level "update overlaps" routine (the name is inferred) plus a begin-touch and an end-touch routine. It has no
  registered native name; it was found from the hard-coded names `Touch` (name index 306), `UnTouch` (307) and `PostTouch`.
- **When it runs:** after the actor moves, is teleported or spawned, and when actor collision is switched on or off (a dozen
  callers, one of them the registered native `PrimitiveComponent.SetActorCollision`). Nothing here is periodic; a player
  who does not move and whose collision does not change produces no new overlap check.
- **Candidate selection (inferred, shape test not read):** the routine asks the world's collision hash for the components that
  overlap the updating actor's collision component at its current location and extent (bounding boxes first), skips the
  actor itself and any candidate that is based on it (its `Base` chain leads up to the updating actor), and skips pairs that the actor's "do these two block each other"
  predicate rejects (touch is for pairs that do not block; the predicate compares collision flags of the two components,
  including a separate flag for zero-extent actors). Pairs that survive are touched. The precise cylinder-versus-cylinder
  test lives in the per-component overlap code, which was **not** read; standard UE3 cylinder overlap (planar distance
  below the sum of radii and vertical distance below the sum of half heights) is the expected rule, **UNVERIFIED**.
- **Begin touch (per ordered pair):** if `Other` is already in the recipient's `Touching` array, nothing happens (one `Touch`
  per pair until an `UnTouch`). Otherwise `Other` is appended and the script event `Touch(Other, OtherComp, HitLocation,
  HitNormal)` is sent, **only if** the recipient's current state frame has the `Touch` probe bit set (names 300 to 331 are
  probes, G3) or has no state frame at all. `WillowWaypoint` implements `Touch`, so its probe bit is set. After the event the
  routine reports whether `Other` is still in `Touching` (script may have removed it).
- **Order inside a pair:** the updating actor receives `Touch(candidate)` first; the candidate receives `Touch(updating actor)`
  only if the first touch persisted. Which of the two is "updating" depends on who moved (the player when walking in; the
  waypoint when it is spawned or its collision is enabled). The register that carries the recipient was lost by the
  decompiler, so the order is the less certain part (**low**).
- **End touch:** removes `Other` from `Touching`, sends `UnTouch(Other)` (probe bit 7, same gating) and, symmetrically, tells
  the other actor. `WillowWaypoint` has no `UnTouch` override, so leaving the cylinder does nothing.
- **Hit location and normal:** computed from the two components' positions (normal is the normalised vector between them, with
  (0,0,1) when degenerate). Irrelevant to the waypoint.
- **Implementer checklist:** raise `Touch` once per overlapping pair on entry and `UnTouch` on exit; keep and expose a per-actor
  `Touching` list; the waypoint's reaction needs the list (see below); a host overlap event is an acceptable replacement
  as long as Marcus's overlap is delivered to the same handler and filtered there.
- **Open:** the shape test; the callers' identities; the debug-only registration loop inside begin touch (runs only in a mode
  flagged by a global); whether a player standing in the cylinder at level start receives an initial `Touch`.

## WillowWaypoint (script) as it drives GoToRange

Class `WillowGame.WillowWaypoint` extends `WillowTrigger` (extends `Engine.Trigger`) and implements `IMission` (the observer
interface). Properties: `WaypointInfo` (struct `MissionObjectiveWaypointData`: `LinkedObjective`, `ObjectiveSetRestrictions[]`),
`AreaRadius`, `bUpdateObjectiveOnPlayerTouch`, `bEnabled`, `TouchVolumes[]`. The instance `WillowWaypoint_9` sets only
`LinkedObjective` = `RockPaper_GoToRange` and `bUpdateObjectiveOnPlayerTouch` = true (decoded with the project reader);
`ObjectiveSetRestrictions` and `TouchVolumes` are empty, `bEnabled` is the class default (true), `AreaRadius` is the class
default **0**. Its `CylinderComponent_3166` has `CollisionRadius` 357.81 and `CollisionHeight` 145.31 (half height), actor
location (9595.84, 9384.23, 3404.0), scale 1 (UE3 scales a cylinder by the owner's draw scales; both are 1 here, UNVERIFIED).

- **PostBeginPlay** (authority only, and only when `bEnabled`): returns without doing anything if `LinkedObjective` is None, if any
  restriction set is None or does not contain the linked objective, or if the objective's mission is already **Complete**.
  Otherwise it attaches a new `MissionObjectiveWaypointComponent` (copy of `WaypointInfo`, `WaypointRadius` = `AreaRadius`),
  registers it with the tracker (`RegisterWaypoint(component, objective.Outer)`), and, if `bUpdateObjectiveOnPlayerTouch`,
  registers the waypoint itself as a mission observer of that mission (`RegisterMissionObserver`, which immediately delivers a
  level-load reaction that does nothing here). If `TouchVolumes` is non-empty it associates itself with each volume and
  shrinks its own cylinder to 0 by 0; with no volumes the authored cylinder stays.
- **Touch(Other, ...)**: if `bUpdateObjectiveOnPlayerTouch` and `Other.IsA('Pawn')` and `Other.IsPlayerOwned()`, it runs
  **ProcessPlayerTouch**. It then always calls the `Trigger` base `Touch`, which activates any Kismet `SeqEvent_Touch` attached
  to this actor (none references `WillowWaypoint_9` in the mission's Kismet closure).
- **ProcessPlayerTouch:** takes the tracker from the world's replication info; proceeds only if
  `IsMissionObjectiveActive(LinkedObjective)`; if `ObjectiveSetRestrictions` is non-empty, also requires that at least one of
  those sets satisfies `IsObjectiveSetActive`; then calls `UpdateObjective(LinkedObjective)` with the bit omitted (0). Each
  qualifying `Touch` therefore adds one to the progress of an objective whose `ObjectiveCount` is above 1; for GoToRange
  (count 1) only the first counts, because the objective stops being active.
- **MissionReactionObjectiveSetChanged** (an observer reaction, raised by the tracker after the active set switched): for each
  actor currently in `Touching` whose `IsPlayerOwned()` is true, run ProcessPlayerTouch. The other five reactions are empty. This
  is what completes GoToRange when the player is already inside the cylinder at the moment `GoToRange_ObjSet` becomes
  active (for example, standing there while Marcus's kickoff dialog finishes).
- **ClearWaypoint** (not reached on this route): removes the waypoint components and clears `bEnabled`.
- **Enabled only while active?** No. The cylinder collides from level start; the objective check is the gate.
- **Implementer checklist:** completion = (a player-owned pawn overlaps the cylinder, on entry or when the set becomes active)
  and (the tracker says the objective is updatable) and (restrictions pass); then exactly one `UpdateObjective(GoToRange, 0)`.
  Ignore AI pawns (Marcus). Do not require the player to leave and re-enter after the set activates.
- **Open:** whether a vehicle driven by the player counts (the ownership walk below says yes, UNVERIFIED); `Role` handling in
  co-op.

## Actor.IsPlayerOwned

- **Signature:** `native simulated function bool IsPlayerOwned()` (a virtual; the registered native only dispatches).
- **Reads:** the actor's `Owner` chain and, at the root, its controller.
- **Does:** follows `Owner` links to the root owner, asks the root for its controller (an actor with no controller answers
  none; a controller answers itself; a pawn answers its `Controller`), and, if one exists, asks that controller's own
  "is player owned" virtual. Result false when there is no controller.
- **Edge cases:** an AI pawn's controller answers false (this is why Marcus and the dummy do not complete GoToRange).
  The controller-side virtual was not read; "true for a `PlayerController`" is the expectation, **medium**.
- **Implementer checklist:** player pawn true, AI pawn false, a bare world actor false; unowned or controllerless pawn false.

## MissionTracker.IsMissionObjectiveActive / IsObjectiveSetActive / IsMissionObjectiveComplete

These are thin natives over three tracker routines that the rest of the tracker also uses.

- **IsMissionObjectiveActive** (this is the tracker's "can this objective be updated" test, also the first gate of every
  `UpdateObjective`): false for None; then the mission record of the objective's mission (its `Outer`) must exist and its status
  must be **Active (1) or RequiredObjectivesComplete (2)** (NotStarted, ReadyToTurnIn, Complete, Failed are all false); the record
  must have an active primary objective set; the objective must be in the mission's `ObjectiveDefs`; the active set must
  contain it (a virtual `ContainsObjective` on the set, which the branching and collection subclasses override; sub-sets of a
  collection are covered by the collection's override, not re-read here); and the progress must be **below** `ObjectiveCount`.
  For an objective with `bRememberItemsWithinObjective` the stored mask is first turned into a count through the objective's
  `TranslateObjectiveCount`.
- **IsObjectiveSetActive:** false for None or an unknown mission; true if the set is the record's active primary set or appears
  in its active sub-set list.
- **IsMissionObjectiveComplete:** false for None; false for NotStarted and Failed missions; for Active, RequiredObjectivesComplete,
  ReadyToTurnIn and Complete, true when the (translated) progress **equals** `ObjectiveCount` (exact equality, as B1).
- **Implementer checklist:** use the same predicate for the waypoint check, the dummy's condition (G2) and the tracker's own
  update gate; "complete" is not the negation of "active".

## MissionTracker.UpdateObjective: what it adds to B1 (fan-out to players)

B1 in `NATIVE_MISSION_DISPATCH.md` stands. New facts about step 4:

- The native entry point reads the objective and the optional bit (default 0) and queues the request on `ObjectiveUpdates`;
  the drain applies requests first-in first-out (B1). It does **not** check the network role (the decrement and clear routines
  do check authority); in single player this makes no difference.
- Order of effects inside one applied update, after progress was written: (1) observers get the "objective updated" reaction
  (kind 3, G2); (2) the **tracked mission's waypoint components** are told to refresh (below); (3) every **local player
  controller** of class `WillowPlayerController` (all of them, in the engine's player list order) receives the script event
  `UpdateMissionObjective(Objective, Bit)` (see its section), and, if the objective's `Outer` is a mission definition, the tracker
  itself receives the script event `TriggerMissionObjectivesChangedDelegates(Mission)` (HUD listeners); (4) every **non-local
  controller** in the world's controller list that is a `WillowPlayerController` receives `ClientReceiveMissionObjective`
  with the same two values; (5) the behavior event named after the objective, id 3, with the new count (B1/B7).
  If the count was reached, matching defend-mission entries are dropped, observers get kind 5, the objective's `StatId` is
  counted when set, the set-completion evaluation runs, the id-2 event fires, and a last pass runs over the registered mission directors.
- **There is no instigating-player argument anywhere in this path.** A touch by player A and a behavior hit caused by player B
  update the same world record; every player's HUD hears about it.
- **Implementer checklist:** the host's single local player gets exactly one `UpdateMissionObjective(objective, bit)` per applied
  update (not per touch, not when the update is refused by the gate), after the observers and waypoint refresh and before the
  behavior events.

## Behavior_UpdateMissionObjective (script)

- **Class:** `WillowGame.Behavior_UpdateMissionObjective` extends `BehaviorBase`, one property `MissionObjective`. Not native.
- **Does:** only when the world runs as authority: casts the `ContextObject` to `IMissionObjective`; if the cast succeeds, asks it
  for `GetObjectiveBit()` (placed `WillowInteractiveObject` and `WillowUsableItem` implement it; pawns do not); then, with the
  tracker taken from the world's replication info, calls `UpdateObjective(MissionObjective, bit)`; with a failed cast the bit is 0.
- **Who is "the player":** nobody. The context object matters only for the bit. For the dummy's `Fire` behavior the properties
  set are just `MissionObjective`; the context is the default one (G2, "Context objects"), a pawn, so the bit is 0 and one
  update means progress +1.
- **Default output:** the class default `bSupportsDefaultOutputLink` is true (the script behavior makes no explicit output call).
- **Implementer checklist:** `UpdateObjective(Fire, 0)` through the world's tracker, no player needed; a context object that
  implements `IMissionObjective` supplies the bit (G8 covers those classes).

## Behavior_AdvanceObjectiveSet: additions to B3

- **Reads:** world net mode, `ObjectiveSetToAdvanceTo`, the behavior's **self object**, the mission record of the target set's `Outer`.
- **New rule:** the behavior does nothing unless its *self object* (the consumer the thread belongs to) is the `MissionTracker`.
  This holds for the other native mission behaviors below. The mission's own behavior provider is run with the tracker as
  consumer (the tracker implements the consumer interface and fires mission events on its own handle), so mission-authored
  advances work; an `AdvanceObjectiveSet` placed in a pawn's provider would be silently ignored.
- **Target rule:** as B3 (the target must be the `NextSet` of the active set or of an active sub-set, or the mission's
  `InitialObjectiveSet` while no set is active; anything else is ignored). When a collection's sub-sets are involved, every
  active sub-set whose `NextSet` is the target is advanced.
- **After advancing (as G2's table, kind 2):** observers "set changed", the tracked mission's waypoints refresh, the HUD
  refresh, `StartBlockingSet`/`StopBlockingSet` handling, and the completion evaluation when the new set has
  `bCanCompleteMission`. A set whose `bCanCompleteMission` is set is evaluated at once.
- **Implementer checklist:** reject unless the consumer is the mission tracker; never advance from a pawn provider.

## Behavior_MissionRemoteEvent

- **Signature/props:** virtual native, property `EventName`.
- **Does:** nothing on a network client, nothing when `EventName` is None, nothing unless the self object is the `MissionTracker`.
  Otherwise it finds every `WillowSeqEvent_MissionRemoteEvent` in the world's main Kismet sequence **recursively** (the
  sequence's find-by-class), keeps those whose `EventName` equals the behavior's, whose `bEnabled` is set and whose
  `AssociatedMissionDefinition` equals the mission that owns the behavior (the behavior's `Outer`), and activates each of them
  (all outputs) with the world info as originator and the tracker as instigator. The usual event activation rules of
  NATIVE_MISSION_DISPATCH A4 apply (`MaxTriggerCount`, `ReTriggerDelay`).
- **Why the mission match matters:** this map has events with the same name in several missions (for example
  `RocksPaper_MoveMarcusToRange` exists for Fire, Shock and Corrosive, all wired to the same scripted-move action). Only the event
  whose mission matches runs.
- **Edge cases:** no main sequence, no match: nothing happens and no error.
- **Implementer checklist:** the same filter the host already applies (`kismet.missionRemoteEvent(missionPath, name)`); add the
  enabled flag and the consumer-is-tracker rule; activate every match, not only the first.

## Behavior_ClearObjective, Behavior_DecrementObjective, MissionTracker.DecrementObjective

Not used by the Fire route (no instance in the mission data); recorded because C1 listed them as unread.

- **ClearObjective:** nothing on a client; needs `ObjectiveToClear` and a tracker as self object. Cleared only while the mission
  is **Active** (status 1, not RequiredObjectivesComplete) and the objective is in its objective list: progress is set to 0,
  and, on the authority copy, observers get kind 4, the waypoints refresh, local controllers get script
  `ClearMissionObjective(Objective, ActiveObjectiveSet)`, non-local ones `ClientReceiveClearedMissionObjective(Objective)`, and
  the objective event with id 5 fires (the id table of NATIVE_MISSION_DISPATCH B7 says "cleared"). No set-completion
  evaluation was seen in this routine.
- **DecrementObjective (behavior and native):** the behavior needs a non-client world, `ObjectiveToDecrement`, and the world's
  replication-info tracker (it does not use its self object). The tracker acts only if the objective has **`bAllOrNothing`**
  set and is currently updatable: progress minus one, floored at 0; then local `DecrementMissionObjective(Objective,
  ActiveObjectiveSet)`, remote `ClientReceiveDecrementedMissionObjective`, and the objective event (id 4 in B7).
- **Implementer checklist:** low priority; same gates as above.

## MissionTracker.RegisterWaypoint / UnregisterWaypoint, MissionObjectiveWaypointComponent (presentation, brief)

- **Data:** the tracker keeps `MissionWaypoints`: one record per mission with the list of registered `WaypointComponent`s.
  `MissionObjectiveWaypointComponent` carries `WaypointInfo` (linked objective, set restrictions), `WaypointRadius`, and
  `bActive` (the marker is currently shown).
- **RegisterWaypoint(Waypoint, Mission):** ignored for None; ignored when the mission is **Complete and not `bRepeatable`**;
  also skipped when the component's owner is of a certain pawn kind (a test on the owner's class and an engine flag that was
  not decoded; not relevant to a stock `WillowWaypoint`); otherwise the component is added to the mission's list, and if that
  mission is the tracker's **tracked** mission (`ActiveMission`) the component is told to refresh at once. Unregistering removes it from the list.
- **Refresh (the component's own routine):** if the mission is Complete (non-repeatable), the component removes itself.
  Otherwise the marker should be visible when the linked objective is updatable (`IsMissionObjectiveActive`) and, with
  non-empty restrictions, at least one restriction set is active. On a change from hidden to visible the component's owner
  actor is added to the HUD's marker lists; on visible to hidden it is removed. A waypoint with `WaypointRadius` > 0 uses one
  of 4 "area" slots (it also carries a controller-side notification); radius 0 uses one of 6 "point" slots.
  Slot overflow means the marker is not shown.
- **Which waypoint is the "target":** all registered components of the tracked mission whose objective is updatable. For
  GoToRange that is `WillowWaypoint_9` (a point marker, since `AreaRadius` is 0). For Fire it is the component the den's
  aspect attaches to each spawned dummy (G2; created only while the mission is not Complete).
- **When the tracker refreshes them:** after objective updates (kind 3), set advances (kind 2), clears (kind 4) and status
  changes; also whenever the tracked mission changes.
- **RemoveWaypoint (the component's native):** unregisters from the tracker, hides the marker if shown, then detaches the component.
- **Implementer checklist:** presentation only; for the HUD, show a marker at `WillowWaypoint_9`'s location while GoToRange is
  updatable and the mission is the tracked one, hide it when the objective completes, and show a marker on the dummy while Fire
  is updatable.
- **Open:** the exact HUD class and the controller-side notification for area markers; the status-change path (internal kind 7).

## WillowPlayerController.UpdateMissionObjective (script hook, presentation)

- **Signature:** `simulated function UpdateMissionObjective(MissionObjectiveDefinition MissionObjective, int ObjectiveBit)`.
- **Does:** takes the tracker; returns if it is None or its data is not valid; copies the world's progress for that mission into
  the player's own mission record (`SetPlayersMissionProgressToWorldsMissionProgress`); if the objective's `ProgressMessage` is not
  empty, shows a HUD message of class `MissionFeedbackMessage`: for `ObjectiveCount` above 1 the text is
  "`<message>: <count> / <ObjectiveCount>`" (the tracker's `GetObjectiveCount` supplies the count), otherwise "`<message>`" plus
  the localised colon text plus the localised `SingleCountObjectiveComplete` text; then plays the mission fanfare with type 4
  if the objective is now complete and type 3 if not (arguments: type, false, mission, objective); finally refreshes the LCD
  status. For GoToRange the message is "Go to firing range".
- **Implementer checklist:** the host UI may print the progress message and play the fanfare; nothing here changes mission state.

## Damage to OnTakeDamage: Pawn.TakeDamage, NotifyTakeHit, AIClassDefinition.OnTakeDamage

Only the part not covered by G2 (G2 covers consumers, providers, enable conditions and the filter class).

- **Chain (script plus native):** `Engine.Pawn.TakeDamage` (script) runs its precondition check, momentum scaling, the
  friendly-fire and healing branches, `GameInfo.ReduceDamage`/`AdjustDamage`, `Actor.TakeDamage`, applies the health loss, and then
  (in the ordinary damage branch, not the healing branch, and not when the friendly-fire gate skipped everything) calls
  the native `Pawn.NotifyTakeHit(InstigatedBy, HitLocation, Damage, DamageType, Momentum, Pipeline)`. The `Damage` it passes is the
  value `TakeDamage` holds at that point, `DamageType` is the damage-source **class** `TakeDamage` received, `Pipeline` is the
  `DamagePipeline` object. That native forwards to the pawn's `Controller` (or, with no controller, the driver controller of
  its vehicle) as `Controller.NotifyTakeHit(InstigatedBy, HitPawn = the pawn, HitLocation, Damage, DamageType, Momentum,
  Pipeline)`.
- **The AI controller's override (`WillowMind.NotifyTakeHit`, native, identified by signature and by reading its body;
  that it sits in the controller's virtual slot was inferred):** returns at once when the world is not running. Then, in
  this order: (1) if `InstigatedBy` is set, records the attacker with a global AI bookkeeping object (threat/target
  bookkeeping; not read further); (2) if the pawn has an `AIClass` that owns a behavior provider, fires the behavior events
  described next; (3) always runs the base AI reaction.
- **The events (step 2):** two events named `OnTakeDamage` are fired for the pawn's consumer handle, in this order: first
  `AIDefinition.OnTakeDamage` (the AI definition's provider; no filter callback is passed), then `AIClassDefinition.OnTakeDamage`
  (the class provider, filter callback passed; the dummy's provider `CharClass_TargetDummy.BehaviorProviderDefinition_5` is
  the class provider). Both have the same five outputs. If the damaged pawn is a vehicle (a pawn-level virtual answers true)
  the vehicle pair `OnVehicleTakeDamage` fires instead, with six outputs (the vehicle as a second object output). No
  threshold or damage test gates the firing itself apart from the filter below; zero damage still fires.
- **Output variables of `AIClassDefinition.OnTakeDamage`** (the five variables the provider's `OnTakeDamage` event can bind; this is
  the "DamageType (ConnectionIndex 4)" the dummy's data compares):

  | Output | Value |
  |---|---|
  | `Instigator` | the `InstigatedBy` **Controller** (not its pawn); None for environmental damage |
  | `Damage` | the `Damage` argument of `NotifyTakeHit` |
  | `ShieldDamage` | `Pipeline.DamageSummary.DamageDealtToShields` (0 with no pipeline) |
  | `DamageSource` | the damage-source class argument of `TakeDamage` (for example the status-effect source class) |
  | `DamageType` | `Pipeline.DamageTypeDef`, a `WillowDamageTypeDefinition` object (None with no pipeline) |

  The mapping of the last two to the positions of the helper's arguments is read from the order of arguments, and it
  agrees with the data: the `FireDamage` sequence compares `DamageType` with the object
  `GD_Incendiary.DamageType.DmgType_Incendiary_Impact`, which is a damage-type definition, not a class.
- **The filter callback is supplied for the class-level OnTakeDamage** (this answers G2's open question): the helper that builds
  the event passes a small filter function (the AI-definition-level and vehicle helpers pass none). A filter object of class `EventFilter_OnTakeDamage` rejects the event when
  `Damage + ShieldDamage` is **strictly below** its `DamageThreshold` (class default 0, so nothing is rejected; the dummy's three
  filter objects are all at default). A non-`EventFilter_OnTakeDamage` filter object (none on this event) passes.
- **Which hits reach it:** a hit must reach `Pawn.TakeDamage`'s ordinary branch (preconditions pass, not skipped by the
  friendly-fire gate, not a healing damage type) and the pawn must have a controller. In the host: raise the event from the
  host's damage path after applying health, with the pipeline's damage-type-definition object as `DamageType`; a fire pistol
  shot qualifies if its pipeline's definition is `DmgType_Incendiary_Impact` (the damage-over-time definition is a different
  object and does not match the `FireDamage` comparison).
- **Implementer checklist:** `Fire` completes on the first `OnTakeDamage` whose `DamageType` object is the incendiary-impact
  definition, **after** the health change, through the `FireDamage` chain (G2's enable condition, then `Behavior_CompareObject`,
  then `Behavior_UpdateMissionObjective`); the instigator is a controller; no kill is required.
- **Open:** the AI-definition-level helper's body; the vehicle variants; the bookkeeping object; whether the controller virtual
  slot used here is the `WillowMind` override for every AI pawn.

## Not read yet

- The per-component overlap shape test and the collision-flag meanings behind "do these two block each other"; the callers
  of the overlap update (movement, teleport, spawn) by name.
- Whether the player standing inside the cylinder at level start gets an initial `Touch`.
- `AIDefinition.OnTakeDamage`, `OnVehicleTakeDamage`, the threat bookkeeping object and the base AI reaction.
- The controller-side "is player owned" virtual; the HUD class that owns the marker slots; internal tracker kind 7 for waypoints.
- `Behavior_ActivateMission` and `Behavior_CompleteMission` (same self-object rule expected, not read);
  `WillowPlayerController.SetPlayersMissionProgressToWorldsMissionProgress` and `ClientDoMissionStatusFanfare`.
- The non-authority paths of `Pawn.TakeDamage` (replication of the objective update to remote clients is irrelevant to the slice).

## Corrections to earlier notes

- `NATIVE_MISSION_DISPATCH.md` B1 step 4: the "script hooks" are `UpdateMissionObjective(Objective, Bit)` on **all local**
  `WillowPlayerController`s and `ClientReceiveMissionObjective` on all non-local ones; `TriggerMissionObjectivesChangedDelegates`
  is also fired on the tracker; the waypoint refresh sits between the observer notification and these hooks.
- `NATIVE_MISSION_DISPATCH.md` B3 and `NATIVE_MISSION_SCRIPT_BRIDGE.md`: `Behavior_AdvanceObjectiveSet` (and, by the same cast,
  `Behavior_MissionRemoteEvent`, `Behavior_ClearObjective`) ignores the call unless the consumer is the `MissionTracker`.
- `NATIVE_BEHAVIOR_POPULATION.md` G3: a filter callback **is** supplied when `OnTakeDamage` is fired (threshold test described
  above); the other callers read there still pass none.
- `SANCTUARY_RPG_MISSION.md`, "stand-ins": the real `CompareObject` input for `FireDamage` is the pipeline's
  `DamageTypeDef` object, not a damage-type class; "player capsule touches the cylinder while the objective is active" differs in
  two ways: the touch is delivered at any time and gated by the tracker, and the set-changed reaction re-checks actors that are
  already inside; AI pawns (Marcus) never complete it.
- `SLICE_WORLD_PLACEMENT.md` 1a: the completion rule is confirmed; additions: PostBeginPlay prerequisites (mission not Complete,
  restriction sets contain the objective) and the observer re-check.
