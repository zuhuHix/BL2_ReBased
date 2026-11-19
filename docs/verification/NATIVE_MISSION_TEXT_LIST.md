# Link 10's next stop: the accept screen's text list, its focus pair, and the mission-status telemetry (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Analyst lane ANALYST-LINK10-NEXT (decision 2026-11-19-NNN, number to be given by
the hub).
- **Sources:**
  - one local slice-gate run on a throwaway merge of #496 (629d0b9, which carries #493) and #497 (d4c57cc) on main
    d72b96d, then two local, uncommitted probes on that merge (section 1);
  - `research/script_disasm.py` on the accept screen's script;
  - the game executable, read with Ghidra (`tools/ghidra/run.ps1` read-only queries and the local table reader, one
    headless run at a time under the machine lock);
  - field names and their order from the cooked packages (`tools/ghidra/class_layout.py`, and the export tables where
    that tool stops on a map property).
- **Own words only:** no decompiler output, pseudo-code, addresses, offsets or listing structure. Raw output stays
  under ignored `local/analysis/link10next/`.
- **No live session** was run. **Every rule is UNVERIFIED in the running game.**

**Clean room.** This lane implements nothing. A fresh implementer codes the Off answers from this note, together with
NATIVE_MISSION_NUMBERS.md (decision 2026-11-19-86) and NATIVE_GFX_BRIDGE.md.

## 1. The stop, and what lies behind it

**The gate on the merge** (installed, Off, strict, under the machine lock). Links 10 to 12 all stop at the accept:

| # | Link | Result | Steps passed / driver steps | First stop |
|---|---|---|---|---|
| 10 | mission_accept | notyet | 3 / 8 | `accept`: stopped after partial writes, "the mission screen's Flash text list (not hosted)" (`LayerAbsent`, NATIVE_GFX_BRIDGE.md) |
| 11 | mission_objective | notyet | 0 / 8 | blocked by link 10 (the same stop) |
| 12 | mission_turn_in | notyet | 0 / 6 | blocked by link 11 (the same stop) |

So `GetAllMissions` and `GetCurrentPlaythrough` are passed, and the accept now reaches
`QuestAcceptGFxMovie.UpdateMissionTextList`. NATIVE_MISSION_NUMBERS.md section 3 named this native's `LayerAbsent`,
by the hub's ruling on UI-only effects.

**Two probes** (local, never committed; each answered the calls named, then the gate was rerun):
1. `UpdateMissionTextList` doing nothing when `MissionTextList` is None (section 2's reading). The accept then stops
   at `GFxMoviePlayer.GetVariableObject`: "a Flash movie query (not hosted)" (`LayerAbsent`). So
   `MissionTextList.PositionHighlightBar()` on a None list passes in the port's VM.
2. That, plus `GetVariableObject` answering None and `FocusOn` doing nothing. The accept then stops at
   **`WillowLeviathanService.RecordMissionStatusChangedEventForPlayer(object,byte,name)`**, an unbound native. That is
   an `error` row, not a `NotYet` (section 5).

The order past the settings refresh is therefore:
1. the text list;
2. `SetFocus`'s two calls;
3. the mission-status telemetry.

Sections 2 to 5 read each in turn.

## 2. `QuestAcceptGFxMovie.UpdateMissionTextList()`: the body (settles NATIVE_MISSION_NUMBERS.md open question 3)

**Which body runs.** The native's entry calls one virtual of the movie. The accept screen's table was found through its
class registration and its constructor, and that slot holds the body read here. No subclass of `QuestAcceptGFxMovie`
was looked at.

**The fields.** The layout tool cannot lay out the movie classes (a map property in a parent class). The fields are
named by the class's own declaration order from the export table:
- `MissionTextList` (an object) is followed directly by `MissionCategories` (a string array);
- the body tests the first and hands the second to the rebuild, which fits that order;
- the list container's own fields (`SortableArray`, `TextEntries`) are laid out by the tool, and the body's helpers
  match them.

So the names are by fit, not by layout: UNVERIFIED by name, high confidence.

**What it does:**
1. **When `MissionTextList` is None: nothing at all.** No field is read beyond that test, nothing is written, and no
   global is touched.
2. **Otherwise:**
   1. `MissionCategories` is emptied;
   2. each entry of the list's `SortableArray`, in order, gets a category index from a per-entry rule. That is
      NATIVE_GFX_BRIDGE.md's status lookup: available, turn-in, or -1;
   3. the list rebuilds its `TextEntries` from the sortable entries and the categories;
   4. one process-wide flag is cleared. Its identity was not read.

NATIVE_GFX_BRIDGE.md's reading of steps 2.2 and 2.3 stands. This lane confirmed the structure, not each detail.

**Why this matters for the ruling.** NATIVE_MISSION_NUMBERS.md answered `LayerAbsent` "whether `MissionTextList` is
None or set". Its reasons were that the body was unresolved, and that the read-back check left 10 undecoded functions
and native readers open. With the body read, the None branch writes nothing. So nothing can read back state it never
made: the read-back question does not arise for the None branch.

## 3. `QuestAcceptGFxMovie.SetFocus()` (script) and `GFxMoviePlayer.GetVariableObject`

**`SetFocus`** is one statement:
- `FocusOn(GetVariableObject("missions.selections.selection" $ ((HighlightedEntry - IndexOfTopEntry) + 1)))`;
- the two indices are `MissionTextList`'s;
- with a None list, the VM's None-context rule reads both as 0, so the path is `missions.selections.selection1`.

**`GetVariableObject(Path, optional Type)`:**
1. It asks the Flash player for the variable at `Path` only when the movie has a native player object and that player
   has a movie view.
2. On a found variable, it makes a wrapper object of `Type`, or of `GFxUI`'s `GFxObject` class when `Type` is None
   (that class is looked up once and cached).
3. **With no native player, no view, or no variable, no wrapper is made.** The answer is then None. That matches
   NATIVE_GFX_BRIDGE.md ("None for a missing path or movie"). The listing's return on this branch was read by fit,
   not proven: UNVERIFIED.
4. It writes no script-visible field.

The port's movies are stand-ins with no native player. So the game's own rule for that state is "no lookup, None".
**But** the game always plays this movie (its `Start` makes the list and the player), so the no-player state is the
port's, never observed in the game.

## 4. `WillowGFxMovie3D.FocusOn(Thing)`

**Which body runs.** The entry unpacks one object and calls one virtual of the movie, found in the accept screen's
table. It runs, in order:
1. **The chain step.**
   - With `Thing` None this step is skipped, and steps 2 and 3 still run.
   - Otherwise it registers `Thing` and its `_parent` chain in `ChainedObjectMap`, through Flash lookups.
2. **`FocusObject` = `Thing`** (a script-visible write).
3. **The offset step:**
   - `FocusOffset` is zeroed. It is a `Core.Object.Vector` (the property's struct import), so X, Y and Z are all set
     to 0, and `InterpFocusOffset` is not written;
   - with a `FocusObject` set, it is centred on the object's bounds (Flash queries);
   - a final clamp limits its Y to plus or minus half the margin between the stage height and the stage height times a
     scale.
   - The stage size comes from the native player's view. **With no native player, the size is never written**, so the
     clamp's bound is undefined in the game.
   - The scale is (1, 1) when a flag of the movie is set, else two values of an engine object. Neither the flag's nor
     the object's identity was read.

**So for `FocusOn(None)` on a movie with no native player, the game's result is undefined.** That case reads an
unwritten stage size, and only a scale of exactly 1 makes the clamp's bound 0. The hub ruled on the port's answer for
that state (section 6, R2).

Field names: `FocusObject`, then `FocusOffset`, by the class's declaration order (`ChainedObjectMap`,
`RedundantClipArray`, `FocusObject`, `FocusOffset`, `InterpFocusOffset`), by fit, UNVERIFIED.

## 5. `WillowLeviathanService.RecordMissionStatusChangedEventForPlayer(Player, Status, Name)`

**The call at the accept.** `WillowPlayerController.UpdateMissionStatus` calls it as a static function of the service
class. The player it passes is `Self`, the controller whose mission status changed (in the slice, the bridge's player
controller). The status is the new status, and the name is the mission's object `Name`.

**What it does:**
1. It unpacks an object, a byte and a name.
2. It builds a temporary event record that holds the byte and the name, the name packed with a number from an engine
   lookup that was not read.
3. It hands the record and the player to the statistics submit routine.
4. It frees the temporaries.

**The submit routine** returns at once for a None player. Otherwise it asks the player one virtual and takes one of two
submit paths, neither of which was read.

**Which events share it.** The same submit routine ends `RecordCharacterRevivedEventForPlayer` and
`RecordCharacterKilledEventForPlayer` (this lane read both natives' bodies up to that call).
- NATIVE_RESPAWN_STATIONS.md section 3.5 reads the revived event as "telemetry only" in one sentence, "in what was
  read". It does not say that the submit paths were read, so that reading does not establish them.
- No write to a script-visible field was seen in this native or in its record step.

## 6. Off answers, step by step (for an implementer)

The hub's rulings apply:
- preflight what is cheap and certain;
- a `NotYet` after a write is terminal and keeps its kind and record. The natives below answer plainly; the bridge's
  status hook marks them "stopped after partial writes" (decision 2026-11-19-93);
- grade by the `VmError` variant and the `NotYet` kind, never by text;
- **Off only; Reference is unchanged.**

**Records and layers.**
- The screen natives' existing `NotYet`s keep their record, NATIVE_GFX_BRIDGE.md, and their layer, "a hosted Flash
  movie (the port hosts none)", as `ow_world::world::mission_numbers` has them today.
- The screen natives do not move to this note. The new answers below (the no-player answers of R1 and R2, and R2's
  `NotRead`) cite this note as their record.

**Hub ruling (2026-11-19, review of #504), R1.** The question was: may the read None branch of
`UpdateMissionTextList` (step 1), and `GetVariableObject`'s no-player answer (step 2), answer for the port's stand-in
state, which the game does not produce? (The game always runs `Start`, so its list and its player are always set.)
Ruling: **yes, for both. This is one ruling for steps 1 and 2 together**; neither is allowed without the other. The
answers below are the note's proposals, now allowed.

1. **`UpdateMissionTextList`:**
   - with `MissionTextList` None: return, no write. This is the read None branch (section 2), allowed by R1;
   - with it set: keep `NotYet { LayerAbsent }`, "the mission screen's Flash text list (not hosted)". Its record is
     NATIVE_GFX_BRIDGE.md and its layer "a hosted Flash movie (the port hosts none)". Its steps rebuild a container
     that only a hosted movie fills;
   - a `MissionTextList` or `MissionCategories` the movie's class does not declare is a `VmError::Script` naming it.
2. **`GetVariableObject`:**
   - for the port's no-player state (a stand-in movie with no native player): None. No wrapper is made, the class
     cache is untouched, and nothing is written. Allowed by R1; the return is read by fit (open question 2);
   - for a world that models a native player: keep `NotYet { LayerAbsent }`, "a Flash movie query (not hosted)", with
     record NATIVE_GFX_BRIDGE.md and layer "a hosted Flash movie (the port hosts none)".
3. **`FocusOn`.** **Hub ruling (2026-11-19, review of #504), R2.** The question was: what does the port answer for
   `FocusOn(None)` in its no-player state, where the game's clamp reads a stage size that was never written?
   - **Default:** for a world that models a native player or a hosted movie, keep `NotYet { LayerAbsent }`, "the 3D
     movie's Flash focus (not hosted)", with record NATIVE_GFX_BRIDGE.md and layer "a hosted Flash movie (the port
     hosts none)".
   - **The port's no-player state, with `Thing` None:**
     - write `FocusObject` = None, then `FocusOffset` = zero, in that order, and make no other write. The ruling's
       "(0, 0)" is read as every component zero: `FocusOffset` is a `Vector`, so X, Y and Z are all 0 (section 4,
       step 3). `InterpFocusOffset` and `ChainedObjectMap` are not written;
     - a Y bound that would be negative or NaN is `NotYet { NotRead }`, "WillowGFxMovie3D.FocusOn: the focus offset's
       clamp bound", record this note. A garbage stage height, or a scale above 1, gives such a bound. The port models
       no stage size, so this applies only to a world that supplies a size and a scale;
     - UNVERIFIED, bounded by open question 3.
   - **The port's no-player state, with `Thing` set:** that cannot arise there, because R1's `GetVariableObject`
     answers None. If it did, keep the default `LayerAbsent`.
4. **The telemetry.** **Hub ruling (2026-11-19, review of #504), R3.** The question was: should the shared submit
   routine be answered as NATIVE_RESPAWN_STATIONS.md 3.5's "telemetry only"? Otherwise a None player does nothing and
   any other player is `NotRead`.
   - Ruling: the port has no telemetry service, as it has no online subsystem. So in Off, the shared submit routine
     answers nothing, with no write, for every player. This is a port answer, UNVERIFIED.
   - It is one ruling for every event that ends in that routine:
     - `RecordMissionStatusChangedEventForPlayer` (this note);
     - `RecordCharacterRevivedEventForPlayer`, which #500 already answers that way;
     - `RecordCharacterKilledEventForPlayer`, which shares the routine (section 5) and which #500 answers `NotRead`
       today. R3 covers it too; the hub sends #500 a follow-up.
   - The record step makes and frees only temporaries, so "nothing" is the whole answer for each of them.

**The expected stops** under R1 to R3. A local, uncommitted probe applied all four answers on the same throwaway merge
(#496 at 9b591ad, #497 at 7af3ab7, on main d72b96d):
- link 10 `mission_accept`: **ok, 8 / 8**;
- link 11 `mission_objective`: notyet, 4 / 8. It stops at `hit:fire`, stopped after partial writes, on "a pawn's
  QueryInterface(GearboxFramework.IAIInterface) goes to its Mesh, a Engine.SkeletalMeshComponent: what a skeletal
  mesh component answers to QueryInterface is not read" (record NATIVE_PAWN_AI_INTERFACE.md, an existing named
  `NotRead`);
- link 12 `mission_turn_in`: blocked by link 11 (the same stop).

Without R1, the stop stays at step 1's `LayerAbsent`.

## Open questions

1. The process-wide flag `UpdateMissionTextList` clears on its set branch (section 2, step 2.4).
2. `GetVariableObject`'s return on the failure branch (read by fit).
3. `FocusOn`'s scale source: the movie flag and the engine object's two values (section 4, step 3). It bounds R2's
   port answer (section 6, step 3).
4. The statistics submit paths of section 5, and the engine lookup that packs the name. R3 answers them as a port
   answer (no telemetry service); they stay unread.
5. Whether a subclass of `QuestAcceptGFxMovie` replaces the text-list body.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **The text list.** At the accept, hook `UpdateMissionTextList` and read `MissionCategories` and
   `MissionTextList.TextEntries` before and after (expect them rebuilt; the None branch cannot be seen in the game).
2. **The focus.** Hook `FocusOn` at the accept and read `FocusObject` and `FocusOffset` after it. Expect
   `FocusObject` to be the selection clip's wrapper, and `FocusOffset` the clip's centre clamped as in section 4.
3. **The telemetry.** Hook `RecordMissionStatusChangedEventForPlayer` at the accept, and compare the player's and the
   mission's script-visible state before and after. Expect no change.
