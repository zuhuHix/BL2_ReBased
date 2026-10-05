# Native controller and engine helpers the Fire mission script calls most (2026-10-05)

AI-assisted (Claude), analyst lane G4. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script call sites were read in the local WillowGame listing
(`research/script_disasm.py`); script signatures come from the package declarations; field names were mapped with
`tools/ghidra/class_layout.py` (a computed layout, see "Layout caveats"). Lane C1 covered the tracker and reward natives
([NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md)); behavior-kernel internals belong to lane G2, loot to
G1, timers/spawn/iterators/states to G3. Ranking of these natives: [NATIVE_SLICE_CENSUS.md](NATIVE_SLICE_CENSUS.md).

## The picture in five sentences

1. Most `WillowPlayerController` mission helpers are thin: they read the controller's `MissionPlaythroughs` array for the
   **current playthrough**, and the current playthrough itself is one integer on the world's replication info
   (`WillowGameReplicationInfo.CurrentPlaythrough`).
2. Several natives are only thin front ends for C++ virtual methods (`GetCurrentPlaythrough`, `NativeGetMissionIndex`,
   `UpdateLcdMissionStatus`, `GetHUDMovie`); the behaviour is in the virtual method, described here.
3. `UpdateLcdMissionStatus`, `PopulateMissionDataFromStatus` and `PlayUIAkEvent` are presentation only (hardware LCD
   display text, UI sound); a headless VM may no-op them.
4. `GetWillowGlobals`, `GetGearboxGlobals` and `GetBehaviorKernel` all read one pointer held by the engine's world object;
   the same instance is behind all three.
5. `Localize` reads `.int` files (UTF-16 ini files in `<Game>\Localization\INT\<Package>.int`); a missing entry returns a
   visible `?INT?Package.Section.Key?` placeholder, not an empty string.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| WillowPlayerController.GetCurrentPlaythrough | `native function int GetCurrentPlaythrough()` | Fire: every mission helper (32 dyn. calls) | high | UNVERIFIED |
| WillowPlayerController.NativeGetMissionIndex | `native function int NativeGetMissionIndex(MissionDefinition InMission)` | Fire: accept, status update (17 dyn. calls) | high | UNVERIFIED |
| WillowPlayerController.UpdateLcdMissionStatus | `native function UpdateLcdMissionStatus()` | presentation only, no-op | high | UNVERIFIED |
| WillowPlayerController.GetHUDMovie | `native function WillowHUDGFxMovie GetHUDMovie()` | Fire: HUD fanfare guards | high | UNVERIFIED |
| WillowPlayerController.CanAffordToUseUsableObject | `native function bool CanAffordToUseUsableObject(IUsable UsedObject, IUsable.EUsabilityType UsabilityType)` | Fire: talking to Marcus / use (4 dyn.) | medium-high | UNVERIFIED |
| WillowPlayerController.PayForUsedObject | `native function PayForUsedObject(IUsable UsedObject, IUsable.EUsabilityType UsabilityType)` | Fire: same use path | medium-high | UNVERIFIED |
| WillowPlayerController.DoesObjectCostToUse | `native function bool DoesObjectCostToUse(IUsable UsableObject, EUsabilityType UsabilityType, out ECurrencyType CurrencyType, out int CostsAmount)` | the cost query behind the two above (extra) | medium-high | UNVERIFIED |
| WillowPlayerReplicationInfo.GetCurrencyOnHand / AddCurrencyOnHand | `native function int GetCurrencyOnHand(ECurrencyType)` / `native function AddCurrencyOnHand(ECurrencyType, int)` | Fire: pay path, rewards (extra) | high | UNVERIFIED |
| WillowPlayerController.GetPawnInventoryManager | `native function WillowInventoryManager GetPawnInventoryManager()` | Fire: weapon equip/reward (2 dyn., 32 sites) | high | UNVERIFIED |
| WillowPlayerController.PlayUIAkEvent | `native function PlayUIAkEvent(AkEvent Event)` | Fire: mission fanfare sound | high | UNVERIFIED |
| WillowPlayerController.PopulateMissionDataFromStatus | `native function string PopulateMissionDataFromStatus(MissionStatusPlayerData MStatus)` | none (no script caller in WillowGame) | medium | UNVERIFIED |
| WillowPlayerController.GetLevelForMission | `native function name GetLevelForMission(MissionDefinition InMission)` | none (no script caller in WillowGame) | low-medium | UNVERIFIED |
| WillowPlayerController.LocalMissionDependenciesMet | `native function bool LocalMissionDependenciesMet(MissionDefinition InMission, optional int ForPlaythrough)` | none in WillowGame script | medium-high | UNVERIFIED |
| WillowPlayerController.IsMissionObjectiveCompleteLocal / ActiveLocal | `native function bool ...(MissionObjectiveDefinition MissionObjective, optional int ForPlaythrough)` | none in WillowGame script | medium-high | UNVERIFIED |
| WillowPlayerController.GetActiveMissionNumber / GetLocalActiveMissionNumber | `native function int GetActiveMissionNumber()` / `(optional int ForPlaythrough)` | online status only | medium | UNVERIFIED |
| PlayerController.IsPrimaryPlayer | `native final function bool IsPrimaryPlayer()` | Fire: gates (8 dyn., 90 sites) | medium-high | UNVERIFIED |
| MissionTracker.IsDataValid | `native function bool IsDataValid()` | Fire: gates AddMission etc. (6 dyn.) | high (read), medium (who sets it) | UNVERIFIED |
| WorldInfo.IsMenuLevel | `native static function bool IsMenuLevel(optional string MapName)` | Fire: HUD fanfare gate (10 dyn.) | high (no-argument form) | UNVERIFIED |
| GearboxGlobals.GetBehaviorKernel | `native static function BehaviorKernel GetBehaviorKernel()` | Fire: behavior activation (6 dyn.) | high | UNVERIFIED |
| GearboxGlobals.GetGearboxGlobals / WillowGlobals.GetWillowGlobals | `native static function ... ()` | Fire: globals access (338 sites) | high | UNVERIFIED |
| WillowAIPawn.IsComponentUsable | `native function bool IsComponentUsable(ActorComponent UsableComponent, IUsable.EUsabilityType UsedType)` | Fire: NPC use (5 dyn.) | high | UNVERIFIED |
| Object.Localize | `native static function string Localize(string SectionName, string KeyName, string PackageName)` | all text (3 dyn., 71 sites) | high (lookup), medium (post-processing) | UNVERIFIED |
| Object.QueryInterface | `native function Interface QueryInterface(class<Interface> InterfaceClass)` | Fire: IMissionDirector, behaviors (254 sites) | medium-high | UNVERIFIED |

Enums used (confirmed from the package enums): `EMissionStatus` NotStarted 0, Active 1, RequiredObjectivesComplete 2,
ReadyToTurnIn 3, Complete 4, Failed 5. `EUsabilityType` UT_Primary 0, UT_Secondary 1. `ECurrencyType` (declared in
`Engine.WillowInventoryDefinition`) Credits 0, Eridium 1, SeraphCrystals 2, Reserved_A 3 .. Reserved_J 12, MAX 13; the
script treats type 3 as the golden-key "currency".

## Layout caveats

Offsets are given as field names. `class_layout.py` needs a stand-in size for `MapProperty` (any value from 20 up leaves
`WillowPlayerController.MissionPlaythroughs` at 0x1194, which agrees with the native reads; only fields declared after the
map move). The computed layout of `WillowAIPawn` drifts from the native one by a few bytes in the middle (native-only
members), so the AI pawn cost fields below are named, not numbered. Fields used in this note:

- `WillowPlayerController.MissionPlaythroughs` (array of `MissionPlaythroughData`, element 0x38 bytes: `MissionList` array of
  `MissionStatusPlayerData`, `UnloadableDlcMissionList`, `UnloadableDlcPendingMissionRewards`, `FilteredMissions`,
  `ActiveMission`, `PlayThroughNumber`).
- `IMission.MissionStatusPlayerData` (0x2c bytes): `MissionDef`, `Status` (byte), `ObjectivesProgress` (int array),
  `ActiveObjectiveSet`, `SubObjectiveSets`, `GameStage`, `bNeedsRewards`, `bHeardKickoff`.
- `WillowGameReplicationInfo`: `CurrentPlaythrough`, `PlaythroughOverride`, `MissionTracker`. `Engine.WorldInfo.GRI` is the
  replication info.
- `Engine.Controller.Pawn`, `.PlayerReplicationInfo`; `Engine.PlayerController.Player`, `.myHUD`, `.NetPlayerIndex`.
- `WillowPlayerReplicationInfo.Currency[13]` (struct `CurrencyState`, 0x14 bytes each: `FormOfCurrency`, `StatName`,
  `CurrentAmount`, `LastKnownAmount`).

## WillowPlayerController.GetCurrentPlaythrough

- **Signature:** no parameters, returns int.
- **Reads:** the engine's single global world object, its persistent level's `WorldInfo`, that object's `GRI`, and
  `CurrentPlaythrough` on it.
- **Does:** returns `CurrentPlaythrough` of the world's `WillowGameReplicationInfo`. It does **not** use the controller's own
  world, does **not** consult `PlaythroughOverride` and does not clamp. Returns **0** when there is no world, no world info,
  no replication info, or the replication info is not a `WillowGameReplicationInfo`.
- **Who sets the value:** script (`WillowGameReplicationInfo.SetCurrentPlaythrough`, called from the front end and from the
  save-game load path, which then calls `InitializeWorldMissionState` on the primary controller). A fresh game is
  playthrough 0 (script says Normal = index 0).
- **Calls into script / other natives:** none.
- **Edge cases:** callers index `MissionPlaythroughs[CurrentPlaythrough]` themselves; the native does not check the index.
- **Implementer checklist:** (1) return the replication info's `CurrentPlaythrough`, else 0; (2) no side effects.
- **Open:** whether a replicated client sees the replicated value only (it reads the local copy).

## WillowPlayerController.NativeGetMissionIndex

(Section added at the orchestrator's request: this native had been implemented provisionally from the script twin
`WillowPlayerController.GetMissionIndex`.)

- **Signature:** `(MissionDefinition InMission)`, returns int.
- **Reads:** `GetCurrentPlaythrough()` (above), `MissionPlaythroughs[playthrough].MissionList[i].MissionDef`.
- **Does:** if the current playthrough is a valid index of `MissionPlaythroughs` (0 <= p < Num), scans that playthrough's
  `MissionList` from index 0 and returns the index of the **first** entry whose `MissionDef` is the same object as
  `InMission` (pointer equality; a `None` argument matches the first entry whose `MissionDef` is `None`). Otherwise returns
  **-1**. No side effects.
- **Confirms the script twin:** the script `GetMissionIndex` (same listing) does the same search with the same result;
  the native is the fast path every other helper calls through a virtual slot (`LocalMissionDependenciesMet`,
  `GetLevelForMission`, objective checks below all use it, always with the **current** playthrough even when they were given
  another one, see their sections).
- **Implementer checklist:** (1) -1 when the playthrough index is out of range; (2) first match wins; (3) compare objects by
  identity; (4) no allocation, no events.
- **Open:** none for the slice.

## WillowPlayerController.UpdateLcdMissionStatus

- **Presentation only.** It is a thin front end for a virtual method that only does something when a hardware LCD display
  (the Logitech-style keyboard LCD service obtained from the globals) is present: it is skipped unless the controller is a
  local controller, the globals object exists, and the LCD service answers yes. When active it builds up to 10 strings for
  the active missions (see `PopulateMissionDataFromStatus`) and hands them to the LCD service.
- **Callers (script):** `UpdateMissionStatus`, `UpdateActiveMission`, `UpdateActiveObjectiveSet`, `UpdateMissionObjective`,
  `ClearMissionObjective`, `DecrementMissionObjective`, `SubObjectiveSetAdvanced`, `ApplyMissionSaveGameData`.
- **Implementer checklist:** no-op, no result, no events. Nothing else in script depends on it.

## WillowPlayerController.PopulateMissionDataFromStatus

- **Presentation only (LCD text); no script caller in WillowGame.** Takes a `MissionStatusPlayerData` by value and returns a
  string, via a virtual method taking the mission and its status.
- **Does:** returns an empty string unless the world's replication info has a mission tracker and the mission is not
  `None`. Otherwise the result starts with the mission's `MissionName`; then, if the status is ReadyToTurnIn (3), it appends
  a segment built from the localised text `Localize("HUDWidget_Missions", "MissionTurnInString", "WillowGame")` in the form
  `|<text>~0 / 1`; for any other status it appends, for each objective the tracker lists for the mission, `|<objective
  ProgressMessage>~<current> / <ObjectiveCount>`. (`|` separates segments, `~` separates label and counts.)
- **Open:** how the objective list is produced (tracker helper, not read) and the exact order of appended pieces.

## WillowPlayerController.GetHUDMovie

- **Signature:** no parameters, returns `WillowHUDGFxMovie`.
- **Reads:** `PlayerController.myHUD`; `WillowHUD.HUDMovie`.
- **Does:** if `myHUD` is a `WillowHUD`, returns that HUD's `HUDMovie`; otherwise `None`. No side effects. (The native
  `WillowHUD.GetHUDMovie` returns the same field.)
- **Slice relevance:** 53 script sites call one of the two. The script nearly always tests the result for `None` first
  (for example the fanfare, the mission widget refresh, contextual prompts). A host without the Scaleform HUD movie
  returns `None` and those branches are skipped. `RefreshHUDMissionWidget` and `ClientDoMissionStatusFanfare` then do
  nothing visual, but `ClientDoMissionStatusFanfare` still plays its sound through `PlayUIAkEvent` before the guard.
- **Implementer checklist:** (1) `None` when no HUD or the HUD is not a `WillowHUD`; (2) otherwise the stored movie
  object; (3) never create the movie here (creation is `WillowHUD.CreateHUDMovie`/`OpenHUDMovie`, script).

## WillowPlayerController.PlayUIAkEvent

- **Presentation only.** `None` event: nothing. Otherwise calls the script event `ClientPlayAkEvent(Event)` on the
  controller itself (a script client function inherited from `Engine.PlayerController`, which in turn calls
  `WwiseClientHearSound(Event, Self, Location, true, false)`). Also exists as `WillowPlayerPawn.PlayUIAkEvent` (forwards to
  a virtual on the pawn) and `WillowVehicle.PlayUIAkEvent`.
- **Callers (script):** `ClientDoMissionStatusFanfare` (7 sites; the sound comes from `GlobalsDefinition`
  `NewMissionAcceptedAkEvent`, `ActiveMissionChangedAkEvent`, `MissionObjectiveCompleteAkEvent`,
  `MissionObjectiveIncrementedAkEvent`, `MissionObjectiveClearedAkEvent`, `MissionReadyToTurnInAkEvent`, ...),
  `DoLevelUpNotifications`, injured-state setup/clear, skill respec confirmation.
- **Implementer checklist:** no-op or forward to the audio layer; the event may be `None`; no return value; do not fail if
  `ClientPlayAkEvent` is not implemented.

## WillowPlayerController.CanAffordToUseUsableObject / PayForUsedObject / DoesObjectCostToUse

- **Signatures:** `(IUsable UsedObject, EUsabilityType UsabilityType)`; `Can...` returns bool, `Pay...` returns nothing. An
  `IUsable` value is a pair (object, interface pointer); either half null counts as "no object".
- **Cost query (DoesObjectCostToUse and the first step of both):** asks the **object** through its `IUsable` interface for
  (does it cost, currency type, amount) for this usability type, passing the controller's `Pawn` as the user. The
  interface method is a native-only virtual (no script name):
  - `WillowInteractiveObject`: costs if `bCostsToUse[UsabilityType]` is nonzero, and then reports
    `CostsToUseType[UsabilityType]` and `CostsToUseAmount[UsabilityType]`.
  - `WillowAIPawn`: same three fields (names identical on the pawn).
  - `WillowPawn` base implementation: always "no cost".
  - Other implementers were not read.
  The script sets these fields through the behavior `Behavior_ChangeUsabilityCost`.
- **CanAfford:** true when the object is null, or it does not cost anything, or (type is 3, the golden-key type) the
  amount is <= the controller's golden-key count, or (any other type) the amount is <= the controller's
  `WillowPlayerReplicationInfo.GetCurrencyOnHand(type)`. If the controller's replication info is not a
  `WillowPlayerReplicationInfo` the answer is true.
  Golden-key count (a virtual on the controller): in a standalone local game it is the sum over the controller's key
  records of (a "granted" byte minus a "used" byte); otherwise a stored count field. Only needed for the golden-key chest.
- **Pay:** does nothing for a null object or a free object. For type 3 it fires the script event `SpendGoldenKey()` on the
  controller once per unit of cost. For any other type it asks the replication info for the current amount (result unused)
  and then calls `AddCurrencyOnHand(type, -amount)`. **It never checks affordability**; the script calls
  `CanAffordToUseUsableObject` first (`PerformedUseAction`, `PerformedSecondaryUseAction`, `ServerUseWithoutConfirmation`).
- **Calls other natives:** `GetCurrencyOnHand`, `AddCurrencyOnHand` (next section).
- **Implementer checklist:** (1) free or absent object: Can = true, Pay = no-op; (2) cost compared with `<=`; (3) Pay
  subtracts through `AddCurrencyOnHand`, never directly; (4) Pay does not refuse.
- **Open:** the IUsable method for implementers other than the two classes above; the golden-key record layout.

## WillowPlayerReplicationInfo.GetCurrencyOnHand / AddCurrencyOnHand

(Extra: the pay path and the reward path (C1 note) depend on them.)

- **GetCurrencyOnHand(type):** type above 12 returns 0. Type 3 asks the replication info's `Owner`, if it is a
  `WillowPlayerController`, for its golden-key count; otherwise (and for every other type) returns
  `Currency[type].CurrentAmount`.
- **AddCurrencyOnHand(type, delta):** does nothing at all when type is above 12 **or type is 3**. For `delta <= 0`:
  `CurrentAmount += delta`, clamped to a minimum of 0. For `delta > 0`: `CurrentAmount += delta`, clamped to the cap of that
  type. **Caps (table in the executable, int per type 0..12):** credits 99,999,999; Eridium 500; types 2..12 each 999.
  After either update it sets a "needs replication" bit in the replication info's flag word (0x80000 in the actor's
  net-dirty flag word) and fires the script event `NotifyCurrencyDelegates()` on the replication info (even when the amount
  did not change).
- **Implementer checklist:** keep amounts per type; clamp as above; fire `NotifyCurrencyDelegates` after every accepted
  call; ignore type 3 in `Add`.

## WillowPlayerController.GetPawnInventoryManager

- **Reads:** `Controller.Pawn`, its `InvManager`.
- **Does:** `None` when the controller has no pawn. Otherwise takes the controller's pawn, except that if the pawn answers
  a "vehicle/base" virtual with a non-null object, the manager is taken from that vehicle's `Driver` instead. The chosen
  pawn's `InvManager` is returned if it is a `WillowInventoryManager`, else `None`.
- **Edge:** the base `WillowPawn` implementation of that virtual is a shared do-nothing stub (returns nothing), so for
  an on-foot Vault Hunter this is simply `Pawn.InvManager`. Vehicle seats were not read (medium confidence).
- **Implementer checklist:** (1) `Pawn.InvManager` as `WillowInventoryManager`; (2) `None` for no pawn or a wrong class;
  (3) vehicle redirection can wait.

## WillowPlayerController.GetLevelForMission

- **No script caller found in WillowGame** (other packages not scanned). Low-medium confidence.
- **Does:** finds the mission's index (current playthrough) and takes its status and `ActiveObjectiveSet`; with no record it
  uses status 0 and no set. Then asks a mission helper for the **map name of the place to go**:
  `None` if the mission has no `TravelStation`; status 0: `TravelStation.StationLevelName`; status 3/4 (turn-in): the
  `TurnInStation`'s level name if set, otherwise as for 1/2; status 1/2: `TravelStation.StationLevelName`, overridden by
  the `StationOverride` of the objective sets walked from `InitialObjectiveSet` along `NextSet`, up to and including the
  active set (the last override seen wins); status 5: `TravelStation`'s level name.
- **Open:** whether the station for statuses 3/4 and 5 is read exactly as stated (the helper's receiver was inferred).

## WillowPlayerController.LocalMissionDependenciesMet

- **Signature:** `(InMission, optional int ForPlaythrough)`, default -1 = current playthrough.
- **Does:** `None` mission: false. For each mission in `InMission.Dependencies` (in order): its index (via
  `NativeGetMissionIndex`, **always the current playthrough's list for the index**) must exist, the entry's status in the
  list of `ForPlaythrough` must be Complete (4), and the dependency's own `LocalMissionDependenciesMet` must hold
  (recursive); any failure returns false. With all dependencies satisfied (or none), then the mission's
  `ObjectiveDependency` (`Objective`, `Status`): if no objective, result true; otherwise true when
  `IsMissionObjectiveCompleteLocal(objective)` holds, or when `Status` is 1 and `IsMissionObjectiveActiveLocal(objective)`
  holds; else false.
- **Quirk to keep:** with a non-current `ForPlaythrough` the index comes from the current playthrough but is applied to the
  other list (reading a possibly wrong or out-of-range entry). A clean implementation may use `ForPlaythrough` for both; it
  makes no difference when they are equal (all slice calls).
- **Client/local counterpart of** `MissionTracker.MissionDependenciesMet` (C1).

## WillowPlayerController.IsMissionObjectiveCompleteLocal / IsMissionObjectiveActiveLocal

- **Signature:** `(MissionObjectiveDefinition MissionObjective, optional int ForPlaythrough)`, default -1 = current.
- **Common steps:** `None` objective: false. The mission is the objective's `Outer`, cast to `MissionDefinition`. Index via
  `NativeGetMissionIndex` (current playthrough). If not found: false. The record is taken from the `ForPlaythrough` list.
  The objective's index inside the mission (`MissionDefinition.ObjectiveDefs`, first match, -1 if absent) selects an entry
  of the record's `ObjectivesProgress` (out of range: false). The **progress count** is that integer, except that when the
  objective has `bRememberItemsWithinObjective` the integer is a bit mask and the count is its number of set bits.
- **Complete:** status 0 or 5: false. Status 1..4: true when the progress count **equals** the objective's `ObjectiveCount`.
- **Active:** only status 1 or 2 (Active, RequiredObjectivesComplete), and the record's `ActiveObjectiveSet` must be set;
  true when that set's `ObjectiveDefinitions` contains the objective **and** the progress count is **less than**
  `ObjectiveCount`. Sub-objective sets are not consulted.
- **Implementer checklist:** as above; equality (not >=) for "complete".

## WillowPlayerController.GetActiveMissionNumber / GetLocalActiveMissionNumber

- **Online-status only** (`UpdateOnlineGameSettings`, matchmaking UI, `SavePlayerSaveGameData`); not needed for the slice.
- **GetLocalActiveMissionNumber(ForPlaythrough=-1):** playthrough index out of range: 0. If the playthrough's
  `ActiveMission` exists, has a record and is neither NotStarted nor Complete, and its local dependencies are met, returns
  that mission's `MissionNumber`. Otherwise scans the playthrough's list: among plot-critical missions without a DLC
  expansion and with status 1/2/3, the lowest `MissionNumber`; among non-plot missions with status 1/2/3/5 that are not
  filtered (asks the script event `IsMissionFiltered`), the lowest number; plot result wins, then side result; none found:
  the helper returning the highest `MissionNumber` of Complete plot missions, else 0.
- **GetActiveMissionNumber():** uses the world tracker when its data is valid: the tracker's `ActiveMission`'s
  `MissionNumber` if one is tracked; otherwise the first plot-critical, non-DLC mission with status 1/2/3 in tracker
  order (highest completed number if none); falls back to the local function above with the current playthrough.
- **Open:** exact tie-breaking; low priority.

## PlayerController.IsPrimaryPlayer

- **Signature:** no parameters, returns bool (declared on `Engine.PlayerController`; 90 script sites).
- **Reads:** `PlayerController.Player`, `NetPlayerIndex`, engine local-player count.
- **Does:** computes a "player index" starting at `NetPlayerIndex`, and a helper flag:
  - `Player` is `None`: flag false.
  - `Player` is a `LocalPlayer`: when the engine has more than one local player (split screen) the index becomes this
    player's position in the engine's local player list and the flag is true; with one local player the flag is false.
  - `Player` is a `NetConnection` that has child connections: flag true, index stays `NetPlayerIndex`.
  - `Player` is a `ChildConnection` with a parent: index = its position among the parent's children + 1, flag true.
  The result is **false only when the flag is true and the index is not 0**, otherwise true.
- **For the slice:** single local player, no split screen, standalone: **true**.
- **Implementer checklist:** return true for the sole local controller; false for a second split-screen or child-connection
  controller.
- **Open:** which engine array is the local player list (name only; one local player makes it irrelevant).

## MissionTracker.IsDataValid

(Section added at the orchestrator's request: previously implemented provisionally.)

- **Signature:** no parameters, returns bool.
- **Reads:** the tracker's `bDataValidated` bit and nothing else.
- **Does:** returns `bDataValidated`. No side effects, no role check.
- **Who sets it:** only the native `MissionTracker.ValidateData()` (a no-parameter native, sets the bit to true; found as
  the single writer of that bit in the executable). `ValidateData` is called only from the script function
  `WillowPlayerController.ClientValidateMissionData`, which (in the same function) then calls `GrantDefaultWeaponIfEligible`
  on every local controller, `NotifyUIRefresh` and `ClientSetOnlineStatusAllPlayers`. No script caller of
  `ClientValidateMissionData` exists in WillowGame; the tracker native `SendMissionData` (called by the script
  `ServerRequestMissionData`) is the one that references the name of that client function, so the validation arrives as the
  reply to a mission data request. How a standalone local run triggers that request was not read.
  `InitializeWorldMissionState` (C1 area) does **not** set the bit.
- **Script users:** `AddMission` (read in full) and about a dozen other WillowGame functions (14 call sites) test
  `Tracker != None && Tracker.IsDataValid()` before touching mission state; with the bit false they silently skip the work.
- **Implementer checklist:** (1) report the flag; (2) the flag must be true for the Fire flow to run: either implement
  `ValidateData` and the request path, or set it when the world mission state has been initialised (a shortcut, mark it
  as such); (3) not replicated by this native.
- **Open:** the standalone trigger (above); whether the flag is reset on level change (not seen).

## WorldInfo.IsMenuLevel

- **Signature:** static; `optional string MapName`. **All 43 script call sites pass nothing.**
- **Does (no argument):** uses the engine's global world, takes its persistent `WorldInfo` and returns that object's
  `bIsMenuLevel` bit, unless a global engine flag (probably an editor-mode switch, unverified) is set, in which case it
  returns false. With a non-empty `MapName` it compares the name with a global menu-map name string instead (not read).
  Returns false when no world exists.
- **For the slice:** Sanctuary's `WorldInfo` is a gameplay level; expected false. The bit is data of the map's WorldInfo
  (front-end and loader maps set it).
- **Implementer checklist:** read `bIsMenuLevel` of the persistent level's WorldInfo; false if absent.
- **Open:** the named-map branch.

## GearboxGlobals.GetGearboxGlobals / WillowGlobals.GetWillowGlobals / GearboxGlobals.GetBehaviorKernel

- **How the globals object is found:** the engine's global world object holds one pointer to the **globals instance**.
  `GetGearboxGlobals` and `GetWillowGlobals` return exactly that pointer (or `None` when there is no world); calling either
  on any class gives the same object. Which class the instance has is configuration: `[GearboxFramework.GearboxGlobals]
  GlobalInstanceClassName=WillowGame.WillowGlobals` in `WillowGame/Config/DefaultEngine.ini`, and its data object is
  `[WillowGame.WillowGlobals] DefaultGlobalsDefinitionName=GD_Globals.General.Globals`. Script reads game rules from
  `GetWillowGlobals().GetGlobalsDefinition()` (the `GD_Globals.General.Globals` object, for example the
  `*AkEvent` fields used by the mission fanfare). The native that creates the instance and stores the pointer was not
  found (it runs at startup); the implementer must create one instance of `WillowGame.WillowGlobals` at world start and
  keep it for the session.
- **GetBehaviorKernel:** returns the globals instance's `TheBehaviorKernel` field, or `None` if there is no globals instance.
  (Also makes sure the class default object exists first; no visible effect.) The kernel's internals are lane G2's.
- **Implementer checklist:** one singleton globals object, all three natives read it; `TheBehaviorKernel` is created
  with it (G2 note).
- **Open:** creation site and the exact moment (before the first script runs).

## WillowAIPawn.IsComponentUsable

- **Signature:** `(ActorComponent UsableComponent, EUsabilityType UsedType)`, returns bool.
- **Reads:** the pawn's `InstanceDataState.Data` array (instance data entries, element `InstanceDataUnion`, 0x58 bytes),
  entry field `ComponentData` (`Component`, `bIsUsable`, `bIsSecondaryUsable`).
- **Does:** false for a `None` component. Otherwise finds the **first** entry whose `ComponentData.Component` is that
  component (identity). Not found: false. `UsedType` 0 (primary): that entry's `bIsUsable`; 1 (secondary): its
  `bIsSecondaryUsable`; any other value: false.
- **Callers:** `WillowAIPawn.UseObject`, `NotifyUserCouldNotAffordAttemptedUse`. `WillowInteractiveObject.IsComponentUsable`
  is a separate native that calls a virtual on the object (not read).
- **Implementer checklist:** as above; these flags are set by `SetUsabilityForComponent` / the usability behaviors.

## Object.Localize

- **Signature:** `(SectionName, KeyName, PackageName)`, all strings, returns string.
- **Lookup (language default `INT`, a global language string):** the engine has an ordered list of localisation root
  folders from config: `Engine\Localization`, `Gearbox\Localization`, `WillowGame\Localization` (DLC folders are added by
  the game; `LocalizationPaths` in `BaseEngine.ini`, `GearboxEngine.ini`, `DefaultEngine.ini`). For each root, **from the
  last to the first**, it opens `<root>\<Language>\<PackageName>.<Language>` (for example
  `WillowGame\Localization\INT\WillowGame.int`; file names are matched case-insensitively), finds the section named
  `SectionName` (case-insensitive) and the key `KeyName` (case-insensitive) inside it, and returns the first hit. The files
  are UTF-16 ini files: `[Section]` lines and `Key=Value` lines; values may be wrapped in double quotes, which are removed
  (the entry `ColonAndSpace=": "` yields `: `). Examples read by the Fire script: `("Missions","ColonAndSpace","WillowGame")`,
  `("Missions","SingleCountObjectiveComplete","WillowGame")`, `("HUDWidget_Missions","MissionTurnInString","WillowGame")`
  (these sections exist in the INT `WillowGame.int`).
- **Fallback chain:** if the current language is not `INT` and nothing was found, the same search is repeated for `INT`.
  Command-line switch `SHOWMISSINGLOC`: when set, a hit found only through this INT fallback is replaced by the
  placeholder below, so untranslated strings stand out.
- **Missing entry:** returns the literal text `?<Language>?<PackageName>.<SectionName>.<KeyName>?`, for example
  `?INT?WillowGame.Missions.NoSuchKey?`. It is **not** empty. (The optional "return empty" mode is not reachable from
  script: the native always asks for the placeholder.)
- **Before the localisation system is ready** (engine globals not yet initialised): returns `KeyName` unchanged.
- **Post-processing (medium confidence):** the final string is cut at the first occurrence of `###` (case-sensitive); the
  remainder is discarded. No `.int` file in the shipped INT folder contains `###`, so this is a no-op for real data.
- **Calls other natives:** none visible to script.
- **Implementer checklist:** (1) parse the three `.int` roots lazily and cache; (2) order Engine, Gearbox, WillowGame, with
  later roots winning; (3) quotes stripped; (4) placeholder format exact; (5) language INT only for the slice.
- **Open:** escape handling inside values (backslash sequences) was not read; whether `Key[0]=` array syntax matters (not
  used by the Fire script).

## Object.QueryInterface

- **Signature:** `(class<Interface> InterfaceClass)`, returns an `Interface` value (object + interface pointer pair).
- **Does:** the engine's `GetInterfaceAddress` logic: `None` interface class, a class that is not flagged as an interface,
  or the root `Interface` class gives the null interface (both halves zero). Otherwise it walks the object's class and its
  superclasses; each class has a list of implemented interfaces; if one of them **is or derives from** the requested
  interface class, the result is (this object, interface pointer). For interfaces implemented natively the pointer is the
  address of the object's `VfTable_I<Name>` slot; for script-only interfaces the pointer is the object itself. Not
  implemented: (null, null).
  A static re-entrancy guard returns the null interface when the same object is already being queried (nested call).
- **For the VM:** an interface value is "valid" iff its object half is non-null. Script compares with
  `NotEqual_InterfaceInterface` against `InterfaceCast(<Interface>, None)`. Slice uses (script): `IBodyPawn`,
  `IWorldBody`, `ITargetable`, `IAIInterface`, `IPlayerBehavior`, `IControllerLocator`, `IInstanceData`,
  `IProjectileBehavior`, `IBehaviorConsumer`, `ISpawnActor`, `IBalancedActor`, `ISkillBehavior`, `IAnimBehavior`,
  `IMissionDirector` (5 sites), `IStatusEffectTarget`, `IDesignerAttributeProvider`, `IDestroyBehavior`,
  `IBasicBehavior`, `IWeatherBehavior`.
- **Implementer checklist:** the class's `Interfaces` table (from the package: each `Class` property lists implemented
  interfaces; the computed `VfTable_I*` pointer properties are the implementation markers) decides; subclass-of-interface
  matches count.
- **Open:** whether classes with `Implements` lists inherited through script-only parents behave as stated (the logic
  walks every superclass, so yes by reading).

## Not read yet

- `WillowInteractiveObject.IsComponentUsable` (virtual), `WillowHUD` HUD movie creation, `WillowPlayerController`
  golden-key records, vehicle branch of `GetPawnInventoryManager`.
- The native that creates and stores the globals instance; how `SendMissionData`/`ClientValidateMissionData` is triggered
  in a standalone game.
- The named-map branch of `IsMenuLevel`.
- `GetLevelForMission` receiver details; no known caller.

## Corrections to earlier notes

- NATIVE_SLICE_CENSUS.md lists `GetWillowGlobals` as "a static call through a class constant": confirmed, and both globals
  getters read the same pointer (no per-class instance).
- The census stub for `Object.Localize` (empty or key text) differs from the engine: the missing-entry result is the
  `?INT?Package.Section.Key?` placeholder. The provisional `IsDataValid` must be a real flag (`bDataValidated`) set by
  `ValidateData`, not a constant.
