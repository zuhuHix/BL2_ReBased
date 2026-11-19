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
     `WillowOnlineGameSettings`. With no store or no settings, the script does nothing
     more (the Off contract for this case is section 5).

No gameplay state is read back from these settings on the accept path.

## 2. UIInteraction.GetDataStoreClient()

A static native. It answers the data store client held by the engine's UI interaction object: the one the game
viewport's UI controller holds. When the engine, its viewport or that UI object is missing, it answers the client of
the `UIInteraction` class default object instead, and None only when that is missing too. A UI object that is present
but holds no client gives None: the fallback is taken only when the object itself is missing. It changes nothing.

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
  among the client's global data store classes (next to the online game search store). It is not a player data store
  class.
- **Its defaults** (installed package): `Tag` is `WillowOnlineGameSettings`; `GameSettingsCfgList` has one entry,
  whose `GameSettingsClass` is `WillowGame.WillowOnlineGameSettings` and whose `GameSettings` and `Provider` are empty;
  `SelectedIndex` is 0.
- **Inference (a), not read:** that `FindDataStore` returns an object of that class. The configuration only lists the
  class. Which object answers depends on what the client registered at run time and on the player-store search
  coming first (section 3), neither of which was read.
- **Inference (b), not read:** who fills `GameSettings`. Public UE3 has the online game settings store create a
  settings object of `GameSettingsClass` for each entry when it is initialised. This executable's version was not
  read, so it may differ (open question 2).

## 5. For an implementer: the contract for Off (hub ruling, 2026-11-19)

The hub ruled that the path is modelled faithfully: our host has no online subsystem, which is a fact about our
engine, not a game rule.

**The gate, before any lookup** (`SetNeedsUpdateOnlineGameSettings` and `UpdateOnlineGameSettings`): the controller's
role, `PlayerController.IsPrimaryPlayer` (already bound by the mission bridge) and `IsLocalPlayerController` (Off
since decision 2026-11-19-15).

**Branch 1, the online subsystem** (first in section 1, step 2). With no online subsystem in the world, there are no
`Game` settings from it, and the refresh takes branch 2. This is not a `NotYet`.

**Branch 2, the data store:**
- `GetDataStoreClient`: Off answers the world's one modelled client. While no lane models a client, it stays the
  named `NotYet` it is today (layer absent, decision 2026-11-19-19), never None. The class-default fallback of
  section 2 is out of scope for Off: the port has no engine without a viewport, so the world's client is the answer.
- `FindDataStore`: section 3, over the modelled client's player and global stores.
- **The settings entry** (`GameSettingsCfgList[0].GameSettings`): Off does not assume it is empty. Unless a later read
  names what fills it in a world with no online subsystem (open question 2), reading it is a named `NotYet` that
  names open question 2. It is never "empty, so return with no writes".

**After the settings are found**, the writes of section 1 step 2, in order. Each is a named `NotYet` in Off until
it is read, unless the index already has an Off rule for it:
1. the level: the player replication info's `ExpLevel` plus the game replication info's `OverpowerLevelModifier`, then
   `UpdateExpLevel`;
2. `GetActiveMissionNumber` (NATIVE_CONTROLLER_HELPERS.md), then `UpdateActiveMission`;
3. `GetActivePlotCriticalMissionNumber` (NATIVE_SAVE_LOAD.md), then `UpdateActivePlotMission`;
4. `WillowGameReplicationInfo.GetCurrPlaythrough`, then `UpdatePlaythrough`;
5. `GameEngine.GetDLCManager`; only with a manager, `HasCompatibilityData` and `StaticGetDlcContentIdForCurrentMap`,
   then `UpdateMapDlcContentId` and `UpdateDlcFlag`.

Each `Update...` is script that writes one integer property through `Settings.SetIntProperty` (a native, not read:
a named `NotYet`). The push to the online subsystem is not reached on this path (section 1, step 2).

## Open questions

1. **UNVERIFIED:** whether, in a running single-player game, the online subsystem already answers game settings
   named `Game` (a session exists), so that the data store route is never taken.
2. **UNVERIFIED:** whether the online game settings store's `GameSettings` is filled when the engine starts (section 4),
   and by what.
3. Settled by the hub (2026-11-19): the path is modelled faithfully (section 5).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- In single player, read the engine's UI interaction `DataStoreClient`, then call `FindDataStore` with
  `WillowOnlineGameSettings` (with and without the local player). Record the answer's class and its
  `GameSettingsCfgList[0].GameSettings`.
- Call the controller's `OnlineSub.GameInterface.GetGameSettings` with `Game`, and record whether it is None
  (open question 1).
- Hook `UpdateOnlineGameSettings` during a mission accept. Record which settings object it uses and the integer
  properties before and after.
- Call `FindDataStore` with None and with an unknown tag; both should be None.
