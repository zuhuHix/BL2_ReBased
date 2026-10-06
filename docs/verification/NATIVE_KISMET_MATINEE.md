# Native Kismet and Matinee: Matinee playback, Move/Event tracks, scripted walk, remote events, populated events (2026-10-05)

AI-assisted (Claude), analyst lane G10. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script-level facts (UnrealScript bodies, class defaults, state code)
were read with `research/script_disasm.py` and the package reader and are labelled "script".

Impulse scheduling, event activation checks, `ActivateOutputLink` and the per-frame op loop are already in
[NATIVE_MISSION_DISPATCH.md](NATIVE_MISSION_DISPATCH.md) A4 and are not repeated. Overlaps: dialog started by the
`GearboxSeqAct_TriggerDialogName` op is G6 ([NATIVE_DIALOG.md](NATIVE_DIALOG.md)); behavior-kernel and population spawning
internals are G2 ([NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md)); generic actor movement, timers and
`Actor.TriggerEventClass` are G3 ([NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md)); mission-tracker events are C1/G9.

Terms used below. "The op" is the `SeqAct_Interp` node. "Group actor" is the actor bound to a Matinee group through the
op's object-variable link whose description equals the group name (Fire world: group `door` -> `InterpActor_13`, group
`Target` -> `InterpActor_4`). Field names are the script names.

## Summary
| Native / behavior | Kind | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| SeqAct_Interp activation (inputs Play, Reverse, Stop, Pause, Change Dir, Last Frame) | virtual natives `Activated`, `UpdateOp` | range door, target Matinee | medium-high | UNVERIFIED |
| SeqAct_Interp position advance (`StepInterp`) and apply (`UpdateInterp`) | virtual natives | door, target | medium-high | UNVERIFIED |
| SeqAct_Interp `Deactivated` (Completed / Reversed outputs, InterpolationFinished) | virtual native | door, target | medium-high | UNVERIFIED |
| `SeqAct_Interp.SetPosition`, `Stop`, `AddPlayerToDirectorTracks`, `IsNetworkReady` | registered natives | not used by the slice graph | low | UNVERIFIED |
| InterpGroup.UpdateGroup / InterpTrack.UpdateTrack wrapper | virtual natives | door, target | medium | UNVERIFIED |
| InterpTrackMove key evaluation (`GetLocationAtTime`, curve eval, `InitTrackInst`) | virtual natives | door, target | medium-high for the maths, low for who applies the pose | UNVERIFIED |
| InterpTrackEvent.UpdateTrack (event keys -> named op outputs) | virtual native | target bool outputs | high | UNVERIFIED |
| SeqAct_ActivateRemoteEvent.Activated / PlayerController.ServerRemoteEvent | virtual native / script | all remote-event hops | high | UNVERIFIED |
| Behavior_MissionRemoteEvent.ApplyBehaviorToContext | virtual native | mission -> Marcus walk | medium | UNVERIFIED |
| WillowSeqAct_AIScripted -> WillowMind.OnAIScripted, Action_GoToScriptedDestination, state FollowMoveNodes | script (AI) plus natives ReachedDestination, FollowPath, WaitForPath, WaitForPawnToTurn | Marcus walk | medium-high for flow, low for arrival maths | UNVERIFIED |
| SeqEvent_ArrivedAtMoveNode / SeqEvent_LeavingMoveNode | fired from the AI state code (script) | door open/close | high | UNVERIFIED |
| SeqEvent_PopulatedActor / SeqEvent_PopulatedPoint (`NotifyPopulatedActor`) | script event, fired by native spawn code | den/point -> Instigator variable | medium | UNVERIFIED |

## Fire-mission world graph (script data, for orientation)
From the mission closure graph (`RocksPaperGenocide` sequence): `Interp_0` (group `Target`, bound to `InterpActor_4`,
`bRewindOnPlay` true) is played by `SeqCond_CompareBool_0`.True and reversed by `RocksPaper_TargetKilled_Reset` /
`RocksPaper_SendTargetBack`. Its event tracks fire `ChangeBool_FALSE` (forward only, key at 0) and `ChangeBool_TRUE` (reverse
only, key at 0; a second key at 2.0 named `WheelBackward` has no output in the graph). `Interp_2` (group `door`,
`InterpActor_13`, defaults) is played by `ArrivedAtMoveNode` events of nodes 12 and 39 and by `RE_Ep14_OpenMarcusDoor`, and
reversed by the events of nodes 26 and 18 and by `RE_Ep14_CloseMarcusDoor`. Marcus: `WillowSeqAct_AIScripted_2` (`FocusStyle`
`ESF_Path`) is entered by the four `WillowSeqEvent_MissionRemoteEvent` nodes named `RocksPaper_MoveMarcusToRange` (Fire,
Shock, Corrosive, Amp) and has Destination = move node 12; `AIScripted_0` (to the shop) has Destination = node 39.
Both Move tracks have exactly two keys, `CIM_CurveAutoClamped`, stored tangents all zero (the target's last key also stores a
tiny leave tangent that is never used), `MoveFrame` `IMF_RelativeToInitial`, all lookup-group names `None`.

## SeqAct_Interp: inputs, state and timing

Inputs (script defaults): 0 Play, 1 Reverse, 2 Stop, 3 Pause, 4 Change Dir, 5 Last Frame. Outputs: 0 Completed, 1 Reversed,
then one output per distinct event-track key name (added by the editor; the slice data carries them as extra outputs).
The op is latent (`bLatentExecution` true). Flags below are the script booleans of the op.

- **Reads:** `InterpData` (its `InterpLength`), `GroupInst`, `Position`, `PlayRate`, `bIsPlaying`, `bPaused`, `bReversePlayback`,
  `bLooping`, `bRewindOnPlay`, `bNoResetOnRewind`, `bRewindIfAlreadyPlaying`, `bForceStartPos`/`ForceStartPosition`,
  `bIsSkipped`, `bSkipNextUpdate`, the op's input impulses, `LatentActors` (the actors bound through the op's object variables).

### Activated (the op becomes active with an impulse on some input)
- Does nothing if the op is already playing (the running `UpdateOp` handles later impulses, below).
- Otherwise it ignores Stop and Pause impulses and proceeds only for Play, Reverse, Change Dir or Last Frame.
- If Play arrived and `bRewindOnPlay` is set (and the op is not playing, or `bRewindIfAlreadyPlaying`), `Position` is set to 0
  **before** the group instances are built.
- If Last Frame arrived, the op marks itself `bLastFrameEventFired` and `bSkipNextUpdate`.
- Builds the group instances (`InitInterp`, below), then applies the starting input: Play -> "play" action; else Reverse ->
  playing + reverse + not paused (no position change); else Change Dir -> "change direction" action.
- For every object-variable actor that has a group instance: the actor is added to `LatentActors`, and the script event
  `InterpolationStarted(InterpAction=this op, GroupInst)` is invoked on it.
- A Last Frame impulse additionally applies the end position (`UpdateInterp` at `InterpLength`, jump) and stops the op.

### InitInterp (group instances)
- If group instances already exist they are all terminated and the array cleared; the op's `InterpData` is re-resolved from
  its Data variable link. `bShouldShowGore` is derived from the game settings (default allowed).
- One group instance per InterpGroup (a folder group is skipped); the group actor is the variable-bound actor whose link
  description matches the group name; the instance creates one track instance per track and runs each track instance's
  init (for Move tracks this captures the initial transform, see below). Nothing in the terminate path restores an actor's
  pose; poses persist after playback.
- **Consequence:** every start from the stopped state re-captures the initial transform from the actor's current pose.

### UpdateOp (called every frame the op is active, after Activated in the activation frame)
Order of effects in one call:
1. If `bSkipNextUpdate` is set: clear it and return "not done" (nothing else).
2. Input handling, first match wins, using the op's input impulses:
   - playing and Pause impulse: toggle `bPaused` (only while playing) -> notify actors.
   - Play impulse: "play" action -> notify actors.
   - else Reverse impulse: `bIsPlaying`=true, `bReversePlayback`=true, `bPaused`=false -> notify actors.
   - else Stop impulse: `bIsPlaying`=false, `bPaused`=false (no notification).
   - else Change Dir impulse: `bIsPlaying`=true, `bPaused`=false, `bReversePlayback` toggled -> notify actors.
   - else (no impulse): if not playing, return **done** (the op deactivates, see Deactivated); if playing, continue.
3. Clear the impulses of inputs 0 to 4 (not input 5).
4. Run `StepInterp(DeltaTime, preview=false)`; return "not done".
"Notify actors" = invoke the script event `InterpolationChanged(InterpAction)` on every `LatentActors` entry and call
`MatineeActor.Update` on the replicated actor if one exists. `InterpActor.InterpolationChanged` plays the moving sound
(script).
- **"Play" action:** if `bForceStartPos` and not playing -> `UpdateInterp(ForceStartPosition, jump)`; else if `bRewindOnPlay`
  and (not playing, or `bRewindIfAlreadyPlaying`) -> optionally re-baseline relative Move tracks (only when
  `bNoResetOnRewind`, see InterpTrackMove) then `UpdateInterp(0, jump)`; then `bIsPlaying`=true, `bPaused`=false,
  `bReversePlayback`=false. A Play impulse while playing in reverse therefore turns playback forward from the current position.
- Consequence for timing: because a finished run is only seen as "not playing" on the **next** `UpdateOp`, the Completed or
  Reversed output impulse is raised one frame after the last position update.

### StepInterp(DeltaTime) (advance)
- Returns at once if not playing, paused, or no `InterpData`. If `bClientSideOnly` and `bSkipUpdateIfNotVisible`, it skips
  while no bound actor was rendered in the last second (not relevant to the slice).
- Forward: new = `Position` + `PlayRate` * DeltaTime. If new <= `InterpLength`: apply it. If new > length and not looping
  (or `bIsSkipped`): clamp to `InterpLength` and mark finished. If looping: apply the length, then apply 0 as a jump, wrap by
  subtracting the length until within range.
- Reverse: new = `Position` - `PlayRate` * DeltaTime; if new >= 0 apply it; if below 0 and not looping: clamp to 0, finished;
  looping: apply 0, then apply the length as a jump, then add the length until >= 0.
- The new position is always applied through `UpdateInterp` (below), **including the clamped end position**, so the end pose is
  exactly reached. When finished: `bIsPlaying`=false and `bPaused`=false. The replicated actor, if any, is refreshed.
- `PlayRate` default 1; `Position` default 0.

### UpdateInterp(NewPosition, preview, jump)
- Clamps the position to [0, InterpLength], orders group instances so a group whose actor is attached to another group's actor
  updates after it, drops group instances whose actor has been destroyed, then for each group instance calls the group update
  (below), then (only with `bInterpForPathBuilding`) a path-build key step, then stores `Position` = the clamped value.
- If Last Frame was requested, only groups with `bRunTracksWhenSkippingToLastFrame` are updated.

### Deactivated (when `UpdateOp` returned done)
- If `InterpData` exists: if `Position` >= 0.0001: when `Position` > `InterpLength` - 0.0001 activate output 0 **Completed**
  (unless that output is disabled, or the last-frame-event suppression applies: `bLastFrameEventFired` set and
  `bFireCompleteEventWhenJumpToLastFrame` clear). If `Position` < 0.0001: activate output 1 **Reversed**. A stop in the middle
  fires neither.
- Clears `bLastFrameEventFired` and `bSkipNextUpdate`; invokes `InterpolationFinished(InterpAction)` on every `LatentActors`
  entry that is alive and calls `MatineeActor.Update`; empties `LatentActors`. The group instances stay.
- `InterpActor.InterpolationFinished` (script, see the mover prototype): plays the opened/closed sound, starts the
  `StayOpenTime` timers `FinishedOpen`/`Restart`, tells a `DoorMarker` opened/closed, and for `bMonitorMover` notifies
  controllers; `bNoResetOnRewind && bRewindOnPlay` enables forced net relevance (irrelevant to single player).
- **Implementer checklist (SeqAct_Interp):**
  - Play from stopped with `bRewindOnPlay`: Position = 0 first, instances built from the actor's current pose, forward play.
  - Reverse from stopped does not touch Position; after a completed forward run Position equals `InterpLength`, so reverse plays back.
  - Each frame advance by PlayRate*dt, apply the clamped position, flag finished at the clamp; Completed/Reversed one frame later.
  - Completed only if Position is at the end (within 0.0001), Reversed only if at 0 (within 0.0001); Stop in the middle: neither.
  - `InterpolationStarted` at activation, `InterpolationChanged` on Play/Reverse/Pause/Change Dir impulses, `InterpolationFinished` at deactivation.
- **Open:** Pause/Change Dir were read only structurally; `bIsSkipped`/skipping, director groups, camera cuts, replication,
  `SetPosition`/`Stop` as script calls (they are thin natives: `Stop` clears playing and paused; `SetPosition(pos, jump)` builds
  instances if none exist and applies the position) were not traced in detail.

## InterpGroup.UpdateGroup and the InterpTrack.UpdateTrack wrapper
- `UpdateGroup(pos, groupInst, preview, jump)`: runs the group's tracks in array order, skipping tracks with `bDisableTrack` or
  `bIsRecording`, in two passes: every track that is not a FaceFX track first, FaceFX tracks last. Each call is the track's
  preview update when `preview`, otherwise its update with the jump flag. Afterwards a group post step applies morph/animation
  weights and, in preview with an anim-control track, notifies the actor (editor).
- Wrapper rules (`UpdateTrack`): the track is *active* when not disabled, and its `ActiveCondition` (Always / gore enabled / gore
  disabled) matches the op's `bShouldShowGore`, and `TrackPlayDirection` (both / only forward / only reverse) matches the op's
  current `bReversePlayback`. Active: call the track's own update; not active: call the track instance's "restore actor state"
  (a no-op for Move and Event tracks). This is why `ETPD_PlayOnlyForward` / `PlayOnlyReverse` event and Ak tracks fire only
  in their direction.

## InterpTrackMove: how keys become a pose

### Curve evaluation (position and Euler tracks, `FInterpCurveVector`)
Each key stores `InVal` (time), `OutVal` (vector), `ArriveTangent`, `LeaveTangent`, `InterpMode`
(0 linear, 1 CurveAuto, 2 constant, 3 CurveUser, 4 CurveBreak, 5 CurveAutoClamped). Evaluating at time t:
- No keys: zero vector. One key, or t at/below the first key time: the first key value. t at/above the last key time: the
  last key value (no extrapolation).
- Otherwise find the first key `i` with t < `InVal`(i); segment is key `i-1` to key `i`, dt = `InVal`(i) - `InVal`(i-1);
  if dt <= 0 the previous key value is used. The mode of the **previous** key selects: constant -> previous `OutVal`;
  linear -> lerp with s = (t - `InVal`(i-1)) / dt; any curve mode -> cubic Hermite using the previous key's `LeaveTangent` and
  the next key's `ArriveTangent`.
- Hermite (default evaluation, the track's curve method is not "broken tangent"): with s as above,
  p = (2s^3-3s^2+1) P0 + (s^3-2s^2+s) (dt T0) + (s^3-s^2) (dt T1) + (-2s^3+3s^2) P1, T0 = leave tangent of the previous key,
  T1 = arrive tangent of the next key. If the curve's method byte equals 2 (broken-tangent evaluation) the tangents are not
  multiplied by dt. The stored tangents are used as saved; nothing here recomputes auto tangents.
- **For the Fire tracks** both tangents are zero, so every segment is the smoothstep 3s^2 - 2s^3 on position and on Euler
  angles: door 1.5 s, target 2.0 s, ease in and out.
- If a key's lookup-group name is not `None`, that key's value is the world location of the named group's actor instead
  (lookup track); not used by the slice data.
- Rotation: when `bUseQuatInterpolation` is clear, Euler (X roll, Y pitch, Z yaw, degrees) is evaluated per axis as above and
  converted to rotator units with x 65536/360 truncated to integer. When set, orientation is slerped between the two neighbouring
  keys' orientations ignoring modes and tangents. With sub-tracks (per-axis tracks) the legacy tracks are ignored.

### `GetLocationAtTime` and the move frame
- Evaluates raw position and Euler at the time, then composes with the reference frame: world transform = KeyTransform(t)
  x InitialTransform x BaseTransform, row-vector convention (key applied first). Result location = the key translation carried
  through that product; result rotation = the rotation of that product plus whole-turn winding taken from the key's rotator.
- `MoveFrame`: world (0) uses the identity as reference; **relative to initial (1)** uses the track instance's captured
  initial transform multiplied by the actor's base transform (identity when the actor has no base). The reference's rotation
  axes are re-normalised.
- `RotMode` (byte 2 = ignore): the actor keeps its current rotation; (1 = look-at group): rotation is aimed at the actor of the
  `LookAtGroupName` group; (0): keyframed.
- `bDisableMovement` forces the evaluated time to 0.

### `InitTrackInst` (captured at the start of every run from stopped)
- Initial transform = inverse(KeyTransform(t0)) x ActorTransform, where ActorTransform is the group actor's current location and
  rotation (expressed relative to its base if it has one) and **t0 is the op's current `Position`** when the track instance is
  created (0 when called for a rewind re-baseline). Consequence: the actor is **not** moved to key 0's absolute coordinates; at
  the start time it is exactly at its current pose, and later times add the key-to-key change expressed through the key
  transforms. Keys therefore may be authored in absolute-looking coordinates (the door's target keys are) and still play as
  relative motion.
- Replay: after a forward run Position = `InterpLength`, so Reverse captures the initial transform at the end key from the actor's
  open pose, and reverse playback returns the actor to its original pose. A Play with `bRewindOnPlay` first sets Position = 0, so
  the baseline is key 0 and the actor stays where it is at the start of the new run (it is not reset to a historical pose).
- `bNoResetOnRewind` with a mid-run rewind re-baselines the relative tracks at time 0 from the current pose; without it a mid-run
  rewind jumps back to the pose captured at the start of the run.

### Applying the pose (UNCERTAIN)
- The function that turns an evaluated transform into an actor change sets the actor location and rotation (through an actor
  move call that takes an "ignore rotation" flag for `RotMode` ignore), forces component updates, and then refreshes actors
  attached to the group actor. Only this function evaluates Move keys.
- **Not resolved:** in the vtable found for `InterpTrackMove` the runtime `UpdateTrack` implementation slot is an empty function and
  the pose application above sits in the *preview* slot. Either the runtime reaches it by a path not found, or the vtable I
  resolved is not the one used at run time. The pose maths above is unaffected. An implementation should evaluate each step and
  place the actor (a sweep that pushes or a teleport) and refresh attached actors. How hard the door pushes the player
  (encroachment) is not read: see `InterpActor.EncroachingOn` script in the mover prototype.
- **Implementer checklist (Move):** build track state from the actor's current pose at start (time = Position); evaluate keys with
  the Hermite rules; compose key x initial x base; at the clamped end time the value equals the last key's relative result.
- **Open:** `bUseQuatInterpolation` default for these two tracks was not read (the graph lists it unset); sub-track path, look-at
  and lookup tangents were not traced.

## InterpTrackEvent.UpdateTrack (event keys -> op outputs)
- Keys: `Time` + `EventName`. Track flags: `bFireEventsWhenForwards`, `bFireEventsWhenBackwards`,
  `bFireEventsWhenJumpingForwards`. Track instance keeps `LastUpdatePosition`.
- "Backwards" = the op is playing in reverse, or (jumping, not playing and the new position is before the last one).
- Forward step fires every key with `LastUpdatePosition` <= Time < new position (an end position equal to the length is nudged up
  by 0.0001 so the final key fires). Backward step fires every key with new position < Time <= `LastUpdatePosition` (a new
  position of exactly 0 is nudged down by 0.0001 so a key at 0 fires). Gating by the matching `bFireEvents...` flag; for a jump,
  events fire only when the jumping flag is set and the move is forward, or when the op is skipped.
- Firing key k = find the op output link whose description equals the key's `EventName`; if found and not disabled, set its
  impulse. Missing name: nothing (e.g. `WheelBackward`). Afterwards `LastUpdatePosition` = new position.
- Result for the target: starting a forward run fires `ChangeBool_FALSE` on the first step; finishing a reverse run fires
  `ChangeBool_TRUE` when position reaches 0; so the bool is true exactly when the target is at rest.
- **Implementer checklist:** half-open intervals as above; direction filter via `TrackPlayDirection` before this runs.
- **Open:** AkEvent tracks (sound) not read.

## Remote events

### SeqAct_ActivateRemoteEvent.Activated
- Originator = the world object (the world's info); Instigator = the node's `Instigator` property, defaulting to that same world
  object when unset (the pawn is **not** substituted).
- Collects every `SeqEvent_RemoteEvent` object (subclasses of that class) in the game sequence tree (all loaded levels'
  sequences), keeps those whose `EventName` equals the action's `EventName` (name index and number) and that are enabled
  (`bEnabled`), runs a per-event virtual (a preparation step, not identified) and then the event's activation check with
  (originator, instigator, no test, no indices) -> A4 rules (`MaxTriggerCount`, `ReTriggerDelay`).
- Matching is by name across **all levels' sequences**; there is no per-level or per-sequence scoping.
- `WillowSeqEvent_MissionRemoteEvent` is a different class (extends `SequenceEvent`), so this action never triggers it.

### PlayerController.ServerRemoteEvent (console `RE`/`RemoteEvent`, script)
Same search on the game sequence (class `SeqEvent_RemoteEvent` only), name equality, activation check with Instigator = the
controller's pawn. No enabled test in the script (the check does it).

### Behavior_MissionRemoteEvent.ApplyBehaviorToContext
- Skipped on a network client. The context argument must be an instance of a WillowGame class (cast) or nothing happens;
  the class was not identified (the Fire call sites pass a pawn or controller context; confirm with G2).
- Collects every `WillowSeqEvent_MissionRemoteEvent` in the game sequence tree; fires each one whose `EventName` equals the
  behavior's `EventName`, that is enabled, and whose `AssociatedMissionDefinition` equals the object that owns the behavior
  (the mission definition the behavior sequence lives in) via the activation check (originator = world object,
  instigator = the context actor).
- The plain `Behavior_RemoteEvent` native only stores its name from the context (it is the base of the script-level remote
  custom events); it does not search Kismet.
- **Implementer checklist:** key = (mission definition identity, event name); both must match; an unmatched mission fires nothing;
  4 of the 8 Fire events are the same name on different missions.
- **Open:** the cast class; the instigator the mission events receive (relevant if a downstream op reads `Instigator`).

## Marcus's scripted walk

### WillowSeqAct_AIScripted (action) -> mind
- Class extends `SeqAct_Latent`; fields `LookAt`, `Destination[]` (variable link), `Stance`, `FocusStyle`; `Target` link
  (here a named variable "Marcus"). The activation is delivered to each target pawn's mind (`OnAIScripted`, script):
  1. clear any previous scripted move (abort);
  2. if `Destination` has elements, pick **one at random** as `ScriptedMoveTarget`;
  3. `ScriptedFocus` = `LookAt` (a controller becomes its pawn); `ScriptedStance` = `Stance`; `ScriptedFocusStyle` =
     `FocusStyle` (`ESF_Path` here: face along the path); `bScriptedCanAttack` = (style == the "can attack" value 1);
  4. fire the AI event `Scripted` to the AI component.
- The latent action's completion is the **only** source of the `Finished` output: `ClearScriptedMove` clears the target, the
  last scripted node, resets stance/focus style and calls `ClearLatentAction(WillowSeqAct_AIScripted, aborted, override)`.
  A new scripted action, hold, or abort ends the previous one.

### Action_GoToScriptedDestination (the AI action)
- State chosen each update: look-at-player, hold, follow actor / formation, **FollowMoveNodes** when the mind is in scripted
  movement and the target is a move node, otherwise plain scripted move. The `Scripted` event restarts the action state.
- Speed: `SetPawnMovementSpeed(stance)` when `ScriptedStance` is set; with a target and no stance, speed 1; else default 0.
  Facing: for non-zero move style with a `ScriptedFocus`, face that focus; with `ESF_Path`-like styles face the path/target.
- `Start` sets the mind's `bCurrentlyScripted`; `Stop` clears it and `bWantsToFireWeapon`.

### State FollowMoveNodes (the walk loop, script state code)
For the current move node N (the scripted target):
1. Set speed and facing; clear `bReachedNode`/`bPathInterrupted`.
2. If the pawn already "reached" N (engine `Pawn.ReachedDestination`) treat as arrived; else repeat `WaitForPath`, `FollowPath`,
   yield one frame, until interrupted, flying-reached or reached.
3. If not interrupted: `bReachedNode` = true, drop the path, then **fire `SeqEvent_ArrivedAtMoveNode` on N** (the event is looked
   up among N's generated events by class; Instigator = Marcus, i.e. the Kismet event whose `Originator` is the node
   activates) and run N's `Behaviors` collection with (pawn, N, mind).
4. If `bFaceNodeDirection`: turn to N's rotation direction and wait (`WaitForPawnToTurn`: latent, finishes when the pawn has
   turned to the requested yaw).
5. Wait out special moves queued from N, then perch handling, then N's `HoldTime` (timer-based wait; the pawn stops path
   following and faces N's rotation).
6. Fire `SeqEvent_LeavingMoveNode` on N, then ask N for its next node (`GetNextMoveNode`, a native over the node links); if none
   and `bFaceNodeDirection`, turn first. Set the next node as the scripted target and loop.
- **A walk does not stop at the Destination node.** After the Destination the AI keeps following the linked move nodes
  (each node fires Arrived then Leaving, applies HoldTime, behaviors, special moves) until a node has no next node; only then
  the target is cleared and the action's `Finished` fires. This is why the door events sit on nodes 12, 39, 26 and 18: they
  are hit in walking order (the graph does not say which order; read it from the node links in the level).
- Arrival radius: the path is created to the node with radius `PawnArrivalRadius` and, if `bFuzzyArrival`, may arrive early;
  with no radius the default for an actor path is 128 units (`CreateActorPath` default). The final test is the engine's
  `Pawn.ReachedDestination` against the node, whose body was not recovered (the first-level reading shows a virtual call on the
  pawn and a navigation-handle check, UNKNOWN radius).
- Interrupt (the action is re-entered, `InterruptPath`): sets `bPathInterrupted`, stops perch, clears the path and timer and
  interrupts the latent action. The walk then restarts from the new state; no Arrived event fires for an interrupted node.
- Moving, turning and path following use the AI navigation handle (nav mesh); speed values of the stances are data in the stance
  definitions (not read).
- `AWillowAIPawn.HoldAIForMatinee(bool)` / `ReleaseAIFromMatinee()` are thin virtual natives; they were not read further.
- **Implementer checklist:** (a) pick one destination at random, (b) walk node chain, (c) Arrived event once per node reached,
  after the pawn is within the arrival test and before behaviors run, (d) HoldTime delays, (e) Finished only at chain end or abort,
  (f) every other `AIScripted` activation aborts the previous one (no Finished for the aborted one in the abort path of
  `ClearScriptedMove` with aborted=true: it still completes the latent action with the aborted flag).
- **Open:** the real arrival radius numbers, walking speeds, nav-mesh pathing, the pawn teleport fallback; the move-node link
  graph around nodes 12/18/26/39 (level data, not read); `Marcus` named-variable resolution (a `SeqVar_Named` bound by name in a
  parent sequence).

## SeqEvent populated events
- Classes `SeqEvent_PopulatedActor` and `SeqEvent_PopulatedPoint` extend `SequenceEvent`; fields `DestPopulationOpportunity` and
  `SpawnPoint`. Their `Originator` is the placed den (population opportunity den) or population point (Fire: dens 11, 13, 26,
  27; points 39, 40).
- When a den or point finishes spawning an actor, native spawn code walks **its own** generated events and, for each event of
  the matching class, calls the event's script `NotifyPopulatedActor(opportunity, spawnedActor, spawnPoint)`. The script stores
  the opportunity and spawn point in the event and runs the activation check with (originator = opportunity,
  instigator = the spawned actor). The event's single output `Out` therefore fires once per spawned actor.
- The event's Instigator variable link then holds the spawned pawn (the activation records Instigator, A4); downstream ops
  read it from the linked `SeqVar_Object` (Fire: `SeqVar_Object_9/10/11/13` for dens; `_0/_3` for points -> `SetPhysics` and
  `AttachToActor`/`Destroy`). Publication of the instigator into the linked variable at activation was **not traced** (engine
  convention; UNVERIFIED).
- The den-side caller also forwards the spawned actor to other den members; spawn timing and counts are G2's.
- **Open:** which spawn path calls it for dynamic dens, and the third argument (spawn point object) for dens.

## Not read yet
- Pause/Change Dir, skipping, director groups, camera cuts, replication of `MatineeActor`, `AddAIGroupActor`.
- InterpTrackAkEvent, Toggle, Visibility, Animation-control tracks.
- Encroachment/pushing by `InterpActor.EncroachingOn` at native level (script in mover prototype), attached-actor update details.
- `Pawn.ReachedDestination` body and navigation-handle internals; stance speeds; `WaitForPath`/`FollowPath` thunks (virtual).
- Variable publication into `SeqVar` links; `SeqAct_Toggle`/`SetPhysics`/`AttachToActor`/`Destroy` natives (G8/G3).

## Corrections to earlier notes
- `src/mover.hpp` and the mover prototype describe relative composition "from first-key deltas". The native reading is a matrix
  product: key transform x (inverse(key transform at the current position) x the actor's pose at start) x base transform; the
  first-key-delta shortcut agrees for pure translation, differs when keys carry rotation. The "parking pose" reading in
  NATIVE_SKY_APPROXIMATION.md (first key added to the serialized placement) is not what the native does: the actor starts at its
  placed pose.
- `src/kismet.cpp` fires output Completed immediately at the end of motion; natively it is raised one frame after the last
  position update and only when the position is within 0.0001 of the end (Reversed: of 0).
- Remote events `SeqAct_ActivateRemoteEvent` match every enabled `SeqEvent_RemoteEvent` by name across all loaded sequences; mission
  remote events match by (mission definition, name).
