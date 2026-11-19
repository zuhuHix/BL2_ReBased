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
   - **Correction:** NATIVE_CONTROLLER_HELPERS.md names the local active-mission function as the fallback. The body
     read here calls the plot-critical function instead.

**Off answers, step by step** (for an implementer; hub rulings of 2026-11-19 apply):
1. **The inputs**, through the VM:
   - the world's `WorldInfo.GRI`;
   - its class checked against `WillowGameReplicationInfo`;
   - its `MissionTracker`;
   - the tracker's `IsDataValid()`, `ActiveMission`, `GetAllMissions()` and `GetMissionStatus(definition)`, as the
     mission bridge binds them (`ow_mission::bridge`).
   - A tracker native the bridge does not bind is the VM's answer for that native (its own `NotYet` or refusal),
     propagated unchanged.
2. **No world, no replication info of that class, no tracker, or data not valid:** the fallback (section 2).
3. **The rest** as steps 2 to 4 above. Nothing is written, so there is nothing to preflight and nothing to roll back.
   The refresh's own write mark (decision 2026-11-19-80) still marks a `NotYet` from a tracker native after the
   refresh wrote.
4. **A definition without a readable `bPlotCritical`, `DlcExpansion` or `MissionNumber`** is bad data: a script error
   naming the field, propagated unchanged.

**The port's case today.** The bridge's tracker holds one mission. The installed slice's answer then depends on two
things: whether the bridge sets `ActiveMission` when it accepts, and whether that mission is plot-critical. An
implementer reads both from the VM; this note does not assume them.

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
1. **The current playthrough:** the controller's `GetCurrentPlaythrough()` answer, called through the VM. If the
   controller's class declares no such function, it is a `NotYet` of kind `NotRead`, "the controller's current
   playthrough query" (UNVERIFIED that the virtual is it).
2. **The list:** the controller's `MissionPlaythroughs[p].MissionList` as the VM holds it. The mission bridge keeps one
   playthrough with the bridge's mission in it.
3. Steps 2 to 4 above. Nothing is written. A record without a readable `MissionDef` or `Status`, or a definition
   without the three fields, is bad data, propagated unchanged.

**After both numbers** (NATIVE_SETTINGS_FILL.md section 4, steps 6 and 7): the refresh's scripts `UpdateActiveMission`
and `UpdateActivePlotMission` each write the number plus one through `SetIntProperty`, which is bound in Off.

## 3. What the accept reaches after the refresh

**The script path** (read with `research/script_disasm.py`). The slice's `accept` step is the accept screen's
`extAcceptConfirmed`:
1. a UI sound;
2. `bAcceptedMission` set;
3. the controller's `AcceptMission` for the selected entry, whose path runs the settings refresh;
4. then `DetermineQuestEntries`, `GotoCorrectPartOfMovie(false)` and `extPopulateQuestEntries`.

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

**Off answers, step by step:**
1. **The port's screen is a stand-in.** The mission bridge makes a VM object of `QuestAcceptGFxMovie` from its class
   defaults. The movie's `Start`, the only script seen to create `MissionTextList` (a new `GFxTextListContainer`), is
   not run. So `MissionTextList` is None, unless something sets it.
2. **`UpdateMissionTextList` with `MissionTextList` None:** it returns and changes nothing (the record's first step).
   This is the whole Off rule for the stand-in. It is read on the live object at the call, so nothing has to be
   preflighted.
3. **`MissionTextList` set** (not the port's case): the record's steps follow. They need localised category labels
   (`Object.Localize`), the list's native rebuild and its Flash refresh, none of which the port has. Off answers a
   `NotYet`: kind `LayerAbsent`, "the mission screen's Flash text list (not hosted)". It is checked before the first
   write (emptying `MissionCategories`). It comes after the accept's own writes, so under the hub ruling it is
   terminal ("stopped after partial writes", its kind kept), and nothing is rolled back.
4. **What follows:**
   - `MissionTextList.PositionHighlightBar()`: a call on a None object, which the VM skips, as it skipped `EmptyData`
     and `AddDataEntry` in the probe.
   - Then `SetFocus()` asks `GFxMoviePlayer.GetVariableObject("missions.selections.selection<N>")`. NATIVE_GFX_BRIDGE.md
     says `None` for a missing movie. Off answer: None, since the stand-in hosts no movie (UNVERIFIED that an unhosted
     movie counts as missing).
   - Then `WillowGFxMovie3D.FocusOn(None)`: NATIVE_GFX_BRIDGE.md says a no-op is safe with `None` tolerance. Off
     answer: nothing.
   - Both are listed today as Reference-only presentation stubs (`ow-cli`'s `REFERENCE_STUBS`). In Off they are
     unbound, so each would be the next stop until bound.
5. **Past `SetFocus`**, `extAcceptConfirmed` returns. What the slice's accept step checks after that was not read in
   this lane.

**For the hub:** steps 2 and 4 treat "the port hosts no Flash movie" as the engine's fact, like the online subsystem and
the DLC manager rulings. If the hub prefers a named refusal, steps 2 and 4 become the `NotYet` of step 3 instead.

## Open questions

1. The engine-wide condition in section 1, step 1 (an unresolved global).
2. The controller's current-playthrough virtual in section 2, step 1.
3. The movie's slot 536 body (NATIVE_GFX_BRIDGE.md's reading is used).
4. Whether an unhosted movie's `GetVariableObject` answers None in the game (it has a Flash file loaded there).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **The tracked mission wins.** With a side mission tracked and a plot mission active, read `GetActiveMissionNumber()`.
   Expect the side mission's `MissionNumber` (section 1, step 2). Untrack it, and expect the plot mission's.
2. **The plot scan.** With no tracked mission, one plot mission Complete and another Active, expect the Active one's
   number from both natives. With only Complete plot missions, expect the largest of their numbers.
3. **A DLC plot mission** in progress is skipped by both: expect the base game's answer.
4. **The fallback.** Read `GetActiveMissionNumber()` before the tracker's data is valid (early in a load, hooking
   `MissionTracker.ValidateData`). Expect `GetActivePlotCriticalMissionNumber(-1)`'s answer, not the local
   active-mission one: a tracked side mission is ignored there.
5. **The playthrough index.** `GetActivePlotCriticalMissionNumber(5)`: expect 0. `(-1)` and the current index: the
   same answer.
6. **The text list.** Hook `QuestAcceptGFxMovie.UpdateMissionTextList` on accept. Record `MissionTextList` (expected
   set in the game) and the order of `extPopulateQuestEntries`, `UpdateMissionTextList`, `PositionHighlightBar`,
   `SetFocus`.
