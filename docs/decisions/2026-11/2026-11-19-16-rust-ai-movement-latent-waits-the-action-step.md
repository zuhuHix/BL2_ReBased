## 2026-11-19: Rust AI movement: the latent waits, the action-sequence step, target records and the mind queries

AI-assisted (Claude Code). Lane W7-AI part 3 (implementer), the fresh implementer of
`docs/verification/NATIVE_AI_MOVEMENT.md` (the analyst lane's note, branch `docs/rust-ai-part3-notes`). This lane coded
from the note alone and opened no analysis output. There was no real-game session, so every rule is **UNVERIFIED** in
the running game. No dependency was added, no bounds check was loosened and no existing `NotYet` guard was relaxed.
Everything is `ParityMode::Off` only: there is no C++ AI, so Reference keeps what it had (R3). The lane was
rebased onto main after #389 (the VM's host latent seam, which the waits use) landed.

- **What was added.**
  - `crates/ow-ai/src/awareness/movement.rs` (+ tests), bound from `actions::Pack` before that pack's own `NotYet` for
    the latent moves, which now cover only Reference.
  - In `crates/ow-vm/src/states.rs`:
    - `process_action_sequence`, the action-sequence state step (`process_state` is unchanged: the two share one step
      routine with a `StepRule`);
    - `MAX_ACTION_SEQUENCE_RESTARTS` (4);
    - `has_state_code`.
- **The waits**, on the action object, through the VM's host latent seam (start in one step, first poll in the next):
  - **`ActionSequence.Sleep(Seconds)`.**
    - It starts only with `bInitialized` set: `LatentFloat` = `Seconds`.
    - Each poll subtracts the delta; the wait ends when the remainder is below half the delta. The end writes
      `LatentFloat` 0, and the arithmetic is single precision.
    - The poll changes nothing while `bInitialized` is clear.
  - **`InterruptLatentAction`:** the wait ends at once and `LatentFloat` becomes 0.
  - **`WaitForPath`.**
    - No `MyNavHandle`, or a handle whose `bInitialized` is clear, means no wait.
    - Otherwise the wait ends on the first poll where the movement host's `PathIsValid` answer (the same as the
      non-latent native) is true.
    - With no movement host the poll is `NotYet { LayerAbsent }`, naming the navigation mesh.
  - **`FollowPath`:** an invalid path means no wait. A valid one is `NotYet { LayerAbsent }` naming the navigation
    mesh: the handle's start, its "active path" answer and the action's "path ended" routine are not movement-host
    answers, and the note leaves them open (its question 2).
  - **`FinishPath`:** no handle means no wait. With one it is `NotYet { LayerAbsent }`: the handle's "finished" answer
    is a navigation-mesh answer.
  - **`WaitForPawnToTurn`.**
    - It always waits. With no pawn, no mind, or `FacingPolicy` = `FACE_NoChange`, the first poll ends it.
    - With `MyWillowPawn` set it is `NotYet { NotRead }`: the direction the pawn query returns is the note's question 3.
    - The note's end rule is coded as `turn_wait_continues`, and `yaw_of` gives atan2 in 65535 units. Both have tests
      and wait for that direction.
  - **`WaitForPawnToStop`:** it always waits, and ends when `MyGearboxPawn`'s `Velocity` and `Acceleration` are exactly
    0. With no `MyGearboxPawn` it is refused (`NotYet { NotRead }`), as the note asks.
- **The action-sequence state step** (`ow_vm::states::process_action_sequence` and `ow_ai::...::movement::step_action`):
  - the pending poll runs first;
  - code runs only while `bInitialized` stays set, checked before each run and after each statement. A run stopped by
    the gate resumes at the next statement;
  - the fifth state change ends the step. The actor step allows five restarts, which is six runs; this one gives five
    runs;
  - the note's million-statement guard (clear the code and end the step) is not ported as such: the VM's per-run
    step budget stands in for it, stricter, and errors with a step limit instead (review r1 of #397);
  - a cleared `bInitialized` inside a `foreach` of the state code is `NotYet`, as a latent call or a `PushState`
    there is: how the loop survives the stop is not read (review r1 of #397);
  - when the step runs the code out on an action with a `ParentSequence`, the parent's notice is
    `NotYet { NotRead }` (the `ExecuteSequence` wait is not read).
  - **The brain does not call the step yet.** Which entry point the AI component uses for a running tree action, with
    which delta and in what order against `Update`, is the note's open question 1. So the bullymong's state code, and
    with it these waits, runs in no world yet; only a caller of `step_action` reaches them.
- **Target records.**
  - `AIComponent.GetTargetRecord` is `GetTargetRecordFor(CurrentTarget)`.
  - `GetTargetRecordFor(A)` returns the first non-empty record in `Targets` whose `Target` is `A`. No actor answers
    None.
  - `bParent` climbs to the parent component. No component has a parent in this port yet (`awareness::vm`, "No
    parent"), so a lone one searches its own list; one whose awareness state says it has a parent is `NotYet` (the
    den's records; review r1 of #397).
  - The port keeps its records in Rust and writes no `WillowMindTargetInfo` objects into `Targets`, so the answer is
    still None. That is the note's reason the attacks find no record at `Target_Acquired`.
  - Mirroring the records as VM objects needs the record's fields as the game fills them, which the note does not
    give. That stays an open item for the hub.
- **The mind's queries** (`WillowMind`):
  - `IsTooFarFromHome` is false with no `Pawn`; otherwise it needs the den.
  - `GetPopOpDen` is `NotYet { NotRead }`: the world's population master and its opportunity list are named only as
    "read as".
  - `IsInScriptedMovement` gives the note's answer when `FollowActor` is None, and `NotYet` when it is set (question 5).
  - `ShouldLookAtPlayer` is false with no `MyWillowPawn` or `AIClass`; otherwise `NotYet` (question 4 and the global
    condition).
  - `CheckForPerch` is `NotYet` (question 6).
  - `Action_GenericAttack.InitData` is not bound: the hub did not ask for it, and the radius field's offset is
    UNVERIFIED in the note.
- **Left alone, as asked:** the Pawn IAIInterface refusal and the `WillowAIComponent` hold question.
- **Proof.**
  - **Synthetic tests** (13 in `movement/tests.rs`, on an invented `ToyAction` that is an `Object`, not an actor):
    - the Sleep timeline (0.5, then 0.25, then the end with 0);
    - the `bInitialized` gate (no code, a frozen poll, a stop after the statement that clears it);
    - five runs against the actor step's six;
    - the interrupt;
    - `WaitForPath` with no handle, an uninitialised handle, no host (`NotYet`), then an invalid and a valid path;
    - `FollowPath` and `FinishPath` against the nav mesh;
    - the turn wait and the stop wait;
    - the turn rule and `yaw_of`;
    - the parent notice;
    - the target records;
    - the mind queries' branches;
    - the stock paths bound in Off and nothing under Reference.

    The existing `ow-vm` tests (192) and `ow-ai` tests pass unchanged.
  - **Installed, before and after** (pc-hummer, Off; "before" is 22e0964 with `--slice-gate` applied; outputs under
    ignored `local/p2/ai-movement/`):
    - `--combat-run` with COMBAT_DEATH.md's kill steps, and with S-f's steps (`maya:8 players:1 spawn:adult:7
      cash:3000` and four melee swings, then `bleed:50`): the same rows (0) and the same errors before and after; only
      `elapsed_ms` differs. W6-COMBAT's own 15-step list is not recorded verbatim in the tree, so these recorded lists
      stand in for it.
    - `--slice-gate`, strict and past-errors: byte-identical (0/5/9 and 0/12/2).
    - **Link 4 does not move:** it stops at `spawn:adult:7` on the game replication info query (the map load's
      handler), before any AI runs.
    - The ignored installed action-tree test (`installed_actions`) is byte-identical. It runs in Reference, where
      these bindings do not apply.
- **The next stops for the AI path**, for the hub:
  - every Off spawn's replication info handler;
  - who steps a tree action's state code, and with which delta (question 1);
  - target records as VM objects;
  - the navigation mesh's handle answers;
  - the pawn's turn direction (question 3).
