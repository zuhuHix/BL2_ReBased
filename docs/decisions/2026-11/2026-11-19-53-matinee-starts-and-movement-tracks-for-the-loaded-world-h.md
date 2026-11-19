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
    - `Driver` gains `StartupUnknown`, `Incomplete`, `AtLoad` and `Outside`, decided per actor over every op that
      binds it.
    - `MatineeBindings` gains `by_actor` and `starts`.
    - `level_packages` gives both scans the loader's own level packages; `scan_matinee` runs the binding scan and the
      graph once after the map load.
    - `physics_rule` takes the new drivers. **No new driver answers `Ok`:** after review r1 (below) the only no-op left
      for an interpolating actor is H-PHYSICS-MODES' unbound case.
    - A failed tick is terminal and says it stopped after partial writes (the delta review, below).
- **The graph.**
  - Every `Engine.SequenceOp` export in the loaded levels' map files is loaded: 1,162 ops on Sanctuary, 81 of them
    Matinee ops. The levels are the map loader's own (`MapReport::levels`), so a streaming level that places no actor
    is read too (review r1, finding 2).
  - Edges come from `OutputLinks[].Links[].LinkedOp`, except a sequence's own outputs (delta review, finding 3): an
    impulse into a sequence goes on only through its activation events, and out of it only through a finish op, so a
    sequence is no source of its outputs' targets.
  - Three kinds of edge that no link holds are added:
    - a remote event by its `EventName` from each action naming it, both sides by inheritance from
      `SeqEvent_RemoteEvent` and `SeqAct_ActivateRemoteEvent` (finding 3); a name that does not read, on either side,
      makes the graph incomplete (delta review, finding 4);
    - into a subsequence: a `SeqEvent_SequenceActivated` from its outer sequence;
    - out of a subsequence (finding 1): a `SeqAct_FinishSequence` to each op its sequence's output links name, for the
      output whose `LinkDesc` is the finish op's `OutputLabel`.
  - **Incompleteness** (findings 1 and 5) is kept by reason and count, and makes every op's start `Incomplete`, a
    `NotYet` naming the reasons. The reasons are: an export class that does not resolve (outside `NOT_OP_CLASSES`) or
    whose reference cannot be read; an op whose `OutputLinks`, an output's `Links` or a link's `LinkedOp` is missing or
    of another shape (a decoded null `LinkedOp` links to nothing and is no fault); a remote-event action or a remote
    event with no `EventName`; a finish op with no label or sequence, or whose label names no output. Sanctuary's graph is complete.
- **The start rule.** The record: an op moves its actors only after an impulse on one of its inputs activates it. The op
  class declares `bLooping`, `bRewindOnPlay` and `bForceStartPos`, but no auto-play or play-on-load property. Walking
  the graph backwards from an op gives the events its impulses can start from:
  - A `SeqEvent_LevelLoaded` or `SeqEvent_LevelStartup` among them: **starts at load**. It answers `NotYet`, layer
    absent: the world runs no Kismet, so it fires no load event and plays no op.
  - Else, an event that something outside the graph can fire: **started from outside**. It answers `NotYet`, not read,
    naming the event classes. Eligibility is the event's class, whatever sources it also has in the graph (finding 3:
    a dead sender of the same name, or a closed remote-event cycle, no longer hides that a script can fire the event):
    any event but a level-load event, a touch, a console command or a subsequence's activation event. On Sanctuary
    these are `WillowSeqEvent_MissionRemoteEvent`, `SeqEvent_RemoteEvent`, `SeqEvent_PopulatedActor` and
    `SeqEvent_ArrivedAtMoveNode`.
    - The hub's brief allowed treating any op that no load event reaches as not started. This lane is stricter for
      these events: BL2's mission system and population fire them from outside Kismet, and whether they fire while the
      map loads (to restore story state, or as a den spawns) is that system's question. It cannot be shown from the
      graph.
  - Else: **startup unknown** (`StartupUnknown`, with whether a touch reaches it). Through the graph its impulses can
    come only from a touch, a console command, or nowhere (an op no link reaches included).
  - **No op is answered "not started at load"** (the hub's rule on review r1, finding 4, and the delta review, finding
    1). In Off that answer needs a complete graph and every other startup path ruled out, and two are not: a startup
    script calling the op's `SetPosition`, which builds group instances and applies a position with no impulse
    (NATIVE_KISMET_MATINEE.md), and, when a touch reaches the op, an actor spawned overlapping the trigger. Ruling them
    out needs the scripts' and the spawn's side, which this lane does not read. So the start is unknown, and an actor
    only such ops bind answers `NotYet` (not read), naming both. A proven exclusion would be a new answer with its
    proof.
- **Per actor:** an incomplete graph wins, then an op that starts at load, then one started from outside the graph,
  then one whose start was not worked out (the old playback `NotYet`), then the startup-unknown `NotYet`. An incomplete
  binding scan stays `Unknown` (#420 review r1).
- **Sanctuary's 81 ops:**
  - 20 start at load (`SeqEvent_LevelLoaded`), 6 of them looping;
  - 53 are started from outside the graph (44 before review r1, which keeps remote events with an in-graph sender
    firable from outside): 8 only from mission remote events, 10 only from remote events, and the rest from mixes with
    mission, population or move-node events;
  - 8 have an unknown startup (no load or outside event reaches them through the graph), 1 of them reached by a
    touch. Dropping the sequence pass-through edges (delta review) changed none of the 81.
  - None is playing at load (`bIsPlaying` false on all 81).
- **Movement tracks** (`track::read_move`, `Curve::eval`, `MoveTrack::key_pose`).
  - **Read by name:** `PosTrack` and `EulerTrack` (`Points[]`: `InVal`, `OutVal`, `ArriveTangent`, `LeaveTangent`,
    `InterpMode`; `InterpMethod`), `LookupTrack.Points[].GroupName`, `SubTracks`, `MoveFrame`, `RotMode`,
    `bUseQuatInterpolation` and `bDisableMovement`. Enum values are read by name, the key modes included (through the
    struct field's enum), never by position.
  - **Evaluated by the record's rules, in f32:** the ends hold; the previous key's mode picks constant, linear or the
    cubic Hermite; tangents are scaled by the segment length unless the method is `IMT_UseBrokenTangentEval`; Euler
    degrees become rotator units by `x 65536 / 360`, truncated, and not wrapped (two turns are 131,072 units). A key
    time, value or tangent that is not finite is refused while decoding (finding 6: a NaN time defeats an order check
    of `<` alone), and keys out of time order are refused, not sorted.
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
  - 57 interpolating actors pass, the unbound ones only, as before this lane. (Before review r1, 59 passed: the 2 the
    not-at-load rule let through now answer `NotYet`.)
  - 14 stop at a load-started op.
  - 51 stop at an op started from outside the graph.
  - 1 stops at the startup-unknown `NotYet` (a script's `SetPosition`).
  - 23 stop at the world's `Base` guard.
  - 13 stop at the walking, nav-mesh walking and rotating modes.
  - The first stop is the load-started op that binds a `WillowSpectatorPoint`.
- **A failed tick** (delta review, finding 2; the hub's ruling, as for #437 and #439: no rollback). By the time the
  actor pass reaches a refusal, the tick has advanced the clock, written the `WorldInfo` time fields and may have run
  earlier actors' script. Nothing is undone: the halt is terminal, `Halt::partial_writes` says the tick stopped after
  partial writes and the report prints it (`play world halted at tick 1 (NotYet, stopped after partial writes)`), and
  the world's step itself refuses every later tick, not only its link. A read-back failure after a finished tick halts
  the same way, without the partial-writes mark.
  - Review r3: the mark follows the phase the tick reached. It is set once the world clock advanced (the tick's first
    write), and not for a tick refused before it, such as a NaN delta, which wrote nothing; the halt is terminal
    either way.
- **Off only** (review r3, finding 1): the delta review's two graph rules (a sequence is no source of its outputs'
  targets; a remote event with no readable name makes the graph incomplete) apply in `ParityMode::Off`. Reference
  keeps the graph as it was before them, so an op it answered `AtLoad` (layer absent) through a sequence the load
  event enters stays so. A test runs both modes on the gated child.
- **Proof.**
  - **Synthetic** (invented values):
    - `world::matinee::track::tests`: no keys, one key, the held ends, each segment mode, tangents scaled and broken,
      keys at one time, Euler to rotator units, disabled movement.
    - `world::tests`, start rules on hand-made graphs: nothing links to it; a load event two links up; a mission remote
      event; only a touch and a dead action; a remote event fired by a load-started action, and one with no action; a
      cycle with no event.
    - `world::tests`, the per-actor driver over several ops, and each physics rule (the not-at-load `NotYet` with and
      without a touch, the incomplete-graph `NotYet`); on hand-made graphs, a remote event whose only sender is dead
      stays external, and an incomplete graph gives `Incomplete`.
    - **Through the readers** (review r1, finding 7): an invented Kismet package built with the package helpers and run
      through `read_graph`. A level-loaded event inside a child sequence reaches a Matinee op in the parent through the
      finish op and the child's output (`AtLoad`); a remote event in a closed cycle with its own action stays `Outside`;
      a touch gives `NotAtLoad` with the touch; an op nothing links to gives `NotAtLoad`; over the map with two exports
      of a class that does not resolve, the graph is incomplete with that reason counted twice. The same package's
      movement tracks through `read_move`: a broken-tangent segment (0.125 at 1 s of a 2 s segment with leave tangent
      1) and yaw wound to 720 degrees (131,072 units); a track whose middle key time is NaN is refused.
    - **Delta review:** in the same package, a level-loaded event entering a second child sequence in which only a
      touch reaches the finish op leaves the op behind the child's output `StartupUnknown` with the touch, not `AtLoad`
      (finding 3); over an engine whose remote event declares no `EventName`, the graph is incomplete with "a remote
      event with no EventName" counted once (finding 4); a streamed level that places no actor holds the level-loaded
      event firing the remote event that starts the persistent level's op binding its pawn: through the loader's levels
      the pawn's driver is `AtLoad`, over the persistent level alone it would be `Outside` (finding 5); a world whose
      bound pawn's refusal stops tick 1 keeps the clock, the time fields and the pawn's transform as the stop left
      them, marks the halt as after partial writes, and a second step changes nothing (finding 2).
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
    - **After review r1, two runs again:** identical text apart from timings, the same digests, segment digests and
      first stop.
    - **After the delta review, two runs again:** the same, and the halt line now reads "stopped after partial
      writes".
    - **The tick count does not rise.**
- **Open items:**
  - A Kismet layer in the world that fires the level-load events and runs the load-started ops: 14 actors. The move
    frame and the pose application go with it.
  - The mission system's, population's and scripts' remote events at load: 51 actors.
  - The startup paths outside the graph: a script's `SetPosition` on a Matinee op, and touches from spawn overlaps (1
    actor today, whose start is unknown).
  - The world's `Base` guard (23), the AI walking modes (11) and rotating (2).
  - The non-movement track types.
