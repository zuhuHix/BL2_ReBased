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
1. **The chain step.** With `Thing` None it returns at once. Otherwise it registers `Thing` and its `_parent` chain in
   `ChainedObjectMap`, through Flash lookups.
2. **`FocusObject` = `Thing`** (a script-visible write).
3. **The offset step:**
   - `FocusOffset` is zeroed;
   - with a `FocusObject` set, it is centred on the object's bounds (Flash queries);
   - a final clamp limits its Y to plus or minus half the margin between the stage height and the stage height times a
     scale.
   - The stage size comes from the native player's view. **With no native player, the size is never written**, so the
     clamp's bound is undefined.
   - The scale is (1, 1) when a flag of the movie is set, else two values of an engine object. Neither the flag's nor
     the object's identity was read.

**So for `FocusOn(None)` on a movie with no native player, the game's result is undefined.** That case reads an
unwritten stage size, and only a scale of exactly 1 makes the clamp's bound 0. Field names: `FocusObject`, then
`FocusOffset`, by the class's declaration order (`ChainedObjectMap`, `RedundantClipArray`, `FocusObject`,
`FocusOffset`, `InterpFocusOffset`), by fit, UNVERIFIED.

## 5. `WillowLeviathanService.RecordMissionStatusChangedEventForPlayer(Player, Status, Name)`

1. It unpacks an object, a byte and a name.
2. It builds a temporary event record that holds the byte and the name, the name packed with a number from an engine
   lookup that was not read.
3. It hands the record and the player to the statistics submit routine.
4. It frees the temporaries.

**The submit routine** returns at once for a None player. Otherwise it asks the player one virtual and takes one of two
submit paths, neither of which was read.

The same submit routine ends `RecordCharacterRevivedEventForPlayer`, which NATIVE_RESPAWN_STATIONS.md section 3.5 read
as "telemetry only, changes no game object" in what it read. No write to a script-visible field was seen in this
native or in its record step.

## 6. Off answers, step by step (for an implementer)

The hub's rulings apply:
- preflight what is cheap and certain;
- a `NotYet` after a write is terminal and keeps its kind and record. The natives below answer plainly; the bridge's
  status hook marks them "stopped after partial writes" (decision 2026-11-19-93);
- grade by the `VmError` variant and the `NotYet` kind, never by text;
- **Off only; Reference is unchanged.**

1. **`UpdateMissionTextList`:**
   - with `MissionTextList` None: return, no write. This is the read None branch (section 2), not a UI skip. **Hub
     call:** the port's list is None only because the stand-in movie never runs `Start`, while the game's is always
     set. The hub decides whether the read branch may answer for a state the game does not produce;
   - with it set: keep `NotYet { LayerAbsent }`, "the mission screen's Flash text list (not hosted)". Its steps
     rebuild a container that only a hosted movie fills;
   - a `MissionTextList` or `MissionCategories` the movie's class does not declare is a `VmError::Script` naming it.
2. **`GetVariableObject`:** keep `NotYet { LayerAbsent }`, "a Flash movie query (not hosted)". The no-player answer
   (None) is the game's rule for a state the game does not reach, and its return is read by fit. The same hub call as
   step 1 applies: if the hub allows answering for the port's no-player state, the answer is None, with no write.
3. **`FocusOn`:**
   - keep `NotYet { LayerAbsent }`, "the 3D movie's Flash focus (not hosted)" (the existing text), in every case. With `Thing` None and no native
     player, the game's clamp reads a stage size that was never written (section 4), so there is no defined answer to
     port;
   - **not** "nothing": `FocusObject` and `FocusOffset` are written in every case.
4. **`RecordMissionStatusChangedEventForPlayer`:** bind it, so it is no longer an unbound `error`:
   - a None player: nothing (the submit routine returns at once, and only temporaries are made and freed);
   - any other player: `NotYet { NotRead }`, "WillowLeviathanService.RecordMissionStatusChangedEventForPlayer: the
     statistics submit paths";
   - if the hub accepts NATIVE_RESPAWN_STATIONS.md 3.5's "telemetry only" for the shared submit routine (lane
     IMPL-GO-DOWN-NATIVES binds the revived sibling that way, #500), the same answer applies here: nothing, no write.

**The expected stops.**
- Today: step 1's `LayerAbsent`.
- If the hub lets steps 1 and 2 answer for the port's state, the stop is `FocusOn`'s `LayerAbsent` (step 3), which stays
  until a movie is hosted.
- The probe of section 1, which also let `FocusOn` pass, met step 4 next; what follows it was not probed.

## Open questions

1. The process-wide flag `UpdateMissionTextList` clears on its set branch (section 2, step 2.4).
2. `GetVariableObject`'s return on the failure branch (read by fit).
3. `FocusOn`'s scale source: the movie flag and the engine object's two values (section 4, step 3).
4. The statistics submit paths of section 5, and the engine lookup that packs the name.
5. Whether a subclass of `QuestAcceptGFxMovie` replaces the text-list body.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **The text list.** At the accept, hook `UpdateMissionTextList` and read `MissionCategories` and
   `MissionTextList.TextEntries` before and after (expect them rebuilt; the None branch cannot be seen in the game).
2. **The focus.** Hook `FocusOn` at the accept and read `FocusObject` and `FocusOffset` after it. Expect
   `FocusObject` to be the selection clip's wrapper, and `FocusOffset` the clip's centre clamped as in section 4.
3. **The telemetry.** Hook `RecordMissionStatusChangedEventForPlayer` at the accept, and compare the player's and the
   mission's script-visible state before and after. Expect no change.
