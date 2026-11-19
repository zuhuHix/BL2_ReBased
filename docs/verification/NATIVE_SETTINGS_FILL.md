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
3. the provider is then bound to the new settings object (it takes them as its source; the binding builds the
   provider's own view of the settings' properties for the UI, which the refresh does not use).

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
`CurrPlotMission`, `DlcFlag`, `DlcMapPackageId` and `DlcMapContentId`. The refresh's five writes (level, active
mission, active plot mission, playthrough, DLC content id and flag) each target one of these. All their values start
at 0.

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

- **The store's initialisation:** Off fills each entry as section 1 says, when the world's data store client makes
  the store: a `WillowOnlineGameSettings` object from its class defaults as `GameSettings`, and a
  `UIDataProvider_Settings` as `Provider`. The provider's binding (section 1, step 3) is UI-side. It is a named
  `NotYet` only if something on the slice reads the provider (nothing on the accept path does). The moment of
  initialisation (open question 1) is the client's creation in the port.
- **`Registered`:** if the port runs the store's script `Registered` event, its property-notification native
  (`AddPropertyNotificationChangeRequest`, not read) is a named `NotYet`.
- **The settings entry read** (the NotYet IMPL-ONLINE-GAME-SETTINGS stops at): it answers the object above, so the
  refresh goes on to its writes.
- **`SetIntProperty`:** section 3's rule, in Off.
- **`GetCurrPlaythrough`:** the field read, in Off.
- **`GetActiveMissionNumber`, `GetActivePlotCriticalMissionNumber`:** the rules of their notes, wherever the port has
  them in Off. Otherwise they stay named `NotYet`s.
- **`GetDLCManager`:** the world engine's `DLCManager`. The port's engine has no DLC manager, as it has no online
  subsystem: it answers None, and the refresh skips the DLC writes (the script tests the manager first). This is a
  fact about our engine, the same kind of ruling as the online subsystem. Whether the shipped game always has a
  manager is open (open question 2), and the hub should confirm it.
- **With a manager** (not the port's case): `HasCompatibilityData` and `StaticGetDlcContentIdForCurrentMap` are named
  `NotYet`s.

## Open questions

1. **UNVERIFIED:** when the client initialises its global stores in this executable (at engine start, as in public
   UE3, or later), and whether `Registered` runs for a global store with no player.
2. **UNVERIFIED:** whether the shipped game's engine always has a `DLCManager` in single player, and what
   `HasCompatibilityData` and `StaticGetDlcContentIdForCurrentMap` answer there.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Right after the main menu, read the online game settings store's `GameSettingsCfgList[0]`: the class of
  `GameSettings` and `Provider`, and the eight `Properties` with their values.
- During a mission accept, hook `Settings.SetIntProperty` and record each property id and value. Compare them with
  `GetCurrPlaythrough` + 1, the mission numbers + 1, and the player's level plus the overpower modifier.
- Read `GameEngine.DLCManager` in single player, and call `HasCompatibilityData` and
  `StaticGetDlcContentIdForCurrentMap` on a base-game map (open question 2).
- Call `SetIntProperty` with an id the settings do not list, and confirm `Properties` is unchanged.
