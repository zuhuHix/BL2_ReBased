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
unchanged). **Conservative form** (the hub's heads-up of 2026-11-19, while #473's last review round runs): what rests
on an unresolved read, or on a "no read-back" argument, is a named `NotYet`. The rules behind the two unresolved reads
are coded and unit-tested behind a `Reads` switch, which the natives pass as nothing read.

- **`GetActiveMissionNumber`** (section 1):
  - In Off: `NotYet { NotRead }`, "GetActiveMissionNumber's engine-wide condition (an unresolved global)", before
    anything is read (open question 1).
  - Behind the switch, taken as read: the level's `WorldInfo.GRI` as a `WillowGameReplicationInfo`, its
    `MissionTracker` and the tracker's `IsDataValid()`, through the VM.
    - The tracked `ActiveMission` answers its `MissionNumber`, with no status check.
    - Otherwise the tracker's `GetAllMissions` list is scanned. Only plot-critical, non-DLC missions count, graded
      through `GetMissionStatus`: the first in progress answers; else the largest Complete; else 0.
    - A failure of the first step, or a -1 answer, takes the fallback.
- **`GetActivePlotCriticalMissionNumber`** (section 2):
  - An explicit, non-negative index is answered: out of range is 0; otherwise that playthrough's `MissionList` is
    scanned the same way, graded by each record's own `Status`.
  - A negative or omitted index needs the controller's current-playthrough virtual, which the note does not resolve:
    `NotYet { NotRead }`, "the controller's current playthrough query" (open question 2). Behind the switch, it is
    `GetCurrentPlaythrough()` through the VM.
- **Both:** they write nothing, so nothing is preflighted or rolled back. A tracker or controller function's own
  answer propagates unchanged. Inside the settings refresh, its write mark (decision 2026-11-19-80) makes these
  `NotYet`s terminal, prefixed, with kind and record kept. Missing or mistyped fields, and a `None` in the tracker's
  list, are a `VmError::Script` naming the field.
- **The mission screen** (section 3), whose stand-in is reached in the port only after the accept's writes:
  - `QuestAcceptGFxMovie.UpdateMissionTextList`: `NotYet { LayerAbsent }`, "the mission screen's Flash text list
    (not hosted)", with the list None or set. The note's "nothing" with no list rests on a read-back check that is
    under review, so it is not taken. It is marked "stopped after partial writes", kind and record kept.
  - The focus pair `SetFocus` calls next, `GFxMoviePlayer.GetVariableObject` and `WillowGFxMovie3D.FocusOn`: the same
    `LayerAbsent`, "the mission screen's Flash focus", never the note's None and nothing. On the mission screen it is
    marked. On another movie nothing is known about earlier writes, so it is not marked.
  - What the accept checks after the screen returns is not read, and nothing here answers for it.
- **`datastore::UNREAD`** keeps only the DLC manager's two queries. Two refresh-mark tests of #464 used
  `GetActiveMissionNumber` as their unread native; they now expect its engine-wide `NotYet`, marked or unmarked as
  before.
- **`ow-cli`'s `REFERENCE_STUBS`:** the three screen entries say Off answers a named `NotYet`.

**Installed Off `--slice-gate`:** links 10 to 12 stop in `accept` at "stopped after partial writes:
GetActiveMissionNumber's engine-wide condition (an unresolved global)" (`NotRead`). With the condition taken as read
(the earlier form of this branch), they stopped at the bridge's `MissionTracker.GetAllMissions` `LayerAbsent`, since
the bridge never sets the tracker's `ActiveMission`.

**Tests:** 11 new tests on invented packages in `mission_numbers_tests.rs`, covering:
- the tracked mission, the scan's first in-progress and largest-complete answers, and the empty scan;
- the five fallback causes, the playthrough index rules and the missing query;
- a tracker `NotYet` passed through, and bad data;
- the natives' two `NotRead`s, and the screen's marked and unmarked `LayerAbsent`s.
