# Native and script route of the player's class: how a player controller gets its `PlayerClass` (2026-11-19)

AI-assisted (Claude Code). Lane LINK-1-NEXT, analyst only (clean room, docs/LANE_BRIEF.md; decision 2026-11-19-31). Script
read with `research/script_disasm.py` / `ow-package --disasm`; natives read locally from `Borderlands2.exe` with the project's
Ghidra tooling (read-only queries and a local read-only table reader; raw output only under ignored `local/analysis/`), in
our own words. Nothing here was observed in the running game: **UNVERIFIED** until it is. The lane that wrote this note does
not implement it; the natives below stay `NotYet` in Off until a fresh implementer codes them.

## Why: link 1's stop

Measured on a scratch merge of `origin/main` with `fix/rust-pawn-data-manager` and `fix/rust-mind-adopt` (lane FIX-OFF-STOCK's
branches, not landed at the time), installed game, Off:

- `ow-package --slice-gate`: strict 0 ok / 11 `NotYet` / 3 errors of 14. Links 1, 3, 4, 5, 13 and 14 all stop at
  `--combat-run`'s `maya:8`, on `WillowPlayerPawnDataManager.FindAlreadyLoadedObject` with an **empty path**: the mission
  bridge's player controller has **no `PlayerClass`**, so `PlayerClass.PawnArchetypePath` reads empty. Links 10 to 12 stop
  at `AIComponent.ActivateEvent(name)` (noted, NATIVE_SLICE_OFF_STOPS.md).
- `--combat-run` without the bridge (`player:8`, the W6-COMBAT spawns): every spawn stops at `WillowPawn.SetDefaultPhysics`
  (noted in NATIVE_MIND_ROUTE.md, decision 2026-11-19-25, and being implemented by FIX-OFF-STOCK), ahead of the hold and
  travel stops behind it.

So link 1's next step is not an unread native but a missing input: the class the game gives the player controller. This note
reads where that comes from, and the two natives on the way that had no note.

## 1. Where a player controller's class comes from (script)

- **`WillowPlayerController.ClientApplySaveGame`**: with a cached save the player may use, the character data is loaded
  through the pawn data manager (section 3) and the save's `PlayerClassDefinition` becomes the pending class switch when the
  load reports back. **Without a save**, it calls `ServerPlayerSelectClass("")`.
- **`ServerPlayerSelectClass(ClassName)`**: the class is `GBXDefinition.GetDefinition(PlayerClassDefinition, ClassName)`
  (section 2). When that finds none and the controller has no `PlayerClass` yet, it is
  `GetDefinition(PlayerClassDefinition, DefaultPlayerCharacterClassName)`, a controller field whose class default is
  `GD_Soldier.Character.CharClass_Soldier` (read from the package). Then `SwitchPlayerClass(class)`.
  `PlayerSelectClass` (the character select) reaches the same function with the chosen name.
- **So a fresh start without a save gets the Soldier class, not Maya's.** Maya's class comes from a save (decision D9) or a
  class-select call naming `GD_Siren.Character.CharClass_Siren`. Which input the slice uses is the hub's or the maintainer's
  call, not a native rule.
- **`SwitchPlayerClass(NewPlayerClass)`**, only on the authority and with a class:
  1. a class different from the current one clears `bLoadingCharacterPackageForClassSwitch` and becomes both `PlayerClass`
     and `Controller.CharacterClass`;
  2. the pawn archetype is `FindAlreadyLoadedObject(PlayerClass.PawnArchetypePath)` as a `WillowPlayerPawn` (noted in
     NATIVE_PAWN_DATA.md);
  3. **found**: `ClientNotifyClassChanged(PlayerClass)`; `ApplyCharacterClassStartingValues(CharacterClass)` (the player
     controller's override, NATIVE_COMBAT_GROUNDWORK.md section 1.10); with a pawn already, that pawn's inventory is
     discarded and the pawn destroyed, and the controller marks a respawn from the class change; the game info's
     `RestartPlayer(Self)` (script) runs with its `bRestartLevel` set around the call; when the controller then has a pawn,
     the replication info's save flag is handled, `ClientSetHUD` gets the game info's `HUDType` and
     `PendingClassSwitchPawnArchetype` is cleared; with no pawn after the restart, `RetryClassSwitch` is timed for 0.1 s;
  4. **not found**, and no load already pending: `bLoadingCharacterPackageForClassSwitch` is set and
     `LoadPlayerPawnDataAsync(PawnArchetypePath, Self, PlayerClass, Self, 0, 0.0, 'SwitchPlayerClass')` (section 3) is asked
     for it.
- **The load's report** reaches the controller's script `RunStreamingDataEvent` (`IStreamingDataEvent`): for the event
  `SwitchPlayerClass` (and for `RestartPlayerAfterLogin` and `ClientApplySaveGame`) it calls `SetPendingClassSwitch(class,
  loaded pawn archetype)`, which only stores both into `PendingClassSwitchDefinition` and `PendingClassSwitchPawnArchetype`.
  The retry timer then calls `SwitchPlayerClass(PlayerClass)` again, which now finds the archetype loaded.

## 2. `GBXDefinition.GetDefinition(DefinitionClass, DefinitionName)` (native, static)

Parameters from the package: a class, a string; returns an object. In own words (high for the branches, medium for what each
engine helper is, by its arguments and use):

- No class, or an empty name (compared without case): None.
- Otherwise an object of that class with that name, among the objects already loaded in any package.
- Failing that, the object is loaded by that name.
- Failing that, one more attempt: something named from a fixed text is loaded first (a package, by its use; which one was
  not separated), and the load by name is tried again.
- The result is the object found or loaded, or None. No class check beyond the lookup's own class filter was seen.

For the implementer: in Off, "loaded" can be the definition store's objects; whether a store load counts as the engine's
"load by name" is the implementer's question, and the retry step stays `NotYet` until its text is read.

## 3. `WillowPlayerPawnDataManager.LoadPlayerPawnDataAsync(...)` (native)

Parameters from the package, in order: `PawnDataPath` (string), `PlayerContext`, `Payload`, `Instigator` (objects),
`IntParam` (int), `FloatParam` (float), `CallbackCustomEventName` (name); no result. The script native passes all seven to one
virtual of the manager, read in the manager's own function table (from its class registration's constructor). In own words
(medium):

1. The package part of `PawnDataPath` is the text before the first `.` (none when the path has no `.`).
2. The manager looks for the object already loaded at the path: the same routine `FindAlreadyLoadedObject` uses (high: the
   two reach one slot of the manager).
3. **Found**: the report is sent at once. When `PlayerContext` implements `IStreamingDataEvent` (high: the interface class is
   named by its registration), its `RunStreamingDataEvent` gets `CallbackCustomEventName`, `Payload` (as the event's
   instigator), the found object (as the other participant), `IntParam` and `FloatParam`. `Instigator` is not passed. A
   context without the interface gets no report.
4. **Not found, with a package part**: a request holding all seven values is kept. Under one engine flag (not identified)
   the object is loaded at once and reported as in step 3; otherwise the request is queued for an asynchronous load of the
   package, and the report comes later (the queue's processing was not read).
5. **Not found, no package part**: nothing happens, and no report is sent.

For the slice, when Maya's pawn archetype is already loaded, step 3 applies and the class switch continues synchronously.

## Not read

- The fixed text of `GetDefinition`'s retry, and whether the engine's "load by name" there can fail silently.
- The engine flag of section 3 step 4 and the queue's processing (the asynchronous report).
- Where the controller's `ClientApplySaveGame` runs on a fresh start (the login path; NATIVE_MAP_LOAD.md section 4).

## Live confirmation (bl-sdk)

In Sanctuary with a save loaded: read the controller's `PlayerClass` and `PendingClassSwitchPawnArchetype` around the load;
hook `RunStreamingDataEvent` and log its event name and arguments; call `GBXDefinition.GetDefinition` with Maya's class
path and with an empty name.
