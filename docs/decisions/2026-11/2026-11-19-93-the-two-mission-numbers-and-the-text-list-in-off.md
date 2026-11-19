## 2026-11-19: The two mission numbers and the mission screen's text list in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-MISSION-NUMBERS, implementer.

- **Sources:** `docs/verification/NATIVE_MISSION_NUMBERS.md` (#473 at f586ab9c, its review r2 and final round; analyst lane
  ANALYST-MISSION-NUMBERS, decision 2026-11-19-86, not this thread). No native code or analysis
  output was opened.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged rules:** no dependency was added, no bounds check was loosened and no existing `NotYet` guard was
  relaxed.
- **The base:** first stacked on `fix/rust-settings-fill` (#464); rebased onto main (d3d76fed) after #464 landed.

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
    `NotYet { NotRead }`, "the controller's current-playthrough virtual" (open question 2). Behind the switch, it is
    `GetCurrentPlaythrough()` through the VM.
- **Both:** they write nothing, so nothing is preflighted or rolled back. A tracker or controller function's own
  answer, an unbound native's `UnboundNative` included, propagates unchanged. Inside the settings refresh, its write
  mark (decision 2026-11-19-80) makes these `NotYet`s terminal, prefixed, with kind and record kept. Missing or
  mistyped fields are a `VmError::Script` naming the field.
- **The mission screen** (section 3), whose stand-in is reached in the port only after the accept's writes. The note's
  read-back check does not cover the 10 undecoded script functions or native readers, so no call is answered
  "nothing":
  - `QuestAcceptGFxMovie.UpdateMissionTextList`: `NotYet { LayerAbsent }`, "the mission screen's Flash text list
    (not hosted)", record NATIVE_GFX_BRIDGE.md, with the list None or set. The native does not mark it; the
    bridge's status hook does (review r2 of #480, below), kind and record kept.
  - Not reached while it refuses: `GetVariableObject`, "a Flash movie query (not hosted)", and `FocusOn`, "the 3D
    movie's Flash focus (not hosted)", each `NotYet { LayerAbsent }`, marked the same way at the hook, never by
    the native itself.
  - What the accept checks after the screen returns is the bridge's own code (the note's section 3, step 3); nothing
    here answers for it.
- **`datastore::UNREAD`** keeps only the DLC manager's two queries. Two refresh-mark tests of #464 used
  `GetActiveMissionNumber` as their unread native; they now expect its unread playthrough virtual (the toy has no
  level, so the fallback answers), marked or unmarked as before.
- **`ow-cli`'s `REFERENCE_STUBS`:** the three screen entries say Off answers a named `NotYet`.

**Installed Off `--slice-gate`:** links 10 to 12 get past the two numbers' entry and stop in `accept` at "stopped after
partial writes: MissionTracker.GetAllMissions", the bridge's own `LayerAbsent` (the bridge holds one record and sets no
`ActiveMission`). That is the tracker branch's failure, the stop the note's r1 expects. The mission screen is not
reached yet.

**Tests:** 15 new tests on invented packages in `mission_numbers_tests.rs` (and 2 in the bridge's tests), covering:
- the tracked mission, the scan's first in-progress and largest-complete answers, and the empty scan;
- the five fallback causes, the playthrough index rules and the missing query;
- a tracker `NotYet` passed through, and bad data;
- the two branches agreeing, differing and failing, the natives' unread playthrough virtual, and the screen's marked
  and unmarked `LayerAbsent`s.

**Rebase and the note's final round.** Rebased onto #464's accepted head (8ea5f6d5) with
`git rebase --onto origin/fix/rust-settings-fill e9bfd1d5`; GATES.md kept both entries. The final note (f586ab9c)
answers no screen call "nothing": `UpdateMissionTextList` is the named `LayerAbsent` with record NATIVE_GFX_BRIDGE.md,
`GetVariableObject` "a Flash movie query (not hosted)" and `FocusOn` "the 3D movie's Flash focus (not hosted)", each
marked on the mission screen. The playthrough `NotYet` is named "the controller's current-playthrough virtual". The
engine-wide condition keeps the both-branches rule, and the `None` entry its `NotRead`, as the note states.

**Review r1 of #480, folded in.**
1. **Lazy loads.** A field read on a store definition that `DefStore::is_unloaded` reports is loaded first, and the
   load counts as a write: it marks a running settings refresh, and a `NotYet` that leaves the native after it is
   marked "stopped after partial writes" once, kind and record kept. Tested with a genuinely unloaded synthetic
   export read from a loaded holder.
2. **The mark at the write boundary.** The screen natives no longer mark their own `NotYet` by the movie's class. The
   mission bridge's accept and turn-in mark a `NotYet` that leaves the controller's script after the mission's status
   changed, in Off only; with the status unchanged, or under Reference, it is recorded as before. Tested: a direct
   call on a fresh movie is not marked (`GetVariableObject` included), and a post-activation `NotYet` through the
   bridge is marked and terminal.
3. **The fallback is dispatched.** `GetActiveMissionNumber`'s fallback calls the controller's
   `GetActivePlotCriticalMissionNumber(-1)` through the VM, so an unbound callee is `VmError::UnboundNative`, unchanged
   (tested).

**Review r2 of #480, folded in.**
- **H1, the hook is the write boundary.** The screen's only script route is the controller's `UpdateMissionStatus`,
  which the bridge runs from the status routine's hook, a nested run after the status was written. The accept, the
  turn-in and the reward page (`ServerGrantMissionRewards`, from `UpdateMissionStatus`) all go through it. The hook now
  marks a `NotYet` "stopped after partial writes" (Off only, once, kind and record kept) before its own run records
  it, so the accept and the turn-in stop on it. The accept's and the turn-in's own frames keep their status-change
  mark. Tested through the bridge for the accept (status stays Active) and the turn-in (Complete), and unchanged under
  Reference.
- **L1.** The load count is kept on the VM, so a load inside the VM-dispatched fallback marks the outer call's later
  `NotYet` too (tested).
- **L2.** A handle whose load already failed is not counted as a new write (`DefStore::is_failed`, a read-only query
  added beside `is_unloaded`); the mark on a running refresh comes before the count.
- **L3.** This file, GATES.md and the index rows now name the hook as the marking boundary.
