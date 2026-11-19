## 2026-11-19: The tracker's mission list build in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-MISSIONLIST-BUILD, implementer. The decision number is pending from the
hub (three digits); NNN is a placeholder.

- **Sources:** `docs/verification/NATIVE_MISSION_LIST_BUILD.md` (analyst lane ANALYST-MISSIONLIST-BUILD, 4b1a2003,
  not this thread) and the hub's three rulings of 2026-11-19. No native code or analysis output was opened.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged rules:** no dependency was added, no bounds check was loosened and no existing `NotYet` guard was
  relaxed.
- **The base:** the note #493 at its review r1 (5b880555, on main 6f9789b after #480 and #486 landed). The refusal
  texts follow that fix: "a reference the set-up did not load" and "the visit order of equal
  mission names".

**The hub's rulings, each a port Off answer, UNVERIFIED:**
1. **When:** once, at the mission bridge's world set-up, right after the tracker is made and before any script runs or
   reads the list. A second build request is `NotYet { NotRead }`.
2. **Which definitions:** every export whose class is exactly `WillowGame.MissionDefinition`, with `Default__` objects
   left out, in every package the store has loaded when the build starts. Each is loaded there as its own definition.
   - This is the port's stand-in for the note's "every `MissionDefinition` in memory".
   - The load is part of set-up, before any script runs, so it is not a mid-call write.
   - No subclass of `MissionDefinition` is declared in the installed WillowGame package.
   - A definition the build reads that is still unloaded after set-up (a dependency, a chain member, an expansion) is
     `NotYet { NotRead }`, before any write.
3. **The order:** the engine's name-table order is approximated by first insertion, then the instance number.
   - Each name text gets the index of its first appearance when the loaded packages' name tables are read in load
     order, each table in its own order.
   - Names the engine registers itself come first in the game. The port has no list of them, and none is taken to name
     a mission.
   - Loaded packages with no missions still count toward the name indices; the key is the object name.
   - The order is fitted, not proven, and it is not a `NotYet`. Two definitions with the same object name and number
     are `NotYet { NotRead }`, "the visit order of equal mission names": where the game's binary search puts the
     second was not read.

**What changed:**
- **`crates/ow-mission/src/mission_list.rs`** (new; Off only, Reference builds nothing) does the build and the
  numbering, as the note's sections 2 and 3 give them:
  - per definition, in list order: `bGameStageLocked` cleared, `MissionNumber` -1, `GameStage` 0, then a zeroed record
    (status 0) with its `MissionDef`;
  - then the numbering: each expansion group from its `MissionNumberBase` (the base game from 0), the main chain, the
    side missions' anchors and buckets, the placement inside a bucket, the second pass, and the numbers.
  - **Before the first write:** every read, every check and the whole numbering are computed first, so every
    `NotYet` and every bad-data error leaves nothing changed. Bad data is a `VmError::Script`: a missing or mistyped
    field, a `NextMissionInChain` loop, or a cycle among side missions' dependencies. The second pass's edge case (a
    dependency that stands after its mission) is `NotYet { NotRead }`. A dependency or chain target that is not a
    `MissionDefinition` is bad data.
  - **After the first write:** anything that fails is terminal. It goes through `ow_world::pools::after_partial_writes`
    (prefixed, kind and record kept), and nothing is rolled back.
  - The writes go to the world's own definition objects (its VM's object space), so nothing leaks across worlds.
  - A tracker class that declares no `MissionList` builds nothing; the installed packages declare it, but invented
    package sets in other crates' tests do not.
- **`crates/ow-mission/src/bridge.rs`:**
  - runs the build in `MissionScript::new`, in Off;
  - `MissionTracker.GetAllMissions` appends each record's `MissionDef` to the caller's array, in the list's order. A
    world whose list was not built keeps the old `LayerAbsent`;
  - `build_mission_list` is the second-build refusal;
  - a `NotYet` of the set-up build does not fail the set-up (the hub's steer). The bridge records it, kind and record
    kept, and `GetAllMissions` answers with that same `NotYet` at the first list read. Other errors fail the set-up.
- **`crates/ow-package/src/store.rs`** (a sensitive crate): one read-only accessor, `PackageStore::loaded_count`, so the
  build can read the loaded packages in load order. No parsing or bounds behaviour changed.

**Installed Off `--slice-gate`:** links 10 to 12 get past `GetAllMissions`. They still pass 3 of 4 driver steps, and now
stop in `accept` at "stopped after partial writes: the controller's current-playthrough virtual" (`NotRead`, decision
2026-11-19-93). Every other link is as on the base 1f43a95b (verified with a base run): 0 ok / 5 NotYet / 9 errors
both times.

**Tests:** 10 tests on invented packages in `mission_list_tests.rs`, covering:
- the main chain, the side buckets and the unnumbered cases;
- the resets, the records and the first-insertion order;
- an expansion group, loaded or not;
- both cycle kinds as bad data, with nothing written;
- the second build's `NotRead`, the second pass, and equal names' `NotRead`;
- the second pass's edge (`NotRead`, nothing written) and a non-mission dependency target (bad data).

The bridge's `GetAllMissions` test now checks the built list and the second build's refusal. A new bridge test checks
that a set-up refusal is answered, unchanged, by the first list read.

**Open item (hub ruling of 2026-11-19, not now):** a `DlcExpansion` (or any other reference) that set-up did not load
stays `NotYet { NotRead }`, "a reference the set-up did not load", as the note says. The installed slice does not reach one. Adding expansion packages
to the set-up's load list is a later lane.

**Review r1 of #496, folded in.**
1. The set-up build is recorded as run whatever its outcome, so every later request is the second-build `NotRead`, also
   after a set-up refusal (tested). The request method runs in Off only.
2. `OUTSIDE` is gone. A loaded `Dependencies` or `NextMissionInChain` target that is not a `MissionDefinition` is bad
   data naming the field (section 4); a target missing from the list is inconsistent data.
3. The hub's ruling: the candidate loads count as writes. The equal-key check runs on the candidates' keys before any
   load; every `NotYet` from the first load on goes through `after_partial_writes`. A candidate the store cannot find
   is a `VmError::Script`, not a silent drop.
4. The not-loaded refusal reads "a reference the set-up did not load".
5. An edge test, and the test counts corrected.
