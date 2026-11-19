## 2026-11-19: Player class route: GetDefinition and LoadPlayerPawnDataAsync, link 1's missing PlayerClass (analyst)

AI-assisted (Claude Code, Opus 5.5). Lane LINK-1-NEXT, **analyst only** (clean room, docs/LANE_BRIEF.md). No crate code
changed. The two natives read here stay `NotYet` in Off until a different thread implements them. There was no real-game
session: everything is **UNVERIFIED** in the running game.

**The stops, measured.** The run was on a scratch merge of `origin/main` with FIX-OFF-STOCK's `fix/rust-pawn-data-manager`
and `fix/rust-mind-adopt` (neither landed at the time), installed game, Off.

- `--slice-gate` strict: 0 ok / 11 `NotYet` / 3 errors of 14.
- Links 1, 3, 4, 5, 13 and 14 stop at `maya:8` on `FindAlreadyLoadedObject` with an empty path. The mission bridge's
  controller has no `PlayerClass`.
- Links 10 to 12 stop at `AIComponent.ActivateEvent` (noted).
- Without the bridge, every spawn stops at `WillowPawn.SetDefaultPhysics`, which is noted (decision 2026-11-19-25) and with
  FIX-OFF-STOCK.

**What changed (docs only).**

- `docs/verification/NATIVE_PLAYER_CLASS.md`, new.
  - **Section 1, the installed script route of the player's class.**
    - Without a save, `ClientApplySaveGame` calls `ServerPlayerSelectClass("")`. That falls back to the controller's
      `DefaultPlayerCharacterClassName`, whose class default is the Soldier's class, not Maya's, and then
      `SwitchPlayerClass`.
    - `SwitchPlayerClass` sets `PlayerClass` and `CharacterClass`. With the pawn archetype already loaded, it notifies,
      applies the starting values and restarts the player through the game info's script. Otherwise it asks the pawn
      data manager to load it.
    - The report comes back through the controller's script `RunStreamingDataEvent`, which only stores the pending
      class and archetype (`SetPendingClassSwitch`). What carries the switch on from there is not read (UNVERIFIED);
      the not-found path arms no retry timer.
    - **Maya's class needs an input**: a save (D9) or a class-select call with her class. That choice is the hub's or
      the maintainer's, not a native rule.
  - **Section 2, `GBXDefinition.GetDefinition`.** No class or an empty name gives None. Otherwise it returns a loaded
    object of that class by name in any package, else one loaded by name, else it loads once more after loading
    something named by a fixed text (not separated).
  - **Section 3, `WillowPlayerPawnDataManager.LoadPlayerPawnDataAsync`.**
    - When the object is already loaded (the same lookup `FindAlreadyLoadedObject` does), the report is sent at
      once through the context's `IStreamingDataEvent.RunStreamingDataEvent` (name, payload, object, int, float).
    - When it is not loaded and the path has a package part, the request is kept and the object is loaded at once or
      queued, depending on an unidentified engine flag.
    - With no package part, nothing happens.
- `docs/verification/NATIVE_INDEX.md`: two rows.

**Second pass, after #410 and #412 landed** (main 388a55ee plus the unlanded `fix/rust-pawn-data-manager` 56737e3c).

- **The stops are unchanged.** `--slice-gate` strict is still 0 / 11 / 3, with the same link 1 stop. `maya:8` stops the
  same way. Without the bridge, every spawn still stops at `SetDefaultPhysics`, noted and with FIX-MIND-ROUTE.
- **One more native question answered, behind `SetDefaultPhysics`** on links 3 and 13: the hit's `DemigodOnHold` on a
  `WillowAIComponent`, which Off refuses as an unread subclass override.
  - `WillowAIComponent` keeps `AIComponent`'s hold behaviour for the target, movement, demigod, god and dialog kinds;
    only the base kind differs.
  - So NATIVE_AI_HOLDS.md's rule applies to that class for those kinds. The note and its index row say so.
  - The earlier attempt (decision 2026-11-19-06) had not settled it.

**Tools.** `research/script_disasm.py` and `ow-package --disasm` for the script. Ghidra 12.1.4, read-only, one run at a
time under the machine lock (waiting whenever the project was held): 7 query runs and 3 runs of the local read-only table
reader. `class_layout.py` was used for the controller's interface field; its fitted total equals the class's registered
size. Raw output only under ignored `local/analysis/link1/`; run outputs under `local/p2/link1-next/`.

**Open.**

- `GetDefinition`'s retry text; the asynchronous queue and its flag.
- Where `ClientApplySaveGame` runs on a fresh start.
- The hub's choice of Maya's class input for the bridge.
- bl-sdk reads listed in the note.
