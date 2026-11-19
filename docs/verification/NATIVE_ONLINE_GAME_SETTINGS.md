# Native rules of the online game settings a mission accept updates: GetDataStoreClient and FindDataStore (draft, 2026-11-19)

AI-assisted (Claude), analyst lane ANALYST-NEXT-STOP (decision 2026-11-19-61). The stop was found with the installed
Off `ow-package --slice-gate` on main b985dcbf. The script path to it was taken from a local run's script stack (a
temporary diagnostic, not committed) and read with `research/script_disasm.py`. The two natives were read from the
game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock). The data store's defaults
were read from the installed package with `ow-package --object-dump`, and the engine configuration names it. Written
in our own words, as behaviour only: no decompiler output, pseudo-code, quoted script statements, addresses, offsets
or call structure is reproduced here. **Every rule is UNVERIFIED in the running game.** The last section lists the
live observations that would confirm them.

**Clean room.** This lane implements nothing. `UIInteraction.GetDataStoreClient` stays a `NotYet` (layer absent) in Off
(decision 2026-11-19-19), and `DataStoreClient.FindDataStore` stays unbound, until a fresh implementer codes them
from this note alone.

**Why it matters.** Slice links 10 to 12 (mission accept, objective, turn-in) all stop at the accept step, on
`GetDataStoreClient`. It is the stop that blocks the most links among those no lane owns (the hand-back lists the
rest).

## 1. Where the stop sits: the accept updates the online game settings

Accepting the Fire mission runs the controller's installed `AcceptMission`, which reaches `AddMission`. On the way,
`AddMission` asks the controller to refresh its **online game settings**: the session properties a listed game shows
other players (level, current mission, playthrough, DLC). It is not mission logic. In order:
1. **The refresh request** (`SetNeedsUpdateOnlineGameSettings`) does anything only when the controller is the
   authority, the primary player and locally controlled. It then refreshes once at once (without pushing the result
   online), marks a refresh for the next tick, and arms the deferred-operations timer on the pause ticker.
2. **The refresh** (`UpdateOnlineGameSettings`) has the same three-way gate. It first asks the controller's online
   subsystem for its current game settings named `Game`. Only when there are none does it take them from the data
   store (step 3). It then writes these into the settings, each through the settings object's integer properties:
   - the player's level plus the overpower level;
   - the active mission number and the active plot-critical mission number, each plus one;
   - the current playthrough plus one;
   - the current map's DLC content id and a DLC flag, when the engine has a DLC manager.

   The result is pushed to the online subsystem only when asked to refresh online data. This path does not ask.
3. **The data store route** (`GetOnlineGameSettingsViaDatastore`, `GetOnlineGameSettingsDatastore`):
   - `GetDataStoreClient` (section 2) gives the client. With none, there are no settings and the refresh does nothing;
   - otherwise `FindDataStore` (section 3) looks up the tag `WillowOnlineGameSettings` for the controller's local
     player, and the answer is cast to `WillowOnlineGameSettings_DataStore`;
   - the settings are the `GameSettings` of that store's first `GameSettingsCfgList` entry, cast to
     `WillowOnlineGameSettings`. With no store or no settings, the refresh does nothing.

No gameplay state is read back from these settings on the accept path.

## 2. UIInteraction.GetDataStoreClient()

A static native. It answers the data store client held by the engine's UI interaction object: the one the game
viewport's UI controller holds. When the engine, its viewport or that UI object is missing, it answers the client of
the `UIInteraction` class default object instead, and None only when that is missing too. It changes nothing.

In the shipped game the engine always has a client (public UE3: it is made when the engine starts), so None is not
an answer the game gives once it is running (decision 2026-11-19-19, UNVERIFIED here).

## 3. DataStoreClient.FindDataStore(Tag, optional Player)

1. **No tag** (`None`): None.
2. **With a player:** that player's own data stores are searched first, in their order. The first whose data store
   tag is `Tag` is the answer.
3. **Otherwise, or when the player has none with that tag:** the client's global data stores, in their order. The
   first with that tag is the answer.
4. **No match:** None.

It changes nothing and creates nothing.

## 4. The data store the route needs

- **Registered globally.** The installed engine configuration lists `WillowGame.WillowOnlineGameSettings_DataStore`
  among the client's global data store classes (next to the online game search store). It is not a player data store,
  so `FindDataStore` finds it in step 3.
- **Its defaults** (installed package): `Tag` is `WillowOnlineGameSettings`; `GameSettingsCfgList` has one entry,
  whose `GameSettingsClass` is `WillowGame.WillowOnlineGameSettings` and whose `GameSettings` and `Provider` are empty;
  `SelectedIndex` is 0.
- **Who fills `GameSettings`:** not in the defaults, so the store's own initialisation must create a settings object
  of `GameSettingsClass` for each entry (public UE3 behaviour of the online game settings store). Not read in this
  executable (UNVERIFIED; open question 2).

## 5. For an implementer, in the port's terms

- `GetDataStoreClient` should answer one client per world (the engine's), not None.
- The client holds the global data stores of the engine configuration. For this path only the store tagged
  `WillowOnlineGameSettings` matters. `FindDataStore` is section 3's two searches over the player's stores and
  then the global ones.
- That store needs its first entry's `GameSettings` to be a `WillowOnlineGameSettings` object for the refresh to
  write anything (section 4). With it empty, the refresh does nothing, which only matches the game if the game's
  store is also empty (open question 2).
- The next natives the refresh then meets, in order (only some already indexed):
  - `PlayerController.IsPrimaryPlayer` (NATIVE_CONTROLLER_HELPERS.md);
  - the online subsystem's game interface (`OnlineSub`, `GetGameSettings`);
  - `WillowPlayerController.GetActiveMissionNumber` (NATIVE_CONTROLLER_HELPERS.md) and
    `GetActivePlotCriticalMissionNumber` (NATIVE_SAVE_LOAD.md);
  - `WillowGameReplicationInfo.GetCurrPlaythrough`;
  - `GameEngine.GetDLCManager`, `WillowDownloadableContentManager.HasCompatibilityData` and
    `StaticGetDlcContentIdForCurrentMap`;
  - the settings' integer property write (`SetIntProperty`).

  None of them is read here.

## Open questions

1. **UNVERIFIED:** whether, in a running single-player game, the online subsystem already answers game settings
   named `Game` (a session exists), so that the data store route is never taken.
2. **UNVERIFIED:** whether the online game settings store's `GameSettings` is filled when the engine starts (section 4),
   and by what.
3. **UNVERIFIED:** whether the port needs these settings at all for the slice: nothing on the accept path reads them
   back. Faithful behaviour (a client, the store, the settings) or a recorded decision to answer "no settings" is the
   hub's choice, not this note's.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- In single player, read the engine's UI interaction `DataStoreClient`, then call `FindDataStore` with
  `WillowOnlineGameSettings` (with and without the local player). Record the answer's class and its
  `GameSettingsCfgList[0].GameSettings`.
- Call the controller's `OnlineSub.GameInterface.GetGameSettings` with `Game`, and record whether it is None
  (open question 1).
- Hook `UpdateOnlineGameSettings` during a mission accept. Record which settings object it uses and the integer
  properties before and after.
- Call `FindDataStore` with None and with an unknown tag; both should be None.
