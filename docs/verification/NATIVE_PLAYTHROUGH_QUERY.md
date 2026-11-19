# Native rules of the current playthrough and the engine-wide condition: WillowPlayerController.GetCurrentPlaythrough (draft, 2026-11-19)

AI-assisted (Claude Code), analyst lane LINK-10-MISSIONLIST (decision 2026-11-19-102, filed as 2026-11-19-99), for slice links 10 to 12 (the
Fire mission's accept, objective and turn-in). It settles the two named `NotYet`s that NATIVE_MISSION_NUMBERS.md (#473)
leaves open, open questions 1 and 2:
- the engine-wide condition of `GetActiveMissionNumber`;
- the controller's current-playthrough query.

It also records why these two answers alone do not move links 10 to 12, and what does.

**How it was read.**
- No bl-sdk session was granted, and the recorded evidence has no capture of these natives.
- The script was read with `research/script_disasm.py`, and the native list with `tools/ghidra/script_natives.py`.
- The natives, the class registrations and the replication info's virtual method were read from the game executable
  with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock).
- Field names come from the cooked packages (`tools/ghidra/class_layout.py`).

Written in our own words, as behaviour only: no decompiler output, pseudo-code, addresses, offsets or call structure
is reproduced here. **Every rule is UNVERIFIED in the running game.** The last section lists the live observations
that would confirm them.

**Clean room.** This lane implements nothing. Everything here stays a named `NotYet` in Off until a fresh implementer
codes it from this note alone.

## Summary

| Piece | Kind | Confidence | Status |
|---|---|---|---|
| The engine-wide condition of `GetActiveMissionNumber` | native (a global of the engine) | high | UNVERIFIED |
| `WillowPlayerController.GetCurrentPlaythrough()`: the controller's current-playthrough query | native, through a virtual of the controller | high | UNVERIFIED |
| The game replication info's answer to it | native, a virtual of `WillowGameReplicationInfo` | high | UNVERIFIED |
| How the tracker's `MissionList` is built | native | not found in this pass | open question 3 |

## 1. The engine-wide condition of GetActiveMissionNumber

- **What it is:** the engine's current world is set. The global that `GetActiveMissionNumber` tests first is the
  engine's pointer to the current world. When it is set, the same object is the one the native's next step reads its
  `WorldInfo` from.
- **So the condition is not a separate switch:** it holds exactly when there is a world to read. With no current world,
  the native takes its fallback (NATIVE_MISSION_NUMBERS.md section 1, step 4), as it does when the world's `WorldInfo`,
  its replication info or its tracker is missing.
- The same test opens the current-playthrough query (section 2).

## 2. WillowPlayerController.GetCurrentPlaythrough()

- **The script native and the virtual are one.** The script declares `GetCurrentPlaythrough()` on
  `WillowPlayerController` as a native. Its exec function only calls a virtual method of the controller and returns
  its answer. That virtual is the "current-playthrough virtual" of NATIVE_MISSION_NUMBERS.md section 2, step 1. It is
  the one `GetActivePlotCriticalMissionNumber` calls for a negative or omitted index.
  `MissionTracker.AllExpansionSideMissionsComplete` calls it too (NATIVE_MISSION_OBJECTIVES.md, its "controller
  virtual").
- **The `WillowPlayerController` rule,** in order:
  1. The engine's current world (section 1). None: the answer is 0.
  2. That world's `WorldInfo`. None: 0.
  3. The `WorldInfo`'s `GRI`, tested for being a `WillowGameReplicationInfo` (or a subclass). None, or another class:
     0.
  4. Otherwise the answer is a virtual on the game replication info. For `WillowGameReplicationInfo` that virtual
     answers the replication info's `CurrentPlaythrough` field, as stored.
- **What it does not read:** the controller's own fields (any controller gives the same answer, the world's), and the
  replication info's `PlaythroughOverride`.
- **It writes nothing.**
- **Not read:** overrides of the controller's virtual in subclasses of `WillowPlayerController`, and overrides of the
  replication info's virtual in subclasses of `WillowGameReplicationInfo`. Only the two classes' own versions were read
  (open question 1).

## 3. What this means for links 10 to 12

The slice's accept runs the settings refresh, and with it `GetActiveMissionNumber` (NATIVE_MISSION_NUMBERS.md
section 1). On the installed data with `fix/rust-mission-numbers` (#480), links 10 to 12 stop in `accept` at the
mission bridge's `MissionTracker.GetAllMissions`: `NotYet` (layer absent), because the bridge holds one tracker record.

- **This note's two answers do not move that stop by themselves.**
  - With the engine-wide condition settled, the native takes the tracker branch alone.
  - The bridge never sets the tracker's `ActiveMission`, so that branch still asks `GetAllMissions` for the whole list.
- **A local probe confirmed the order** (uncommitted, reverted, not a rule): the bridge's `GetAllMissions` answered
  its one record. The stop then moved to the current-playthrough `NotYet` of #480. So with the whole list in place, the
  next need is section 2's query, as #480's both-branch form computes the fallback too. With the condition settled,
  the fallback is not computed when the tracker branch answers.
- **The tracked-mission route does not help.** In the recorded golden trace (FIRE_MISSION_GOLDEN_TRACE.md, step 11)
  the plot mission tracked before the accept is `GD_Episode04.M_Ep4_WelcomeToSanctuary`. Its stored `MissionNumber` in
  the installed package is -1. A world fixture that tracked it would answer -1, which takes the fallback, and so
  section 2's query, without the list. Whether the running game assigns that number at run time was not read (open
  question 4). This lane does not propose that fixture.
- **What moves the slice:** the tracker's whole `MissionList` in the bridge's world (open question 3), then section
  2's query and section 1's condition as Off answers.

## 4. The Off answers, step by step (for an implementer)

The hub's rulings apply:
- preflight before writes;
- a `NotYet` after a write is terminal, with the "stopped after partial writes: " prefix, its kind and record kept;
- no rollback.

The natives here write nothing, so nothing is preflighted for them. Inside the settings refresh they come after the
refresh's writes, and a `NotYet` they answer is marked by the refresh's own rule.

| Step | Off answer |
|---|---|
| The engine-wide condition | holds when the port's world exists: the world whose `WorldInfo` the world query answers. With it, `GetActiveMissionNumber` runs the tracker branch alone, and the both-branch comparison is no longer needed. With no world, the fallback |
| `WillowPlayerController.GetCurrentPlaythrough()` on a `WillowPlayerController` | the world, its `WorldInfo`, its `GRI` as a `WillowGameReplicationInfo`: the replication info's `CurrentPlaythrough` field. Any missing piece, or a replication info of another class: 0 |
| on a subclass of `WillowPlayerController` | a named `NotYet` (not read), "a subclass override of the controller's current-playthrough query" (open question 1) |
| a `GRI` that is a subclass of `WillowGameReplicationInfo` | a named `NotYet` (not read), "a subclass override of the replication info's current-playthrough answer" (open question 1) |
| `GetActivePlotCriticalMissionNumber` with a negative or omitted index | the playthrough from the row above, then NATIVE_MISSION_NUMBERS.md section 2's rule |
| `MissionTracker.GetAllMissions` | unchanged: the bridge's `NotYet` (layer absent) while it holds one record (open question 3) |

## Open questions

1. **UNVERIFIED:** overrides of the controller's current-playthrough virtual in subclasses of
   `WillowPlayerController`, and of the replication info's answer in subclasses of `WillowGameReplicationInfo`. Not
   read.
2. **UNVERIFIED:** that the global is the engine's current world in every build state. It is read from how the value
   is used: the next step reads the world's `WorldInfo` from it. The code that writes it was not read in detail.
3. **Not found:** how the tracker's `MissionList` is built. Known from the existing notes: one record per mission
   definition, kept sorted by the definition's full name, found by a binary search (NATIVE_MISSION_OBJECTIVES.md,
   NATIVE_SAVE_LOAD.md). The tracker's functions that use the list were surveyed in this pass. None of them adds
   records, so the builder is elsewhere (the tracker's set-up path, not reached here).
4. **UNVERIFIED:** whether a mission's `MissionNumber` is assigned at run time. The plot mission tracked in the golden
   trace stores -1 in the installed package.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Call `GetCurrentPlaythrough()` on Maya's controller in Sanctuary and compare it with
  `WorldInfo.GRI.CurrentPlaythrough`, on the first and the second playthrough. Set `PlaythroughOverride` to a different
  value and confirm the answer does not follow it.
- Call `GetActiveMissionNumber()` and `GetActivePlotCriticalMissionNumber()` with and without a tracked mission, and
  record the tracker's `ActiveMission` and its `MissionNumber`.
- Read the tracker's `MissionList`: its length, its order (by name), and whether it holds a record for every mission
  definition loaded, or only for the current map's. Record `M_Ep4_WelcomeToSanctuary`'s `MissionNumber` at run time.
