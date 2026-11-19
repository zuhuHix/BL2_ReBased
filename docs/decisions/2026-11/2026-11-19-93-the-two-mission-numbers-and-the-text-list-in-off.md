## 2026-11-19: The two mission numbers and the mission screen's text list in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-MISSION-NUMBERS, implementer.

- **Sources:** `docs/verification/NATIVE_MISSION_NUMBERS.md` (#473, its head eb24ea43 with the hub's ruling on the
  screen; analyst lane ANALYST-MISSION-NUMBERS, decision 2026-11-19-86, not this thread). No native code or analysis
  output was opened.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged rules:** no dependency was added, no bounds check was loosened and no existing `NotYet` guard was
  relaxed.
- **The base:** stacked on `fix/rust-settings-fill` (#464, 223a2ced).

**What changed** (`crates/ow-world/src/world/mission_numbers.rs`, bound by `datastore::register`, Off only; Reference
unchanged).

- **`GetActiveMissionNumber`** (section 1):
  - It takes the level's `WorldInfo.GRI` as a `WillowGameReplicationInfo`, then its `MissionTracker`, and asks the
    tracker's `IsDataValid()` through the VM.
  - The tracked `ActiveMission` answers its `MissionNumber`, with no status check.
  - Otherwise the tracker's `GetAllMissions` list is scanned. Only plot-critical, non-DLC missions count, each graded
    through `GetMissionStatus`: the first in progress answers; else the largest Complete; else 0.
  - Any failure of the first step, or a -1 answer, takes the fallback.
  - The note's engine-wide condition is taken to hold (open question 1).
- **`GetActivePlotCriticalMissionNumber`** (section 2):
  - A negative or omitted index is the controller's `GetCurrentPlaythrough()`, called through the VM. A class without
    it answers `NotYet { NotRead }`, "the controller's current playthrough query".
  - An index out of range answers 0.
  - Otherwise that playthrough's `MissionList` is scanned the same way, graded by each record's own `Status`.
- **Both:** they write nothing, so nothing is preflighted or rolled back. A tracker or controller function's own
  answer (a `NotYet` or a refusal) propagates unchanged; inside the settings refresh, the refresh's write mark
  (decision 2026-11-19-80) makes it terminal. Missing or mistyped fields, and a `None` in the tracker's list, are a
  `VmError::Script` naming the field.
- **`QuestAcceptGFxMovie.UpdateMissionTextList`** (section 3, the hub's ruling on UI-only side effects):
  - With `MissionTextList` None (the port's stand-in screen): nothing. Labelled a port Off answer for a headless world
    with no Flash movie hosted, UNVERIFIED.
  - Read-back check (the note's, over every decoded script function): no script reads `MissionCategories`. The one
    reader of `MissionTextList` that links 10 to 12 can reach, `GetSelectedIndex`, reads None whether or not the
    native ran.
  - With the list set: `NotYet { LayerAbsent }`, "the mission screen's Flash text list (not hosted)", marked "stopped
    after partial writes" since it comes after the accept's writes, its kind kept.
  - `GetVariableObject` and `FocusOn`, which `SetFocus` calls next, are not bound here. In Off they would be the next
    stop once the accept gets that far.
- **`datastore::UNREAD`** keeps only the DLC manager's two queries. Two refresh-mark tests of #464 used
  `GetActiveMissionNumber` as their unread native; they now expect the toy's playthrough-query `NotYet`, which is
  marked or unmarked in the same way.
- **`ow-cli`'s `REFERENCE_STUBS`:** the `UpdateMissionTextList` entry says Off binds it.

**Installed Off `--slice-gate`:** links 10 to 12 get past both mission numbers. They still stop in `accept`, now at
`MissionTracker.GetAllMissions`'s own `LayerAbsent` (the bridge holds one record, NATIVE_MISSION_OBJECTIVES.md),
marked "stopped after partial writes". The bridge does not set the tracker's `ActiveMission`, so the scan asks for the
list. `UpdateMissionTextList` is not reached yet.

**Tests:** 10 new tests on invented packages in `mission_numbers_tests.rs`, covering:
- the tracked mission, the scan's first in-progress and largest-complete answers, and the empty scan;
- the five fallback causes, the playthrough index rules and the missing query;
- a tracker `NotYet` passed through, bad data, and both text-list cases.
