# Native rules of the online game settings entry and the refresh's writes (draft, 2026-11-19)

AI-assisted (Claude), analyst lane ANALYST-SETTINGS-FILL (decision 2026-11-19-69). It answers open question 2 of
NATIVE_ONLINE_GAME_SETTINGS.md (who fills the online game settings store's `GameSettings` entry) and reads the natives
the refresh then meets. The natives were read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`,
under the machine lock), the script with `research/script_disasm.py`, and the class defaults from the installed
packages with `ow-package --object-dump`. Written in our own words, as behaviour only: no decompiler output,
pseudo-code, quoted script statements, addresses, offsets or call structure is reproduced here. **Every rule is
UNVERIFIED in the running game.** The last section lists the live observations that would confirm them.

**Clean room.** This lane implements nothing. A fresh implementer codes from this note alone.

## Summary

| Piece | Kind | Confidence | Status |
|---|---|---|---|
| The online game settings store's initialisation (fills `GameSettings` and `Provider`) | native | high | UNVERIFIED |
| `Settings.SetIntProperty(PropertyId, Value)` | native | high | UNVERIFIED |
| `WillowGameReplicationInfo.GetCurrPlaythrough()` | native | high | UNVERIFIED |
| `GameEngine.GetDLCManager()` | native, static | high | UNVERIFIED |
| `WillowDownloadableContentManager.HasCompatibilityData`, `StaticGetDlcContentIdForCurrentMap` | native | low (not read past their entry) | UNVERIFIED |
| `GetActiveMissionNumber`, `GetActivePlotCriticalMissionNumber` | native | already read elsewhere | UNVERIFIED |

## 1. Who fills the settings entry (open question 2)

**The store's own initialisation does**, and it needs no online subsystem, no player and no configuration beyond
the class defaults. For each entry of `GameSettingsCfgList`, in order:
1. a new object of the entry's `GameSettingsClass` is made, with that class's defaults, and stored as the entry's
   `GameSettings`;
2. only if that worked, a new settings provider is made and stored as the entry's `Provider`. Its class is the store's
   `SettingsProviderClass` when that is `UIDataProvider_Settings` or a subclass of it, and `UIDataProvider_Settings`
   otherwise;
3. the provider is then bound to the new settings object: its `Settings` becomes that object and its `DataClass`
   that object's class. The binding then builds the provider's per-setting list providers
   (`SettingsArrayProviders`) for the UI. That last part was not read, and the refresh does not use it.

For the Willow store, the defaults give one entry of class `WillowOnlineGameSettings` and a `SettingsProviderClass`
of `Engine.UIDataProvider_Settings` (installed package). So after initialisation, `GameSettingsCfgList[0].GameSettings`
is a new `WillowOnlineGameSettings` with its class defaults.

**When the initialisation runs** was not read in this executable. Public UE3 has the engine's data store client make
each configured global store, and initialise it, when the client itself is made at engine start (inference,
UNVERIFIED; open question 1). The store's script `Registered` event, which runs after registration, only asks each
entry's provider for property-change notifications.

**Open question 1 of NATIVE_ONLINE_GAME_SETTINGS.md** (whether the online subsystem already answers `Game` settings in
single player) is not settled by this reading. The hub's ruling stands: our host has no online subsystem, so the
refresh takes the data store route.

## 2. The settings object the refresh writes into

`WillowOnlineGameSettings`' class defaults already list, in `Properties`, eight integer properties. Each has
advertisement type 1, and `PropertyMappings` names them: `CurrMission`, `PlayThrough`, `HostExpLevel`, `IsPublic`,
`CurrPlotMission`, `DlcFlag`, `DlcMapPackageId` and `DlcMapContentId`. The refresh writes four of them before the
DLC gate (level, active mission, active plot mission, playthrough), plus two when a DLC manager is present (DLC
content id and DLC flag). All their values start at 0.

## 3. The natives the refresh meets

- **`Settings.SetIntProperty(PropertyId, Value)`:** finds the first entry of the settings' `Properties` whose
  `PropertyId` matches. When there is one and its data is of the integer type, its value becomes `Value`. Otherwise
  nothing happens: no entry is added, and a non-integer entry is left alone. No class on this path replaces it
  (`OnlineGameSettings` keeps `Settings`' rule). Nothing else is read or written.
- **`WillowGameReplicationInfo.GetCurrPlaythrough()`:** the game replication info's `CurrentPlaythrough` field.
- **`WillowPlayerController.GetActiveMissionNumber()`:** NATIVE_CONTROLLER_HELPERS.md has its rule (the tracker's
  active mission, then the fallbacks).
- **`WillowPlayerController.GetActivePlotCriticalMissionNumber(optional playthrough)`:** NATIVE_SAVE_LOAD.md has its
  rule (the first active plot-critical, non-DLC mission of the playthrough, else the largest completed one).
- **`GameEngine.GetDLCManager()`:** the engine's `DLCManager` field when the engine is a `GameEngine`, otherwise None.
- **`WillowDownloadableContentManager.HasCompatibilityData()`:** a query the manager answers by its own class's rule,
  which was not read.
- **`WillowDownloadableContentManager.StaticGetDlcContentIdForCurrentMap(optional bool)`:** with a world, looks up
  the current map's name in the DLC content table. The flag lets a placeholder entry stand for the first real one.
  Without a world, the answer is empty. The table and the lookup were not read.

## 4. What Off does

**The provider's lifecycle, one answer for each moment:**
- **At store creation** (when the world's data store client makes the store): Off makes both objects of section 1 for
  each entry. `GameSettings` is a `WillowOnlineGameSettings` from its class defaults, and `Provider` is a
  `UIDataProvider_Settings` (the class rule of section 1, step 2).
- **At binding** (straight after): Off sets the provider's `Settings` to the new settings object and its `DataClass`
  to that object's class. It does not build `SettingsArrayProviders`, because that part was not read: the array stays
  empty.
- **At registration:** Off does not run the store's script `Registered` event when it makes the store. Whether and
  when the game runs it for a global store with no player is open question 1.
- **The `NotYet`:** any call on the provider, native or script, answers the named `NotYet` "UIDataProvider_Settings:
  the settings provider's list providers are not built (NATIVE_SETTINGS_FILL.md section 4)". That includes
  `UIDataProvider.AddPropertyNotificationChangeRequest`, which `Registered` would make. Nothing on the accept path
  calls the provider.

**The refresh, in order** (NATIVE_ONLINE_GAME_SETTINGS.md section 5, carried forward):
1. The gate: the controller's role, `IsPrimaryPlayer` and `IsLocalPlayerController`, as that note says.
2. Branch 1, the online subsystem: bypassed, since the port has none (hub ruling).
3. Branch 2, the data store: `GetDataStoreClient` and `FindDataStore`, as that note says.
4. **The settings entry:** that note's named `NotYet` at the entry read (decision 2026-11-19-65's override of
   `GetOnlineGameSettingsViaDatastore`) **is replaced** by the initialised object above. The read answers the entry's
   `GameSettings`, and the refresh goes on.
5. The level write: `ExpLevel` (player replication info) plus `OverpowerLevelModifier` (game replication info), two
   field reads; then the script `UpdateExpLevel`, which writes through `SetIntProperty`.
6. `GetActiveMissionNumber`, then the script `UpdateActiveMission`, which writes the number plus one. Until a lane binds the rule of
   NATIVE_CONTROLLER_HELPERS.md in Off, it answers its current named `NotYet` (a `NotYet` of kind not-read, "behind the
   online game settings refresh, not read", bound by decision 2026-11-19-65). The refresh stops there.
7. `GetActivePlotCriticalMissionNumber`, then the script `UpdateActivePlotMission`, which writes the number plus one. The same holds: its current
   named `NotYet` from decision 2026-11-19-65, until the rule of NATIVE_SAVE_LOAD.md is bound in Off.
8. `GetCurrPlaythrough` (the field read of section 3, in Off), then the script `UpdatePlaythrough`, which writes the
   playthrough plus one.
9. **The DLC gate:** `GetDLCManager`. **Hub ruling (2026-11-19, settled):** the port's engine has no DLC manager, the
   same kind of ruling as the online subsystem, so `GetDLCManager` answers None in Off and the refresh skips the two
   DLC writes. This is revisited when DLC content is in scope. With a manager (not the port's case),
   `HasCompatibilityData` and `StaticGetDlcContentIdForCurrentMap` are asked (both named `NotYet`s), then the scripts
   `UpdateMapDlcContentId` and `UpdateDlcFlag` make the two DLC writes.

The six write steps, in order: `UpdateExpLevel` (level plus overpower level), `UpdateActiveMission` (plus one),
`UpdateActivePlotMission` (plus one) and `UpdatePlaythrough` (plus one) before the DLC gate; then, only with a
manager, `UpdateMapDlcContentId` and `UpdateDlcFlag`. In Off the last two are skipped.

Every `Update...` step writes through `SetIntProperty`, section 3's rule in Off. The push to the online subsystem is
not reached on this path.

## Open questions

1. **UNVERIFIED:** when the client initialises its global stores in this executable (at engine start, as in public
   UE3, or later), and whether `Registered` runs for a global store with no player.
2. **UNVERIFIED, separate from the hub's ruling:** whether the shipped game's engine always has a `DLCManager` in
   single player, and what `HasCompatibilityData` and `StaticGetDlcContentIdForCurrentMap` answer there.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

Each check says which case it tells apart.
- **Startup, before any accept:** at the main menu and again just after a map loads, read the online game settings
  store's `GameSettingsCfgList[0]`: the classes of `GameSettings` and `Provider`, and the eight `Properties` with their
  values. Filled at the main menu shows only that initialisation happened by the menu: engine start and menu
  construction give the same result. Empty at the menu and filled after the map means later. To pin the time, hook the
  store's creation and the first read that finds a populated entry from launch; until then the exact timing stays
  UNVERIFIED (open question 1).
- **Registration:** hook `UIDataStore_OnlineGameSettings.Registered` from launch. Record whether it fires for this
  store, when, and with which player (none, or the local player) (open question 1).
- **The provider's settings source:** read `Provider.Settings` and `Provider.DataClass`. Expect the entry's
  `GameSettings` object and its class (section 1, step 3). Also read `Provider.SettingsArrayProviders`: its length
  shows whether the unread list build made one entry per localized setting.
- **The writes:** during a mission accept, hook `Settings.SetIntProperty` and record each property id and value, in
  order. Compare them with the player's level plus the overpower modifier, the mission numbers plus 1 and
  `GetCurrPlaythrough` plus 1. Four calls mean no DLC manager; six mean one.
- **The setter's misses:** call `SetIntProperty` with an id the settings do not list, and confirm `Properties` is
  unchanged.
- **The setter's type test:** call `SetIntProperty` on the id of a non-integer entry (add a float entry to a scratch
  `Settings` object first), and confirm that entry is unchanged.
- **The setter's first match:** on a scratch `Settings` object with two entries of one id, call `SetIntProperty` and
  confirm that only the first entry changes.
- **The DLC manager:** read `GameEngine.DLCManager` in single player, and call `HasCompatibilityData` and
  `StaticGetDlcContentIdForCurrentMap` on a base-game map (open question 2).
- **No check tells this case apart:** the provider class fallback of section 1, step 2 (a `SettingsProviderClass`
  that is not a `UIDataProvider_Settings`). The installed data has only the default class.
