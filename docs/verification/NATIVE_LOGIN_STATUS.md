# Native and script rules of the login status: UIInteraction.GetLoginStatus and IsInvalidName (draft, 2026-11-19)

AI-assisted (Claude Code), analyst lane ANALYST-LOGIN-STATUS (decision 2026-11-19-89). It covers slice link 1's likely
next stop after `InitPlayerSkillTree`. NATIVE_INIT_PLAYER_SKILL_TREE.md (#466) section 4 has
`ApplyCharacterClassStartingValues` reach `UIInteraction.GetLoginStatus` through `IsInvalidName`, for a local player.

**How it was read.**
- No bl-sdk session was granted, and the recorded evidence has no capture of the login status.
- The script was read from the installed packages with `research/script_disasm.py`.
- The two natives on the path were read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`,
  under the machine lock). The Steamworks virtual was found through the class's registration and vtable.
- The list of natives came from `tools/ghidra/script_natives.py`, field names and offsets from
  `tools/ghidra/class_layout.py`, and the enum names from the cooked packages.

Written in our own words, as behaviour only: no decompiler output, pseudo-code, addresses, offsets or call structure
is reproduced here. **Every rule is UNVERIFIED in the running game.** The last section lists the live observations
that would confirm them.

**Clean room.** This lane implements nothing. Everything here stays unported, or a named `NotYet` in Off, until a fresh
implementer codes it from this note alone.

## Summary

| Piece | Kind | Confidence | Status |
|---|---|---|---|
| `UIInteraction.GetLoginStatus(ControllerId)` | **script** (static, final; not a native) | high | UNVERIFIED |
| `GameEngine.GetOnlineSubsystem()` | native (static, final) | high | UNVERIFIED |
| `OnlineSubsystemSteamworks.GetLoginStatus(LocalUserNum)` | native, through a C++ virtual | high | UNVERIFIED |
| `WillowPlayerReplicationInfo.IsInvalidName` and the base `PlayerReplicationInfo.IsInvalidName` | script | high | UNVERIFIED |
| What the two `IsInvalidName` branches reach next (`ServerSetPlayerNameAsCharName`, `PlayerController.SetName`) | script | medium (read cheaply) | UNVERIFIED |
| The login status a local, offline, single-player Maya gets in the shipped game | not read: it depends on Steam's state | — | open question 1 |

## 1. The login status enum

`OnlineSubsystem.ELoginStatus`, as the packages declare it: 0 `LS_NotLoggedIn`, 1 `LS_UsingLocalProfile`, 2
`LS_LoggedIn` (and `LS_MAX`).

## 2. UIInteraction.GetLoginStatus(ControllerId) (script)

A static script function of `UIInteraction`, not a native. In order:
1. The answer starts at 0 (`LS_NotLoggedIn`).
2. When `ControllerId` is -1, it answers 0.
3. It asks `GameEngine.GetOnlineSubsystem()` (section 3). With no subsystem, it answers 0.
4. It reads the subsystem's `PlayerInterface` (an `OnlinePlayerInterface` interface field of `OnlineSubsystem`). When
   that is empty, it answers 0.
5. Otherwise it answers that interface's `GetLoginStatus`, with `ControllerId` cast to a byte (the low 8 bits).

It writes nothing.

## 3. GameEngine.GetOnlineSubsystem() (native)

- It takes the engine object. When that is a `GameEngine`, it answers the engine's `GameEngine.OnlineSubsystem`
  field, as stored (None included). When the engine is not a `GameEngine`, it answers None.
- It writes nothing.
- `PlayerController.GetOnlineSubsystem` (script) is a caching wrapper of the same call on the controller's
  `OnlineSub`. The login path of section 2 calls the engine's native directly, not that wrapper.

## 4. OnlineSubsystemSteamworks.GetLoginStatus(LocalUserNum) (native)

- The interface's script declaration (`OnlinePlayerInterface.GetLoginStatus`) has no body of its own. For the
  Steamworks subsystem the call is the native `OnlineSubsystemSteamworks.GetLoginStatus`, which hands the byte to a
  C++ virtual of the subsystem.
- **The rule:** when `LocalUserNum` (the byte, widened) equals the subsystem's `LoggedInPlayerNum` (an int field),
  the answer is the subsystem's `LoggedInStatus` byte field, as stored. For any other number the answer is 0
  (`LS_NotLoggedIn`).
- It writes nothing.
- **Not read here:** who sets `LoggedInStatus` and `LoggedInPlayerNum` (the subsystem's login handling), and which
  object the subsystem stores in its `PlayerInterface` field (open question 2).

## 5. IsInvalidName (script), as ApplyCharacterClassStartingValues reaches it

`OnPlayerClassChange` calls it on the controller's replication info and ignores the result (NATIVE_INIT_PLAYER_SKILL_TREE.md
section 4, step 3).

**`WillowPlayerReplicationInfo.IsInvalidName`:**
1. It casts the info's `Owner` to a `WillowPlayerController`. With no such owner, or an owner whose
   `IsLocalPlayerController` answers false, the answer is false and nothing else is reached.
2. With a local owner, it asks `UIInteraction.GetLoginStatus` (section 2). The id it passes is the owner's
   `PlayerController.Player` cast to a `LocalPlayer`, then that player's `ControllerId`.
   - With no local player, the script's read of `ControllerId` through a None reference gives 0, the int default (the
     VM's usual accessed-None answer).
   - So the id is 0, not -1, and section 2 goes on to step 3.
3. **Status 0** (`LS_NotLoggedIn`): it answers the result of its own `ServerSetPlayerNameAsCharName`, called with the
   owner and the owner's `PlayerPreferredCharacterName` (section 6.1).
4. **Any other status:** it answers the base `PlayerReplicationInfo.IsInvalidName` (below).

**The base `PlayerReplicationInfo.IsInvalidName`:**
1. It asks `GameEngine.GetOnlineSubsystem()`; with none, false.
2. The owner cast to a `PlayerController`; with none, false. With the controller's `bOverrideOnlineProfileName` set,
   false.
3. The controller's `Player` cast to a `LocalPlayer`; with none, false. Then the subsystem's `GameInterface` and
   `PlayerInterface` must both be set; otherwise false.
4. The `PlayerInterface`'s `GetLoginStatus` for that player's `ControllerId` (as a byte) must be at least 1
   (`LS_UsingLocalProfile` or `LS_LoggedIn`); otherwise false.
5. Then it reads the interface's `GetPlayerNickname` for that id. For Steamworks that is script and answers the
   subsystem's `LoggedInPlayerName`, whatever the id.
6. When that name differs from the info's `PlayerName`, it calls the controller's `SetName` with it (section 6.2) and
   answers true. When it is the same, false.

## 6. What the two branches reach next (read cheaply)

1. **`WillowPlayerReplicationInfo.ServerSetPlayerNameAsCharName(PC, NewPreferredName)`** (script, a server function):
   - it stores `NewPreferredName` in `PC.PlayerPreferredCharacterName`;
   - it asks `PC.GetPlayerUIPreferences` (script) for the character name. That is the preferred name when it is not
     empty, otherwise `GetCharacterClassDefaultName` (script). It reads the preferred colours too, unused here;
   - when that name differs from the info's `PlayerName`, it calls `PC.SetName` with it (6.2) and answers true;
     otherwise false.
2. **`PlayerController.SetName(S)`** (script; `WillowPlayerController` does not override it):
   - with an empty `S`, nothing happens;
   - unless `bOverrideOnlineProfileName` is set, and when the controller has a `LocalPlayer`, both of its
     `OnlineSub`'s game and player interfaces are set, the login status is at least 1 and the game interface has
     game settings named `Game`: `S` is replaced by the player interface's nickname;
   - then it calls `ServerChangeName(S)` (script, a server function: with a non-empty name it asks the game info's
     `ChangeName(Self, S, true)`, not read here);
   - then it updates the URL option `Name` with `S` and saves the config (a native of `Object`). Those are not read
     here.
3. **After `IsInvalidName`**, `OnPlayerClassChange` runs `SetupInputDevices` (NATIVE_INIT_PLAYER_SKILL_TREE.md
   section 4, step 4):
   - it calls the native `WillowPlayerInput.SetupInputDevices` when the controller's `PlayerInput` is a
     `WillowPlayerInput`;
   - then it clears `InputContextSelector` and calls the native `WillowPlayerController.UpdateInputContext`.

   Neither native is read here. They are the stops after this one when a player input object exists.

## 7. The Off answers, step by step (for an implementer, under the hub's rulings)

The hub's rulings:
- everything that can refuse is decided before the first write;
- a refusal after a write is terminal: its text starts "stopped after partial writes", and its kind and record are
  kept;
- there is no rollback, and other errors pass unchanged.

Sections 2 to 5 write nothing, so every answer on the login path is decided before any write of its own. The first
writes are in section 6, after the status is known.

| Step | Off answer |
|---|---|
| `WillowPlayerReplicationInfo.IsInvalidName`: owner cast, `IsLocalPlayerController` | the script, as installed (the VM runs it; `IsLocalPlayerController` is already Off, decision 2026-11-19-15) |
| the id: `Player` cast to `LocalPlayer`, `ControllerId` | the script as installed: with no local player, the VM's accessed-None answer, 0 |
| `UIInteraction.GetLoginStatus` | the script, as installed: it is not a native, so there is nothing to bind |
| `GameEngine.GetOnlineSubsystem()` | the engine's `OnlineSubsystem` field, read when the port's engine object is a `GameEngine`; None when it is another engine. **While the port has no engine object or no online subsystem: a named `NotYet` (layer absent) naming the online subsystem**, never None. None would send the slice down the `LS_NotLoggedIn` branch, a choice the shipped game's Steam state does not make (open question 1) |
| the subsystem's `PlayerInterface` | a plain read; empty gives 0 (section 2) |
| `OnlineSubsystemSteamworks.GetLoginStatus` | the rule of section 4: `LoggedInStatus` when the byte equals `LoggedInPlayerNum`, else 0. Both fields are plain reads. **Which values a headless world gives them is not read: a named `NotYet` (not read) naming the subsystem's login state** until a lane decides how the port's subsystem is logged in (open question 1) |
| another subsystem class behind `PlayerInterface` | a named `NotYet` (not read) naming the class (only the Steamworks rule was read) |
| status 0: `ServerSetPlayerNameAsCharName`, `GetPlayerUIPreferences`, `GetCharacterClassDefaultName` | the script, as installed. Its first write is `PlayerPreferredCharacterName`; any `NotYet` it meets after that (in `SetName`) is "after a write" |
| status at least 1: the base `IsInvalidName`, `GetPlayerNickname` | the script, as installed. `GetPlayerNickname` reads `LoggedInPlayerName`; no write before `SetName` |
| `PlayerController.SetName`, `ServerChangeName`, the URL update, the config save | **not read here.** The URL update and the config save would write outside the world (the player's settings). A port lane decides them; until then, a named `NotYet` (not read) naming `SetName`'s URL and config writes, decided before `ServerChangeName` runs |
| `SetupInputDevices` next | the script, as installed; its two natives are the next stops (section 6.3) |

**The slice minimum.** On the headless world the first answer is the online subsystem's `NotYet` (layer absent), at
`GameEngine.GetOnlineSubsystem`. That is before any write of `IsInvalidName`. Since `OnPlayerClassChange` has written
before it (the skill tree, the name and movement type of #466's section 4), the link's stop text carries "stopped
after partial writes", with its kind kept.

## Open questions

1. **UNVERIFIED:** the login status a local, offline, single-player Maya gets: `LoggedInStatus` and
   `LoggedInPlayerNum` of the shipped game's Steamworks subsystem. It is likely `LS_LoggedIn` for player 0 with Steam
   running, but that was not read, and it decides which `IsInvalidName` branch runs.
2. **UNVERIFIED:** which object `OnlineSubsystem.PlayerInterface` holds for the Steamworks subsystem, and who sets
   it.
3. **UNVERIFIED:** `PlayerController.SetName`'s URL update and config save, and the game info's `ChangeName`. Not read.
4. **UNVERIFIED:** `WillowPlayerInput.SetupInputDevices` and `WillowPlayerController.UpdateInputContext`, the next
   natives on the path. Not read.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Read `GameEngine.OnlineSubsystem` (its class), its `PlayerInterface`, `LoggedInStatus`, `LoggedInPlayerNum` and
  `LoggedInPlayerName`, in the main menu and in Sanctuary, offline and with Steam online. Record Maya's `LocalPlayer`
  `ControllerId`.
- Hook `UIInteraction.GetLoginStatus` and `OnlineSubsystemSteamworks.GetLoginStatus` during a class change (a new
  character, or a save load). Record the argument and the answer each time. Expect the subsystem's `LoggedInStatus`
  for an id equal to `LoggedInPlayerNum`, and 0 for any other id.
- Hook `WillowPlayerReplicationInfo.IsInvalidName`, `ServerSetPlayerNameAsCharName` and `PlayerController.SetName`.
  Record which branch runs, and `PlayerName` and `PlayerPreferredCharacterName` before and after.
- Compare the info's `PlayerName` after the class change with `LoggedInPlayerName`. Equal values point at the base
  branch (status at least 1) renaming the player to the Steam name.
