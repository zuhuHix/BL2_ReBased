## 2026-11-19: Rust mission tracker objective progress in Off; the tracker's record queries read as analyst

AI-assisted (Claude Code). Lane FIX-MISSION-OFF (SLICE_RUN_PLAN.md step 4, links 10 to 12; owners `ow-mission` and
`ow-world`). Number 05 was given by the hub. No dependency, no CLI flag, no parsing change. There was no real-game
session; every rule here is **UNVERIFIED** in the running game. Reference (`OW_PARITY=reference`) is unchanged: it
keeps the C++ stub in each place (R3).

- **Where Off stood on main 86c28d00.** Both installed runs already pass the accept and the range objective (decision
  2026-11-16-81 bound `Actor.LocalPlayerControllers` and the lent weapon's region-stage rule). The error left in this
  lane's area was `MissionTracker.GetObjectivesProgress(object,array,bool)`, refused as unbound in Off in
  `--mission-run` (the bridge's accept) and in `--slice-run`. The whole `--slice-run` stops later, at `hit:fire`, on
  the Pawn IAIInterface refusal. That stop waits on a maintainer-approved live read and is not touched here.
- **1. `MissionTracker.GetObjectivesProgress`, implemented in Off** from NATIVE_SLICE_OFF_STOPS.md, the note of a
  different lane (FIX-SLICE-OFF-1, decision 2026-11-16-81). This lane did not open the executable for it, and the code
  was written before this lane's own analyst work below. `ow-mission` (`bridge`), outside Reference only, on the
  bridge's tracker:
  - another mission has no record: the out array is left as passed;
  - the bridge's mission: the array becomes one value per entry of the mission's `ObjectiveDefs`, each the record's
    stored progress (the count, or the mask of a bit-mask objective, NATIVE_MISSION_DISPATCH.md B1; 0 for an objective
    without progress or in no set). A new `MissionSystem::stored_progress` gives that value;
  - with the flag set, the note zeroes an all-or-nothing objective that fails the objective's "is complete" test. That
    test is listed under "Not read" in the note. So an entry of a `bAllOrNothing` objective that is not already 0 is
    `NotYet { NotRead }`; an entry that is 0 stays 0 whatever the test says.
  - The installed controller script reaches it from `AddMission` (flag omitted) and from
    `SetPlayersMissionObjectiveProgressToWorldsMissionObjectiveProgress` (flag true). On the Fire route neither run
    raises the `NotYet`.
- **2. Analyst only (clean room): NATIVE_MISSION_OBJECTIVES.md.** The next Off stop, `MissionTracker.GetActivePrimaryObjectiveSet`,
  had no record. This lane read it, and the two other tracker queries of the Fire turn-in, `GetAllMissions` and
  `AllExpansionSideMissionsComplete`, in the executable (Ghidra, read-only, two queries, raw output under ignored
  `local/analysis/`). It wrote the own-words note and three NATIVE_INDEX rows. It implements none of them: they stay
  unbound in Off (the gameplay stub policy refuses them), and `--slice-run`'s Reference allow list keeps their C++
  stubs. Only their reason text changed. A fresh implementer codes them from the note alone.
- **Verified (automated, local, pc-hummer).**
  - Synthetic tests (invented packages; `bridge/tests.rs`): `objectives_progress_in_off_mode` covers no record, before
    accept, the values per `ObjectiveDefs` entry, the flag with an all-or-nothing entry at 0, and the `NotYet` with one
    in progress. `objectives_progress_is_unbound_in_reference_mode` checks that Reference leaves the native unbound.
    The toy fixture gained `ObjectiveDefs`, `bAllOrNothing` and a declared native with an `array<int>` out parameter.
    The default mission is unchanged.
  - Installed `--mission-run` on the Fire mission (`stage:8 player:8:2000 script:accept tick:0.5 script:screen
    script:turnin pool`), before (main 86c28d00) and after:
    - Off: errors before `GetObjectivesProgress`, after `GetActivePrimaryObjectiveSet`; the step records equal;
    - Reference: stdout byte-identical.
  - Installed `--slice-run`, Off, the verify tool's steps through `tick:1` after `spawn` (12 steps, the last before the
    Pawn refusal):
    - the 12 step records equal before and after;
    - errors before: `TriggerDetachSprites`, `AIComponent.ActivateEvent`, `GetObjectivesProgress`;
    - errors after: `TriggerDetachSprites`, `AIComponent.ActivateEvent`, `GetActivePrimaryObjectiveSet`.
  - The full 22-step Off `--slice-run`: exit 3 at the Pawn IAIInterface refusal, stdout byte-identical before and
    after.
- **Next stops** (Off), for the hub:
  - a fresh implementer for NATIVE_MISSION_OBJECTIVES.md (`GetActivePrimaryObjectiveSet` first);
  - the analyst question the note leaves open: the "is complete" test of `GetObjectivesProgress`'s flag;
  - `TriggerDetachSprites` and `AIComponent.ActivateEvent` (NATIVE_SLICE_OFF_STOPS.md; not this lane's owners);
  - the Pawn IAIInterface refusal (a maintainer-approved live read).
