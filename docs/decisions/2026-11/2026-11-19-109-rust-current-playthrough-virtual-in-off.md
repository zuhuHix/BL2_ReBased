## 2026-11-19: The current-playthrough virtual and the engine-wide condition, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-PLAYTHROUGH-VIRTUAL, slice links 10 to 12. Coded from
docs/verification/NATIVE_PLAYTHROUGH_QUERY.md alone, the note at 3bb3f6de (clean room: lane LINK-10-MISSIONLIST wrote
it, decision 2026-11-19-99, PR #486). The executable and the analysis output were not opened. Synthetic fixtures only.
Based on main d72b96d4 (#480 and #486 landed).

- **`WillowPlayerController.GetCurrentPlaythrough()` as a rule** (`ow_world::world::mission_numbers::current_playthrough`),
  the note's section 4 steps in order, with exact-class tests:
  1. a controller whose class is not exactly `WillowPlayerController`: `NotYet` (`NotRead`), "a subclass override of
     the controller's current-playthrough query";
  2. and 3. no world, or no `WorldInfo` in the world's actor list: 0;
  4. the `WorldInfo`'s `GRI` None, or not a `WillowGameReplicationInfo`: 0;
  5. a `GRI` of a strict subclass: `NotYet` (`NotRead`), "a subclass override of the replication info's
     current-playthrough answer";
  6. the `GRI`'s `CurrentPlaythrough`, an int or a byte. Any other kind is a `VmError::Script`, not 0.

  It reads nothing of the controller but its class, so every exactly-`WillowPlayerController` controller gets the
  world's answer. It writes nothing and loads nothing, since these are run-time objects.
- **The mission bridge's Off binding** (`n_playthrough`) now answers that rule on any controller, reading the world's
  `WorldInfo.GRI`, not the bridge's own replication object. Reference is unchanged: the bridge's controller only, and
  "outside the binding" on any other. The bridge's internal reads (its playthrough record and its reward experience) still use
  the bridge's own current playthrough, its replication object: the same object as the world's `GRI` today. Only the
  binding follows the note (review r1 of #497).
- **Direct callers.** `GetActivePlotCriticalMissionNumber` with a negative or omitted index uses the rule as a function
  on the controller, not a VM dispatch of `GetCurrentPlaythrough`. `MissionTracker.AllExpansionSideMissionsComplete`
  has no Off body in the tree yet; when it gets one, it calls the same function.
- **The engine-wide condition** of `GetActiveMissionNumber` is the port's world existing: with the world's `WorldInfo`,
  the tracker branch alone answers; without it, the fallback.
  - #480's both-branch comparison and its `ENGINE_GATE` `NotYet` are retired, with the `Reads` switches
    (`engine_gate` is now the Off behaviour), `PLAYTHROUGH_QUERY` and the tests' `TestReads`.
- **Tests.**
  - `mission_numbers_tests.rs`:
    - the rule's steps (no world, no `GRI`, another class, a strict subclass, a wrong kind, two controllers);
    - another controller class and a subclass;
    - the natives reading the world's playthrough;
    - the tracker branch alone with a world;
    - the post-load mark, now on the tracker list's `None` entry.
    - review r2 of #480's L1 test (a load inside the dispatched fallback marking the outer gate `NotYet`) is
      rewritten: no outer refusal is left after the fallback. With the world, the tracker answers and the fallback's
      load never runs; without it, the fallback loads and answers.
  - `datastore_tests.rs`: the refresh-mark tests use a strict-subclass controller, whose `NotYet` is now their
    post-write refusal.
  - The bridge: in Off the world's `WorldInfo.GRI` points at a second replication object with another playthrough,
    and both the bridge's and another controller get the world's value. A strict-subclass controller gets the
    controller-override `NotYet` (`NotRead`). Reference is unchanged.
  - NATIVE_INDEX.md's three rows and NATIVE_MISSION_NUMBERS.md's both-branch bullets (now retired history) follow
    this decision (review r1 of #497).
- **Installed `--slice-gate`, Off, strict: 0 ok / 12 not yet / 2 error, every row byte-identical to the base.**
  - Link 10 passes 3 of its 16 driver steps (`stage:8`, `player:8:21000`, `use`) and stops at `accept`. The stop is
    "stopped after partial writes: MissionTracker.GetAllMissions": needs the tracker's full mission list (the bridge
    holds one record). That is the note's section 3 prediction.
  - Links 11 and 12 are blocked behind link 10.
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note: subclass overrides of
  either virtual (open question 1), the global's identity in every build state (2), how the tracker's `MissionList` is
  built (3), and run-time mission numbers (4).
