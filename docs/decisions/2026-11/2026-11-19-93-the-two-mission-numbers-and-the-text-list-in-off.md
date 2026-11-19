## 2026-11-19: The two mission numbers and the mission screen's text list in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-MISSION-NUMBERS, implementer.

- **Sources:** `docs/verification/NATIVE_MISSION_NUMBERS.md` (#473 at 0a32108b, its review r1; analyst lane
  ANALYST-MISSION-NUMBERS, decision 2026-11-19-86, not this thread). No native code or analysis
  output was opened.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged rules:** no dependency was added, no bounds check was loosened and no existing `NotYet` guard was
  relaxed.
- **The base:** stacked on `fix/rust-settings-fill` (#464, 223a2ced).

**What changed** (`crates/ow-world/src/world/mission_numbers.rs`, bound by `datastore::register`, Off only; Reference
unchanged). **Conservative form** (the hub's heads-up of 2026-11-19, while #473's last review round runs): what rests
on an unresolved read, or on a "no read-back" argument, is a named `NotYet`. A `Reads` switch lets the tests run the
rules with the two unresolved reads taken as read; the natives pass it as nothing read.

- **`GetActiveMissionNumber`** (section 1):
  - The inputs: the level's `WorldInfo.GRI` as a `WillowGameReplicationInfo`, and its `MissionTracker`. Without
    them, the fallback answers.
  - The engine-wide condition is not guessed (open question 1; the note's r1). With a tracker, both branches are
    computed, the tracker branch first. Equal answers give the number; different ones are `NotYet { NotRead }`, "the
    engine-wide condition of GetActiveMissionNumber". A failing branch is the answer, unchanged.
  - The tracker branch, through the VM: `IsDataValid()` false is the fallback.
    - The tracked `ActiveMission` answers its `MissionNumber`, with no status check.
    - Otherwise the tracker's `GetAllMissions` list is scanned. Only plot-critical, non-DLC missions count, graded
      through `GetMissionStatus`: the first in progress answers; else the largest Complete; else 0.
    - A -1 answer takes the fallback.
    - A `None` entry in the list is `NotYet { NotRead }`, "a None entry in the tracker's mission list".
- **`GetActivePlotCriticalMissionNumber`** (section 2):
  - An explicit, non-negative index is answered: out of range is 0; otherwise that playthrough's `MissionList` is
    scanned the same way, graded by each record's own `Status`.
  - A negative or omitted index needs the controller's current-playthrough virtual, which the note does not resolve:
    `NotYet { NotRead }`, "the controller's current playthrough query" (open question 2). Behind the switch, it is
    `GetCurrentPlaythrough()` through the VM.
- **Both:** they write nothing, so nothing is preflighted or rolled back. A tracker or controller function's own
  answer, an unbound native's `UnboundNative` included, propagates unchanged. Inside the settings refresh, its write
  mark (decision 2026-11-19-80) makes these `NotYet`s terminal, prefixed, with kind and record kept. Missing or
  mistyped fields are a `VmError::Script` naming the field.
- **The mission screen** (section 3), whose stand-in is reached in the port only after the accept's writes. The note's
  r1 labels its "nothing" and None answers as port Off answers, with a wider read-back check. Under the hub's heads-up
  they stay named `NotYet`s until its last review round settles them:
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

**Installed Off `--slice-gate`:** links 10 to 12 get past the two numbers' entry and stop in `accept` at "stopped after
partial writes: MissionTracker.GetAllMissions", the bridge's own `LayerAbsent` (the bridge holds one record and sets no
`ActiveMission`). That is the tracker branch's failure, the stop the note's r1 expects. The mission screen is not
reached yet.

**Tests:** 12 new tests on invented packages in `mission_numbers_tests.rs`, covering:
- the tracked mission, the scan's first in-progress and largest-complete answers, and the empty scan;
- the five fallback causes, the playthrough index rules and the missing query;
- a tracker `NotYet` passed through, and bad data;
- the two branches agreeing, differing and failing, the natives' unread playthrough virtual, and the screen's marked
  and unmarked `LayerAbsent`s.
