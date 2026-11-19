## 2026-11-19: Rust player pawn data manager natives in Off

AI-assisted (Claude Code). Lane FIX-PAWN-DATA-B, implementer. Number 24 was given by the hub. The branch is `origin/main`
5ce1679a (first written on f10034aa, then rebased). The note, `docs/verification/NATIVE_PAWN_DATA.md` (decision
2026-11-19-18), was read from its branch at 71a93909 and is on main since #403; this lane does not change it. This lane did not
analyse the natives and opened no `local/analysis/`, Ghidra or REA output. No dependency, no CLI flag, no parsing
change. There was no real-game session; every rule is **UNVERIFIED** in the running game. Reference is unchanged:
neither native has a C++ stand-in, so both stay unbound there (R3).

- **New pack `ow_world::world::pawn_data`** (Off only), registered by `--combat-run` next to the skill manager's pack.
  - **`WillowGlobals.GetPlayerPawnDataManager()`**: the note's plain read of `ThePlayerPawnDataManager`. A None field
    is `NotYet { NotRead }` naming open question 1, because the game fills the field in a globals set-up routine whose
    call site is not found, and it is expected to have run before controller script.
  - **The set-up step (`pawn_data::set_up`)**: what the note reads of that routine, for this field.
    - A set field is kept.
    - An existing manager that the routine would pick is `NotYet { NotRead }` (open question 2, the engine check).
    - Otherwise a new manager is made from its class defaults, with no name, and stored. The note reads its outer as
      the engine's transient package (UNVERIFIED, question 2); the port has none, so it has no outer.
    - `--combat-run` calls it in Off where its bridge has made the globals object, before the player's script runs.
      That timing is open question 1, UNVERIFIED.
  - **`WillowPlayerPawnDataManager.FindAlreadyLoadedObject(Path)`**: the object already in memory at that full path,
    any class, never loading. The port reads "in memory" as an object live in the heap whose full path equals `Path`,
    in any case.
    - Nothing at that path: None.
    - A definition the store has made but not loaded: `NotYet { NotRead }`. Whether the game holds it depends on the
      unread `LoadPlayerPawnData` family; the note leaves this to the port.
    - A partial path: `NotYet { NotRead }` (open question 3).
    - An empty path: `NotYet { NotRead }`. The note reads it as a lookup of an empty name without saying what that
      finds, and None would run `BodySelectClass(None)`, which the note says the game is not expected to do here.
- **Where link 1 stops now.** Off `--combat-run maya:8` passes both getters and stops at `FindAlreadyLoadedObject("")`.
  The empty path comes from the script reading `PlayerClass.PawnArchetypePath` on the controller's own `PlayerClass`,
  which is None on the mission bridge's controller. The bridge sets it only around a level-up. In the game the
  player class is set before `ApplyCharacterClassDefaults` runs. Giving the bridge's controller its player class for
  the `maya` step is a separate change (the bridge's or `ow-combat`'s), not taken here. Open question 4 (the next
  native) therefore still waits.
- **Verified (automated, local, pc-hummer).**
  - **Synthetic tests (`ow-world`):**
    - the getter is `NotYet` before the set-up and the same object after it;
    - a set field is kept;
    - the set-up is `NotYet` with a manager already alive;
    - the lookup finds a run-time object at its full path, in any case;
    - None for an absent path; `NotYet` for a partial and for an empty path;
    - both natives are unbound in Reference.

    Not covered by a synthetic test: an unloaded definition handle.
  - **Installed `--combat-run maya:8`:**
    - Off, before (main f10034aa, and again on 519660eb): `unbound native WillowGlobals.GetPlayerPawnDataManager()`,
      exit 1;
    - Off, after: the `NotYet` at `FindAlreadyLoadedObject` with the empty path, exit 1;
    - Reference: equal before and after, apart from timing fields.
  - **`--slice-gate`** (`feature/rust-slice-run-off` b456f983 merged into main f10034aa, in a throwaway worktree, with
    this change applied for "after"), strict grading:
    - before: 0 ok / 5 notyet / 9 error;
    - after: 0 / 11 / 3;
    - links 1, 3, 4, 5, 13 and 14 move from the unbound getter (error) to the `FindAlreadyLoadedObject` `NotYet`;
    - links 10 to 12 stay at `AIComponent.ActivateEvent` (error); the five driverless links are unchanged.
- **Records.** NATIVE_INDEX: the "Implemented" column of the note's two rows.
- **After the rebase onto 5ce1679a** (#405's `ow-world` natives merged beside these): workspace tests 1,950 passed /
  0 failed / 43 ignored, `ow-world` 101 / 0 / 5, `crate_edges_test.py` OK, and Off `maya:8` still stops at the empty
  `FindAlreadyLoadedObject` path.
- **Gates** (on 519660eb): workspace build clean; workspace tests 1,917 passed / 0 failed / 43 ignored; `ow-world`
  99 passed / 0 failed / 5 ignored; `crate_edges_test.py` OK; decisions check OK; Fire checker in Reference: 33 of 33
  rules.
