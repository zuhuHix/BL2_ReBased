# Native save and load: player and mission state, load order, save triggers (2026-10-06)

AI-assisted (Claude), analyst lane G15. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. No real save file was opened and the game was not run.

Scope: what the Fire-mission slice needs from save and load (accept, progress, turn in, quit, reload, resume): which
mission, experience, skill, inventory and place fields a save holds and how they relate, the order in which a loaded save
is applied (and what the mission tracker does when it is restored), and when the game saves. The on-disk container is
described only as fields and relations; our own save tooling (tools/real_game/, docs/verification/REALGAME_GROUND_TRUTH.md)
already reads the real files. A large part of save and load is **UnrealScript, not native**: the sections "Save trigger
points", "What a save holds" and "Load order" are read from the script listing (research/script_disasm.py output) and are
the part a script swap runs by itself. The natives listed below are what that script calls into.

Cross references (not repeated): skill-tree save and apply ([NATIVE_SKILLS.md](NATIVE_SKILLS.md), section 3.2), inventory
slot sizes and mission weapons ([NATIVE_INVENTORY_EQUIP.md](NATIVE_INVENTORY_EQUIP.md)), controller mission helpers
([NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md)), tracker status routine and rewards
([NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md)), observer kinds and the behavior side of a mission
([NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md)), experience ([NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md)).

## Summary

| Native | Script signature (read from call sites) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| MissionTracker.InitializeWorldMissionState | `native function bool InitializeWorldMissionState(array<MissionStatusPlayerData> MissionList, array<MissionDefinition> FilteredMissions, WillowPlayerController WPC)` | Fire: resume (restores every mission record, re-arms objectives, weapons and behaviors) | medium (flow), low (a few helpers) | UNVERIFIED |
| MissionTracker.GrantMissionWeaponsToClientPlayer | `native function GrantMissionWeaponsToClientPlayer(WillowPlayerController WillowPC)` | Fire: lent pistol after load (non-primary controllers only) | medium | UNVERIFIED |
| WillowPlayerController.AttemptPreSaveGameLoadFixup | `native function AttemptPreSaveGameLoadFixup(PlayerSaveGame SaveGame)` | Fire: runs first on load; clamps level, points, currency, slots | medium-high | UNVERIFIED |
| WillowPlayerController.AttemptPostSaveGameCreateFixup | `native function AttemptPostSaveGameCreateFixup(PlayerSaveGame SaveGame)` | Fire: runs last when a save is generated | medium-high | UNVERIFIED |
| WillowPlayerController.ConditionalFixWeaponReadyMax | `native function ConditionalFixWeaponReadyMax(PlayerSaveGame SaveGame)` | load: weapon slot floor (detail added to the equip note) | high | UNVERIFIED |
| WillowPlayerController.GetExpLevelLoadedFromSavedGame / GetExpPointsLoadedFromSavedGame | `native function int ...()` | resume: floor for the experience read | high | UNVERIFIED |
| WillowPlayerController.GetExpPoints | `native function int GetExpPoints()` | save: the experience written | medium-high | UNVERIFIED |
| WillowPlayerController.GetActivePlotCriticalMissionNumber | `native function int GetActivePlotCriticalMissionNumber(optional int PlaythroughIndex)` | save: PlotMissionNumber field | high | UNVERIFIED |
| WillowPlayerController.GetLocalActiveMissionNumber | `native function int GetLocalActiveMissionNumber(optional int PlaythroughIndex)` | save: ActiveMissionNumber field | medium | UNVERIFIED |
| WillowPlayerController.SaveStatsSaveGameData / ApplyStatsSaveGameData | `native function ...(PlayerSaveGame SaveGame)` | save and load of the player stats blob | medium | UNVERIFIED |
| WillowPlayerController.GenerateSaveGameGuid / AreSaveGuidsEqual | `native function bool GenerateSaveGameGuid(PlayerSaveGame SaveGame)`, `bool AreSaveGuidsEqual(PlayerSaveGame A, PlayerSaveGame B)` | save identity, reload check | high | UNVERIFIED |
| WillowPlayerController.AddExpansionSavedataToUnloadableItemData / ExtractExpansionSavedataFromUnloadableItemData | `native function ...(PlayerSaveGame SaveGame, optional bool bX)` | save and load of base-game numbers that outgrew the format, packed as marker entries | medium | UNVERIFIED |
| WillowPlayerController.SaveDLCExpansionData | `native function SaveDLCExpansionData(PlayerSaveGame SaveGame)` | none for the slice | medium | UNVERIFIED |
| WillowPlayerController.FixupSavedWeapons | `native function FixupSavedWeapons(out array<WeaponSaveGameData> Weapons)` | load: patch-compat for a few known weapons, none for the slice | medium | UNVERIFIED |
| WillowPlayerController.ReloadDefaultSaveGame | `native function ReloadDefaultSaveGame()` | new character without a save | low | UNVERIFIED |
| WillowPlayerController.NotifyReadyToLoadPendingSavegame | `native function NotifyReadyToLoadPendingSavegame()` | co-op only; no-op standalone | medium | UNVERIFIED |
| WillowSaveGameManager.Save / SaveGame / BeginLoadGame / EndLoadGame / Get-SetCachedPlayerSaveGame / LoadRawData / GetLastSaveGame / ValidateSaveData (and siblings) | see section | the storage boundary of save and load | high (that they are front ends) | UNVERIFIED |
| PlayerSkillTree.SaveSkillSaveGameData / ApplySkillSaveGameData | pointer | see NATIVE_SKILLS.md | n/a | UNVERIFIED |

## Save trigger points (script, read from the listing)

The game never saves from the mission tracker. A mission accept, objective update or turn-in does not write a save by
itself. The controller's `SaveGame` runs only from these places:

- **Fast-travel station use** (`RegisterStationForPlayer` -> `SaveAtStationIfNecessary`): when the station is newly
  discovered, or when at least 60 seconds of real time passed since the last station save. On the PC path it calls
  `SaveGame` when `CanSaveGame` allows it; the console path goes through `SaveAllPlayers` without a dialog. The
  station-registration call is made both on use and on load (with a flag for a load that suppresses the discovery side).
- **Closing the status menu** (inventory, skills, missions: `StatusMenuExGFxMovie.Hide`) when the menu's own
  `bShouldSaveGame` flag is set.
- **Quit to menu / return to title** (`PauseGFxMovie.QuitToMenu` through its `SaveNextPlayer`, and the controller's
  `ReturnToTitleScreen` through `SaveAllPlayers`): every local player is saved, one after the other, with a saving dialog
  and a minimum display time (3 seconds on console, 0 on PC).
- **Preference change** (`SetPlayerUIPreferences`: character name or colours changed, and the name is valid),
  **choosing a playthrough** in the front end (`FrontendGFxMovie.OnChoosePlaythrough_Click`), **a coliseum award
  certificate**, and the invite/patch paths (`OnDownloadPatcherFilesForInvite`).
- `QuickSave` and `QuickLoad` are empty.

**Gates inside `SaveGame`** (all must hold, otherwise nothing is written): no loading movie and (on console) no external UI
or Bink movie is playing (otherwise it retries after 0.25 s); the save-game manager exists; the pawn that owns the inventory
is a player pawn; a filename is known (`SaveGameName`); `bSaveGameLoaded` is true (a save never overwrites a slot before the
load of that slot finished); and the player skill tree object exists. `CanSaveGame` (used by the station path) additionally
needs a live pawn and `bSaveGameLoaded`. On success `SaveGame` echoes the target, marks "save games are available", calls
the manager's `SaveGame(controllerId, saveObject, filename, splitScreenUser)`, then runs `WillowGameEngine.CheckCIV`.

## What a save holds (script: GeneratePlayerSaveGame and its Save* helpers)

`GeneratePlayerSaveGame` creates a fresh `PlayerSaveGame` object and fills it in this order: player basics
(`SavePlayerSaveGameData`), skills (`SaveSkillSaveGameData`), resource pools (`SaveResourceSaveGameData`), backpack items
(`SaveItemSaveGameData`), inventory slot sizes (`SaveInventorySlotSaveGameData`), weapons (`SaveWeaponSaveGameData`), stats
(native `SaveStatsSaveGameData`), missions (`SaveMissionSaveGameData`), visited stations, UI preferences, DLC expansion
data (native), marketing codes, challenges, game stages, discovered areas, the bank, black-market upgrades, applied
customizations, unloadable DLC content, lockouts, queued training messages, the awesome-skill flag, vehicle customization
and steering mode; then a GUID (native `GenerateSaveGameGuid`), the expansion marker entries (native
`AddExpansionSavedataToUnloadableItemData`) and last the native post-create fixup. Field names below are those of
`PlayerSaveGame` (tools/ghidra/class_layout.py reads the layout from the packages).

**Player basics** (`SavePlayerSaveGameData`): `PlayerClassDefinition` (the controller's `PlayerClass`), `ExpLevel` (the
replication info's level), `ExpPoints` (native `GetExpPoints`, below), `GeneralSkillPoints` and `SpecialistSkillPoints`
(the replication info's unspent counters), `CurrencyOnHand[13]` (one entry per currency type), `PlaythroughsCompleted` (the
replication info's highest completed playthrough), `SaveGameId` (the controller's `SaveGameFileId`), `PlotMissionNumber`
(native `GetActivePlotCriticalMissionNumber`), `ActiveMissionNumber` (native `GetLocalActiveMissionNumber`),
`bReceivedDefaultWeapon`, `TotalPlayTime` (play time loaded with the character plus the game replication info's elapsed
time), `LastSavedDate` (the manager's date string), `SaveGuid` (copied from the cached save, so an existing save keeps its
GUID), the level-challenge unlock and one-off completion lists, `bIsBadassModeSaveGame` (cleared when a game info exists),
`LastPlaythroughNumber` (the world's current playthrough; outside a network client; otherwise copied from the cached save),
`bShowNewPlaythroughNotification`, `NumGoldenKeysNotified`.

**Skills:** one (skill, grade) entry per skill of the tree, grade 0 included (see NATIVE_SKILLS.md).

**Resource pools:** one entry for every pool whose definition has `bSerializeInSaveGame` and which was modified since
creation: resource, pool definition, amount, upgrade level. Whether health and shield pools are included depends on the same flag (not checked); the load path sets health and shield to their maxima anyway (`ServerItemSaveGameDataCompleted`).

**Backpack items** (`ItemData`): every item the inventory manager lists with the default filter: item definition data,
quantity, `bEquipped` (the item's `bReadied`), `Mark` (favorite/trash mark). Unloadable DLC items are kept as stored.

**Weapons** (`WeaponData`): every weapon the manager lists (readied and backpack) for which `CanBeSaved` is true: the full
weapon definition data, `QuickSlot` (the weapon's `QuickSelectSlot`, i.e. the quick-slot assignment) and `Mark`. **Mission
weapons are excluded** because `CanBeSaved` is false for them (NATIVE_INVENTORY_EQUIP.md). Current ammunition is **not** a field of
the weapon record: the load path creates each weapon with stored ammo zero. The ammunition numbers presumably travel in the
resource-pool data above (UNVERIFIED reading; which pools carry `bSerializeInSaveGame` was not checked).

**Inventory slot data:** backpack size (`InventorySlotMax_Misc`), weapon-ready count (`WeaponReadyMax`),
`NumQuickSlotsFlourished`.

**Places:** `VisitedTeleporters` (the names of activated fast-travel stations), `LastVisitedTeleporter` (a name; the last
station set as "last visited"), `RegionGameStages` (per region and playthrough: the game stage; the controller's list is
saved as is).

**Missions** (`SaveMissionSaveGameData`): for **every** playthrough the controller holds (not only the current one) one
`MissionPlaythroughs` entry with: `PlayThroughNumber`; `MissionData`, a copy of every record of that playthrough's mission
list (`MissionDef`, `Status`, `ObjectivesProgress` (one integer per objective), `ActiveObjectiveSet`, `SubObjectiveSets`,
`GameStage`, `bNeedsRewards`, `bHeardKickoff`); `ActiveMission`, the **tracked mission stored as the full definition name
string** (empty when none); `PendingMissionRewards` (for the current playthrough the controller's `UnclaimedRewards`, for
the others the controller's list kept for other playthroughs); the unloadable-DLC mission and reward lists (missions of DLC
that is not installed, kept by name); and `FilteredMissions` (missions the player has filtered out of the HUD list).
Nothing else about a mission is stored: kill counts, timers, dialogs heard and Kismet state are not part of a save.

**Not saved:** mission weapons, the mission tracker's own data (it is rebuilt from the controller's list, below),
Kismet/behavior state, the world, enemies and loot on the ground, health and shield values, the pawn's position (the
player restarts at `LastVisitedTeleporter`; see `SetInitialTeleportDestination` below).

## Load order (script, with the natives marked)

1. **Front end / character select** (`OnLoadSaveGame` -> `FinishSaveGameLoad`): the loaded save is cached in the manager per
   controller id (`SetCachedPlayerSaveGame`), `SaveGameFileId` and `LastLoadedSaveGame` are taken from it, the native
   `ExtractExpansionSavedataFromUnloadableItemData` runs on it, and `ClientPublishCachedSaveGameToPRI` does the **first
   mission apply**: `ApplyMissionSaveGameData(save, bManageRewards = false)` (fills the controller's playthrough and mission
   lists; no reward handling), `ApplyVisitedTeleporterData`, `UpdateSavegameForPlaythroughCompletion`, and puts level and
   class-mod name on the replication info. In a menu level the game info's current playthrough is then set from
   `LastPlaythroughNumber` (`SetCurrentPlaythrough`, which itself calls `InitializeWorldMissionState` on the primary
   controller).
2. **Game start:** `ClientApplySaveGame` (from `BeginStartGame`, `ClientSetHUD`, `ApplySaveGamesBeforeLevelTransition`)
   asks the pawn data manager to load the saved class asynchronously; when it completes (streaming event
   `ClientApplySaveGame`) the class switch is set pending and the busy dialog hides. With no usable save it selects an
   empty class instead.
3. **First pawn spawn** (`SpawningProcessComplete` on an initial spawn or class change; `ShouldLoadSaveGameOnSpawn` is true
   exactly then): `LoadCachedSaveGame` runs. It retries every 0.1 s until the controller has a pawn **and** that pawn has an
   inventory manager, then takes the cached save and calls `LoadPlayerSaveGame`. With no cached save it calls the native
   `ReloadDefaultSaveGame`, `WriteLastSavedId` and `StartNewPlaySession` instead (a new character). On a spawn that does
   not load, `ServerSetSaveGameData` is called with zeros so the replication info is initialised.
4. **`LoadPlayerSaveGame(save)`** (only when the save's class equals the controller's `PlayerClass`; a mismatch only
   echoes an error). In order: native `AttemptPreSaveGameLoadFixup`; overpower choice; (client: replicate the save to the
   server and wait; standalone: `bSaveGameLoaded` is set true now); UI preferences; **`ApplyPlayerSaveGameData`** (level,
   experience, skill points, currency, play time, playthrough; see below); customization; **`ApplySkillSaveGameData`**
   (the tree's grades; requires the tree definition to have loaded, which is why `RunStreamingDataEvent` schedules
   `NotifyReadyToLoadPendingSavegame` after `SkillTreeDefinitionLoaded`); black-market upgrades; **items**
   (`ApplyItemSaveGameData`: backpack and equipped items; each is spawned and given to the pawn, equipped ones through
   `ServerSetItemSaveGameData`; ends with `ServerItemSaveGameDataCompleted`, which refreshes skills affecting the player,
   recomputes attributes and sets shield and health to their maxima); resource pools; **inventory slots**
   (`SetInventoryMaxSize`, `SetWeaponReadyMax`); **weapons** (`ApplyInventorySaveGameData` -> `ApplyWeaponSaveGameData`:
   first the native `FixupSavedWeapons`, then each weapon with a quick slot through `ServerSetWeaponSaveGameData` (spawn,
   initialize from definition data, set mark, ammo 0, `GiveTo` with the quick slot pending) and each backpack weapon
   through `ClientAddWeaponToBackpack`; then `UpdateBackpackInventoryCount`); native `ApplyStatsSaveGameData`; game stages;
   **`ApplyMissionSaveGameData(save, true)`** (local controller only; see below); visited teleporters; challenges; queued
   training messages; discovered areas; minimap fog; DLC expansion data; the bank; lockouts; awesome-skill flag; vehicle
   data; then (authority) the **skill-point top-up** and `DetectAndRestoreMissingProfileData`; finally
   `GetHighestSaveGameId` and the global `PlayerJoined` event.
5. **`ApplyPlayerSaveGameData`:** copies the challenge lists; on the local controller updates the LCD helper, the currency
   array on the replication info (all 13 entries; the golden-key entry is recomputed), `LoadedCharPlayTime`, the HUD's cached
   experience value and `NumGoldenKeysNotified`; on authority it calls `ServerSetSaveGameData(level, experience,
   skillPoints, specialistPoints, currency, playthroughsCompleted)` and sets `NumOverpowerLevelsUnlocked` (clamped to the
   maximum possible). **`ServerSetSaveGameData`** sets the replication info's level to `max(saved level, 1)`, recalculates the
   attribute initialised state, writes the saved experience into the experience pool (`SetCurrentValue`) and marks it as not
   to be re-initialised, runs `OnExpLevelChange(false, false)` (no level-up feedback; the co-op level-up ding is suppressed),
   writes the **stored** general and specialist skill points, initialises the currency array, sets the highest completed
   playthrough, calls `ResetSkillTree(false, true)`, and, for the primary local player when the saved playthroughs-completed
   differs from the world's current playthrough, sets the world's current playthrough from it.
6. **Skill-point top-up** (end of `LoadPlayerSaveGame`, authority): the expected total is the integer from evaluating
   `GlobalsDefinition.GeneralSkillPointsTotalForCurrentLevel` with the controller as context (`max(0, L - 4)`, confirmed in
   game by NATIVE_PROGRESSION.md). If it exceeds unspent + points spent in the tree, unspent is set to expected - spent.
7. **`ApplyMissionSaveGameData(save, bManageRewards)`:** see the section after the natives. Its tail decides who builds the
   tracker (below, "Who restores the tracker and the mission weapon").

## MissionTracker.InitializeWorldMissionState

- **Signature:** `(MissionList: array of MissionStatusPlayerData, FilteredMissions: array of MissionDefinition, WPC:
  WillowPlayerController)`, returns bool. Called from script `WillowPlayerController.InitializeWorldMissionState` (with the
  current playthrough's `MissionList` and `FilteredMissions` and `Self`), which on a true result calls
  `SetActiveMission(playthrough.ActiveMission)` on the tracker and `RefreshHUDMissionWidget`; on false it re-arms itself with
  `SetTimer(0.01, false, 'InitializeWorldMissionState')`. Other script callers: `WillowGameReplicationInfo.SetCurrentPlaythrough`
  and `SetPlaythroughOverride` (on the primary controller) and `ResetInfiniteVaultHunterPlaythrough`.
- **Reads:** the tracker's `MissionList` (one record per mission definition, kept sorted by the definition's full name, which
  is how a record is found: a binary search), `DependentMissions`, the world's list of levels, the world's name, each mission
  definition's `Dependencies`, `ObjectiveDependency`, `MissionWeapon`, `GameStageRegion`, `bCanBeFailed`, `SecondsToComplete`,
  `DefendMissionSetting`, `ObjectiveDefs`.
- **Does (outcomes, in order):**
  1. Returns false with no effect when the tracker has no world or a readiness test over the world's list of levels fails
     (not decoded; the script's 0.01 s retry covers it). Everything below happens on the true path.
  2. **Resets the tracker:** every record becomes status NotStarted with empty progress, no active set, no sub-sets and the
     flags `bInitialized`, `bHeardKickoff`, `bFiltered` cleared; every mission definition has its `bGameStageLocked` cleared
     and its runtime `GameStage` zeroed. Records whose definition is in `FilteredMissions` get `bFiltered`.
  3. Empties `DependentMissions` and **sets `bDataValidated`** (the same bit `ValidateData` sets; see "Corrections").
  4. **Applies the saved list** entry by entry in saved order (an entry is skipped when its definition is None or the tracker
     has no record for it). An entry is accepted only if its progress array matches the definition: the progress length must
     equal the number of `ObjectiveDefs`, except that a **Failed entry with empty progress** is also accepted. Then:
     - **Dependencies:** if any mission in the definition's `Dependencies` has a record that is not (initialised and
       Complete), the entry is **not applied now**; a copy goes into `DependentMissions`. Dependencies with no record are
       ignored.
     - **Objective dependency:** if the definition has an `ObjectiveDependency` (an objective and a type byte), the record of
       the mission that owns that objective must be initialised and: type 0 needs the objective complete, type 1 needs it
       complete or currently updatable, any other type needs nothing; otherwise the entry goes to `DependentMissions`. If
       the dependency's owner is not a mission definition the entry is dropped.
     - Otherwise the entry is **applied** (the apply routine below).
  5. **The apply routine** (used by step 4, and again for deferred entries by a later pass over `DependentMissions`
     whenever another mission's state has been applied): the record is marked initialised (a second application of an already
     initialised record happens only when it is a forward step: NotStarted or Failed always; Active only to an Active entry
     whose active set is at or after the record's set along the `NextSet` chain, or to RequiredObjectivesComplete,
     ReadyToTurnIn or Complete; RequiredObjectivesComplete to the same set-or-later, ReadyToTurnIn, Complete; ReadyToTurnIn
     only to Complete; Complete never). Applying is skipped unless `MissionDependenciesMet` holds for the definition. Then:
     any existing mission-weapon entry for the definition is removed; the definition's `bGameStageLocked` is set and its
     `GameStage` set from the entry; status, progress array, active set and sub-sets are copied; `bHeardKickoff` is copied.
     The activation part then runs (it is skipped only when a flag argument is non-zero; in a normal running game I read
     that flag as zero, see Open): **for Active or RequiredObjectivesComplete** the active set and every sub-set register
     their objectives' stat listeners (so progress keeps counting), the definition's **`MissionWeapon` is granted again**
     through the tracker's own `GrantMissionWeapon` (so the primary player's lent weapon comes back here), and when no
     active set was saved the first set is started (`AdvanceFromObjectiveSet`). For every restored status the tracker
     refreshes its tracked-objective display, **re-evaluates the mission** (see "Re-evaluation" below), re-establishes
     mission blocking (the definition's `BlockedMissions`, or the global blocker; only for plot-critical definitions as
     read) and, when the status is neither NotStarted nor Complete, restores the "blocking window" between the definition's
     `StartBlockingSet` and `StopBlockingSet`. The dependent-missions pass then runs.
  6. After the list: one controller-specific helper runs when a controller was given and the build is not a preview/editor
     build (not decoded; it looks up the controller's class entry and the current region in a globals table; presentation or stat related).
  7. Every record is set `bInitialized`. For each **Active** record whose definition has a `GameStageRegion`, if the region's
     current game stage (the value a region virtual returns; the decompiler lost it, read as such) is lower than the stage
     stored for the definition, the definition is locked and its stage lowered to the region's.
  8. **Behavior replay:** unless this is a preview build or the current map is the loader map (name compared
     case-insensitively with `Loader`), `ReplayMissionStateEvents` runs for every record, then a further mission-state
     helper runs once (not decoded). `ReplayMissionStateEvents` re-fires the behavior kernel's mission events so that
     mission-gated behavior sequences are re-armed: one status event for the mission (carrying the status byte), and, for
     non-NotStarted missions, one event per objective (flag: objective complete) and one per objective set (flag: set passed,
     set active, or not reached yet). For NotStarted missions the per-objective and per-set events are sent with "false".
  9. **`NotifyMissionObservers(kind 0)` for every record** (the level-load notification of
     [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md), which is what makes every `BehaviorSequenceEnableByMission`
     re-evaluate on load). Returns true.
- **Re-evaluation after restoring Active / RequiredObjectivesComplete:** for an **Active** mission that can fail
  (`bCanBeFailed`, or `SecondsToComplete` > 0, or `DefendMissionSetting` 2), a fail-on-load test (the definition's
  `FailOnLoadObjectiveSet` rule, not decoded) can set the mission to **Failed** immediately (not in a preview build); if it
  has `SecondsToComplete` > 0 and does not fail, the timer logic is restarted. Every other case runs the objective-set
  completion check, which can promote the status (RequiredObjectivesComplete / ReadyToTurnIn) when the restored progress
  already satisfies the set.
- **Calls into script:** none directly; the behavior-kernel event activations and observer notifications reach script
  through the kernel.
- **Calls other natives:** `GrantMissionWeapon`, `RemoveMissionWeapon`, `AdvanceFromObjectiveSet`, `SetMissionStatusImpl`,
  `EvaluateObjectiveSetCompletion`, `IsObjectiveComplete`, `CanUpdateObjective`, `MissionDependenciesMetImpl`,
  `NotifyMissionObservers`, `ReplayMissionStateEvents` (names as in the earlier notes; Fire needs the Active, RequiredObjectivesComplete,
  ReadyToTurnIn and Complete paths).
- **Constants / formulas:** objective-dependency type bytes 0 / 1 / other as above; the loader-map name `Loader`.
- **Edge cases:** the saved progress length check silently drops an entry when a mission definition changed its objective
  count; entries for missions the tracker does not know (DLC not loaded) are dropped here (the controller keeps them in its
  unloadable-DLC list); a Complete mission is restored as Complete with nothing granted by the tracker (rewards are the
  controller's `bNeedsRewards` path, below); the tracker is rebuilt from scratch every call, so calling it twice does not
  duplicate anything except that weapons are removed and re-granted per Active record.
- **Implementer checklist:** (1) reset all records and the tracker's per-mission runtime stage first; (2) apply in saved
  order with the progress-length filter, the dependency and objective-dependency deferral and the forward-only rule for a
  second application; (3) set `bDataValidated`; (4) for Active or RequiredObjectivesComplete: register objective listeners
  for the active set and sub-sets, regrant the mission weapon, start the first set when none was saved; (5) re-evaluate set
  completion for the restored status; (6) set every record initialised; (7) replay behavior events and send observer kind 0
  per record, outside the loader map; (8) the script wrapper then sets the tracked mission from the playthrough's
  `ActiveMission`.
- **Open:** the world-readiness test; the controller helper in step 5; the exact fail-on-load rule; what the tracked-objective
  display helper does; the second mission-state helper after the replay; the decompiler did not recover the flag arguments of
  the apply routine at this call site; the assembly shows the fourth argument constant 0 and the third argument a local that
  I read as the preview-build flag (zero in a normal game), which is why activation is described as running; this should
  be confirmed in game (does the lent weapon come back for the primary player after a load?).

## MissionTracker.GrantMissionWeaponsToClientPlayer

- **Signature:** `(WillowPlayerController WillowPC)`. The only script caller is the tail of `ApplyMissionSaveGameData`, in the
  branch for a controller that is **not the primary player** and whose role is authority (a joining co-op client's controller
  on the host, or a second local player), together with `GrantDefaultWeaponIfEligible`. The primary player's mission weapon
  is not granted here but by the tracker's apply routine (above).
- **Does:** returns at once when the controller's `Player` is a local player **and** a net-mode helper answers yes (the helper
  is not decoded; read as "this game is not running as a plain host"). Otherwise, for each balance in
  `ActiveMissionWeapons` in order: finds the owning mission through the balance's reference, **stops the whole loop if that
  mission has no `GameStageRegion`**; takes the region's game stage and awesome level; rolls the balance once at that stage
  (the roll is the one of [NATIVE_LOOT.md](NATIVE_LOOT.md)); if a weapon results, stamps `SourceResponsibleName` with the
  mission's name and gives it to the controller's pawn (the vehicle driver's pawn when driving) through the weapon's script
  `GiveTo` with the not-readied flag. The array of results is freed.
- **Edge cases:** an empty `ActiveMissionWeapons` does nothing; a balance without a mission reference is skipped.
- **Implementer checklist:** (1) a controller-specific entry point that is **not** used for the standalone primary player;
  (2) the primary player's weapon returns through the tracker's apply routine for Active / RequiredObjectivesComplete
  missions that have a `MissionWeapon`; (3) never saved, always rolled again at the region's stage.
- **Open:** the net-mode helper; the exact pawn the weapon goes to.

## WillowPlayerController.AttemptPreSaveGameLoadFixup (and the two other fixups)

- **Signature:** `(PlayerSaveGame SaveGame)`. The script calls it first in `LoadPlayerSaveGame`. The native is a thin front
  for a virtual method of the controller; the method body (slot right after `AttemptPreSaveGameLoadFixup` in the class's
  native declaration order, found through the class's table) is what is described. Confidence in the body: medium-high.
- **Does (all on the loaded save object, in place):**
  1. Records the save's `ExpLevel`, `ExpPoints`, `InventorySlotData.InventorySlotMax_Misc` (backpack size) and
     `MaxBankSlots` in four controller fields (the first two are the "loaded from saved game" values the natives below
     return).
  2. If `InventorySlotData.WeaponReadyMax` is below 4: raises it to at least `NumQuickSlotsFlourished + 2`; if it is still
     below 4 it calls `ConditionalFixWeaponReadyMax` (below).
  3. Clamps `ExpLevel` to [1, maximum level] (maximum level: 50 plus three DLC level-cap increments whose default values in the
     executable's data are 11, 11 and 8; whether the game zeroes them without the DLC was not read, so the cap is 50 in the
     slice if they are zero), the backpack size to [12, 39], `MaxBankSlots` to [6, 24], and each of the 13 `CurrencyOnHand`
     entries to [0, cap of that currency]. Caps in the executable's table by currency index: 0 -> 99,999,999; 1 -> 500;
     2 to 12 -> 999.
  4. **Recomputes `GeneralSkillPoints`:** total = `ExpLevel - 4` when `ExpLevel` >= 5, else 0; spent = the sum of `Grade`
     over `SkillData`, clamped to [0, total]; `GeneralSkillPoints` = total - spent. The saved unspent counter is therefore
     **replaced**, not trusted.
- **Edge cases:** a save with a different player class skips the whole fixup (`LoadPlayerSaveGame` refuses the save).
- **Implementer checklist:** (1) run this on the in-memory save before applying anything; (2) clamp as above; (3) derive
  unspent skill points from the level and the saved grades; (4) keep the loaded level and experience for the two getters.
- **Post-create fixup (`AttemptPostSaveGameCreateFixup`, last step of `GeneratePlayerSaveGame`):** raises the new save's
  `ExpLevel`, `ExpPoints`, backpack size and `MaxBankSlots` to **at least the values loaded from the save** (so a partial or
  not-yet-finished load never lowers a saved number), re-clamps the currencies, and recomputes `GeneralSkillPoints`
  from the saved level and the grades exactly as above.
- **`ConditionalFixWeaponReadyMax`:** starts at 2; scans **only the first playthrough's** `MissionData` (the save's first
  `MissionPlaythroughs` entry); every Complete (status 4), plot-critical, non-DLC (`DlcExpansion` None) mission whose
  `MissionNumber` is 5 or 10 adds 1; the sum is clamped to [2, 4]; `InventorySlotData.WeaponReadyMax` is raised to at least
  that. (Adds the "first playthrough only" detail to NATIVE_INVENTORY_EQUIP.md.)
- **Open:** whether the maximum-level inputs are zero in the shipped game.

## WillowPlayerController.GetExpLevelLoadedFromSavedGame / GetExpPointsLoadedFromSavedGame / GetExpPoints

- **Does:** the two "loaded" natives return the controller fields the pre-load fixup fills (the saved, clamped level and
  experience); 0 before any load. **`GetExpPoints`** reads the experience pool's current value and rounds/limits it as
  follows: let `current` be the pool's value as an integer and `loaded` the loaded-from-save experience. If `loaded <=
  current`, the result is `min(current, maximum experience)` where maximum experience is the experience requirement
  evaluated at the maximum level (via the same maximum-level function as above and the `ExpPointsRequiredForLevel` curve of
  NATIVE_PROGRESSION.md); otherwise (`loaded > current`, the pool not yet restored) the result is `loaded`. This is the
  experience a save writes.
- **Implementer checklist:** keep `loaded level`, `loaded points`; the saved experience never drops below the loaded value
  while the pool has not caught up; clamp to the experience cap otherwise.
- **Open:** the exact maximum-experience helper (it subtracts a correction when a debug global is set).

## WillowPlayerController.GetActivePlotCriticalMissionNumber / GetLocalActiveMissionNumber

- **`GetActivePlotCriticalMissionNumber(playthrough = -1 for the current one)`:** scans that playthrough's mission list in
  order over definitions that are plot-critical and not DLC. The first one that is Active, RequiredObjectivesComplete or
  ReadyToTurnIn returns its `MissionNumber` at once; otherwise the largest `MissionNumber` among Complete ones (0 if none).
  0 for an invalid playthrough. Written into `PlotMissionNumber` of the save (a menu label, not used on load).
- **`GetLocalActiveMissionNumber(playthrough = -1)`:** if the playthrough's tracked mission (`ActiveMission`) is in its list,
  is neither NotStarted nor Complete and has all its dependencies (and theirs, recursively) Complete and any objective
  dependency met, returns its `MissionNumber`. Otherwise scans the list for missions that are Active, RequiredObjectivesComplete,
  ReadyToTurnIn or **Failed**: plot-critical non-DLC ones give the lowest `MissionNumber`; others (checked through a script
  call that answers whether the mission is of the base game) give a second lowest; the first is preferred. Falls back to
  `GetActivePlotCriticalMissionNumber`. Written into `ActiveMissionNumber`.
- **Implementer checklist:** only needed to fill the two labels; no behavior depends on them.

## WillowPlayerController.SaveStatsSaveGameData / ApplyStatsSaveGameData

- **Does:** the save side clears the save's `StatsData` byte array and, if the controller has a `PlayerStats` object,
  serializes the stats into it through a memory-writer archive; the apply side, when `StatsData` is non-empty and the
  controller has stats, deserializes it back, and on authority runs one more helper (not decoded; reads as pushing the loaded
  numbers to the replication side). Stats are a serialized blob (Unreal archive format) and not individually named fields.
- **Slice use:** stats counters (kills, missions completed) are a blob; the slice can persist its own few stats outside it.
- **Open:** the archive's internal order.

## WillowPlayerController.GenerateSaveGameGuid / AreSaveGuidsEqual

- **`GenerateSaveGameGuid(save)`:** if the save's `SaveGuid` is all zeros, fills it with a newly generated GUID and returns
  true; otherwise returns false and changes nothing. **`AreSaveGuidsEqual(a, b)`:** true when both exist and all four words
  match. `FinishSaveGameLoad` uses it to recognise "the save already cached for this controller is the one being loaded".
- **Implementer checklist:** a GUID is created once per character; a copy of a save keeps it.

## WillowPlayerController.AddExpansionSavedataToUnloadableItemData / ExtractExpansionSavedataFromUnloadableItemData

- **Does:** a few base-game numbers outgrew the original save format; the excess is stored as **marker entries inside
  `UnloadableDlcItemData`** (entries whose stored value is negative; the marker code is the low byte of the negated value
  and the payload the remaining bits shifted by 8). Save side (`Add...`, called last in `GeneratePlayerSaveGame`): removes
  any old marker entries, then, when needed, writes markers for: the amount of the **second currency type above 99**
  (code 1; the save field is then set to 99); **playthrough bookkeeping above 1** (code 2: the last playthrough number and
  playthroughs completed beyond the original single-digit range; the save fields are then clamped to their old range);
  a controller value that must survive (code 3); `NumOverpowerLevelsUnlocked` when it exceeds the original maximum (code 4);
  and the last overpower choice (code 5). Load side (`Extract...`, run on the loaded save in `FinishSaveGameLoad`): reads the
  markers back into the matching fields (second currency amount, last playthrough number and playthroughs completed, the
  controller value, overpower levels, overpower choice) and deletes the marker entries.
- **Slice use:** none (all are zero or default for a first-playthrough Level 1-50 character); keep the round trip only if the
  host's save format needs it.

## WillowPlayerController.SaveDLCExpansionData / FixupSavedWeapons / ReloadDefaultSaveGame

- `SaveDLCExpansionData(save)`: copies the controller's list of installed expansions (an identifier plus two more words
  each) into `DLCExpansionData`. None in the slice.
- `FixupSavedWeapons(weapons)`: walks the 72-byte weapon records of the save and patches a hard-coded set of old weapon
  definitions and materials (a handful of rare Maliwan, Bandit, Tediore, Torgue and Vladof guns) to their current
  definitions. No effect on a save that does not hold those.
- `ReloadDefaultSaveGame()`: when the controller's replication-side owner object exists, passes one value from it to a script
  function on the controller (the function name is built from a global name; not resolved). Called only when there is no
  cached save (new character). Low confidence.

## WillowPlayerController.NotifyReadyToLoadPendingSavegame (and the co-op save channel)

- **Does:** the implementation marks the controller's **save-game replication channel** object (if one exists) as ready and
  lets it process: when the received byte buffer is complete (received count equals expected count), the owner is
  authority, and the ready mark is set, it deserializes a `PlayerSaveGame` from the buffer, extracts the expansion markers
  and starts the load of the remote player's save on the server. With no channel (standalone, host) it does nothing.
  `CreateSaveGameReplicationChannel`, `AttemptReplicateSaveGame`, `HasSentFullSaveGame`, `NotifyReceivedSaveGameChannel` and
  `NotifyClosedSaveGameChannel` are the co-op transport for it and are **not read**.
- **Implementer checklist:** no-op in the slice.

## WillowSaveGameManager natives (the storage boundary)

`Save`, `SaveGame`, `SaveGraveyard`, `SaveRawData`, `BeginLoadGame`, `EndLoadGame`, `BeginLoadWillowOneGame`,
`EndLoadWillowOneGame`, `LoadGraveyard`, `LoadRawData`, `GetSaveGameList`, `GetCrossTitleSaveGameList`,
`BeginGetSaveGameDataFromList`, `EndGetSaveGameDataFromList`, `GetLastSaveGame`, `GetLastSaveGameId`,
`GetHighestSaveIdFromFileList`, `DeleteSaveGame`, `ValidateSaveData`, `GetCachedPlayerSaveGame`, `HasCachedPlayerSaveGame`,
`SetCachedPlayerSaveGame`, `ClearCache`, `NotifySaveStarted`, `NotifySaveComplete`, `GetNumSaveGames` and the rest are
**thin front ends**: each unpacks its script arguments and calls a virtual method of the manager object that the platform
storage layer implements. The ones read (`Save`, `SaveGame`, `BeginLoadGame`, `EndLoadGame`, `LoadRawData`,
`GetLastSaveGame`, `ValidateSaveData`, `GetCachedPlayerSaveGame`, `SetCachedPlayerSaveGame`) do nothing else; none touches
mission or player fields. The relations that matter:

- `SaveGame(controllerId, PlayerSaveGame, filename, splitScreenUser)` writes one character's save; the manager keeps a
  cached `PlayerSaveGame` per controller id, filled by the load (`SetCachedPlayerSaveGame`) and by
  `UpdateSavegameForPlaythroughCompletion`, read by `GetCachedPlayerSaveGame` / script `GetCachedSaveGame`.
- Completion is reported through delegates (`OnSaveComplete`, `OnLoadComplete`, `OnListLoadComplete`...); the controller's
  `SaveAllPlayers` chain waits on `OnSaveComplete`.
- File naming is script: `BuildSaveGameNameFromId` / `GetSaveGameNameFromid` make the name from the numeric id
  (`SaveGameId`); `GetHighestSaveGameId` picks the next id.
- **Implementer checklist:** replace by a small adapter: cache keyed by controller id, `SaveGame` writes the serialized
  object and fires `OnSaveComplete`, `BeginLoadGame` / `EndLoadGame` return the loaded object and fire `OnLoadComplete`.
- **Open:** the on-disk format is not described here (our tooling reads it).

## ApplyMissionSaveGameData (script, with its natives)

Described because it is the load-side counterpart of `SaveMissionSaveGameData` and decides the order around
`InitializeWorldMissionState`.

1. Reads the current playthrough, empties the controller's `MissionPlaythroughs` (and, with `bManageRewards`, its
   `UnclaimedRewards`).
2. For every saved playthrough entry builds a `MissionPlaythroughData`: each `MissionData` record is copied field by field
   (progress, active set, sub-sets, status, definition, game stage, `bNeedsRewards`, `bHeardKickoff`), passes through
   `FixupSavedMissionGameStage` (an entry whose definition's game-stage region is the "Iris battles" region, in playthrough
   index 0 or 1, gets stage 50; entries with no definition or no region are left alone) and is appended. With `bManageRewards`: a record whose `bNeedsRewards` is set
   adds a reward request (alternate-reward flag from `MissionDefinition.ShouldGrantAlternateReward(ObjectivesProgress)`); for
   the **current** playthrough the saved `PendingMissionRewards` entries whose mission matches a record are put into
   `UnclaimedRewards`; for other playthroughs they are kept in the controller's list for other playthroughs. Unloadable-DLC
   records and pending rewards are copied; `FilteredMissions` are copied without duplicates, **at most 512 entries**.
3. The saved `ActiveMission` string is matched against each record's definition **full name**; the record that matches
   becomes the playthrough's `ActiveMission`; if none matches it is None.
4. The playthrough data is appended. With `bManageRewards`, after the loop, `ServerGrantMissionRewards(mission,
   altRewardFlag)` runs for every reward request: credits (and the optional credit/other-currency reward), **`ExpEarn` of
   the mission's experience reward** (source PlotMissionAward for plot-critical missions, SideMissionAward otherwise), and the reward
   UI path for item choices. **So a mission that was saved Complete with `bNeedsRewards` still set pays its experience and
   money again on load**; only `MissionRewardsReceived` (called by the reward UI when the player accepts, or by
   `AcceptOrSaveUnclaimedReward` for a single choice) clears the flag. `UpdateMissionStatus` sets the flag when a mission
   becomes Complete.
5. Tail: the controller's `bReceivedDefaultWeapon` is restored; if fewer playthroughs were loaded than the current index
   needs, an empty playthrough is added; `FixupPlaythroughTwo(PlaythroughsCompleted)` (only with `bManageRewards`; creates the
   playthrough-2 mission records from the first playthrough's definitions when a save has completed playthrough 1 and no
   playthrough-2 list); then:
   - **authority and primary player:** `InitializeWorldMissionState()` (the tracker restore above);
   - **client and primary player:** `RequestMissionData()` (asks the server to send the tracker data; retries every 0.1 s
     until the replication info and tracker exist);
   - **authority but not the primary player:** `GrantDefaultWeaponIfEligible(tracker)` and
     `MissionTracker.GrantMissionWeaponsToClientPlayer(self)`;
   - finally `UpdateLcdMissionStatus`.
   `GrantDefaultWeaponIfEligible` gives the globals' default weapon to a new character that has not received it yet when the
   first-weapon mission is ReadyToTurnIn or Complete and the character has no weapon.

## Who restores the tracker and the mission weapon (answers for the standalone Fire slice)

- The tracker holds **no persistent state of its own**: after a load it is rebuilt from the controller's
  `MissionPlaythroughs[current].MissionList`, the playthrough's `FilteredMissions` and its `ActiveMission`, in that order
  (`InitializeWorldMissionState`, then `SetActiveMission`).
- Mission data is applied **after** the pawn and its inventory manager exist and **after** player data, skills, items,
  slots, weapons, stats and game stages (step 4 of the load order); the first, rewards-free apply into the controller's
  lists already happened at character select.
- Experience and level come back from `ServerSetSaveGameData` (level, experience, `OnExpLevelChange` without feedback), then
  the skills are replayed, then the skill-point top-up; the pre-load fixup has already rewritten the unspent counter.
- The lent Maliwan pistol of the Fire mission is **not in the save** (`CanBeSaved` false). On a standalone load it returns
  through the tracker's apply routine because the Fire mission is Active (or RequiredObjectivesComplete); it is rolled again
  at the mission's region stage and given to the pawn. If the mission is ReadyToTurnIn or Complete nothing is granted
  (UNVERIFIED: the apply routine's weapon grant is tied to the two Active-like statuses as read).

## Implications for the slice's resume test (script swap)

1. The save a swap needs to write per mission record is exactly: definition, status, progress array, active set, sub-sets,
   game stage, `bNeedsRewards`, `bHeardKickoff`, plus the tracked mission by full name, per playthrough; XP, level, skill
   grades and the quick-slot weapons are separate fields.
2. After a load the tracker must go through: reset, ordered apply with the progress-length filter, listener registration for
   the active objective set, weapon re-grant, re-evaluation, behavior replay and `NotifyMissionObservers(0)`; it does not
   replay the status-change events (ids 6 + status) of the original accept.
3. A mission saved between turn-in and reward acceptance pays again on load; a turn-in flow that calls
   `MissionRewardsReceived` (the reward UI path or the empty-reward shortcut) before the save does not.
4. Saves happen only at the trigger points above; a mission progress made since the last station or menu close is lost on
   quit-without-menu (a crash or kill), but is written on pause-menu quit.

## Corrections to earlier notes

- [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) says only `ValidateData` writes `bDataValidated` and that
  `InitializeWorldMissionState` does not set it. As read here `InitializeWorldMissionState` also sets the bit (right after
  clearing `DependentMissions`); `ValidateData` and `IsDataValid` read as before. UNVERIFIED either way.
- [NATIVE_INVENTORY_EQUIP.md](NATIVE_INVENTORY_EQUIP.md) says `GrantMissionWeaponsToClientPlayer` runs after the save's
  missions are applied "(primary player only)". The script call is in the **non-primary, authority** branch; the primary
  player's weapon returns through the tracker's apply routine (see above). Also: `ConditionalFixWeaponReadyMax` scans only the
  first playthrough's missions; and Complete plot-critical missions numbered 5 or 10 are the only counted ones (as noted).
- [NATIVE_SKILLS.md](NATIVE_SKILLS.md) section 3.2 and rule 4 say a load "restores the stored unspent counter, not a
  formula". The pre-load fixup replaces it with `max(0, L - 4) - min(spent, max(0, L - 4))` before it is applied, and the
  end of `LoadPlayerSaveGame` tops it up to the level's total when the tree plus the unspent counter fall short (so a save
  with fewer unspent points than the formula gives is raised). The point total is therefore always the level formula after a
  load.
- [SANCTUARY_RPG_MISSION.md](SANCTUARY_RPG_MISSION.md) (quest save): the host's "experience and skill grades are not
  saved" limitation is gone since 2026-10-05; for fidelity, the real game restores experience as above, always sets
  "no level-up feedback" on load, and does not re-fire the accept-time status events.

## Not read yet

- The world-readiness test and the preview-build test in `InitializeWorldMissionState`; the controller helper and the
  second mission-state helper; the fail-on-load rule; `SetActiveMission` on the load path beyond NATIVE_MISSION_SCRIPT_BRIDGE.md.
- The byte-level behavior replay (`ReplayMissionStateEvents`) beyond the outline above.
- Co-op transport natives (`CreateSaveGameReplicationChannel`, `AttemptReplicateSaveGame`, `HasSentFullSaveGame`,
  `NotifyReceivedSaveGameChannel`, `NotifyClosedSaveGameChannel`), `SetHasSaveGamesAvailable`, `PlayerIsLicensedToSaveGame`,
  `GetCurrPlaythrough`, the profile natives (`SetNeedsProfileWrite`, `AttemptProfileWriteIfNecessary`), the graveyard
  natives and the console-only manager natives.
- Script not yet read in full: `FinishSaveGameLoad` console branches, `ApplyChallenge/Lockout/Bank` data, `ResetInfiniteVaultHunterPlaythrough`,
  `DetectAndRestoreMissingProfileData`.
