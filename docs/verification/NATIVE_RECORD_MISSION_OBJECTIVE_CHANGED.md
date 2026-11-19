# Native rules of WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-LINK12 (decision 2026-11-19-NNN, number pending). Analyst
only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py` (the three callers);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock), for
  the native's body. This native and its siblings were compared only up to their shared submit routine.

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/link12/`. This note is in our own words: no decompiler output, pseudo-code, addresses or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** On a throwaway local merge of main with #496, #497, #500, #509, #512, #524 and #526, slice link 11
(`mission_objective`) stops in `hit:fire` at this native, unbound. Link 12 sits behind it.

## 1. The callers

`RecordMissionObjectiveChangedEventForPlayer(ForPlayer, ObjectiveIndex, ObjectiveProgress, PreviousObjectiveProgress,
MissionDefinition)`, as the installed package declares it (an object, three integers and a name, no return value), is a
static native of `WillowGame.WillowLeviathanService`, the game's statistics (telemetry) service. Its key is
`WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer(object,int,int,int,name)`.

The script calls it as a static function of the service class, from two controller functions:
- **`WillowPlayerController.SetPlayersMissionObjectiveProgressToWorldsMissionObjectiveProgress`**, twice: for an
  objective whose progress changed, and for one newly counted from 0.
- **`WillowPlayerController.ClearMissionObjective`**, with the new value 0.

In every call the player is `Self`, the controller, and the name is the mission object's `Name`.

**The two middle integers: the declared order and the callers' order disagree.**
- **Declared:** the third parameter is `ObjectiveProgress`, then the fourth is `PreviousObjectiveProgress`.
- **Passed:** all three script calls pass the objective's old progress third and its new progress fourth:
  - the changed-progress call passes the previous value, then the current one;
  - the newly-counted call passes 0, then the count;
  - `ClearMissionObjective` passes the current value, then 0.

  So against the declared names, the callers hand the previous progress as `ObjectiveProgress` and the new progress as
  `PreviousObjectiveProgress`.
- **The binding key is unchanged:** `(object,int,int,int,name)`.
- **The values' meaning in the running game** (which one the service records as which) is **UNVERIFIED**. The Off
  answer (section 3) reads neither.

## 2. What it does

1. It unpacks the object, the three integers and the name.
2. It builds a temporary event record holding the three integers and the name. The name is packed with a number from
   an engine lookup that was not read, as `RecordMissionStatusChangedEventForPlayer` packs its name.
3. It hands the record and the player to **the shared statistics submit routine**, the same routine that ends
   `RecordMissionStatusChangedEventForPlayer`, `RecordCharacterRevivedEventForPlayer` and
   `RecordCharacterKilledEventForPlayer` (NATIVE_MISSION_TEXT_LIST.md section 5). Each event passes its own event-kind
   number; this one's differs from the status-changed event's.
4. It frees the temporaries.

No write to a script-visible field was seen in this native or in its record step. The submit routine's two paths were
not read, as for its siblings.

## 3. Proposed ParityMode Off answer (for a separate implementer)

The **hub's ruling R3** (2026-11-19, review of #504, NATIVE_MISSION_TEXT_LIST.md section 6) is one ruling for every
event that ends in the shared submit routine. The port has no telemetry service, as it has no online subsystem, so in
Off the routine answers nothing, with no write, for every player. This native ends in that routine (section 2), so R3
covers it:

- **Bind** `WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer(object,int,int,int,name)`, Off only,
  beside the three siblings R3 already names. Reference keeps the stub policy.
- **The answer:** nothing. No return value, no write, no load. The record step makes and frees only temporaries, so
  "nothing" is the whole answer.
- **No preflight is needed.**
  - R3 adds no telemetry-specific refusal or preflight for a valid call, including an object-valued None player (the
    game's routine returns at once for one).
  - Ordinary VM argument errors still apply, as for the R3 status sibling: a call whose arguments do not match the
    declaration is the VM's own error.
- **A port answer, UNVERIFIED.**

The rest of `RecordMissionObjectiveSetActivatedEventForPlayer` and of the other `Record*` events was not compared.
Each needs its own check that it ends in the same routine before R3 covers it.

## Open questions

1. **UNVERIFIED:** which of the two middle integers the service records as the previous and which as the new progress.
   The declared names and the callers' order disagree (section 1).
2. **UNVERIFIED:** the shared submit routine's two paths, and the engine lookup that packs the name (as for the
   siblings).

## Live check that would confirm the rule (bl-sdk, the RTSE way)

Hook the native while advancing the Fire mission's objective, and log its arguments. Expect the controller, the
objective's index, the old progress in the third position and the new progress in the fourth (the callers' order), and
the mission's name. Then compare a save-free session's script-visible
state before and after the call (expect no difference).
