# Native respawn and station natives: what the respawn's station path reaches (draft, 2026-11-19)

AI-assisted (Claude Code), analyst lane FIX-STATION-NATIVES. The natives were read from the game executable with
Ghidra 12.1.4 (`tools/ghidra/run.ps1`, two read-only queries with one level of callees, against the project made by
`tools/ghidra_import.ps1`) and are written in our own words. No decompiler output, pseudo-code or listing structure is
reproduced here, and the raw output stays outside the repository. Field names come from the cooked packages
(`tools/ghidra/class_layout.py`). The script steps are the installed script, read with `research/script_disasm.py`.
**Every rule is UNVERIFIED in the running game**; nothing here was observed live.

This note hands the unread natives of the respawn path to an implementer. The analyst lane left each of them unbound
in `ParityMode::Off` (decision 2026-11-19-30), so a fresh implementer builds the Off rules from this note alone
(docs/LANE_BRIEF.md, "Clean room").

## 1. The path, in the script

What the installed script does on a respawn, in our own words. The selection itself is script (architecture rule
R2), and so are the registration steps. Decision 2026-11-16-60 and `crates/ow-mission/src/stations.rs` hold the
selection order.

1. `WillowPlayerPawn.ResurrectPlayer` clears the player's injured screen effect, resets a counter on every player
   controller, and returns early while the controller uses a vehicle (`Controller.IsUsingVehicleEx`) or the pawn
   drives one.
2. The respawn point: the destination station the caller gave, else `GetBestPlayerPlacementPoint`. That walks the
   level's travel stations (`AllActors`) and asks each `CanResurrectHere`. Both are script, and they need no native
   beyond the stock operators, which the VM binds, and the `AllActors` iterator, which the interpreter runs.
3. The holding cell, unless skipped: the first `HoldingAreaDestination` (`AllActors`) and its next exit point
   (`TeleporterDestination.GetNextExitPoint`). The exit point uses `Actor.CollidingActors` to skip an occupied
   point. Then the station's own next exit point, or `GetBestPlayerStartPoint` without a station.
4. The camera, the gear and two coordinated effects: `WillowGlobals.GetEffectCoordinator`, then `HasEffect` and
   `PopEffect`, each twice.
5. `ResurrectAtLocation`: the health and pools back, the move (`NoFailSetPawnLocation`, `SetLocation`,
   `SetRotation`), the dead and injured checks (`Pawn.IsDead`, `Pawn.IsInjured`, `Controller.IsDead`), the revive
   telemetry (`WillowLeviathanService.RecordCharacterRevivedEventForPlayer`) and the teleport effects.
6. At the arrival, the station registers again (`Behavior_RegisterStation`, observed live in COMBAT_DOWN_RESPAWN.md).
   That reaches `GetTravelStationDefinition`, `MissionTracker.FastTravelStationRegistered`,
   `FastTravelStationsLookup.AddStationToNameListInOrder`, `WillowGameReplicationInfo.AddFastTravelLocation` and
   `IsStationInList`.

## 2. The natives

| Native | Record | State in Off after this lane |
|---|---|---|
| `Pawn.IsDead` | NATIVE_DOWN_TIMERS.md | implemented from that record (decision 2026-11-19-30) |
| `Actor.CollidingActors` | NATIVE_ENGINE_CORE.md | unbound: needs the world's collision query, which the VM world does not hold, and an engine iterator the interpreter does not run |
| `Pawn.SetAnchor`, `WillowGameInfo.ResetAI` | NATIVE_DOWN_TIMERS.md (no rule, a stub judgement) | unbound |
| `Actor.SetLocation`, `Actor.SetRotation` | none | unbound, not read here (stock engine moves with encroachment and touch side effects; a separate read) |
| `Controller.IsUsingVehicleEx` | this note, section 3.1 | unbound |
| `WillowGlobals.GetEffectCoordinator` | this note, section 3.2 | unbound |
| `WillowEffectCoordinator.HasEffect` | this note, section 3.3 | unbound |
| `WillowEffectCoordinator.PopEffect` | this note, section 3.4 (partial) | unbound |
| `WillowLeviathanService.RecordCharacterRevivedEventForPlayer` | this note, section 3.5 | unbound |
| `FastTravelStationsLookup.AddStationToNameListInOrder` | this note, section 3.6 (partial) | unbound |
| `TravelStation.GetTravelStationDefinition` and its fast-travel and level-travel declarations, `Controller.IsDead`, `WillowGameReplicationInfo.AddFastTravelLocation`, `IsStationInList` | this note, section 4: not readable here | unbound |
| `MissionTracker.FastTravelStationRegistered` | this note, section 4: not read | unbound |

## 3. What was read

### 3.1 `Controller.IsUsingVehicleEx(bCheckDrivenVehicle, out Vehicle)`

- With no `Pawn`, the answer is false and the out vehicle is None.
- Otherwise it asks the pawn, through one of the pawn's virtual functions, for a vehicle. That slot was not resolved
  (section 4); its name and the base pawn's answer are not known. If the answer is a vehicle, that vehicle is the out
  argument and the result is true.
- If not, and the flag is set and the pawn's `DrivenVehicle` is set, the same virtual is asked of the driven vehicle.
  Its answer is the out argument, and the result is whether it is a vehicle.
- Otherwise the out argument is None and the result false.
- The flag counts as true for any nonzero value.

### 3.2 `WillowGlobals.GetEffectCoordinator()`

Returns the globals object's `TheEffectsCoordinator` field, as stored. Nothing else.

### 3.3 `WillowEffectCoordinator.HasEffect(Target, EffectDefinition, EffectOwner)`

- The coordinator's `Processes` array is searched in order for the first process whose `Target` is the given target.
  With none, the answer is false.
- In that process only (a second process with the same target is never looked at), its `Threads` are searched for a
  thread whose `EffectDefinition` is the given definition, whose `EffectOwner` is the given owner, and which is not
  `bPendingDelete`. The answer is whether one exists.
- The comparisons are object identity, None included: a None owner matches a thread whose owner is None.

### 3.4 `WillowEffectCoordinator.PopEffect(Target, EffectDefinition, EffectOwner, optional bool = true)` (partial)

- The target is first redirected through a helper chain that was not read. It asks the target for a replacement
  object and uses that object when there is one; the condition and the replacement were not read.
- The process for the target is then found (the helper was not read; presumably as in `HasEffect`). With a
  definition given, the thread for the definition and the owner is found by another helper that was not read.
- When a thread is found, `bPendingDelete` is set on it and the answer is true. Otherwise the answer is false.
  Nothing else was seen to change; the thread stays in the array marked for deletion.
- What the optional bool does was not seen in the parts read.

### 3.5 `WillowLeviathanService.RecordCharacterRevivedEventForPlayer(Player, Byte)`

It builds a local event record with event code 31 for the player and the byte, hands it to a telemetry sender, which
was not read, and frees the record. In what was read it changes no game object. For a slice run, a no-op that returns
normally matches what was read, provided the sender has no side effect; that was not read.

### 3.6 `FastTravelStationsLookup.AddStationToNameListInOrder(Station, out NameList)` (partial)

- The list holds names. If the station object's own `Name` (with its number) is already in the list, the answer is
  false and the list is unchanged.
- Otherwise, when a global lookup object exists and two further conditions hold (both helpers unread), the station is
  resolved to a second object, also through an unread helper. Its name is then inserted into the list:
  - appended, when the list is empty or the lookup's own ordered table does not hold it;
  - else placed after the last list entry whose name comes before it in that table.
- The answer is then true, and it is also true when the conditions fail and nothing is inserted.

## 4. Not readable here

The natives below forward to a native virtual function of their class. Their bodies are those virtuals, and the
analysis could not find the vtables: these classes have no RTTI names in the executable, and the query script found
no registration or constructor pattern for them. Settling them needs another way into the vtables, or a live read
with bl-sdk.

- `TravelStation.GetTravelStationDefinition`, and the fast-travel and level-travel declarations: one shared
  forwarding stub. The live read D60 names is the way to settle them: `GetTravelStationDefinition` on each Sanctuary
  station.
- `Controller.IsDead`.
- `WillowGameReplicationInfo.AddFastTravelLocation(Station)` and `IsStationInList(Station)`.
- The pawn virtual that `IsUsingVehicleEx` asks (section 3.1).
- `MissionTracker.FastTravelStationRegistered`: not a forwarding stub, but a routine over the tracker's missions and
  objectives (it calls the objective-complete check and the mission-data lookup). It was too deep to read reliably in
  this lane. It returns at once when the tracker's `ActiveMission` is None; what it does with an active mission
  (the Fire mission, in the slice) was not read.

## 5. For the slice

On a respawn in a map world with Sanctuary's stations, Off reaches the natives above in roughly the order of section
1. Every one is unbound, so the first reached stops the run. Today no Off run gets that far: `--combat-run`'s S-f and
the `--slice-gate`'s link 14 both stop at the player's setup (`maya:8`, `WillowGlobals.GetPlayerPawnDataManager`) before
any down step runs (decision 2026-11-19-30). In Reference the run has no stations and falls back to the controller's
`StartSpot` (COMBAT_DOWN_RESPAWN.md, item 4).
