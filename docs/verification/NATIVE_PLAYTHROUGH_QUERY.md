# Native rules of the current playthrough and the engine-wide condition: WillowPlayerController.GetCurrentPlaythrough (draft, 2026-11-19)

AI-assisted (Claude Code), analyst lane LINK-10-MISSIONLIST (decision 2026-11-19-99), for slice links 10 to 12 (the
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

Written in our own words, as behaviour only: no decompiler output, pseudo-code, addresses, offsets, call structure or
listing structure is reproduced here. **Every rule is UNVERIFIED in the running game.** The last section lists the live observations
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

**Relation to existing records and code.**
- **The record.** NATIVE_CONTROLLER_HELPERS.md, section "WillowPlayerController.GetCurrentPlaythrough" (and its index
  row), already gives this rule: the world, its `WorldInfo`, the `GRI`, `CurrentPlaythrough`; 0 for each missing piece
  or a replication info that is not a `WillowGameReplicationInfo`; `PlaythroughOverride` ignored; no clamp. This note
  agrees with it.
- **What this note adds:**
  - the script native and the virtual that `GetActivePlotCriticalMissionNumber` and
    `AllExpansionSideMissionsComplete` call are one function, which settles NATIVE_MISSION_NUMBERS.md open question 2;
  - the replication info's answer is itself a virtual, so subclasses of either class are not covered by the reading;
  - section 1's engine-wide condition.
- **The code.** The Off binding of `GetCurrentPlaythrough` exists and is owned by the mission bridge (`ow_mission`,
  the bridge's `n_playthrough`, bound with its other controller natives). It reads the bridge's own replication
  object. A missing field gives 0. It makes no world, replication-info-class or subclass check, and it answers
  "outside the binding" on any controller but the bridge's.
  - #480 (`ow_world` mission numbers) asks the controller's `GetCurrentPlaythrough` through the VM, so today it lands
    on that binding.
  - What this note asks of the binding (an implementer's change, in the bridge): read the world's `WorldInfo.GRI`
    rather than the bridge's own object; a `CurrentPlaythrough` of the wrong kind is bad data, not 0; the two subclass
    refusals of section 4; and the same answer for every exactly-`WillowPlayerController` controller, since the rule
    does not read the controller.

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
- **Section 2 is needed regardless.** The settings refresh calls `GetActivePlotCriticalMissionNumber()` directly
  (NATIVE_SETTINGS_FILL.md section 4, step 7), with the default index, right after `GetActiveMissionNumber` (step 6).
- **The expected stop order once both are bound:** step 6's `GetAllMissions` `NotYet` (layer absent) stays the first
  stop. Only with the tracker's whole list does step 6 answer, and then step 7 reaches section 2's query.
- **The tracked-mission route does not help.** In the recorded golden trace (FIRE_MISSION_GOLDEN_TRACE.md, step 11)
  the plot mission tracked before the accept is `GD_Episode04.M_Ep4_WelcomeToSanctuary`. Its stored `MissionNumber` in
  the installed package is -1. A world fixture that tracked it would answer -1, which takes the fallback, and so
  section 2's query, without the list. Whether the running game assigns that number at run time was not read (open
  question 4). This lane does not propose that fixture.
  - **Correction (NATIVE_MISSION_LIST_BUILD.md):** the tracker's build sets every definition's `MissionNumber` to -1
    and then numbers the main chain at run time, so the stored -1 says nothing about a main-chain mission's number in
    the running game. This argument no longer holds for a main-chain mission (UNVERIFIED).
- **What moves the slice:** the tracker's whole `MissionList` in the bridge's world (open question 3), then section
  2's query and section 1's condition as Off answers.

## 4. The Off answers, step by step (for an implementer)

The hub's rulings apply:
- preflight before writes;
- a `NotYet` after a write is terminal, with the "stopped after partial writes: " prefix, its kind and record kept;
- no rollback.

The natives here write nothing, so nothing is preflighted for them. Inside the settings refresh they come after the
refresh's writes, and a `NotYet` they answer is marked by the refresh's own rule.

**No lazy load.** The reads below are of run-time objects (the world's `WorldInfo`, its replication info, the
controller), not definition-store handles, so none of them loads anything. Where a caller reads through the attribute
evaluator's context, #480's rule for that context applies unchanged.

**The engine-wide condition** holds when the port's world exists: the `WorldInfo` the world's actor list holds
(`Actors::world_info()`). With it, `GetActiveMissionNumber` runs the tracker branch alone. With no world it takes the
fallback.
- The both-branch comparison and its `NotYet`, #480's `ENGINE_GATE`, are retired.
- `Reads::engine_gate` becomes the Off behaviour rather than a test switch.
- Open question 2 needs no refusal. NATIVE_CONTROLLER_HELPERS.md already reads the same global as the engine's world
  object: point 4 of its conventions, and the `GetCurrentPlaythrough` section.

**`WillowPlayerController.GetCurrentPlaythrough()`**, in this order. The class tests are exact-class tests.
1. The controller's class is not exactly `WillowPlayerController`: `NotYet` (not read), "a subclass override of the
   controller's current-playthrough query" (open question 1).
2. No world: 0.
3. No `WorldInfo`: 0.
4. The `WorldInfo`'s `GRI` is None, or not a `WillowGameReplicationInfo` (neither that class nor a subclass): 0.
5. The `GRI`'s class is a strict subclass of `WillowGameReplicationInfo`: `NotYet` (not read), "a subclass override
   of the replication info's current-playthrough answer" (open question 1).
6. The `GRI`'s `CurrentPlaythrough`, read as an int or a byte (as the port's
   `WillowGameReplicationInfo.GetCurrPlaythrough` reads it, `datastore::get_curr_playthrough`). Any other kind is bad
   data, a `VmError::Script`, as NATIVE_MISSION_NUMBERS.md section 1's Off steps treat a field of the wrong kind.

**Direct callers.** `GetActivePlotCriticalMissionNumber` (a negative or omitted index) and
`MissionTracker.AllExpansionSideMissionsComplete` reach the controller's virtual, not the script function. In the port
they use this rule as a function on the controller, not a VM dispatch of `GetCurrentPlaythrough`.

**The slice.** The bridge's world holds exactly a `WillowPlayerController` and a `WillowGameReplicationInfo`, so the
slice does not reach steps 1 or 5.

**`MissionTracker.GetAllMissions`** is unchanged: the bridge's `NotYet` (layer absent) while it holds one record (open
question 3).

## Open questions

1. **UNVERIFIED:** overrides of the controller's current-playthrough virtual in subclasses of
   `WillowPlayerController`, and of the replication info's answer in subclasses of `WillowGameReplicationInfo`. Not
   read.
2. **UNVERIFIED:** that the global is the engine's current world in every build state. It is read from how the value
   is used: the next step reads the world's `WorldInfo` from it. This agrees with NATIVE_CONTROLLER_HELPERS.md (point 4
   of its conventions, and its `GetCurrentPlaythrough` section). The code that writes it was not read in detail. No Off
   refusal follows from this.
3. **Read, UNVERIFIED (NATIVE_MISSION_LIST_BUILD.md):** how the tracker's `MissionList` is built. One record per
   mission definition, found by a binary search, as the existing notes say.
   - **Correction:** the order is not "the definition's full name". It is the object name as the engine's name handle
     compares it: name-table entry, then instance number.
4. **Read, UNVERIFIED (NATIVE_MISSION_LIST_BUILD.md, section 3):** a mission's `MissionNumber` is assigned at run
   time. The build resets every number to -1, then numbers the main chain and the side missions.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Call `GetCurrentPlaythrough()` on Maya's controller in Sanctuary and compare it with
  `WorldInfo.GRI.CurrentPlaythrough`, on the first and the second playthrough. Set `PlaythroughOverride` to a different
  value and confirm the answer does not follow it.
- Call `GetActiveMissionNumber()` and `GetActivePlotCriticalMissionNumber()` with and without a tracked mission, and
  record the tracker's `ActiveMission` and its `MissionNumber`.
- Read the tracker's `MissionList`: its length, its order (by name), and whether it holds a record for every mission
  definition loaded, or only for the current map's. Record `M_Ep4_WelcomeToSanctuary`'s `MissionNumber` at run time.
