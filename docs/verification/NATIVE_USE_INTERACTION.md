# Native use interaction: from the use key to the mission accept / turn-in screen (2026-10-05)

AI-assisted (Claude), analyst lane G8. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script behaviour was read in the local WillowGame listing
(`research/script_disasm.py`) and the Engine package; script signatures come from the package declarations; field names
were mapped with `tools/ghidra/class_layout.py` (a computed layout that drifts a few bytes on `WillowAIPawn`, see "Layout
caveats"); data values (Marcus, the globals, the defaults) were read with `ow-package --properties`. Related notes:
[NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) (lane G4: `CanAffordToUseUsableObject`, `PayForUsedObject`,
`IsComponentUsable`, the cost query; not repeated here), [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) (lane
C1: `ActivateMission`, `CompleteMission`, rewards), [NATIVE_SLICE_CENSUS.md](NATIVE_SLICE_CENSUS.md). The accept screen's
GFx natives belong to lane G5, behavior-kernel internals to G2, the engine state machine and timers to G3, the dialog system
to G6.

## The whole path in one page

Today the host's use key accepts or turns in directly (`UOpenWillowQuest::TryUse`, a labelled stand-in with a 250 cm reach).
In the game the same key press goes through nine stages. Stages marked **[N]** are native C++ with no script body, **[S]** is
script the VM already runs, **[D]** is data.

1. **[N] Choosing the usable.** Every frame the controller runs a native evaluation (no script name) that casts one ray from
   the camera along the view direction, `GlobalsDefinition.PlayerInteractionDistance` (**350 uu**) long, walks the hits nearest
   first and keeps the first one that is a usable thing. Result: `CurrentUsableObject` (an `IUsable`), `CachedUsableHitComponent`,
   per-usability-type "can be used" and "has prompt" flags, the prompt icons and the cost data. There is no cone, no angle
   test and no priority table between NPCs, interactive objects and vehicle parts: first usable hit before a blocking hit wins.
   Sections "Per-frame usable evaluation" and "IUsable on an AI pawn".
2. **[S] The key.** `WillowPlayerController.Use` does nothing in script when a pickupable is current (pickups win the key),
   otherwise it calls `Engine.PlayerController.Use`, which routes to `ServerUse` and then `PerformedUseAction` on the
   authority. Section "The use key in script".
3. **[S+N] Use or refuse.** `PerformedUseAction` -> `CanAffordToUseUsableObject` (G4) -> `ServerUseWithoutConfirmation` ->
   `IUsable.UseObject(Pawn, CachedUsableHitComponent, UT_Primary)` -> `PayForUsedObject` (G4). The per-frame cache is zeroed so the
   next frame re-evaluates.
4. **[S] The NPC reacts.** `WillowAIPawn.UseObject` fires the AI class's and the AI definition's `OnUsed` behavior event with
   output link **Generic (2)**, counts the NPC's missions by state, and for the primary use also fires the HasMissions (0) or
   NoMissions (1) output and records `LastUsedTime`. Section "WillowAIPawn.UseObject" (script) and "AIClassDefinition.OnUsed".
5. **[D+S] Marcus's own behaviors.** Marcus's AI-definition provider answers `OnUsed` with a chain of "is story sequence X
   enabled" checks and, when none is, plays the mission context dialog and then `Behavior_HasMissions` -> `Behavior_ShowMissionInterface`
   (section "Marcus's chain").
6. **[S] Opening the screen.** `Behavior_ShowMissionInterface` calls `ClientGFxPlayMovie` on the user's controller with
   `GlobalsDefinition.MissionAcceptDefinition` (`UI_Mission.MissionInterface_Definition`) and Marcus as the context object.
   `QuestAcceptGFxMovie.Start` refuses to open (returns false) when the director has neither a redeemable nor an eligible mission.
7. **[S] The lists.** The screen asks the director (`WillowAIPawn`, an `IMissionDirector`) for redeemable, eligible and in-progress
   missions; each answer is script over the NPC's `MissionDirectivesDefinition` table and the tracker's `CanStartMission` /
   `CanEndMission` / `GetCompletedBranch`.
8. **[S] The buttons.** Confirming a "ready to turn in" entry calls `PlayerController.ServerCompleteMission(Mission, Director)`;
   confirming a "not started" entry calls `PlayerController.AcceptMission(Mission, Director)`. Both are the scripts C1 documents
   (they call the tracker natives, then `Director.OnPlayerAcceptedMission` / `OnPlayerTurnedInMission`, and turn-in then
   `PlayTurnIn`).
9. **[S+N] The talk state.** The movie's `Start` calls `BeginFocus` on Marcus and `WillowPlayerPawn.ServerPlayerBeginUseNPC`
   (which runs `WillowAIPawn.BeginUse`); `OnClose` undoes both. While a user is registered the NPC fires the AI `Used` event, keeps a
   "users" list, runs a 30 s linger timer, and the native look-at update points his head at the primary user.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| WillowPlayerController per-frame usable evaluation (no script name; sets `CurrentUsableObject`) | none (native tick) | Fire: choosing Marcus | medium-high (rule), medium (some gates) | UNVERIFIED |
| WillowPlayerController.UpdateInteractionIcon | `native function UpdateInteractionIcon(InteractionIconDefinition.InteractionIconWithOverrides Icon, IUsable.EUsabilityType UsabilityType)` | prompt presentation | high | UNVERIFIED |
| IUsable native virtuals of `WillowAIPawn` (icon, can-be-used, prompt, usable-by-user; no script names) | none (interface vtable) | Fire: Marcus is usable | medium-high | UNVERIFIED |
| WillowAIPawn.SetUsable | `native function SetUsable(bool bNewUsable, ActorComponent UsedComponent, IUsable.EUsabilityType UsedType)` | Fire: gates "talk" prompt | high | UNVERIFIED |
| WillowAIPawn.SetInteractionIcon | `native function SetInteractionIcon(InteractionIconDefinition Icon, IUsable.EUsabilityType UsedType)` | prompt icon | high | UNVERIFIED |
| WillowAIPawn.GetPrimaryUser | `native function Pawn GetPrimaryUser()` | talk state | high | UNVERIFIED |
| WillowAIPawn.HasAnyMissionsForPlayer | `native function bool HasAnyMissionsForPlayer()` (IMissionDirector) | director table, compass icon | high | UNVERIFIED |
| WillowAIPawn.GetMissionDirectorLocation | `native function Vector GetMissionDirectorLocation()` (IMissionDirector) | director table | high | UNVERIFIED |
| WillowAIPawn.GetAllDirectorData | `native function int GetAllDirectorData(out array<MissionDirectorData> OutData)` (IMissionDirector) | data access | high | UNVERIFIED |
| MissionTracker.RegisterMissionDirector / UnregisterMissionDirector | `native function RegisterMissionDirector(IMissionDirector MissionDirector)` | director table (presentation) | medium | UNVERIFIED |
| MissionTracker.ProcessDynamicMissionDirectives | `native function ProcessDynamicMissionDirectives()` | client refresh of the table | medium-low | UNVERIFIED |
| MissionTracker.GetCompletedBranch | `native function IMissionDirector.EMissionBranchEnding GetCompletedBranch(MissionDefinition Mission)` | redeemable filter (Fire: not used, branch None) | medium-high | UNVERIFIED |
| AIClassDefinition.OnUsed / OnSecondaryUsed / OnUserCouldNotAfford(+Secondary); AIDefinition.same | `native function OnUsed(out BehaviorConsumerHandle ConsumerHandle, AIDefinition.ENPCOnUsedOutputs EventOutput, Object Instigator, Object UsedComponent)` | Fire: starts Marcus's chain | high (event name, link), medium (payload shape) | UNVERIFIED |
| WillowAIPawn.UpdateLookAtTarget | `native function UpdateLookAtTarget()` | talk state (head look-at) | medium-low | UNVERIFIED |
| WillowAIPawn.GetFocusLocation / GetFocusRadius / GetFocusScreenOffset (IFocusable) | `native function Vector GetFocusLocation()` / `float GetFocusRadius()` / `Vector GetFocusScreenOffset()` | talk camera | medium-high | UNVERIFIED |
| WillowAIPawn.CanTalk, WillowMind.ShouldLookAtPlayer | `native function bool CanTalk()` / `native function bool ShouldLookAtPlayer()` | talk state (thin front ends) | low | UNVERIFIED |

Script functions on the path (not natives; the VM runs them, read here to settle behaviour): `WillowPlayerController.Use`,
`PerformedUseAction`, `ServerUseWithoutConfirmation`, `GetCurrentPickupable`; `Engine.PlayerController.Use` / `ServerUse`;
`WillowAIPawn.UseObject`, `FireOnUsedBehaviors`, `CountMyMissionsByState`, `GetEligibleMissions`, `GetInProgressMissions`,
`GetRedeemableMissions`, `BeginUse`, `EndUse`, `OnNewPrimaryUser`, `StartLingerTimer`, `OnUsersAreLingering`, `PlayOnUseDialog`,
`PlayDismissalDialog`, `BeginFocus`, `EndFocus`, `OnPlayerAcceptedMission`, `OnPlayerTurnedInMission`, `RegisterMissionDirector`,
`AddMissionDirective`; `WillowPlayerPawn.ServerPlayerBeginUseNPC` / `ServerPlayerEndUseNPC`; `Behavior_ShowMissionInterface`,
`Behavior_HasMissions`, `Behavior_PlayAIMissionContextDialog`; `QuestAcceptGFxMovie.Start`, `DetermineQuestEntries`,
`extChoiceConfirmed`, `extAcceptConfirmed`, `extCompleteConfirmed`, `OnClose`.

Enums read from the package: `IUsable.EUsabilityType` UT_Primary 0, UT_Secondary 1; `AIDefinition.ENPCOnUsedOutputs`
USEDNPC_HasMissions 0, USEDNPC_NoMissions 1, USEDNPC_Generic 2 (the byte of the `OnUsed` call is the link-id filter of the fired
event); `IMissionDirector.EMissionBranchEnding` EMBE_None 0, EMBE_PathA 1, EMBE_PathB 2; mission status as in the C1 note.

## Layout caveats

Offsets are given as field names. The computed `WillowAIPawn` layout is 4 bytes below the native one around `bUsable` /
`bCostsToUse` / `CostsToUseType`, 16 bytes around `MissionDirectives` (the native code reads those fields four and sixteen bytes
higher than `class_layout.py` predicts), so for the AI pawn the names below are matched by meaning (a value set from the same data,
the same width, the neighbouring fields), not by number. For `WillowPlayerController` the computed offsets of `CurrentUsableObject`,
`CurrentInteractionIcon[2]`, `CachedUsableObject`, `CachedUsableHitComponent`, `UsableObjectUpdateTime`, `UsableObjectUpdateRate`,
`CachedTradeProxy`, `bCachedCanBeUsed[2]`, `bCachedHasPrompt[2]`, `CalcViewLocation`, `CalcViewRotation` and `ConnectedVSSTerminal`
agree with every access seen. `GlobalsDefinition` and `AIClassDefinition` agree exactly (the field named in the sections below
is the one read).

## Per-frame usable evaluation (WillowPlayerController native tick, no script name)

- **Signature:** none. It is part of the controller's native per-frame tick (the function that also runs the camera helpers,
  the focus-camera return timer and the revive-target range test, which uses the same `PlayerInteractionDistance`: a
  `ReviveTarget` farther than that from the pawn, or no longer revivable, makes the tick fire the script event
  `ServerStopRevive`). It has no script name, so it is not in `natives.tsv`.
- **Reads:** `Controller.Pawn`, `MyWillowPawn`, `ConnectedVSSTerminal`, `Role`, `ReviveTarget`, the cinematic-mode input
  flags, `CalcViewLocation`, `CalcViewRotation`, `UsableObjectUpdateTime`, `UsableObjectUpdateRate` (class default **30**),
  `GlobalsDefinition.PlayerInteractionDistance` (**350 uu** in `GD_Globals.General.Globals`), the candidate actor's
  `IUsable` interface, its instance-data component table.
- **Writes:** `CachedUsableObject` (object, interface), `UsableObjectUpdateTime`, `CachedUsableHitComponent`, `bCachedCanBeUsed[2]`,
  `bCachedHasPrompt[2]`, `CachedTradeProxy` (and the trade-icon flags), then `CurrentUsableObject` and `CurrentInteractionIcon[2]`
  (through the icon update below).
- **Does (in order):**
  1. **Gate.** Runs only on the authority (`Role` is `ROLE_Authority`, 3; a standalone game always qualifies) and only when the
     controller has a pawn that is not flagged as being destroyed or otherwise excluded (two virtual predicates on the pawn: one is a
     pending-destroy style flag test, the other not identified), has no `ConnectedVSSTerminal`, and the `MyWillowPawn` flag word
     passes an unidentified test. Any failed gate sets the current usable to **None**.
  2. **Cinematic lock.** If any of the three "cinema disables input" flags of the controller (`bCinemaDisableInputMove`, `Look`,
     `Button`, bits 21 to 23 of the controller flag word) is set, the result is None and nothing is evaluated.
  3. **Throttle.** If the current time is earlier than `UsableObjectUpdateTime` the previous result is returned unchanged (the
     cached object, its component and the cached flags), after refreshing the per-type cost query (below). A fresh evaluation sets
     `UsableObjectUpdateTime = now + 1 / UsableObjectUpdateRate`, i.e. at most 30 evaluations per second. `ServerUseWithoutConfirmation`
     and `PerformedSecondaryUseAction` write 0 into `UsableObjectUpdateTime` after a successful use so the next frame re-evaluates.
  4. **Vehicle or foot.** If the pawn is in a vehicle (the pawn's vehicle query, or its driver's, answers), the candidates come from the
     vehicle (a list of usable parts); the first one whose primary use is available wins (not read in detail). Otherwise (on foot):
  5. **The ray.** One multi-hit line check with zero extent from `CalcViewLocation` to `CalcViewLocation + R(CalcViewRotation) *
     (PlayerInteractionDistance, 0, 0)`, source actor = the controller, trace-flag word `0x0102209F` (UNVERIFIED reading with the stock
     UE3 flag names: pawns, movers, level, volumes, others, level-geometry plus the terrain bit and two higher bits; neither stop-at-first-hit
     nor single-result is set, which fits the loop below). The engine returns the hits ordered from the camera outwards (stock UE3 behaviour,
     not read here).
  6. **Walking the hits** (each hit has an actor and a hit component):
     - A hit actor whose actor-flag bit 0x10 (first flag byte after the actor's identity fields) is set is skipped without blocking. Reading: that
       is the "pending destroy" flag; the same bit is what the pawn predicates and the AI pawn's can-be-used answer test.
     - **Usable candidate.** The actor is asked for its `IUsable` interface. If it has one and no candidate has been picked yet, and the
       object says it is usable by this pawn (an interface predicate; always true for an AI pawn), then:
       the **per-component flags** are read from the actor's instance-data table (the entry whose `Component` is the hit component): the entry's
       `bIsUsable` becomes "can be used (primary)", its `bIsSecondaryUsable` "can be used (secondary)", and "has a prompt" is true when the entry
       carries an icon for that type. For each type whose "can be used" is still false the object's own interface answers (`CanBeUsed(User, Component,
       Type)`), and for each type whose "has prompt" is still false the interface's prompt predicate answers. **If "can be used (primary)" is
       still false the hit is not a candidate** and the walk goes on to the blocking test. Otherwise the hit is accepted: `CachedUsableHitComponent`
       = the hit component; the walk continues only to look for a trade proxy.
     - **Player-trade proxy.** A hit whose primary icon is the trade-type icon (an icon definition whose type byte is 14) and that is a
       `PawnInteractionProxy` of another player is never the usable; it is remembered in `CachedTradeProxy` (and sets the trade-icon flags
       on the controller) and the walk continues.
     - **Blocking test** (for hits that were not accepted): a hit on a pawn whose flag word has bit 0x800 set is ignored; otherwise a hit
       with no component blocks when the actor's collision flag 0x40000000 is set, and a hit with a component blocks when the component's
       flag 0x10 is set. **A blocking hit ends the walk**, so a usable object behind a wall, a counter or a closed door is not selected.
  7. **Store.** `CachedUsableObject`, `UsableObjectUpdateTime` and `CachedTradeProxy` are written; the per-type cost query (`IUsable` cost
     slot, G4) fills the prompt's currency, amount and "costs" values.
  8. **Apply** (second native routine): `CurrentUsableObject` takes the result (cleared to None when nothing qualified) and
     `CurrentInteractionIcon[type]` is refreshed per type. The icon choice reads, in order of preference (medium-low confidence): the icon the
     hit component's instance-data entry carries; the global "already discovered object" icon for an already-discovered pickupable; the
     object's own icon for that type (an override, else the AI class default, see below); else `InteractionIcon_Default_Use`
     (`GD_InteractionIcons.Default.Icon_DefaultUse`). Each change goes through `UpdateInteractionIcon`.
- **Calls into script:** none on the common path (the apply step calls the object's icon interface; `Behavior_SetUsableIcon` is the script
  that sets an override).
- **Calls other natives:** `UpdateInteractionIcon`, the `IUsable` slots described next, the engine's multi-line check.
- **Constants / formulas:** 350 uu (`PlayerInteractionDistance`); 30 evaluations per second (`UsableObjectUpdateRate`); trace-flag word
  `0x0102209F`; trade icon type byte 14; blocking flags 0x40000000 (actor), 0x10 (component), pawn exemption 0x800.
- **Edge cases:** the ray is the camera centre line, so the target must sit under the crosshair and within 350 uu of the **camera**, not of
  the pawn; there is no angle cone and no radius sphere (the 30 cm sweep and 250 cm radius the host uses are not stock). Several usable
  objects on one line: the nearest wins. Items and NPCs share the same walk; pickups are not selected here (they have their own
  touched / seen state, see "The use key in script").
- **Implementer checklist:** (1) evaluate the ray at most every 1/30 s per local controller; (2) accept the first hit whose actor
  implements the usable interface, whose component or object says primary-use is available, and that is not behind a blocking hit;
  (3) ignore the trade proxy for the slice; (4) clear the current usable on a cinematic lock, a missing pawn or a pending-destroy pawn;
  (5) invalidate the throttle after a use; (6) expose `CurrentUsableObject`, `CachedUsableHitComponent`, the two flags and the two icons
  to the script (all the use functions read them).
- **Open:** the two unidentified pawn predicates and the `MyWillowPawn` flag; the exact meaning of the trace-flag bits; the vehicle
  branch; the trade-proxy range rule (an id-difference window); which collision flag bits 0x10 / 0x40000000 / 0x800 are named; whether
  the evaluation also runs on a non-authority client (it is gated on authority, a client uses the replicated `CurrentUsableObject`).

## WillowPlayerController.UpdateInteractionIcon

- **Signature:** `(InteractionIconWithOverrides Icon, EUsabilityType UsabilityType)`, no result.
- **Does:** compares the incoming icon record (the definition plus its override fields) with `CurrentInteractionIcon[UsabilityType]`;
  if equal nothing happens. Otherwise it stores the record, marks the controller's replication word dirty, and, when the controller is a
  local controller, forwards the record to the HUD's script-side `UpdateInteractionIcon` handler (presentation).
- **Implementer checklist:** store the record per type; a headless host may only store it; no result.
- **Open:** the exact fields of the override part (names only).

## IUsable on an AI pawn (native-only interface virtuals)

`IUsable` has three script functions (`UseObject`, `SetInteractionIcon`, `NotifyUserCouldNotAffordAttemptedUse`) and a set of native-only
interface slots. Read for `WillowAIPawn` (and the `WillowPawn` base where it differs). Slot semantics are inferred from how the per-frame
evaluation calls them (arguments, how the results are used) and from the code behind each slot; they are not named anywhere in the package.

| Slot (order in the interface) | Meaning | AI-pawn behaviour |
|---|---|---|
| icon for a type | "interaction icon of usability type T" | the pawn's `InteractionIconOverride[T]` (set by `SetInteractionIcon`) if non-None, else the pawn's `AIClass.UsableIconDef` (T = 0) or `UsableIconDefSecondary` (T = 1) |
| can be used (User, Component, T) | whether type T can be used right now | `bUsable[T]` of the pawn is non-zero **and** the actor is not flagged pending-destroy (one flag bit of the actor's flag word) |
| vehicle-part predicate | used only in the vehicle branch | always false for an AI pawn |
| has prompt (User, Component, T) | whether to show the prompt | forwards to "can be used" (same answer) |
| cost (User, T, out currency, out amount) | G4's `DoesObjectCostToUse` backend | `bCostsToUse[T]`, `CostsToUseType[T]`, `CostsToUseAmount[T]` |
| usable by this user | extra eligibility | always true |

For `WillowInteractiveObject` and the other implementers the table is separate and was not read (`WillowInteractiveObject.IsComponentUsable`
stays open, see G4).

- **Where the data comes from for Marcus:** the AI class `GD_Marcus.Character.CharClass_Marcus` has `bUsable` true (copied into the
  pawn's `bUsable[0]` when the mind applies the class defaults; `bSecondaryUsable` into `bUsable[1]`) and `UsableIconDef` =
  `GD_InteractionIcons.Default.Icon_DefaultTalk`. Behaviors change it at run time through `WillowAIPawn.Behavior_ChangeUsability`
  (toggle, enable, disable, optionally for one component) and `Behavior_SetUsableIcon`.
- **Implementer checklist:** a usable AI pawn answers true for primary use when its `bUsable[0]` is set (or its hit component's
  instance-data entry says so) and it is not being destroyed; icon per type as above; cost as G4.

## WillowAIPawn.SetUsable

- **Signature:** `(bool bNewUsable, ActorComponent UsedComponent, EUsabilityType UsedType)`; the exec thunk calls a virtual method.
- **Does:** if `UsedComponent` is not None and has an entry in the pawn's instance-data component table (first entry whose `Component` is
  that component): for `UT_Primary` it sets the entry's `bIsUsable` bit to `bNewUsable`, for `UT_Secondary` the `bIsSecondaryUsable` bit; any
  other type changes nothing; return. If the component is None, or no entry matches, it sets the **pawn-wide** `bUsable[UsedType]` byte
  (no check of the type value: the caller passes 0 or 1) to `bNewUsable`. No events.
- **Implementer checklist:** component entry bits first, then the pawn-wide byte; this is what `Behavior_ChangeUsability` calls.

## WillowAIPawn.SetInteractionIcon

- **Signature:** `(InteractionIconDefinition Icon, EUsabilityType UsedType)`.
- **Does:** stores `Icon` in `InteractionIconOverride[UsedType]` (no range check, no event). The icon query above reads it.

## WillowAIPawn.GetPrimaryUser

- **Signature:** no parameters, returns `Pawn`.
- **Does:** looks at `PawnsUsingMe`. Entries at the front that are no longer valid (a per-pawn virtual says "being destroyed") are removed one by one
  (the array is shrunk and its storage trimmed). If at least one entry was removed, the script event `OnNewPrimaryUser` is fired on the pawn. Returns the
  first remaining entry, or None when the array ends up empty. (Reading: the exec thunk calls a virtual method on the pawn; the base behaviour is this.)
- **Implementer checklist:** drop stale front entries, fire `OnNewPrimaryUser` once if any were dropped, return the front entry.

## WillowAIPawn.HasAnyMissionsForPlayer, GetMissionDirectorLocation, GetAllDirectorData (IMissionDirector)

- **HasAnyMissionsForPlayer:** false when the pawn has no `MissionDirectives` object; otherwise the answer of the directives object (next paragraph).
  The directives object looks at the **first local player's controller** (false when there is none or it has no character class) and walks its
  directive entries (mission, "begins mission", "ends mission", branch ending) in order. True for the first entry that is either
  (a) a **beginning** entry whose mission is startable: status NotStarted or Failed, or Complete with the repeatable flag, its dependencies met and the
  mission not blocked (the same test as `MissionTracker.CanStartMission` of C1); or (b) an **ending** entry whose mission has status
  RequiredObjectivesComplete (2) or ReadyToTurnIn (3), is not blocked, and either the entry's branch is EMBE_None or equals
  `GetCompletedBranch(mission)`. Otherwise false. In-progress missions alone do **not** count.
- **GetMissionDirectorLocation:** returns the pawn's own `Location`.
- **GetAllDirectorData(out OutData):** copies the pawn's `MissionDirectives.MissionDirectives` array into `OutData` and returns its length
  (no filtering).
- **Implementer checklist:** the three are pure reads; `HasAnyMissionsForPlayer` is the director table's membership test (section below).
- **Open:** `WillowInteractiveObject` and `WillowPickup` have their own versions (not read; the interactive-object one is probably the same
  directive walk).

## MissionTracker.RegisterMissionDirector / UnregisterMissionDirector / ProcessDynamicMissionDirectives

Presentation bookkeeping for the compass and the map ("quest giver here" icons); it does not decide what the screen offers.

- **RegisterMissionDirector(Director):** ignores a null director. Adds it to the tracker's list of registered directors (no duplicates). Unless a
  tracker flag (bit 2 of a flag word, meaning not identified) is set it asks the director through one interface method (a presentation refresh, not
  identified). On the authority it then asks `HasAnyMissionsForPlayer`; if true the director is entered in the tracker's **table of directors that
  currently have missions** (14 slots; each slot stores the director, its `GetMissionDirectorLocation` and some presentation data) and the tracker's
  replication word is marked dirty. On a client (not the authority) it walks a 10-slot local table and re-runs the matching slot's update with false.
- **UnregisterMissionDirector(Director):** removes it from the registered list and from the 14-slot table, then refreshes the other directors (an update
  routine that re-tests every registered director with `HasAnyMissionsForPlayer`, removing those with none and adding those that have some).
- **ProcessDynamicMissionDirectives():** called from `MissionTracker.ReplicatedEvent` when the replicated variable `DynamicMissionDirectives` arrives.
  A no-op on the authority; on a client it walks the 10-slot local table and refreshes each occupied slot with true.
- **Script caller:** `WillowAIPawn.RegisterMissionDirector` (called from `AddMissionDirective` and by its own 0.1 s retry while the tracker's data is not
  valid): `Tracker.RegisterMissionDirector(Self as IMissionDirector)` once `Tracker != None && Tracker.IsDataValid()`.
- **Slice relevance:** none for the accept path. A headless host may record the registration and skip the table; the C1 note's "directors get a
  status-changed call" refers to the same registered list.
- **Open:** which interface method the register call makes (slot 3 of the interface), what the 14-slot and 10-slot structures store beyond the director
  and its location, the tracker flag bit.

## MissionTracker.GetCompletedBranch

- **Signature:** `(MissionDefinition Mission)`, returns `EMissionBranchEnding` (byte).
- **Does:** 0 (EMBE_None) when the mission is None or its status is not Complete (4) or ReadyToTurnIn (3). Otherwise it follows `NextSet` from the mission's
  `InitialObjectiveSet` to the **last** set; if the last set is of the objective-set class that lists `ObjectiveDefinitions` (the check is a cast), it returns
  **2** when any of those objectives is not complete and **1** when all are; 0 when there is no initial set or the cast fails.
- **Edge cases:** the Fire mission's last set is `RocksPaper_FinalObj` with one objective, and Marcus's directive for it says EMBE_None, so the answer is
  never consulted for Fire.
- **Implementer checklist:** as above (EMBE_PathA = 1, EMBE_PathB = 2 in the enum order); it only matters for missions whose directive names a branch
  (Marcus: `M_OutOfBody`, `M_SafeAndSound`, both PathA).

## AIClassDefinition.OnUsed / OnSecondaryUsed / OnUserCouldNotAfford / OnUserCouldNotAffordSecondary and the AIDefinition twins

- **Signature (all eight):** `(out BehaviorConsumerHandle ConsumerHandle, AIDefinition.ENPCOnUsedOutputs EventOutput, Object Instigator, Object UsedComponent)`.
- **Does:** each builds a behavior event whose name is the function's own name (`OnUsed` for `OnUsed`; the other three by the same pattern, read for `OnUsed`
  and the class `OnSecondaryUsed` and assumed for the rest) with **link-id filter = EventOutput** and a two-object payload (the instigator and the used component),
  and activates it on the given consumer through the behavior kernel. The class version fires it **on the AI class's own behavior provider only**; the AIDefinition
  version on the AI definition's provider only (same split as `OnSpawned` in the G2 note).
  Behavior-kernel semantics (enabled sequences only, trigger counts, outputs published to the sequence's output variables, links followed depth first) are those of
  [NATIVE_MISSION_DISPATCH.md](NATIVE_MISSION_DISPATCH.md) section A1 and G2's note.
- **Called by:** the script `WillowAIPawn.FireOnUsedBehaviors` (used) and `FireOnUnableToAffordBehaviors` (cannot afford), which pick the primary or secondary variant by usability type and call the class
  version first, then the AIDefinition version, passing the pawn's `ConsumerHandle`.
- **Edge cases:** the output link ids are 0 HasMissions, 1 NoMissions, 2 Generic. A listener that connects to a different id never fires.
- **Implementer checklist:** event name = the function name; filter = `EventOutput`; payload = (instigator, used component); class provider before AIDefinition provider; no
  result.
- **Open:** how the payload objects reach the sequence's named variable (`PlayerWhoUsedMe` in Marcus's provider) is the kernel's output-variable publication (G2); whether
  the other three twins use exactly the names `OnSecondaryUsed`, `OnUserCouldNotAfford`, `OnUserCouldNotAffordSecondary` (Marcus's provider has an `OnSecondaryUsed` event; the others
  are by analogy).

## WillowAIPawn.UpdateLookAtTarget, the IFocusable natives, CanTalk, ShouldLookAtPlayer

- **UpdateLookAtTarget():** not called by script (native tick). Picks the pawn's `LookAtTarget`: with users registered the **primary user** (`PawnsUsingMe[0]`), otherwise the
  AI class's default target; when that candidate is missing or farther than a distance taken from the AI class data, the nearest human-controlled pawn within that distance (a scan of the
  world's pawn list) replaces it; the candidate must pass the controller's "may look at" check. The result is written into `LookAtTarget` with `LookAtOffset` zeroed and the replication
  word marked dirty, unless the mind is in a state that owns the head (a flag on the mind's class data). **Only the target pointer is native**; the head turn itself is the animation's
  `HeadLookAt` skel control. Confidence medium-low.
- **IFocusable (talk camera) natives:** `GetFocusLocation` = the location of the socket or bone named `GlobalsDefinition.FocusSocketName` on the pawn's mesh when that name is set and
  resolves (for Marcus the global value is unset), otherwise the pawn's view location; `GetFocusRadius` = `AIClassDefinition.FocusRadius`, 0 without a class; `GetFocusScreenOffset` =
  `AIClassDefinition.FocusOffset`, zero without a class. The values feed the controller's focus camera (`AdjustViewPointForFocusCam`, native, not read) while `FocusObject` is set.
- **CanTalk():** forwards to the pawn's `DialogComponent` ("can this component talk right now"); **ShouldLookAtPlayer():** forwards to a virtual on the mind. Neither is called on the use path
  (the second is called by `Action_GoToScriptedDestination`); both thin, not read further.
- **Implementer checklist for the slice:** set `LookAtTarget` to the primary user while a user is registered; the head turn and the focus camera are presentation.

## The use key in script

(Read from the Engine and WillowGame bytecode; the VM runs it.)

- **Use** (`WillowPlayerController.Use`, bound to E and to `XboxTypeS_X` by the engine's input config): proceeds only when `GetCurrentPickupable()` is None, the pawn is not in a
  transitional vehicle state and the status menu is not open; then `Engine.PlayerController.Use`, then the player input's `Use` button state is reset (so a held key does not repeat). With a
  pickupable current, `Use` does nothing; the pickup is the `PickupSomething` exec (the pickup binding is native / profile built and was not traced). Net effect for the one key: a
  current pickupable is taken first, otherwise the usable object.
- **GetCurrentPickupable:** None when nothing is touched. When something is touched: if its inventory definition exists and is flagged automatic pickup, the touched one; otherwise the seen one if
  there is one, else the touched one. (`CurrentTouchedPickupable` is set by touch events, `CurrentSeenPickupable` by `SawPickupable` from the native targeting.)
- **Engine.PlayerController.Use:** if `Role < ROLE_Authority` it calls `PerformedUseAction` (which refuses on a client, see below), and in every case calls `ServerUse`. **ServerUse** (the
  WillowGame override) does nothing while the status menu is open, else calls `PerformedUseAction` on the authority.
- **PerformedUseAction:** false on a non-authority. Needs a `CurrentUsableObject` and no `ReviveTarget`. If `CanAffordToUseUsableObject(Object, UT_Primary)` holds: when the object's cost query
  reports currency type 3 (golden key) it shows the confirmation dialog (`ClientShowGoldenKeyUseConfirmationDialog`; the dialog's yes calls `ServerUseWithoutConfirmation`), otherwise it calls
  `ServerUseWithoutConfirmation` and returns its result. If it cannot afford: `NotifyUnableToAffordUsableObject(0)` and the object's `NotifyUserCouldNotAffordAttemptedUse(Pawn,
  CachedUsableHitComponent, 0)`.
- **ServerUseWithoutConfirmation:** `CanAffordToUseUsableObject`; true: result = `CurrentUsableObject.UseObject(Pawn, CachedUsableHitComponent, UT_Primary)`, then
  `UsableObjectUpdateTime = 0`, then `PayForUsedObject`; false: the same unable-to-afford notifications. Returns the `UseObject` result. **The use happens before the payment.**
- **Secondary use** (`UseSecondary` / `PerformedSecondaryUseAction`): the same with `UT_Secondary`, and it additionally requires the `CurrentInteractionIcon[1]` definition to be non-None.
  Marcus has no secondary icon, so it is not offered.
- **For the slice:** call `WillowPlayerController.Use` on the VM controller after the host writes `CurrentUsableObject` and `CachedUsableHitComponent` (the evaluation above, or the host's own
  hit test as a labelled stand-in); `CanAffordToUseUsableObject` and `PayForUsedObject` are G4's natives (Marcus has no cost, so both reduce to "true" and "no-op").

## WillowAIPawn.UseObject (script) and the user list

(Script; listed because it fixes the order of effects.)

- **UseObject(User, UsedComponent, UsedType):** `ActualUsedComponent` is the component if `IsComponentUsable(component, type)` (G4) holds, else None. It returns false when the user has no
  `WillowPlayerController`, or the pawn has no mind, no `AIClass` or the class has no `AIDef`. Then, in order: `FireOnUsedBehaviors(Generic = 2, ...)`. For `UT_Primary`: `CountMyMissionsByState(User, out
  eligible, out inProgress, out redeemable)` (script that calls `GetEligibleMissions`, `GetInProgressMissions`, `GetRedeemableMissions`); if the three counts sum to more than 0, run
  `AIClass.OnUsedBehaviors` through `BehaviorBase.RunBehaviors` and then `FireOnUsedBehaviors(HasMissions = 0, ...)`; otherwise run `AIClass.OnUsedBehaviors_NoMission` and
  `FireOnUsedBehaviors(NoMissions = 1, ...)`; then set `MyWillowMind.LastUsedTime` to the world time; then if `AIClass.bWatchPlayerWhenUsed` (default **true**, `WatchPlayerTime` default 3 s) activate the
  AI component's state-machine event `Scripted`. Returns true. For `UT_Secondary` only the Generic event fires. Marcus's class has no `OnUsedBehaviors` lists and keeps `bWatchPlayerWhenUsed` at its default.
- **BeginUse(User):** ignored for a None user. Activates the AI component's event `Used` (this is what stops and turns an AI standing in a scripted state, G3), appends the user to `PawnsUsingMe`,
  and if it is now the only user calls `OnNewPrimaryUser`. **Called from `WillowPlayerPawn.ServerPlayerBeginUseNPC`, i.e. when the accept screen opens, not by `UseObject`.**
- **OnNewPrimaryUser:** clears `MissionsAcceptedByPrimaryUser` and `StartLingerTimer`: a 30 s timer (`TimeUntilConsideredLingering` evaluated from `GD_NPCShared.Attributes.LingerInMenuTime`, the
  attribute's constant is **30**; the class default is 0) that calls `OnUsersAreLingering`: if users remain, run `AIClass.OnLingeringBehaviors` for the first user, play the lingering dialog, restart the
  timer.
- **EndUse(User):** ignored for None. If the user is the primary one (`PawnsUsingMe[0]`): run `AIClass.OnDismissedBehaviors`, `PlayDismissalDialog` (the global generic dismissal line `DET_GenericDismissal`, played only when
  `GetAcceptedMissionToPlayDismissalDialogFor` finds **no** mission remembered as accepted during this visit; that function scans the remembered list from the last
  entry backwards and prefers a plot-critical mission without a DLC expansion), `ClearMissionsAcceptedByPrimaryUser`, and `OnNewPrimaryUser` when more than one user is registered. In every case the user is then removed from `PawnsUsingMe`.
- **BeginFocus / EndFocus (IFocusable, script):** `BeginFocus(User)` sets the user controller's `FocusObject` to the pawn and stores the current view location, rotation and `FocusFOVAngle = -1` (the focus
  camera input); `EndFocus(User)` clears `FocusObject`, stamps `FocusCamReturnTime` and calls `EndUse(User)`.
- **OnPlayerAcceptedMission / OnPlayerTurnedInMission:** C1 note ("IMissionDirector implementers"). Both ignore calls when nobody is using the pawn; accepted missions are remembered for the dismissal line;
  turn-in plays `PlayMissionTurnedInDialog` (the mission's `TurnInDialogEvent` else the global `DET_MissionTurnedIn`).

## Marcus's chain (data) and the lists (script)

- **Marcus's data:** `GD_Marcus.Character.Pawn_Marcus` owns a `MissionDirectivesDefinition` (`MissionDirectivesDefinition_1`) with seven entries; the Fire mission's entry is `{ M_RockPaperGenocide_Fire,
  bBeginsMission = true, bEndsMission = true, BranchEnding = EMBE_None }` (the Shock, Corrosive and Amp missions are identical; `M_OutOfBody` ends only, `M_SafeAndSound` and `M_ChosenOne` begin and end).
  There is **no `Behavior_AddMissionDirectives` in the Fire mission or in Marcus's providers**; the table is archetype data on the pawn (the ten `Behavior_AddMissionDirectives` instances in the game belong to
  other missions, such as `M_Ep3_CatchARide`).
- **How the lists are built (script):** `GetEligibleMissions` = entries with `bBeginsMission` for which `Tracker.CanStartMission` holds; `GetRedeemableMissions` = entries with `bEndsMission` for which
  `Tracker.CanEndMission` holds and whose branch is None or equals `GetCompletedBranch`; `GetInProgressMissions` = entries with `bBeginsMission` or `bEndsMission` whose `GetMissionStatus` is Active (1) (so
  RequiredObjectivesComplete (2) is redeemable, not in progress). Each returns the count and fills the out array; no entry is deduplicated beyond what `AddDirective` did when the table was built.
  All three need a `MissionTracker` on the replication info.
- **Marcus's AI-definition provider** (`GD_Marcus.Character.AIDef_Marcus.AIBehaviorProviderDefinition_0`, sequence `Brain`, enabled on spawn): its `OnUsed` event has one output link, on the Generic id (2), into a
  chain of nine `Behavior_IsSequenceEnabled` checks. In order the sequences tested are `Ep4_SpeakToMarcusAboutBank`, `Ep4_GetMarcusCrystal`, `Ep14_Rescued`, `M_TheBane`, `M_BearerBadNews`,
  `M_ClaptrapBirthdayBash`, `M_OutOfBody`, `M_SafeAndSound_BringPictures`, `Ep17_TalkToMarcus`. The first one that is enabled fires its own remote custom event on this provider (for example
  `Ep4_SpeakToMarcusCrystal`, `M_OutOfBody_TurnInMarcus`, `M_TheBane_TalkToMarcus`) and the chain stops; those sequences have their own dialog, and the turn-in sequences their own
  `Behavior_ShowMissionInterface`. **If none is enabled** the chain ends in `Behavior_PlayAIMissionContextDialog` -> `Behavior_HasMissions` -> (output 0) `Behavior_ShowMissionInterface`. The provider also
  listens to `OnSecondaryUsed` and `OnBehaviorSequenceEnabled`. With the stock data and the Fire mission's status the story sequences are not enabled (they are enabled by their own missions through
  `BehaviorSequenceEnableByMission`), so use goes to the default end.
  (Provider decoded with `ow-package --properties 30 --array-schema` over `Sanctuary_Dynamic`; `--behavior-run ... fire:2:OnUsed` stops at the first `Behavior_IsSequenceEnabled` because the executor treats it as a
  boundary behavior.)
- **Behavior_PlayAIMissionContextDialog** -> `WillowAIPawn.PlayOnUseDialog(user)` (script): silent when more than one user is registered; triggers on Marcus's dialog component one of the global dialog events
  `DET_OnUse_MissionComplete` (redeemable > 0), else `DET_OnUse_MissionsAvailable` (eligible > 0), else `DET_OnUse_AllMissionsInProgress` (in progress > 0), else `DET_OnUse_NoMissions`, with the user as the
  instigator. G6 owns the dialog event.
- **Behavior_HasMissions** (script): `QueryInterface(IMissionDirector)` on the context object; counts eligible + in-progress + redeemable; fires its output **0** when the sum is above 0, output **1** otherwise
  (`BehaviorKernel.ActivateBehaviorOutputLink`). Marcus's provider connects only output 0.
- **Behavior_ShowMissionInterface** (script): resolves the **user's controller** (a pawn argument is resolved through `Pawn.Controller`), reads `GlobalsDefinition.MissionAcceptDefinition`, and calls the controller's
  `ClientGFxPlayMovie(MissionAcceptDefinition, SelfObject)`; it does nothing when either is None. `SelfObject` is the behavior's own object, i.e. Marcus.
- **QuestAcceptGFxMovie.Start** (script; G5 owns the GFx natives it calls): the movie's context object must be an `IMissionDirector` and the player a `WillowPlayerController`, else it does not open. It stores the
  director, the focus subject (`IFocusable` of the context object) and `ContextNPC` (the context object as `WillowAIPawn`), calls `Director.OnPlayerOpenedMissionUI(PC)` (empty on all implementers),
  `DetermineQuestEntries`, and **returns without opening when there is no redeemable and no eligible mission**. Then the base movie starts, the list widget is created, `GotoCorrectPartOfMovie`, sound `MenuOpen`,
  the pawn's `BeginFocus` (focus subject) and `ServerPlayerBeginUseNPC(ContextNPC)`; the input delegates are installed and the tick rate set to 0.001 s.
- **DetermineQuestEntries:** redeemable missions go first with status 3 (ReadyToTurnIn), then eligible ones with status 0 (NotStarted); in-progress missions go to a second list (status 1).
- **The buttons:** `extChoiceConfirmed` reads the selected entry's status: 3 or 2 -> `extCompleteConfirmed`; 0 or 5 -> `extAcceptConfirmed`.
  `extAcceptConfirmed`: sound `accept_mission`, `PC.AcceptMission(Mission, MissionDirector)`, then `DetermineQuestEntries` and a redraw (the screen stays open; the list is rebuilt, so the accepted mission
  moves to in-progress). `extCompleteConfirmed`: sound `Confirm`, remembers `MissionDefForRewardPage`, `PC.ServerCompleteMission(Mission, MissionDirector)`; for a plot-critical mission with a
  `NextMissionInChain` it sets the plot-turn-in flags (so closing shows the chapter header). The rewards page opens from `ClientSpawnMissionRewardUI` (C1 note) through `DisplayRewardsPage`; `AcceptReward`
  closes or refreshes the list. `OnClose` (called whenever the movie closes) ends the focus (`EndFocus`), calls `ServerPlayerEndUseNPC(ContextNPC)` and `OnPlayerClosedMissionUI` (empty).
- **Calls into the C1 natives:** `AcceptMission` and `ServerCompleteMission` exactly as in the bridge note (`ActivateMission`, `CompleteMission`, the director callbacks, `PlayTurnIn`).

## Not read yet

- `WillowInteractiveObject` / `WillowPickup` versions of `IsComponentUsable`, `HasAnyMissionsForPlayer`, `GetAllDirectorData` and the interface slots; `Behavior_AddMissionDirectives` (native, 10 instances in the game).
- The vehicle branch of the per-frame evaluation; the exact trade-proxy range rule; the two pawn virtual predicates; the `MissionTracker` director table's slot contents; the focus camera
  (`AdjustViewPointForFocusCam`, `ResetFocusCam`).
- How the Use key reaches `PickupSomething` (the key bindings are built natively from the profile; `DefaultInput.ini` only removes the engine defaults).
- `WillowGFxUIManager.PlayMovie` (does not decode; G5), the AI component's `Used` / `Scripted` event handling in Marcus's state machine (G3 / G2).
- The remaining behavior events of Marcus's other providers (class provider `BehaviorProviderDefinition_5` only holds `Ep17_GiveItem`).

## Corrections to earlier notes

- **SANCTUARY_RPG_MISSION.md ("Talk reach of 250 cm", "the use key accepts or turns in directly"):** the stock rule is a camera-centred ray of 350 uu that must hit a usable component with nothing blocking in front; the
  accept / turn-in screen is the script path above, and the buttons call the C1 scripts with the director.
- **NATIVE_CONTROLLER_HELPERS.md (cost query "native-only virtual"):** confirmed; the interface order is icon, can-be-used, vehicle-part predicate, has-prompt, cost, usable-by-user (table above). `IsComponentUsable`
  is used by `UseObject` only to choose `ActualUsedComponent`; it does **not** gate use (the pawn-wide `bUsable` does).
- **NATIVE_MISSION_SCRIPT_BRIDGE.md ("IMissionDirector implementers"):** the `MissionStatusChanged` call goes to the registered directors the tracker keeps; the tracker also keeps a 14-slot table of directors that currently have missions
  for the compass (this note). No change to the status routine.
