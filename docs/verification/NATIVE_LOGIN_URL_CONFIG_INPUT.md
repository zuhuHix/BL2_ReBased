# Native rules after the login check: UpdateURL, SaveConfig and UpdateInputContext (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-LINK1-AFTER-LOGIN (decision 2026-11-19-NNN, number to be given
by the hub).
- **Script first:** the callers were read in the installed packages with `research/script_disasm.py`, and every script
  function of the nine code packages was scanned once for the readers named below (a local helper over the same
  reader; ten functions do not decode, listed in section 5).
- **The three natives** were then read from the game executable with Ghidra under the machine lock:
  `tools/ghidra/run.ps1`
  for `UpdateURL` and `SaveConfig`, and a throwaway local helper for `UpdateInputContext`, whose body sits behind the
  controller class's own table (section 3).
- **Field names** come from the cooked packages (`tools/ghidra/class_layout.py`).
- **Own words only:** no decompiler output, pseudo-code, addresses, offsets or listing structure is reproduced here.
  Raw output stays under ignored `local/analysis/link1login/`.
- **No live session** was run. **Every rule is UNVERIFIED in the running game.**

**Clean room.** This lane implements nothing. Lane H6 implements from this note alone.

**Why it matters.** With #479 and H6's engine object (the port has no online subsystem, so the slot is empty and the
player is "not logged in"), slice links 1, 3, 4, 5, 13 and 14 stop at `PlayerController.UpdateURL`. `Object.SaveConfig`
comes next, then `WillowPlayerController.UpdateInputContext`, which the logged-in branch reaches too.

## Summary

| Native | Does | Writes | Confidence | Off answer |
|---|---|---|---|---|
| `PlayerController.UpdateURL(NewOption, NewValue, bSaveDefault)` | adds the option to the game engine's last URL; with the flag, saves it as a default-player option in the config | `GameEngine.LastURL`'s options; with the flag, the config (cache and file) | medium | nothing, under the read-back check of 1.3 |
| `Object.SaveConfig()` | saves the object's config properties to its class's config | the config (cache and file) | medium (outline) | nothing, under the read-back check of 2.3 |
| `WillowPlayerController.UpdateInputContext()` | raises the player input's "update the input context" flag | `WillowPlayerInput.bUpdateInputContext` | medium-high | the rule of 3.2 |

## 0. Where the slice reaches them (script)

`PlayerController.SetName(S)` (NATIVE_LOGIN_STATUS.md section 6.2) ends, after `ServerChangeName(NewName)`, with:
1. `UpdateURL('Name', NewName, true)`. `NewName` is `S`, or the online nickname when the login test of `SetName`
   replaced it; with no online subsystem it is `S`;
2. `SaveConfig()`, on the controller.

`SetName` is reached from `WillowPlayerReplicationInfo.ServerSetPlayerNameAsCharName`, from
`PlayerReplicationInfo.IsInvalidName` and from `WillowPlayerController.HandleLoginStatusChange` (the script scan). It
does nothing with an empty name. Then `OnPlayerClassChange` runs `SetupInputDevices`, which clears
`InputContextSelector` and calls `UpdateInputContext` (NATIVE_INIT_PLAYER_SKILL_TREE.md section 4, step 4).

`UpdateInputContext` has many more script callers: the walking and driving states' `BeginState`, cinematic mode, the
action skill's start and clean-up, and, on links 13 and 14's path, the injured state's `BeginState` and `EndState`,
`StartInjuredDeathSequence`, `StartInjuredPreDeathSequence`, the death camera's end and the awaiting-respawn views. The
rule of section 3 is the same for each. It supersedes NATIVE_GO_DOWN_PATH.md section 3.2 (#489), which lists this
native as not resolved.

## 1. PlayerController.UpdateURL(NewOption, NewValue, bSaveDefault)

### 1.1 What it does

1. When the engine object is not a game engine (a `GameEngine`, by fit: the test is a class check on the global engine
   object), it does nothing.
2. Otherwise it forms the option text `NewOption=NewValue` and adds it to the game engine's `LastURL` (a script-visible
   `URL` struct whose options are its `Op` array). Whether an option with the same key is replaced or a second one
   appended was **not read**: the add is a helper whose body was not followed.
3. When `bSaveDefault` is true (the slice's call passes true), it also saves the `NewOption` key of the last URL into
   the
   config, in the `DefaultPlayer` section of a config file named by a global that was not identified. That save is a
   helper that was not followed. By its purpose it writes the player's own config on disk.
4. It returns nothing and raises no event.

### 1.2 Writes and loads

- **Writes:** `GameEngine.LastURL`'s options (in-world, script-visible); with the flag, the config cache and the game's
  config file (outside the world: the player's files).
- **Loads:** none seen. Whether the config save first loads the file into the cache was not read.

### 1.3 Off answer: nothing, under a named read-back check

**Off answers nothing: no write, `None` returned.** The port must never write the player's config file (the files stay
byte-identical, as for real-game sessions), and the add rule of step 2 is not read, so neither write is made. That is
allowed only because nothing on the slice path reads either value back. The **read-back check**, which the
implementer keeps as a test or a recorded census:
1. **No script reads `GameEngine.LastURL`.** The scan of every decodable script function of the nine code packages finds
   no reference to it.
2. **The script readers of URL options are travel or restart only.** The natives that read URL options or the saved
   default-player options are `PlayerController.GetDefaultURL`, `WorldInfo.GetLocalURL`, `WorldInfo.GetAddressURL`,
   `Actor.GetURLMap` and `Engine.BrowseToURL`. Their only script callers are `GameInfo.ProcessServerTravel`,
   `GameInfo.RestartGame` and the autotest manager's map travel. None is on the slice path.
3. **None of those natives is bound in the port** (a search of `crates/` at a0d40f5), and the port reads no config file:
   class defaults come from the cooked packages, and no crate reads an `.ini`. So a later step that did reach a reader
   would stop on the unbound native, not read a value the "nothing" answer skipped.

If any of the three stops holding (a slice step that travels, a binding for one of those readers, or a config layer),
the answer becomes `NotYet`, kind `LayerAbsent`, "PlayerController.UpdateURL: the engine URL and the config layer".

**Before the call (preflight):** nothing to check. The answer writes nothing and cannot refuse.

## 2. Object.SaveConfig()

### 2.1 What it does (outline)

It saves the object's config properties (the properties flagged as config) to its class's config section:
1. when the object's class is not a config class, it does nothing. `PlayerController` is one (`config(Game)`);
2. the section is the class's, or, for a per-object-config class, one per object (a class default object uses the
   class's section);
3. each config property is written as a key of that section. A static array writes one key per element. Properties
   flagged as global config take a separate branch.

Steps 2 and 3 are an outline of a long routine. The key formats, the global-config branch, dynamic arrays and whether
and when the file is written out were **not read in detail**.

### 2.2 Writes and loads

- **Writes:** the config cache and, by the native's purpose, the class's config file on disk (the player's own
  files; the flush was not read). No script-visible property of the object changes.
- **Loads:** whether the config file is first loaded into the cache was not read.

### 2.3 Off answer: nothing, under a named read-back check

**Off answers nothing**, and the port never writes a config file. The **read-back check**:
1. **In the game, saved config is read back only by a config load:** a class load, a per-object-config object's
   construction, or an explicit reload. The slice path makes none of these for `PlayerController`'s config.
2. **The port reads no config file** (no crate reads an `.ini` at a0d40f5): class defaults come from the cooked
   packages. No port path can read the save back.
3. **No script calls `Object.GetPerObjectConfigSections`** (the script scan), the one script-visible reader of config
   sections.

The other script callers of `SaveConfig` are settings and binding changes (`Input.SetBind`, the `PlayerInput` invert
toggles, `WillowSystemSettings.UpdateSystemOption`, `WillowPlayerController.SetHand`, the HUD toggles,
`Console.ConsoleCommand`, `AccessControl.KickBan`, `PlayerController.SaveActorConfig`). The same answer holds for each
while the check holds. If it stops holding (a config layer), the answer becomes `NotYet`, kind `LayerAbsent`,
"Object.SaveConfig: the config layer".

## 3. WillowPlayerController.UpdateInputContext()

### 3.1 What it does

The native hands to the controller class's own implementation (an overridable native). The project's class-table query
cannot resolve it (NATIVE_GO_DOWN_PATH.md 3.2). Here it was resolved by following the constructor that
`WillowPlayerController`'s class registration hands over, to the class table that constructor installs (medium-high:
one table matched, and the behaviour below agrees with the script that consumes it). The implementation:
1. takes the controller's `PlayerInput`;
2. when it is a `WillowPlayerInput` (the cast's class is by fit: the flag below is that class's), it sets that input's
   `bUpdateInputContext` to true;
3. otherwise it does nothing.

It writes nothing else, raises no event and loads nothing.

**The flag's identity** is by fit, not by layout: the layout tool cannot lay out `PlayerInput` (a map property stops
it). The bit the native sets matches `bUpdateInputContext` among `WillowPlayerInput`'s declared bools. The script
confirms the role: `WillowPlayerInput.PlayerInput` (the input's per-frame event) tests `bUpdateInputContext`, calls the
controller's `SelectInputContext` (script), and clears the flag when that answers true.

So the context change is deferred to the next input frame: `SelectInputContext` makes the class's input context selector
on first use, picks the context (`DefaultCinematic` while button input is ignored, else the selector's
`SelectContext`) and hands it to `WillowPlayerInput.SetInputContext`. With no `PlayerInput`, or no player class,
`SelectInputContext` answers false and does nothing. None of that runs inside `UpdateInputContext` itself.

### 3.2 Off answer

1. Read the controller's `PlayerInput`.
2. None, or not a `WillowPlayerInput` (nor below it): return, no write. This is the native's own answer, not a stand-in,
   so it needs no read-back check. The headless controller of the slice has no `PlayerInput`, so this is the slice's
   case
   on every path that reaches the native (after the `InputContextSelector` clear of `SetupInputDevices`, and on links 13
   and 14's injured and death calls).
3. A `WillowPlayerInput`: write `bUpdateInputContext` = true on it (a script-visible write;
   `PlayerController.PlayerInput`
   and the flag are declared properties). Nothing else.

**Preflight:** none needed. No path refuses. A world that declares `WillowPlayerInput` without `bUpdateInputContext`,
or not as a bool, is a data error (`VmError::Script`) at the write. That cannot happen on the installed packages.

## 4. What comes after (not read here)

On the login path, `OnPlayerClassChange` goes on after `SetupInputDevices` (NATIVE_INIT_PLAYER_SKILL_TREE.md section 4).
Its next native stop is whatever the first Off run past `UpdateInputContext` names.

## 5. What the script scan could not see

Ten script functions do not decode with the bytecode reader (a size mismatch, an object reference out of range, or a
jump into a statement). They were not scanned: `GameInfo.UpdateBestNextHosts`,
`WillowPlayerController.InnerSetOnlineStatus`, `CreditsDataProviderGFxObject.AppendNewCreditsDataToFront`,
`WillowGameInfo.CheckMapChangeConditions`, `FrontEndPlayerListGFxObject.RefreshPlayerList` and
`HandlePlayerDetailsButtonClick`, `StatusMenuExGFxMovie.GetHighestChainedPlotMissionCompleted`,
`WillowWeapon.InitMeshAnimation`, `WillowGFxColiseumOverlayMovie.extSetupResultsScreen` and
`WillowGFxUIManager.PlayMovie`. None of them is on the slice path as recorded so far. They are network, front-end,
menu and map-change code, and one weapon mesh set-up (`InitMeshAnimation`). By their names none reads URL options or
config, but that was not checked.

## Open questions

1. `UpdateURL`'s add: whether an existing option with the same key is replaced, and the exact config key and file of the
   default-player save.
2. `SaveConfig`'s key formats, the global-config rule and the flush.
3. Whether any C++ subclass of `WillowPlayerController` overrides `UpdateInputContext` (none was looked for).
4. The ten undecodable functions of section 5, once the bytecode reader handles them.

## Live checks that would settle each rule (bl-sdk, the RTSE way)

- **UpdateURL:** read `GameEngine.LastURL.Op` before and after a name change, and repeat the change to see whether the
  `Name` option is replaced or appended. Compare the `DefaultPlayer` section of the config file before and after, on a
  copy of the config, restoring the original afterwards.
- **SaveConfig:** on a copy of the config, call it on the controller and diff the file; restore it afterwards.
- **UpdateInputContext:** read `WillowPlayerInput.bUpdateInputContext` right after the call (expect true), and one input
  frame later (expect false, with the context changed by `SetInputContext`).
