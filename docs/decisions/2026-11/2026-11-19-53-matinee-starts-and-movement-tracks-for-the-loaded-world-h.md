## 2026-11-19: Matinee starts and movement tracks for the loaded world (H-MATINEE)

AI-assisted (Claude Code, Claude Opus 5.5). Lane H-MATINEE, after H-PHYSICS-MODES (decision 2026-11-19-22). Number 53
was assigned by the hub. There was no real-game session, so nothing below is confirmed in the running game. No
dependency was added, no bounds check was loosened and no `NotYet` guard was removed. No package parsing behaviour
changed: everything here is read by property name through the definition store.

- **Why.** On tick 1, 66 interpolating actors stopped at "the Matinee playback that moves a bound actor". Matinee is
  stock UE3 (`SeqAct_Interp`, `InterpData`, `InterpGroup`, tracks), with its playback recorded in
  NATIVE_KISMET_MATINEE.md.
- **What changed** (all in `ow-host`):
  - **New `world/matinee.rs`:** the Kismet graph read backwards, and the start rule.
  - **New `world/matinee/track.rs`:** the movement track reader and its curve evaluation.
  - **`world.rs`:**
    - `Driver` gains `NotStarted`, `AtLoad` and `Outside`, decided per actor over every op that binds it.
    - `MatineeBindings` gains `by_actor` and `starts`.
    - `scan_matinee` runs the binding scan and the graph once after the map load.
    - `physics_rule` takes the new drivers.
- **The graph.**
  - Every `Engine.SequenceOp` export in the placed actors' map files is loaded: 1,162 ops on Sanctuary, 81 of them
    Matinee ops.
  - Edges come from `OutputLinks[].Links[].LinkedOp`.
  - Two kinds of edge that no link holds are added: a remote event by its `EventName` from each
    `SeqAct_ActivateRemoteEvent` naming it, and a `SeqEvent_SequenceActivated` from its outer sequence.
- **The start rule.** The record: an op moves its actors only after an impulse on one of its inputs activates it. The op
  class declares `bLooping`, `bRewindOnPlay` and `bForceStartPos`, but no auto-play or play-on-load property. Walking
  the graph backwards from an op gives the events its impulses can start from:
  - A `SeqEvent_LevelLoaded` or `SeqEvent_LevelStartup` among them: **starts at load**. It answers `NotYet`, layer
    absent: the world runs no Kismet, so it fires no load event and plays no op.
  - Else, an event that something outside the graph fires: **started from outside**. It answers `NotYet`, not read,
    naming the event classes. Such an event has no source in the graph and is neither a touch nor a console command.
    On Sanctuary these are `WillowSeqEvent_MissionRemoteEvent`, a `SeqEvent_RemoteEvent` with no action in the graph,
    `SeqEvent_PopulatedActor` and `SeqEvent_ArrivedAtMoveNode`.
    - The hub's brief allowed treating any op that no load event reaches as not started. This lane is stricter for
      these events: BL2's mission system and population fire them from outside Kismet, and whether they fire while the
      map loads (to restore story state, or as a den spawns) is that system's question. It cannot be shown from the
      graph.
  - Else: **not started at load**. Its impulses can come only from a touch, a console command, or nowhere (an action
    no link reaches never runs). Such an op has built no group instance and moves no actor, so its actors move nothing.
  - UNVERIFIED in the running game:
    - that no native or script code activates a Matinee op outside the graph;
    - that no touch fires while the map loads (a player spawned inside a trigger would; this world has no player pawn
      yet).
- **Per actor:** an op that starts at load wins, then one started from outside the graph, then one whose start was not
  worked out (the old playback `NotYet`). The actor moves nothing only when every op that binds it is not started,
  and only in `ParityMode::Off` (Reference answers `NotYet`, as #420 review r1 set for the unbound case), and only
  while the binding scan is complete (#420 review r1's rule; an incomplete scan stays `Unknown`).
- **Sanctuary's 81 ops:**
  - 20 start at load (`SeqEvent_LevelLoaded`), 6 of them looping;
  - 44 are started from outside the graph: 34 only from mission remote events, and the rest from mixes with remote
    events, population or move-node events;
  - 17 are not started at load.
  - None is playing at load (`bIsPlaying` false on all 81).
- **Movement tracks** (`track::read_move`, `Curve::eval`, `MoveTrack::key_pose`).
  - **Read by name:** `PosTrack` and `EulerTrack` (`Points[]`: `InVal`, `OutVal`, `ArriveTangent`, `LeaveTangent`,
    `InterpMode`; `InterpMethod`), `LookupTrack.Points[].GroupName`, `SubTracks`, `MoveFrame`, `RotMode`,
    `bUseQuatInterpolation` and `bDisableMovement`. Enum values are read by name, the key modes included (through the
    struct field's enum), never by position.
  - **Evaluated by the record's rules, in f32:** the ends hold; the previous key's mode picks constant, linear or the
    cubic Hermite; tangents are scaled by the segment length unless the method is `IMT_UseBrokenTangentEval`; Euler
    degrees become rotator units by `x 65536 / 360`, truncated. Keys out of time order are refused, not sorted.
  - **`NotYet`:** quaternion interpolation, lookup keys (a key whose lookup point names a group), sub-tracks, and the
    look-at rotation mode.
  - **Not done:** composing the keyed transform with the captured initial transform and base, and applying the pose.
    The record marks who applies the pose UNCERTAIN, and the world runs no op yet. The evaluation is ready for the
    Kismet layer that starts ops.
  - **Structural oracles over all 169 movement tracks** in Sanctuary's 81 ops: 168 read.
    - Every track read has as many position keys as Euler keys.
    - Its keys are in time order, and its last key falls within its op's `InterpLength`.
    - The one track not read uses quaternion interpolation.
  - The track classes that are not movement stay `NotYet` by name, and nothing here plays them: `InterpTrackAkEvent`,
    `InterpTrackEvent`, `InterpTrackToggle`, `InterpTrackFloatProp`, `InterpTrackVectorProp`, `InterpTrackColorProp`,
    `InterpTrackFloatMaterialParam`, `InterpTrackVectorMaterialParam`, `InterpTrackVisibility`,
    `InterpTrackAnimControl` and `InterpTrackDirector`.
- **What tick 1 meets now** (the physics census, in list order; it agrees with the runs):
  - 59 interpolating actors pass (57 before).
  - 14 stop at a load-started op.
  - 50 stop at an op started from outside the graph.
  - 23 stop at the world's `Base` guard.
  - 13 stop at the walking, nav-mesh walking and rotating modes.
  - The first stop is the load-started op that binds a `WillowSpectatorPoint`.
- **Proof.**
  - **Synthetic** (invented values):
    - `world::matinee::track::tests`: no keys, one key, the held ends, each segment mode, tangents scaled and broken,
      keys at one time, Euler to rotator units, disabled movement.
    - `world::tests`, start rules on hand-made graphs: nothing links to it; a load event two links up; a mission remote
      event; only a touch and a dead action; a remote event fired by a load-started action, and one with no action; a
      cycle with no event.
    - `world::tests`, the per-actor driver over several ops, and each physics rule.
  - **Installed** (`world::tests::sanctuary_matinee_census` and `sanctuary_physics_census`, ignored, `OW_BL2_ROOT`;
    counts only, output under ignored `local/p2/h-matinee/` and `local/p2/h-physics-modes/`): the numbers above.
  - **Installed, headless** (`--map Sanctuary --play-script` with H4-PLAY's 60 s script, desktop, release, under the
    machine lock).
    - **Before** (H-PHYSICS-MODES): 0 ticks completed, halted at tick 1 on the Matinee playback of a bound
      `WillowSpectatorPoint`.
    - **After, two runs with identical text apart from timings:** 0 ticks completed, `TimeSeconds` 0.016667, halted at
      tick 1 on the level-load event that starts the op binding a `WillowSpectatorPoint`. Path digest
      `d7b95ebfe14df774` and world digest `c233c6faac880f05` are unchanged, and so are all twenty segment digests.
      The world load, the graph read included, took 12.6 s and 9.1 s in the two runs (8.4 to 8.8 s before; the
      desktop was shared with other lanes).
    - **The tick count does not rise.**
- **Open items:**
  - A Kismet layer in the world that fires the level-load events and runs the load-started ops: 14 actors. The move
    frame and the pose application go with it.
  - The mission system's and population's events at load: 50 actors.
  - The world's `Base` guard (23), the AI walking modes (11) and rotating (2).
  - The non-movement track types.
