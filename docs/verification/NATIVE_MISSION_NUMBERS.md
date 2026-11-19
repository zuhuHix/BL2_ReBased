# The two mission numbers of the settings refresh, and what the accept reaches after them

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-MISSION-NUMBERS, decision 2026-11-19-86. Analyst note only (clean
room): written in own words from local, read-only analysis of `Borderlands2.exe` (`tools/ghidra/run.ps1`, one headless
run at a time under the machine lock) and from `research/script_disasm.py`. Raw output stays under ignored
`local/analysis/missionnumbers/`. There is no bl-sdk session and no recorded live evidence, so **every rule here is
UNVERIFIED** in the running game.

It extends two records and corrects one point of each neighbour:
- `WillowPlayerController.GetActiveMissionNumber`, first read in NATIVE_CONTROLLER_HELPERS.md. Its fallback is
  corrected (section 1).
- `WillowPlayerController.GetActivePlotCriticalMissionNumber`, read in NATIVE_SAVE_LOAD.md. This lane confirms it
  (section 2).
- What the mission accept reaches after the settings refresh (section 3). `QuestAcceptGFxMovie.UpdateMissionTextList`
  is read in NATIVE_GFX_BRIDGE.md; that note's "no script caller" is corrected here.

**Where the slice stands.** Links 10 to 12 of `--slice-gate` stop in `accept` at `GetActiveMissionNumber` (the settings
refresh, NATIVE_SETTINGS_FILL.md section 4, step 6). A local probe answered 0 for both numbers. It was never committed,
and its output stays under `local/`. With it, the refresh ran to its end (steps 7 to 9) and the accept went on to
`QuestAcceptGFxMovie.UpdateMissionTextList`, an unbound native in Off (section 3).

**Status values** (the mission status enum, as the accept screen's script reads it): 0 NotStarted, 1 Active, 2
RequiredObjectivesComplete, 3 ReadyToTurnIn, 4 Complete, 5 Failed. "In progress" below means 1, 2 or 3.

## 1. `WillowPlayerController.GetActiveMissionNumber()`

No parameters; returns an int. The native body only reads, and writes nothing.

1. **The world's tracker.** It takes the world's game replication info, as a `WillowGameReplicationInfo`, then its
   `MissionTracker`.
   - It goes on only when an engine-wide condition holds, the world exists, the replication info is one, the tracker
     is set, and the tracker's data is valid (`bDataValidated`, as `MissionTracker.IsDataValid` answers).
   - Any failure takes the fallback (step 4).
   - The engine-wide condition is a global whose name was not resolved. Read as "the game is running" (UNVERIFIED).
2. **The tracked mission.** When the tracker's `ActiveMission` is set, the answer is that definition's
   `MissionNumber`. There is no status check and no plot-critical check.
3. **Otherwise, the tracker's mission list** (the list `MissionTracker.GetAllMissions` gives, in its order). For each
   definition with `bPlotCritical` set and no `DlcExpansion`, its status is read through `MissionTracker.GetMissionStatus`:
   - in progress (1, 2 or 3): the answer is its `MissionNumber`, at once (the first such in list order);
   - Complete (4): it is kept when its `MissionNumber` is larger than the largest kept so far.
   - The answer after the list is the largest kept, else 0.
   - A definition that is not plot-critical, or has a `DlcExpansion`, is skipped whatever its status.
4. **The fallback** is `GetActivePlotCriticalMissionNumber` for the current playthrough (section 2). It is taken when
   step 1 fails. It is also taken when steps 2 and 3 answer -1, which only a `MissionNumber` of -1 can give.
   - **Correction:** NATIVE_CONTROLLER_HELPERS.md named the local active-mission function as the fallback. The body
     read here calls the plot-critical function instead; that note is now fixed in place.

**Off answers, step by step** (for an implementer; hub rulings of 2026-11-19 apply). The native writes nothing, so
nothing is preflighted for it and nothing is rolled back. Every call below goes through the VM:
1. **The inputs:** the world's `WorldInfo.GRI`, its class checked against `WillowGameReplicationInfo`, and its
   `MissionTracker`.
2. **The engine-wide condition** (step 1's unresolved global): Off does not guess it.
   - When the world, the replication info and the tracker are all there, both branches are computed: the tracker
     branch (steps 2 and 3) and the fallback (section 2).
   - Equal answers: that number.
   - Different answers: `NotYet { NotRead }`, "the engine-wide condition of GetActiveMissionNumber". If either branch
     fails, that failure is the answer (below).
   - With no world, no replication info of that class, or no tracker, the condition does not matter: the fallback is
     the answer.
3. **The tracker branch:**
   - `IsDataValid()` false: the fallback.
   - `ActiveMission` set: its `MissionNumber`.
   - Otherwise the list from `GetAllMissions(out Missions)`, then `GetMissionStatus(definition)` for each plot-critical,
     non-DLC entry, as steps 2 and 3 above.
4. **A call that fails propagates unchanged**, its variant and kind kept:
   - a bound native's `NotYet` or error;
   - an unbound native's `VmError::UnboundNative`.
   - Inside the settings refresh, a `NotYet` that leaves after the refresh wrote gets the refresh's prefix (decision
     2026-11-19-80); this native adds none of its own.
5. **A `None` entry** in the list: the game's loop reads each entry's fields with no `None` test, so what it does with
   one was not read. `NotYet { NotRead }`, "a None entry in the tracker's mission list".
6. **A definition without a readable `bPlotCritical`, `DlcExpansion` or `MissionNumber`** is bad data: a script error
   naming the field, propagated unchanged.

**The port's case today, and the stop it would give.** The mission bridge binds `IsDataValid` (the tracker's
`bDataValidated`) and `GetMissionStatus` (its one mission's status, NotStarted for any other). It sets no
`ActiveMission`. Its `GetAllMissions` answers `NotYet { LayerAbsent }` today, because the bridge holds one record and
not the game's list (`ow_mission::bridge`, decision 2026-11-19-10).
- Bound by this note in the installed slice, with the tracker's data valid: the tracker branch reaches `GetAllMissions`
  and stops there with that `LayerAbsent`, marked by the refresh. That is links 10 to 12's next stop, unless the
  bridge's tracker holds the full list by then.
- With the data not valid, and in every case where step 2 computes the fallback: the fallback asks for the current
  playthrough, whose virtual is not read (section 2, step 1). So it is that `NotYet { NotRead }`, which the refresh
  marks.
- So today no installed case of this native gets past a named `NotYet`. The two unresolved decisions that change its
  result, the engine-wide condition and the playthrough virtual, are each a named `NotYet { NotRead }`. No Off
  substitute is authorized for either.

## 2. `WillowPlayerController.GetActivePlotCriticalMissionNumber(optional int PlaythroughIndex)`

Read again in this lane; it agrees with NATIVE_SAVE_LOAD.md.

1. **The playthrough.** A negative index (the default) becomes the current playthrough, through a virtual of the
   controller. Its body was not read; read as `GetCurrentPlaythrough` (UNVERIFIED).
2. **Out of range** (negative, or not below the length of the controller's `MissionPlaythroughs`): the answer is 0.
3. **The playthrough's `MissionList`**, in its order. For each record whose `MissionDef` is set, has `bPlotCritical`,
   and has no `DlcExpansion`, its own `Status` field is read (the controller's record, not the tracker):
   - in progress (1, 2 or 3): the answer is its `MissionNumber`, at once;
   - Complete (4): it is kept when larger than the largest kept so far.
4. The answer is the largest kept, else 0.

**Off answers, step by step:**
1. **The current playthrough** (a negative index, the default): the controller's virtual that gives it was not read,
   and its identity is not guessed. `NotYet { NotRead }`, "the controller's current-playthrough virtual". An explicit
   index of 0 or more does not ask it.
2. **The list:** the controller's `MissionPlaythroughs[p].MissionList` as the VM holds it. The mission bridge keeps one
   playthrough with the bridge's mission in it.
3. Steps 2 to 4 above. Nothing is written. A record without a readable `MissionDef` or `Status`, or a definition
   without the three fields, is bad data, propagated unchanged.

**After both numbers** (NATIVE_SETTINGS_FILL.md section 4, steps 6 and 7): the refresh's scripts `UpdateActiveMission`
and `UpdateActivePlotMission` each write the number plus one through `SetIntProperty`, which is bound in Off.

## 3. What the accept reaches after the refresh

**The script path** (read with `research/script_disasm.py`; the callers come from a scan of every decoded script
function of the nine code packages). The slice's `accept` step calls the controller's `AcceptMission` directly
(`ow_mission::bridge`), not the screen's `extAcceptConfirmed`. The native the probe stopped at is reached by one chain:
1. `AcceptMission` asks the tracker's `ActivateMission`. The status routine runs the controller's
   `UpdateMissionStatus` (the settings refresh runs on this path first, as the probe's order showed).
2. `UpdateMissionStatus` calls `ClientUpdateUIMissionList`. That takes the playing movie (the UI manager's
   `GetPlayingMovie`, which the bridge answers with its stand-in screen) and, when the movie has no reward object,
   calls its `UpdateMissionList(mission)`. FIRE_MISSION_GOLDEN_TRACE.md (after swap 11) has this same nested route
   observed in the game at the accept.
3. `UpdateMissionList` runs `DetermineQuestEntries`, `GotoCorrectPartOfMovie(false)` and `extPopulateQuestEntries`
   when the mission is in its lists.
4. `extPopulateQuestEntries` calls `UpdateMissionTextList`.

Why it is this chain:
- The script callers of `UpdateMissionTextList` are `extPopulateQuestEntries` and the status menu's
  `UpdateListOfMissionTitles` (another movie).
- `extPopulateQuestEntries` is called by `UpdateMissionList`, `extAcceptConfirmed` and `AcceptReward` (both Flash
  callbacks, not driven by the slice).
- `UpdateMissionList` is called only by `ClientUpdateUIMissionList`, whose callers are `UpdateMissionStatus` and
  `ClientSpawnMissionRewardUI` (the turn-in's reward page).
- So at the accept it is `UpdateMissionStatus`'s call, and at link 12's turn-in it can also be the reward page's.

**Terminal refusals.** Both routes reach the native after the accept's or the turn-in's own writes (the mission's
status at least). So a `NotYet` from it is terminal for the step: its text starts "stopped after partial writes: ",
its kind and record are kept, and nothing is rolled back.

`extPopulateQuestEntries` drops the entries without a mission. It then calls the movie's `MissionTextList`:
`EmptyData`, then one `AddDataEntry` per mission. It inserts header rows when there are redeemable or eligible
missions. It then calls the native **`UpdateMissionTextList()`**, then `MissionTextList.PositionHighlightBar()`, then
`SetFocus()`.
- **Correction to NATIVE_GFX_BRIDGE.md:** that note says no script caller of `UpdateMissionTextList` appears in
  `QuestAcceptGFxMovie`. `extPopulateQuestEntries` calls it, and `UpdateMissionList` reaches it through that function.

**The native's entry.** It takes no parameters and calls one virtual of the movie, slot 536. The compiled entry is
shared with another class's unrelated native (the same body shape). The slot's body was not resolved in this lane,
because no registration pattern was found for the class's table. NATIVE_GFX_BRIDGE.md's reading of the behaviour is
taken as the rule. Its first step: when `MissionTextList` is None, nothing at all happens.

**Off answers, step by step.** Hub ruling of 2026-11-19: a UI-only side effect may be answered "nothing" in Off only
when nothing on the slice path reads it back; otherwise it is a named `NotYet` of kind `LayerAbsent`. The read-back
check below does not cover every possible reader (review r2 of #473): the 10 undecoded functions and native readers
are not accounted for. So no call here is answered "nothing".
1. **`UpdateMissionTextList`** is the first call the port cannot support. Off answers `NotYet { LayerAbsent }`,
   "the mission screen's Flash text list (not hosted)" (record NATIVE_GFX_BRIDGE.md), whether `MissionTextList` is
   None or set.
   - The check is certain before the native's own first write (emptying `MissionCategories`): the port hosts no Flash
     movie.
   - It comes after the accept's writes, so it is terminal: prefixed "stopped after partial writes: ", kind and record
     kept, nothing rolled back.
2. **Not reached:** `MissionTextList.PositionHighlightBar()`, `SetFocus()` with `GetVariableObject` and `FocusOn`. The
   focus pair's own state (the clip tables, `FocusObject`, the 3D focus offset) is not answered "nothing" either. If a
   later ruling lets the text list pass, `GetVariableObject` is the next unsupported call: `NotYet { LayerAbsent }`,
   "a Flash movie query (not hosted)", terminal in the same way.
3. **The return through the slice's real caller** (`ow_mission::bridge`, the `accept` the slice step calls):
   - the bridge runs the controller's `AcceptMission` inside its script run, and records the error;
   - an error whose text carries the partial-writes mark is terminal there. The run returns it, the bridge's `accept`
     propagates it, and the slice step stops with it as a `NotYet` (typed, so graded `NotYet`). Later script runs on
     that VM fail with it without running;
   - so the bridge's own check, whether the mission's status changed between before and after the call, is not
     reached on this path;
   - without the mark, the run would record the `NotYet` and return normally, and `accept` would answer whether the
     status changed. The slice gate grades the recorded `NotYet` first.
   - No step of this return is untraced: it is the bridge's code. Its marked-error test is text-based on main today
     and an exact prefix check with #464.

**The read-back check** (lane ANALYST-MISSION-NUMBERS, under the hub ruling; evidence for a later ruling, not an Off
answer). The script functions of the nine code packages were run through `research/script_disasm.py`'s decoder:
12,978 functions, of which 12,968 decoded and 10 did not. The claims below cover the 12,968 that decoded, searched for
the fields named.
- **`MissionCategories`:** no script reads or writes it. Only the native itself uses it (NATIVE_GFX_BRIDGE.md).
- **`MissionTextList`** is used only by the movie's own script: `Start` (which creates it, and is not run in the port),
  `extPopulateQuestEntries` and `SetFocus` (the calls above), `GetSelectedIndex`, `extFillOutQuestAllScreenDetails`,
  `extLogListRollOver`, `extNavigateUp` and `extNavigateDown`.
- **On the slice path:**
  - `GetSelectedIndex` is called by `extAcceptConfirmed`, `extCompleteConfirmed`, `extChoiceConfirmed` and
    `ProtectedFillOutCommonScreenDetails`, all Flash callbacks the slice does not drive. It is also called by the
    controller's `ClientSpawnMissionRewardUI`, which the turn-in of link 12 can reach.
  - It reads `MissionTextList.HighlightedEntry`. With `MissionTextList` None, it reads None whether or not
    `UpdateMissionTextList` ran, so the native's "nothing" changes no answer.
  - The other readers are reached only from Flash callbacks (`UpdateMissionDetails`, `extLogListTrack`, `LogUp`,
    `LogDown`), which the slice does not drive.
- **`FocusOn`'s state:** no script reads `WillowGFxMovie3D.FocusObject`, `InterpFocusOffset`, `ChainedObjectMap` or
  `RedundantClipArray`. `FocusOffset` is read only by four movies' `HandleInputAxis` functions (the accept screen's
  among them), which run on the player's stick input to the movie. The slice drives no input to the screen. The
  offset's other user is the 3D movie's drawing, which a headless world does not do.
- **`GetVariableObject`'s result** is used only as `FocusOn`'s argument inside `SetFocus`.
- **Not covered:** the 10 functions the decoder cannot read, and native readers other than the record's. One of the
  10 is `WillowGFxUIManager.PlayMovie`, which the bridge's stand-in manager does not reach on links 10 to 12 (it binds
  `GetPlayingMovie` only).
- So among the decoded script nothing on links 10 to 12 reads the state back. The 10 undecoded functions and any
  native reader are open, which is why step 1 is a `NotYet`.

## Open questions

1. The engine-wide condition in section 1, step 1 (an unresolved global).
2. The controller's current-playthrough virtual in section 2, step 1.
3. The movie's slot 536 body (NATIVE_GFX_BRIDGE.md's reading is used).
4. Whether an unhosted movie's `GetVariableObject` answers None in the game (it has a Flash file loaded there).
5. The 10 script functions the decoder cannot read, for the read-back check.
6. What the game does with a `None` entry in the tracker's mission list.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

Each check names the rule it tells apart from its alternative.
1. **First in list order, not lowest number** (section 1, step 3; section 2, step 3).
   - Setup: no tracked mission, and two plot-critical, non-DLC missions in progress, the one earlier in the tracker's
     list (and in the playthrough's `MissionList`) with the larger `MissionNumber`.
   - Read: `GetActiveMissionNumber()` and `GetActivePlotCriticalMissionNumber()`.
   - Expect: the earlier mission's number from both, not the smaller number.
2. **The tracked mission wins without a status check** (section 1, step 2).
   - Setup: track a side (not plot-critical) mission, then complete it while still tracked if the game allows, with a
     plot mission in progress.
   - Expect: the side mission's number in both cases.
3. **The fallback is the plot-critical function, not the local one** (section 1, step 4).
   - Setup: a moment before the tracker's data is valid (hook `MissionTracker.ValidateData`), with candidates chosen
     so the two functions differ: a tracked side mission in progress (the local function would answer it), and a plot
     mission in progress with another number.
   - Expect: the plot mission's number.
4. **The Complete maximum** (section 2, step 4).
   - Setup: no plot mission in progress, and three Complete plot missions whose numbers are not in list order.
   - Expect: the largest number.
5. **The playthrough index** (section 2, steps 1 and 2).
   - Read `GetActivePlotCriticalMissionNumber()` (default), `(-1)`, the current index, the other playthrough's index
     (with different missions in its list), and `(5)`.
   - Expect: the same answer for the first three, the other list's answer for the fourth, and 0 for the last.
6. **The screen's state, present case** (section 3).
   - Setup: on accept, hook `UpdateMissionList`, `extPopulateQuestEntries`, `UpdateMissionTextList`, `SetFocus` and
     `FocusOn`.
   - Record before and after each: the order of the calls; `MissionTextList` (expected set in the game);
     `MissionCategories`; the list's displayed rows; `FocusObject` and `FocusOffset`.
7. **Still open:**
   - the -1 sentinel of section 1, step 4 (no mission has number -1);
   - the absent case of section 3 (`MissionTextList` None), which the game's screen does not produce;
   - the engine-wide condition.
