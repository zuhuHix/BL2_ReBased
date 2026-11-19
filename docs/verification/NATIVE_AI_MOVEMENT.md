# Native AI movement: the bullymong's latent moves, the action-sequence state step, InitData, five mind queries and the target record (draft, 2026-11-19)

AI-assisted (Claude), analyst lane W7-AI part 3. Read from the game executable with Ghidra (`tools/ghidra/run.ps1`, read-only
project) and from the installed packages with `research/script_disasm.py`, written in our own words; no decompiler
output, pseudo-code, addresses or listing structure is reproduced here. Field names come from the cooked packages
(`tools/ghidra/class_layout.py`; for `WillowMind` the tool needs a map property's in-memory size, taken as 60 bytes because
that is the size which makes the computed `WillowMind` size equal the size its class registration gives). **Every rule is
UNVERIFIED in the running game**; the last section lists the live observations that would confirm them.

**Clean room.** The lane that wrote this note does not implement it. A fresh implementer codes it from this note alone
(slice plan step 4, W7-AI part 3). The seam for the waits is the VM's host latent actions (decision 2026-11-19-07,
`crates/ow-vm/src/latent.rs`).

## Summary

| Native | Used by the bullymong's actions | Confidence | Status |
|---|---|---|---|
| `ActionSequence.Sleep(float)` (latent) | every state of every action below | high | UNVERIFIED |
| `WillowActionSequencePawn.WaitForPath()` (latent) | every moving state | high (start, end), medium (what "valid" means) | UNVERIFIED |
| `WillowActionSequencePawn.FollowPath()` (latent) | every moving state | high (start), medium (end) | UNVERIFIED |
| `WillowActionSequencePawn.FinishPath()` (latent) | `Action_AnimAttack.RotateToTarget` | high | UNVERIFIED |
| `WillowActionSequencePawn.WaitForPawnToTurn()` (latent) | `Action_CombatPerch.DoPerch`, `Action_Patrol.PatrolPerch`, `Action_GoToScriptedDestination.ScriptedMove` | high (rule), medium (which direction it turns to) | UNVERIFIED |
| `WillowActionSequencePawn.WaitForPawnToStop()` (latent) | not called by the bullymong's states | high | UNVERIFIED |
| `ActionSequence.InterruptLatentAction()` | not called by the bullymong's states | high | UNVERIFIED |
| The action-sequence state step (no script name) | runs every action's state code | high (step), low (who calls it for the AI tree) | UNVERIFIED |
| `Action_GenericAttack.InitData()` | `Action_GenericAttack.Init` (the three `Action_AnimAttack` nodes) | high | UNVERIFIED |
| `WillowMind.CheckForPerch(bool, bool)` | `Action_CombatPerch`, `Action_Patrol` | medium (three per-perch checks not read) | UNVERIFIED |
| `WillowMind.GetPopOpDen()` | patrol and perch code, `IsTooFarFromHome` | high | UNVERIFIED |
| `WillowMind.IsInScriptedMovement()` | `Action_GoToScriptedDestination` | medium (one clearing check not read) | UNVERIFIED |
| `WillowMind.IsTooFarFromHome()` | `Action_Patrol` | high | UNVERIFIED |
| `WillowMind.ShouldLookAtPlayer()` | `Action_GoToScriptedDestination` | medium (two inputs not named) | UNVERIFIED |
| `AIComponent.GetTargetRecord(optional bool)`, `GetTargetRecordFor(Actor, optional bool)` | `WillowActionSequencePawn.ActivateEvent` (`Target_Acquired`) | high | UNVERIFIED |

## What the bullymong's actions call (script first)

The stock regular bullymong's AI definition (`AIDef_PrimalBeast`, its `NodeList`) holds seven action nodes over five
action classes: `GoToScriptedDestination` (`Action_GoToScriptedDestination`), `Pick up Object` (`Action_CombatPerch`),
`Throw Ice Spike`, `Melee` and `Back Up (Throw Rock)` (three `Action_AnimAttack` archetypes; `Action_AnimAttack` extends
`Action_GenericAttack`), `Leap Attack` (`Action_LeapAtTarget`) and `Patrol` (`Action_Patrol`).

Their state code (20 states, every one decoded by the disassembler) calls **no** `MoveTo`, `MoveToward`, `FinishAnim` or
`FinishRotation`. The only latent calls are, with the script's latent flag on each:

| Latent native | Declared on | States that call it |
|---|---|---|
| `Sleep(float)` | `ActionSequence` (not `Actor.Sleep`: an action is an `Object`, not an actor) | all states with code |
| `WaitForPath()`, `FollowPath()` | `WillowActionSequencePawn` | `Action_GenericAttack.MoveToTarget`, `Action_CombatPerch.MoveToPerch`, `Action_Patrol.PatrolRandom` / `RunHome` / `PatrolPerch` / `TryToHide`, `Action_GoToScriptedDestination.ScriptedMove` / `FollowActor` / `FollowActorFormation` |
| `FinishPath()` | `WillowActionSequencePawn` | `Action_AnimAttack.RotateToTarget` |
| `WaitForPawnToTurn()` | `WillowActionSequencePawn` | `Action_CombatPerch.DoPerch`, `Action_Patrol.PatrolPerch`, `Action_GoToScriptedDestination.ScriptedMove` |

Two more latent natives exist and are not called by these states: `WillowActionSequencePawn.WaitForPawnToStop()` and
`ActionSequence.ExecuteSequence(ActionSequence)`. `Action_LeapAtTarget.Attack` and `Action_AnimAttack.Attack` wait through
`Sleep` and animation queries (`IsPlaying`), not through a latent animation call.

The waits therefore live on the **action object**, whose class is not an actor (no `Role`), and its state code is run by
the action-sequence state step below, not by the actor tick.

## The latent wait: how it starts and how it ends

Shared shape, the stock UE3 one: a latent native records a poll number in the action's state frame and returns; the
statement then yields. On each later state step the poll for that number runs first, with the step's delta; when it
decides the wait is over it calls the action's **end-latent routine**, which clears the frame's latent action and sets
`LatentFloat` to 0. The poll numbers are 1000 (`Sleep`), 1001 (`ExecuteSequence`), 1009 (`WaitForPawnToStop`), 1011
(`WaitForPawnToTurn`), 1013 (`FollowPath`), 1014 (`WaitForPath`) and 1015 (`FinishPath`). The polls are bound to these
numbers at start-up; which poll belongs to which number was matched by what each poll tests against what its starter set
up (each pairing is one to one and consistent), not read from the binding itself.

A wait that a starter does **not** begin (the cases marked "no wait" below) lets the statement finish at once; the
script's next statement runs in the same step.

### `ActionSequence.Sleep(Seconds)`

- **Start:** only when the action's `bInitialized` is set: `LatentFloat` becomes `Seconds` and the wait begins. When
  `bInitialized` is clear nothing happens (no wait).
- **Poll:** only while `bInitialized` is set: `LatentFloat` loses the delta; when what is left is below half the delta,
  the wait ends. While `bInitialized` is clear the poll changes nothing (and the step runs no code, below).
- **Compared with `Actor.Sleep`** (NATIVE_ENGINE_CORE.md, as `crates/ow-vm/src/latent.rs` has it): the same subtraction and
  the same half-delta test, single precision. Two differences: the `bInitialized` gate, and the end routine sets
  `LatentFloat` to 0 rather than leaving the remainder.

### `ActionSequence.InterruptLatentAction()`

Calls the end-latent routine at once: whatever the action waits on ends, and its next step continues after the waiting
statement. `WillowActionSequencePawn` does not change the end routine.

### `WaitForPath()`

- **Start:** waits only when `MyNavHandle` is set and the handle's `bInitialized` flag is set; otherwise no wait.
- **Poll:** the wait ends on the first step on which the path is valid, by the same test the non-latent
  `PathIsValid()` answers (the handle judges the action's `PathData`). There is no time-out in the native.

### `FollowPath()`

- **Start:** only when the path is valid (the `PathIsValid()` test); then the handle is asked to start following
  `PathData`. If the handle accepts, it records the action as its `ControllingAction` and the wait begins. If the path is
  not valid or the handle refuses: no wait.
- **Poll:** the wait continues while `MyNavHandle` is set, the handle still reports an active path, and the handle's
  `bInitialized` flag is set. Otherwise the action runs its "path ended" step (an action-sequence routine that was not
  read) and the wait ends. Arrival is therefore the handle's decision; the action waits on it.

### `FinishPath()`

- **Start:** with `MyNavHandle` set, the handle is asked to finish the current path (both arguments zero). If it answers
  that it is done, no wait; otherwise the wait begins. With no handle: no wait.
- **Poll:** asks the same question each step; the wait ends when the handle answers done or when `MyNavHandle` is gone.

### `WaitForPawnToTurn()`

- **Start:** always waits. With `MyWillowPawn` set, the pawn is asked for a direction vector (a pawn virtual that was not
  named, read as the facing target's direction: UNVERIFIED), and its yaw is stored in `LastYawCheck`. Yaw here is
  `atan2(Y, X)` scaled to Unreal rotation units by 65535 / (2 pi) and rounded to the nearest integer.
- **Poll:**
  1. With no `MyWillowPawn`, no `MyWillowMind`, or the mind's `FacingPolicy` equal to `FACE_NoChange`, the wait ends.
  2. Otherwise the target yaw is computed again from the same pawn query. The error is the pawn's
     `DesiredRotation.Yaw` minus the target yaw, and the previous error is `DesiredRotation.Yaw` minus `LastYawCheck`,
     both wrapped to a signed 16-bit angle.
  3. The wait continues (and `LastYawCheck` takes the new target yaw) when the error's magnitude is above 255 **and** at
     least one of these holds: the error and the previous error are on the same side (both positive, or both zero or
     negative); the previous error's magnitude is above 16383; the error's magnitude is above 16383. Otherwise the wait
     ends.
- **In words:** it ends once the pawn's desired yaw is within 255 units (about 1.4 degrees) of the direction, or once the
  error has crossed zero within a quarter turn (an overshoot). It compares the **desired** yaw, not the pawn's current
  rotation.

### `WaitForPawnToStop()`

- **Start:** always waits.
- **Poll:** the wait ends when every component of `MyGearboxPawn`'s `Velocity` and `Acceleration` is exactly 0. (With no
  `MyGearboxPawn` the native reads through a missing object; the port should refuse that case rather than invent an
  answer.)

### `ExecuteSequence(ActionSequence)` and `RunSubSequence(ActionSequence)`

Not used by the bullymong. Read only far enough to note: `ExecuteSequence` starts a child sequence (found again, or made
from the given one), links it as the action's `ChildSequence` with the action as its `ParentSequence`, starts it and
waits; when the child's state code runs out, the child's step tells the parent, which ends the parent's wait.
`RunSubSequence` does the same with nothing for a missing argument. Not needed for the slice; the details are not read.

## The action-sequence state step

Each action runs its own state code; it is not the actor's state step (`crates/ow-vm/src/states.rs`). One step, with
the delta the caller passes:

1. Nothing when the action has no state frame or no code, or when an engine check on the object (read as "is being
   destroyed", UNVERIFIED) says so.
2. The pending wait's poll runs first, with the delta.
3. Then, while `bInitialized` stays set, statements run until there is no code left or a wait is pending. A run of more
   than one million statements in one step clears the code (a guard). When the active state changes, the run continues
   in the new state; the **fifth** change within one step ends the step (the actor step of NATIVE_ENGINE_CORE.md allows
   five restarts; this one stops at the fifth change).
4. After the run, when the code has run out and the action has a `ParentSequence`, the parent is told that its child
   finished (above).

A second, very similar entry point also exists. It runs only while `bInitialized` is set and `bDoWorkFinished` is clear,
and uses the same four-change bound. Which of the two the AI component uses for a running tree action is not read.

**Who calls it, and with what delta.** The step is reached from the rule engine's tick, which load-balances. The engine
adds each frame's time to its own accumulator. Each running rule's sequence adds that accumulated time to its
`StoredDeltaTime`. When the engine's update is due (the accumulator reaches the engine's interval, or the update is
forced), every sequence is stepped with its `StoredDeltaTime`, which then returns to 0. A sequence with
`bDoNotLoadBalance` is stepped on every tick with its own accumulated time. Whether the AI component's tree (NATIVE_AI_AWARENESS.md, "AI tree natives") steps its running action through this path, with which interval,
and in what order relative to the action's `Update` event is **not read** (open question 1). Until it is, the delta a
tree action's state code sees is UNVERIFIED.

## `Action_GenericAttack.InitData()`

Fills the attack's own fields from its `Limits` (an array of `AttackLocation` objects) and the pawn; script calls it from
`Init`. Nothing happens when `MyWillowPawn` or the pawn's collision cylinder is missing. Otherwise, in this order:

1. `Type` becomes `Attack_Far`.
2. The first `Limits` entry that is an `AttackLocation_Range`, if any:
   - the movement is limited when `Range.MaxVal` is below 512 or `Range.MinVal` is below the pawn's collision radius
     (the radius field is read as the cylinder's `CollisionRadius`; the layout tool places that field 16 bytes earlier
     than the native reads, so the reading is UNVERIFIED);
   - the flag the layout tool names `bCanBeLimitedByDownedPlayers` takes that answer. `bLimitedMovement` is **not**
     written. The tool's bit order is taken as right because the neighbouring bit it names `bWantsFullLOSTrace` is the
     one that receives `bFullLineOfSight` in step 4; still UNVERIFIED, and the first live check below settles it;
   - if the entry's `bCanPath` is clear, `Type` becomes `Attack_InPlace`;
   - otherwise, if the movement is limited, `Type` becomes `Attack_Close` and the entry's `Range.MinVal` is set to 0 (the
     entry object itself is changed).
3. The first `AttackLocation_Move` entry, if any: `IdleTime` becomes its `MaxTime`.
4. The first `AttackLocation_LineOfSight` entry, if any: `bWantsFullLOSTrace` becomes its `bFullLineOfSight`.
5. When `CrouchIdleChance` or `CrouchMoveChance` is above 0, `bWantsFullLOSTrace` is set.

"First entry that is a" means the first non-empty element whose class is that class or a subclass of it.

## `AIComponent.GetTargetRecord(optional bool bParent)` and `GetTargetRecordFor(Actor, optional bool bParent)`

`WillowAIComponent` does not override either one.

- `GetTargetRecord(bParent)` is `GetTargetRecordFor(CurrentTarget, bParent)`.
- `GetTargetRecordFor(A, bParent)`:
  - no actor: none;
  - `bParent` set and the component has a parent component (`GetParentComponent`): the parent's
    `GetTargetRecordFor(A, true)`, so the search climbs to the top of the chain;
  - otherwise: the first non-empty record in the component's `Targets` array whose `Target` is `A`, or none.

**Why the attacks answer `NoTarget` in the port today.** `WillowActionSequencePawn.ActivateEvent` (`Target_Acquired`) sets
`TargetRec` and `ParentTargetRec` from these two calls, cast to `WillowMindTargetInfo`. The port keeps target records in
Rust, so the component's `Targets` array holds no record objects, and both calls answer none. For the script to see the
target, `Targets` must hold `WillowMindTargetInfo` objects whose `Target` is the actor, and `CurrentTarget` must be that
actor at the time of the event.

## The five `WillowMind` queries

### `CheckForPerch(bool bCombat, bool bClosest)`

The script calls it as `CheckForPerch(True, bPickClosestPerch)` from `Action_CombatPerch` and as
`CheckForPerch(False, True)` from `Action_Patrol`. It fills one slot: `CombatPerch` when `bCombat` is set, otherwise
`PatrolPerch`.

1. A perch already in the slot that is being destroyed (`bDeleteMe`) is dropped from the slot.
2. The query stops, leaving the slot as it is, when the slot still holds a perch, when the pawn's `BodyClass` is missing,
   when there is no den (`GetPopOpDen`), or when the mind has no `Pawn`.
3. The den is asked to bring its perch list up to date (a den call that was not read), then each of the den's `Perches`
   is a candidate when all of these hold:
   - it has no `User`;
   - it has a `PerchDef`;
   - it is `bEnabled`;
   - it is not being destroyed;
   - a perch check returns false (not named);
   - its `PerchDef.bCombatPerch` equals `bCombat`;
   - a check that takes the pawn's `BodyClass.BodyTag` returns true (not read);
   - a second perch check on the mind returns true (not named);
   - a third check on the mind returns true (not named).
4. With `bCombat` set, a candidate must also be within `UseHeight` of the pawn vertically and within `UseRadius`
   horizontally (2D distance), both strict. Patrol perches have no distance test.
5. When there is at least one candidate:
   - with `bClosest`, the slot takes the candidate nearest the mind's own location (2D, the first of equals);
   - otherwise it takes a uniformly random candidate (the C runtime's `rand()` modulo the count).

   The chosen perch's `User` becomes the mind's `MyWillowPawn`. With no candidate, the slot is cleared.

### `GetPopOpDen()`

The mind's den, through the world's population master (the two engine look-ups on the way are read as "the world
info" and "its population master", UNVERIFIED): the population opportunity at index `PopulationOpportunityIndex`
of the master's opportunity list, answered only when it is a `PopulationOpportunityDen`. The answer is none when there is
no world or master, when the index is out of range, or when the opportunity is not a den.

### `IsInScriptedMovement()`

First, a set `FollowActor` is cleared when an engine check on it (not read; read as "that actor is gone or dead",
UNVERIFIED) says so. Then the answer is true when any of these is set: `ScriptedMoveTarget`, `ScriptedFocus`,
`bScriptedHoldPosition` or `FollowActor`.

### `IsTooFarFromHome()`

True when there is a den and a `Pawn`, and the 2D distance from the pawn's location to the den's location is strictly
greater than the den's `PatrolRadius`. False otherwise, including with no den.

### `ShouldLookAtPlayer()`

1. False when `MyWillowPawn` or `AIClass` is missing, or when a global engine condition is off (read as "a game world
   is running", UNVERIFIED).
2. True when a timed flag of the mind is on (an `Object.Flag` that switches itself off once its duration has passed;
   which flag is not read) or when a pawn check is true (not named).
3. Otherwise true exactly when `AIClass.bWatchPlayerWhenUsed` is set, `LastUsedTime` is above 0, and the time since
   `LastUsedTime` is less than `AIClass.WatchPlayerTime`.

## What this means for the port (facts, not a design)

- The waits the slice needs are `Sleep`, `WaitForPath`, `FollowPath`, `FinishPath` and `WaitForPawnToTurn`, on
  `ActionSequence` objects. Their end conditions are all polls of state the host or the existing `Locomotion` trait holds
  (path validity, the handle's active path and finish answer, the pawn's desired yaw), plus `LatentFloat`.
- Each wait starts in one step and is first polled in the next, as in the VM's host latent seam.
- The state step's restart bound differs from the actor step's (the fifth change ends it), and `Sleep` here is gated by
  `bInitialized`.
- Target records must exist as VM objects in `Targets` for `GetTargetRecord`.

## Open questions

1. Which step entry point the AI component uses for a running tree action, with which delta (the rule engine's
   load-balanced `StoredDeltaTime`, or the frame's), and whether the state step comes before or after the action's
   `Update` event in a tick.
2. What the navigation handle counts as an "active path" for `FollowPath`'s poll, and what the action's "path ended"
   routine does.
3. Which direction the pawn's query in `WaitForPawnToTurn` returns (focal point, move target or something else).
4. In `ShouldLookAtPlayer`: which timed flag of the mind, and which pawn check.
5. In `IsInScriptedMovement`: the check that clears `FollowActor`.
6. In `CheckForPerch`: the den's perch refresh, the two unnamed perch checks, the body-tag check and the mind check.
7. The `ExecuteSequence` poll (number 1001): not needed by the bullymong, not read.
8. Whether the random perch choice draws from the same C runtime generator as other game systems (it matters for
   reproducing a run, not for behaviour).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- A patrolling bullymong: hook its action's state code around `FollowPath` and record the step on which the statement
  after `FollowPath` first runs, the pawn's location at that step, and the handle's state. That confirms the `FollowPath`
  end, and the delta between steps answers open question 1.
- During `WaitForPawnToTurn` in `PatrolPerch`, read `LastYawCheck` and the pawn's `DesiredRotation.Yaw` each frame until
  the statement after it runs; the final error should be within 255 units or show a sign change.
- During a `Sleep`, read the action's `LatentFloat` each frame; it should fall by the frame time and end below half of it.
- After `Target_Acquired`, read `TargetRec.Target` against the component's `CurrentTarget`, and `ParentTargetRec` on a
  den-led pawn.
- Read `Type`, `bLimitedMovement`, `bCanBeLimitedByDownedPlayers`, `IdleTime` and `bWantsFullLOSTrace` on the three
  `Action_AnimAttack` copies after `Init` (this also settles which flag `InitData` writes).
- Call the five mind queries through pyunrealsdk on a live bullymong near and away from its den; for `CheckForPerch`,
  read the two perch slots and the chosen perch's `User`.
