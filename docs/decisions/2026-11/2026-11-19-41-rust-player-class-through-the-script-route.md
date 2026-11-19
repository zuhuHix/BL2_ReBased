## 2026-11-19: The player's class through the script route; `GetDefinition` and `LoadPlayerPawnDataAsync` in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane FIX-PLAYER-CLASS (implementer; slice link 1). This lane implemented
from `docs/verification/NATIVE_PLAYER_CLASS.md` (analyst lane LINK-1-NEXT, decision 2026-11-19-31) alone. It opened
no raw analysis output, Ghidra or REA. That note lands with the analyst's branch, which has to land first.

- **The harness input (hub decision).** In Off, the `maya` step first calls the controller's installed
  `ServerPlayerSelectClass("GD_Siren.Character.CharClass_Siren")`, once. This is the class-select call the note
  names as the route to Maya's class other than a save. It is a labelled input of the combat run, not a save, and it
  goes through the game's own script.
  - The call runs through a `PlayerBridge::select_player_class` hook, which does nothing by default and which the
    mission bridge implements (`ow-cli`'s `MissionBridge`).
  - A controller that already has a `PlayerClass` is left as it is.
  - Reference is unchanged: the hook is not called there.
- **`GBXDefinition.GetDefinition` (Off, `ow-world` `world/class_route.rs`).**
  - No class, or an empty name: None.
  - An object of exactly that class at that full path in memory: that object.
  - Failing that, the definition store's load of the path. That this counts as the engine's "load by name" is this
    lane's reading, UNVERIFIED.
  - `NotYet { NotRead }`:
    - the retry, whose fixed text is not read;
    - a bare name with no `.`, because how the lookup "in any package" resolves it is not read, and the store's own
      resolution of a bare name is the port's identity policy;
    - an object of another or a derived class at the path, because the lookup's class filter is not read.
- **`WillowPlayerPawnDataManager.LoadPlayerPawnDataAsync` (Off, same module).**
  - No package part: nothing, no report.
  - In memory (by `FindAlreadyLoadedObject`'s lookup, now shared): the report at once to a context implementing
    `IStreamingDataEvent`. The context's `RunStreamingDataEvent` gets the event name, the payload, the found object,
    the int and the float, checked against the declared parameter kinds first.
  - `NotYet`: not in memory (the engine flag, the queue and the later report are not read), and a context the VM
    finds not implementing the interface (a "no" is not proven outside `ow-vm`, decision 2026-11-15-20).
  - The interface path, `GearboxFramework.IStreamingDataEvent`, was read from the installed package's declaration.
- **The class switch after an asynchronous report is not driven here.** Per the note as fixed in #422 (head 87602e3):
  the not-found path arms no retry timer; the report reaches the controller's script `RunStreamingDataEvent`, which
  only stores the pending class and archetype (`SetPendingClassSwitch`); each `SwitchPlayerClass` looks the archetype
  up again; and what carries the switch on after the report is not read. This lane codes only the already-loaded
  path, and the not-found path stays `NotYet` before anything is queued or reported.
- **Reference.** Both natives stay unbound there: the C++ has no stand-in. They are registered by the pawn-data pack,
  outside Reference only.
- **Verified (automated, local).** Four `ow-world` tests on an invented `GearboxFramework` package:
  - `GetDefinition`'s in-memory, load-by-name and NotYet cases;
  - the immediate report with its argument order;
  - no package part, not loaded, and a context without the interface;
  - Reference unbound.
- **Installed runs, Off.**
  - Before: `--combat-run maya:8` and the slice gate's links 1, 3, 4, 5, 13 and 14 stopped at `FindAlreadyLoadedObject`
    with an empty path (the controller had no `PlayerClass`).
  - After: the class select runs, `GetDefinition` finds `CharClass_Siren`, and the installed `SwitchPlayerClass` sets
    it. Those links now stop at `FindAlreadyLoadedObject` of the class's streaming pawn archetype
    (`GD_Siren_Streaming.Pawn_Siren`). A package holds it, but nothing has loaded it; that case was already `NotYet`
    (decision 2026-11-19-24). Links 10 to 12 are unchanged (`AIComponent.ActivateEvent`).
- **Not committed: an experiment for the hub.** With the archetype preloaded through the definition store (a second
  harness input, which this lane did not add), the switch takes the note's found branch. It then stops at the unbound
  `WillowPlayerController.ApplyCharacterClassStartingValues(object)` (NATIVE_COMBAT_GROUNDWORK.md section 1.10).
- **UNVERIFIED.** Every rule above. Nothing was checked in the running game.
