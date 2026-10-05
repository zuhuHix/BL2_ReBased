# How Sanctuary's non-combat NPCs move and idle: move nodes, perches, scripted-NPC action, load balancer (2026-10-04)

AI-assisted (Claude). Behaviour notes from the installed script (`research/script_disasm.py`), installed class defaults and
level data (`ow-package --properties`, `--exports`, `--behavior-dump`) and a local Ghidra reading of `Borderlands2.exe`,
under the policy in [LEGAL.md](../LEGAL.md) ("Analysing the executable") and
[NATIVE_ANALYSIS.md](../NATIVE_ANALYSIS.md). Nothing below is a listing or pseudo-code. Raw output is under ignored
`local/analysis/E/`.

**Every rule here is `UNVERIFIED` in the running game.** Most of it is script, so it is read exactly; the native parts
(next-node choice, the load balancer, the random idle pick, speed from stance) are marked.

## 1. Summary

There is no "wander" native. Ambient motion is assembled from data and script:

1. A **character class** (`AIClassDefinition`, `CharClass_*`) names an **AI definition** (`AIDef_*`). The AI definition
   holds a graph of AI sequence nodes (`AISeqOp_Action_n`) that run `Action_ScriptedNPC` objects; **AI flags** pick
   which variant runs.
2. `Action_ScriptedNPC` (script) walks a chain of **move nodes** (`WillowAIMoveNode`; `Perch` is a subclass) or a
   target set by a level **Kismet** action (`WillowSeqAct_AIScripted`).
3. At a node it may **hold**, play queued **special moves**, or **perch** (a scripted stand/lean/sit with a start clip,
   a looping idle with random variants for a random time, and a stop clip).
4. A native **NPCLoadBalancer** limits how many load-balanced NPCs path at once.

Export counts in the installed Sanctuary packages (`Sanctuary_P`, `_Dynamic`, `_Combat`, `_Side`; objects of each class,
archetypes included): 246 `WillowAIMoveNode`, 245 `Perch` (a `Perch` is itself a move node; the 246 counts the plain nodes
only), 62 `WillowSeqAct_AIScripted` (+1 `AIScriptedAnim`, +1 `AIScriptedHold`), 25 `Action_ScriptedNPC`, 54 `PerchDefinition`,
23 `AIClassDefinition`, 146 `WillowPopulationPoint`, 109 `WillowPopulationOpportunityPoint`, 49 `PopulationOpportunityDen`, 20
`PopulationFactoryBalancedAIPawn`. The bulk of the perches (148), population points (123), dens (32) and plain nodes (50 of 246) sit
in `Sanctuary_Combat`, the streaming level that holds the AI population; the named story NPCs and their definitions are in
`Sanctuary_Dynamic`. `Action_Patrol` (the combat AI's random roam around a den) appears only twice (Fink, a target-dummy bot)
and is not part of civilian life.

## 2. The character class and body

Example, the Resistance Fighter (`GD_ResistanceFighter`, display name "Pvt. Jessup"):

* `CharClass` values: `GroundSpeed` 190, `WalkingPct` 1, `SlowDownDist` 200, `SlowDownMinPct` 0.2, `RotationRate` yaw 40000 per
  second, physics `PHYS_NavMeshWalking`, `bUsable` true (talk icon), `TimeUntilConsideredLingering` 10.
* `BodyClass`: `DefaultStance` `STANCE_Patrol`, `BodyTag_Human`, default turn definition `TurnDef_NPCShared`. Stances (enum
  order Patrol 0, Run 1, Sprint 2, Injured 3, None 4, Crouch 5) are `StanceTypeDefinition` objects with `SpeedScale`:
  Patrol 0.51, Run 3.0 for this body (`MovementStyle` forward). The stance picks the locomotion animation set and a speed
  scale; the combination with `GroundSpeed`/`WalkingPct` is native (`SetPawnMovementSpeed`) and was not read. The
  natural reading `GroundSpeed x SpeedScale` gives about 97 uu/s for a patrol-stance civilian (`UNVERIFIED`).
* For scripted NPCs the speed used while walking nodes is `GetDefaultMoveNodeSpeed()` = the body's `DefaultStance`, or
  the stance written in the Kismet action (`Stance`) when it is not "None", or the "Run" stance (value 1) when the NPC has
  a target.

## 3. The AI definition and the choice of variant

`AIDef_ResistanceFighter` has an `AIBehaviorProviderDefinition` (dialog, usability, hold behaviors, 100+ behaviors for
mission talk and texture/skin variants) and three `Action_ScriptedNPC` objects selected by AI flags
(`FlagExpressionEvaluator`s on the definition):

| Action object | Properties | Probable gate (inferred from the two flag evaluators on the definition; the node graph was not decoded) |
|---|---|---|
| `Action_ScriptedNPC_552` | `bLoadBalanceNPC`, `bIdleNPC` | flag `Flag_IdleNPC` true |
| `Action_ScriptedNPC_553` | `bLoadBalanceNPC` | flag `Flag_NPCDoNotThrottleMovement` false |
| `Action_ScriptedNPC_554` | neither | otherwise |

The sequence-node graph (`AIDefinition.NodeList`) itself could not be decoded with the reader (undecoded array), so the
exact precedence is inferred from the flag evaluators and names (`UNVERIFIED`). Flags are set by population data or
behaviors (`Behavior_SetAIFlag`). Named story NPCs (Marcus, Zed, Roland, Lilith, Moxxi, Scooter, Tannis, Hammerlock, Claptrap,
Daisy, Fink, Van Owen, John Mamaril, One Winner, the Zed surgery patient, the target dummies) each own one such
`Action_ScriptedNPC` (the Resistance Fighter has three); the ones with an idle flag mainly **perch** and only walk when a
Kismet action or the population data sends them (`UNVERIFIED`).

## 4. `Action_ScriptedNPC` behaviour (script)

States and transitions (`CheckStateTransition` runs on every update and on events):

* If the mind has no scripted move target but remembers `LastPatrolNode`, it is sent to that node (`ForceMoveToActor`) and
  the memory is cleared.
* Desired state: `IdleNPC` when `bIdleNPC`; otherwise the parent (`Action_GoToScriptedDestination`) decides:
  `LookAtPlayer` (the player is near enough to look at or the NPC is talking: the pawn turns its head/body toward the
  player), `ScriptedHold` (Kismet "hold" action), `FollowActorFormation` / `FollowActor`, `FollowMoveNodes` (the scripted
  target is a move node), `ScriptedMove` (path to any other actor), else the action ends ("None") and the NPC falls back
  to its plain idle.
* **IdleNPC (begin state):** if the scripted target is a `Perch`, the pawn is *snapped* to it: `CurrentPerch` set, world
  collision off, rotation and location copied from the perch, then the normal perch start (section 6); AI navigation
  updates are paused.
* **FollowMoveNodes:** section 5. With `bLoadBalanceNPC` each path start is gated by the load balancer (section 7). While
  waiting or idle the load-balanced NPC is set to physics "none", AI `UpdateRate` 0.5 s and tick throttle 0.5 s plus up to 0.5 s of
  random jitter; while it is allowed to path (and is not on a perch) it gets physics `NavMeshWalking`, `UpdateRate` 0.2 s and
  throttle 0.1 s. A load-balanced NPC that is stuck on a perch whose idle clip is playing is released when the balancer
  next allows it to path.
* Speeds, facing: while moving the facing policy is "look along the move" (or at the Kismet `LookAt` focus when one was
  given); at a hold the pawn faces the node's rotation.

## 5. Move nodes and the walking loop

`WillowAIMoveNode` (class defaults): `PawnArrivalRadius` 128 (32 for perches), `AISpeedPercentageHere` 1, `bFuzzyArrival`
(arrive early), `bFaceNodeDirection` true (perch default), plus from the base class: `bEnabled`, `NextNodes`
(`NodeData {Node, Weight}`), `PreviousNodes`, `HoldTime`, `Behaviors`, `SpecialMoves`.

Loop (`Action_FollowPath`, script):

1. Path find to the current node (`CreateActorPath` with the arrival radius; at most once a second after a failure).
2. On arrival (`Stop`): pick the next node with `GearboxAIMoveNode.GetNextMoveNode()` - native, **not read**; from the data
   (`NextNodes` with `Weight`) it is a weighted random choice among enabled nodes (`UNVERIFIED`) - and `SetMoveNode`.
3. Setting a node triggers, in order and each blocking the walk until done: **special moves** (`SpecialMoves` array:
   stop anything playing, queue them all, wait for the last to end), the **hold timer** (`HoldTime` > 0: clear the path,
   face the node's rotation, wait that many seconds), and the **perch** if the node is a `Perch`.
4. Walking itself is the nav-mesh path follower (native) at the stance speed (section 2); `AISpeedPercentageHere` of the
   node scales the speed there (read as a name; use not read).
5. Kismet: `WillowSeqAct_AIScripted` (`Destination[]` actors, `LookAt`, `Stance`, `FocusStyle`) calls the mind's
   `OnAIScripted`: it clears any previous scripted move, takes the **last** `Destination` entry as `ScriptedMoveTarget`,
   copies focus, stance and focus style, and raises the AI event `Scripted`. If the target is a move node the NPC enters
   `FollowMoveNodes` from there. `WillowSeqAct_AIScriptedAnim` plays a special move (optionally holding the AI) with the
   callback `ScriptedAnimEnded`; `WillowSeqAct_AIScriptedHold` holds or releases the NPC.

## 6. Perches and the idle gestures

`Perch` (an actor; extra fields `PerchDef`, `UseRadius` 1536, `UseHeight` 1024, `bOverrideLoopTime` / `LoopTimeOverride`,
`bLookForPlayersInRange`, `CooldownTime`, `User`) plus `PerchDefinition` data:

* `AnimMap`: one entry per `PopulationBodyTag` (e.g. `BodyTag_Human`) giving `StartAnim`, `IdleAnim`, `StopAnim`
  (`SpecialMove_Perch`, `SpecialMove_PerchLoop` or `SpecialMove_PerchRandomLoop`) and an optional `AnimSet`. A body whose
  tag is not in the map cannot use the perch (`GetPerchData` returns nothing and the perch is skipped).
* `LoopTime` and `CooldownTime` (min/max ranges), `LerpTime` (move onto the perch; default 0.2, shared perches 1.0),
  `bUseCollision`, `CanUseExpression`, player-in-range check (frequency 5 s, radius 2500, cooldown 10 s defaults),
  `BehaviorProviderDefinition` (dialog events etc.).
* Sequence: **start** (`PerchStart`): hold AI movement, disable pawn physics, root-motion in the perch's frame, interpolate
  location/rotation to the perch over `LerpTime`, play `StartAnim` (e.g. blend-in 0.5, root motion
  `RootMotion_MoveAndTurnOnGround`) or go straight to idle; **idle** (`PerchPlayIdle`): draw a duration from the
  perch's `LoopTime` (or the override), then, if the idle move is a `SpecialMove_PerchRandomLoop`, play it and re-draw
  when it ends until the time is up, else play the loop once; **stop** (`PerchPlayStop`): play `StopAnim`; **done**
  (`PerchDone`): release movement, clear the nav anchor, restore physics, set the perch's `CooldownTime` to now + a draw
  from the definition's cooldown range, clear the user, restore aim/rotation; continue to the next node.
* **Random variants** (`SpecialMove_PerchRandomLoop`, native pick, read): sum the entries' `Weight`s, draw
  `r = rand() / 32768 * total`, walk the list accumulating weights and take the entry whose cumulative interval
  contains `r`. Example, `Perch_NPC_ArmsCrossed` for humans: loop time 3-5 s; start `Perch_ArmsCrossed_Start`
  (blend-in 0.5), idle = random loop of two `SpecialMove_Perch` entries with weights 1.0 and 0.1 (about 91 % / 9 %),
  stop clip. Clips come from `Anim_Generic_NPC.Anim_Generic_NPC` (gestures in `Anim_Generic_NPC.Gestures`).
* Shared perches in the installed data: ArmsCrossed, ArmsCrossedForever, BangOnSomething, BarrelSit, ChairSit,
  HandsOnHips, KickGround (loop only, blends 0.5/1), LeanOnCounter, LeanOnWall, LookAtGround, LookIntently, PeerUnder, and
  per-character ones (Brick, Claptrap, Lilith, Mordecai, Moxxi, Tannis, Scooter repair).

## 7. The NPC load balancer (native `NPCLoadBalancer`, read)

A global object (`WillowGlobals.GetNPCLoadBalancer`) keeps one record per registered NPC mind: a flag word (wants to path,
on a perch, pathing, allowed) and the time it asked. Defaults from the installed class defaults: `MaxNumberPathing` 7,
`TimeBetweenUpdates` 0.5 s; an optional flag `NumberPathingReducedByPlayerCount` (not set in the defaults) subtracts the
number of players from the limit (minimum 1).

* `CanStartPath(mind)`: the first call records "wants to path" with the current time and runs the arbiter; the answer is
  the record's "allowed" bit. `WantsToPath` returns the wants bit; `CheckPathing` returns the pathing bit;
  `IsPathing(mind, b)` sets the pathing bit and clears wants and allowed; `PathFailed` clears all three; `OnPerch` sets the
  perch bit; `CanContinuePath` is always true.
* **Arbiter** (at most once per `TimeBetweenUpdates`): count the NPCs currently pathing; for every NPC that wants to path
  and is neither pathing nor already allowed compute a priority = (seconds waited) minus (distance to the nearest
  player in thousands of unreal units, squared); the single highest-priority NPC is marked allowed if the pathing count is
  below the limit. So at most one NPC per update interval starts to walk, NPCs near the player and NPCs who have waited
  longest go first, and no more than seven walk at once.

## 8. What Lane B can run from installed data

Data to read (all in the packages): the NPC's `AIClassDefinition` -> `AIDef` -> `Action_ScriptedNPC` variants and their flags;
`WillowAIMoveNode` / `Perch` actors with `Location`, `Rotation`, `NextNodes`, `HoldTime`, `SpecialMoves`, `PerchDef`;
`PerchDefinition.AnimMap` / `LoopTime` / `CooldownTime`; the body class `DefaultStance` and stance speed scales; the
`WillowSeqAct_AIScripted` actions in the level Kismet (`Destination`, `Stance`). A reduced host runner needs: a node graph
with weighted next-node choice, per-node hold timers and special-move queues, a perch state machine with random loop
time and weighted variants, cooldowns on perches, and a limit on concurrent walkers.

## 9. UNVERIFIED and how to confirm

| Statement | Confirmation |
|---|---|
| Next node = weighted random over `NextNodes` | Trace a civilian's `LastPatrolNode`/`MoveNode` across 50+ arrivals at a branching node (`tools/sdk_trace` / `tools/real_game`) and compare frequencies with the `Weight`s |
| Speed = `GroundSpeed x stance SpeedScale` | Trace a walking civilian's `Velocity` magnitude (expect ~97 uu/s for Patrol 0.51 with GroundSpeed 190) |
| Variant selection by `Flag_IdleNPC` / `Flag_NPCDoNotThrottleMovement` | Trace the active action object of a Resistance Fighter with the flags toggled by population data |
| Load balancer limit 7 / update 0.5 s | Count simultaneously pathing NPCs in a crowded area |
| Random loop weights (91 % / 9 %) | Count idle variants over many loops |

## 10. What was not read

`GearboxAIMoveNode.GetNextMoveNode` and `GetNextMoveNodeClosestToPoint` (native), `SetPawnMovementSpeed` and the stance /
locomotion blend (native), the AI-definition node graph, `Perch.CheckStartReplication` and replication, the
nav-mesh path follower, and the player-in-range perch reaction (`OnPlayerInRange`).
