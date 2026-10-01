# Sanctuary slice route: "Rock, Paper, Genocide: Fire Weapons!" (2026-10-01)

AI-assisted. This records the route chosen for the Sanctuary + Maya gameplay loop and what is and is
not established. It is a progress record, not a parity claim: **no paired original-game capture
exists yet for anything below**, so every behavioural statement about the native layers is
`UNVERIFIED`. Extraction, automated checks, host (UE5) results and original-game parity are reported
separately.

## Why this mission

`GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire` (Marcus, Sanctuary) has the smallest dependency
graph found: two objectives (go to the range, "Fire"), two objective sets, one dependency mission
(`GD_Episode03.M_Ep3_CatchARide`), an XP-only reward, a lent mission weapon (a Maliwan elemental pistol
balance, tied to the Fire objective), one enemy class (`GD_TargetDummyBot`) and a Kismet sequence in
`Sanctuary_Dynamic.upk` that already contains the working door. All of it is installed data.

## What is native in this build (so it cannot be run from bytecode)

Disassembly and export sizes show that `MissionTracker` (125 functions), `BehaviorKernel` (28) and the
mission behaviors `AdvanceObjectiveSet`, `MissionRemoteEvent`, `ActivateMission`, `CompleteMission`,
`ClearObjective` have **no script**. `Behavior_UpdateMissionObjective` and `Behavior_ShowMissionInterface`
are script and call into those natives. Kismet op activation (`SequenceOp::Activated`/`UpdateOp`) is
native too. The installed *data* for all of these is readable, so the project implements the native
executors over that data.

## What exists now

| Piece | Where | Evidence |
|---|---|---|
| Reflection-typed arrays/structs inside structs in the VM's tagged-property reader (previously `SeqOpOutputLink.Links` silently decoded empty) | `src/vm.cpp` | `--kismet-run` links now resolve; census below |
| Kismet executor: impulse propagation, delays, Gate, Delay, SetBool, CompareBool, ActivateRemoteEvent, remote and mission remote events; world-acting ops are host bindings, unhandled ops are errors | `src/kismet.*` | `tests/kismet_test.py` (synthetic, in CTest), `--kismet-census` |
| Stock activation of the door: `RE_Ep14_OpenMarcusDoor` / `..CloseMarcusDoor` drive the existing Matinee mover through the installed sequence | `src/mover.*`, `OpenWillowMover.*` | in-engine `tools/test_mover.ps1`: 16/16 PASS (log `local/doors/run-20261001-092749.log`) |
| Mission/behavior native executor: loads the definition, objective sets, behavior provider, packed links; emits ordered effects (remote events, dialog triggers, set-sequence, objective set/complete, status, XP reward, mission weapon grant/removal); save/load of its state | `src/mission.*`, `--mission-run` | CLI run on the real mission, below |
| Event-name oracle over all 133 missions | `local/phase2/mission_events_census.json` (ignored) | 2,277 events: 133 `Default`, 724 objective-named, 633 set-named; 787 other names (custom events, unnamed sets) |
| Original-game call/state trace mod (not yet run) | `tools/sdk_trace/openwillow_gametrace` | **never executed**; see "Capture blocker" |

### Kismet census (Sanctuary_Dynamic, project reader)

19 sequences, 637 ops, 890 outputs, 422 links, **0 unresolved links**, one empty container sequence.
This proves decoding and resolution, not behaviour.

### Mission run (`ow-package Startup.upk --mission-run <mission> --cooked <dir> accept ...`)

Steps `accept`, `tick`, `obj:RockPaper_GoToRange`, `tick`, `obj:Fire`, `turnin` produce, in order: status
Active; `RocksPaper_MoveMarcusToRange`; set `GoToRange_ObjSet`; Marcus dialog 01 (+02 after 1 s);
`RocksPaper_TargetBack` (3 s later); on the first objective: set `RocksPaper_FinalObj`, mission weapon
granted, `RocksPaper_TargetForward`, `RocksPaper_SetFireTargetBool`; 5 s later the Hyperion dialog and
`SetSequence(BehaviorProviderDefinition_5, "Targetable")` (the dummy enemy's provider); on `Fire`: dialog 05,
`RocksPaper_FireCompleted`, weapon removed, status ReadyToTurnIn; turn-in: Complete and the XP reward
(`GD_MissionRewardBalance.XP.XPReward_02_Small`). No errors; no unsupported behavior class on this mission.

## Structural checks and the guesses they leave open

Checked on this mission (and by construction of the loader on every mission it loads): packed
`ArrayIndexAndLength` (index << 16 | length) ranges tile the consolidated link array exactly (21 of 21)
and every linked behavior index is in range. Guesses, all `UNVERIFIED`:

- The high byte of `LinkIdAndLinkedBehavior` is ignored (not an input index for single-input behaviors;
  its real meaning is unknown). Duplicate links to one behavior are collapsed per fired event, because
  the kernel exposes `RecentlyRunBehaviorsForSequence`; that this is its rule is a hypothesis.
- Event firing rule (activation = `Default`; set activation = set name; objective completion =
  objective name), and when an objective set auto-advances to `NextSet` or the mission becomes ready to
  turn in.
- Status names/transitions, when the mission weapon is lent/removed, XP amount (the reward attribute is
  identified, its value is not resolved).

## What is not done

Marcus as a placed NPC with interaction/dialog UI and scripted movement (`WillowSeqAct_AIScripted_2`,
`ArrivedAtMoveNode` events), the target-dummy enemy (spawn through `SeqEvent_PopulatedActor`,
pawn/AI/damage/death/loot), the lent pistol in the host, Phaselock stock behavior, player death/respawn and
persistence in the host, audio, and UE-side wiring of the mission system. The 4 Kismet world ops reached
by this mission's events (`SeqAct_Interp_0` target mover, `WillowSeqAct_AIScripted`, `SeqAct_Toggle`,
`GearboxSeqAct_TriggerDialogName`) are reported at the host boundary, not run.

## Capture blocker (needs the maintainer)

State-faithful comparison needs the real game driven interactively (menus, loading a character, accepting
the mission) with `openwillow_gametrace` enabled. That needs exclusive use of keyboard and screen for
roughly 30-40 minutes and a decision on which save to use. On 2026-10-01 a launch was started and
immediately stopped because the maintainer was using the machine; no input was injected, no trace was
produced. The saves were backed up under ignored `local/capture/save-backup-20261001/`; launching the game
also made Steam Cloud refresh `Save000A.sav`/`profile.bin` (live files carry the newer cloud timestamps;
the backup holds the older content). Nothing was restored over them.

## Reproduce

```powershell
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
$G = "$env:OPENWILLOW_BL2\WillowGame\CookedPCConsole"
build\Release\ow-package.exe "$G\Startup.upk" --mission-run GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire --cooked $G accept obj:RockPaper_GoToRange obj:Fire turnin
build\Release\ow-package.exe "$G\Sanctuary_Dynamic.upk" --kismet-run TheWorld.PersistentLevel.Main_Sequence.RocksPaperGenocide --cooked $G --remote RE_Ep14_OpenMarcusDoor
python tools/export_index.py        # ignored export caches under local/census/exports
```

## Host loop (UE5), 2026-10-01 — `tools/test_quest.ps1`

AI-assisted. Host behaviour only. **Not original-game parity, not a full gameplay loop**: it is a
reproducible scripted pass over the stock mission data with the stand-ins listed below.

Two editor launches (the runner owns the lock and kills its own process): the first plays the loop and
writes `local/quest/save.json`, the second resumes from it. Result: **first run 16/16 PASS, resume run 4/4
PASS** (final pair `local/quest/run-first-20261001-101113.log` and `run-resume-20261001-101145.log`, runner
exit code 0). An earlier script run (`run-first-20261001-101032.log`, also 16/16) stopped after the first launch
and exited 2 because of a runner output-handling bug, since fixed.

What the 16 checks exercise, in order: fresh status NotStarted; accept (Active) and the mission's
`RocksPaper_MoveMarcusToRange` remote event reaching the installed Kismet node (it ends at
`WillowSeqAct_AIScripted_2`, reported at the host boundary, **not run**); range objective advances to the
final set and lends the mission weapon; the dummy takes non-fire damage (objective unchanged) then fire
damage, which runs the dummy's own installed `OnTakeDamage` behavior chain
(`CompareObject` -> `AttemptStatusEffect`/`ChangeRemoteBehaviorSequenceState` -> `UpdateMissionObjective`) and
completes `Fire`; weapon removed; turn-in (Complete, one XP reward, not repeatable); lethal damage to Maya
respawns her without touching mission state; the save file exists. The resume run checks Complete, reward and
respawn counters retained and that the completed mission cannot be re-accepted.

**Stand-ins that are NOT recovered from the packages (all UNVERIFIED):** Marcus is a method call, not a placed
NPC, with no dialog UI or audio (dialog triggers are logged by tag only); the GoToRange trigger is a 700 cm
radius around the door; the dummy is the existing host engine-shape target, not `GD_TargetDummy`'s mesh/pawn,
and nothing spawns it through `SeqEvent_PopulatedActor`; "fire damage" is a host damage-type class and the
`CompareObject` verdict is answered by it (the compared objects sit in an untagged union that is not decoded);
the dummy's `FireDamage` sequence is enabled by the host (the original selects it through an instance-data
switch); the lent weapon is not the stock Maliwan pistol (Maya's existing host weapon fires fire-typed
shots); XP amount is unresolved (counted only); Maya's health (400) and respawn point (session start) are
invented; the dependency mission `GD_Episode03.M_Ep3_CatchARide` is pre-completed by a fixture.
Not run at all: `Behavior_Transform` and `Behavior_RegisterTargetable` (listed as host-boundary entries),
`SeqAct_Interp_0` (target mover), `SeqAct_Toggle`, `GearboxSeqAct_TriggerDialogName`, status effects
(Incendiary), loot, Phaselock behaviour from stock data, and skill upgrades.

Automated checks at this commit: CTest 9/9 (including `kismet-synthetic`), nine package comparisons match,
Kismet census 0 unresolved, door suite 16/16 (earlier run), inventory suite 47 PASS / 2 KNOWN_DIVERGENCE.

## Identity census for the pieces still to bind (extraction only, nothing imported)

From the cached export lists (`tools/export_index.py`, ignored `local/census/exports/`), all in
`Sanctuary_Dynamic.upk` unless noted. These are identities, not verified payloads; no UModel export was run
for them in this pass, so no extraction success/failure counts exist yet.

- **Marcus:** `GD_Marcus.Character.Pawn_Marcus` (WillowAIPawn, export 7243), `AIDef_Marcus` with its
  `AIBehaviorProviderDefinition_0` (mission interaction behaviors: `HasMissions`, `ShowMissionInterface` x3,
  `BehaviorSequenceEnableByMission` x10, `UpdateMissionObjective` x5, dialog and special moves),
  `Char_Marcus.Meshes.Skel_Marcus` (SkeletalMesh 14798), `Mati_Marcus_Body/Head` and four textures,
  `GD_Marcus.Character.AnimTree_Marcus`, dialog group `GD_Dialog_NPCImplementation.Groups.DialogGroup_NPC_Marcus`.
  His world placement is not found by name; he is spawned through the population system (not decoded).
- **Target dummy that completes `Fire`:** `GD_TargetDummy.Character.Pawn_TargetDummy` with
  `CharClass_TargetDummy.BehaviorProviderDefinition_5` (export 5500; sequences Idle, FireDamage, AmpDamage,
  Slagged, Targetable, ObjectiveComplete, ResetTarget), balance `GD_Population_Psycho.Balance.PawnBalance_TargetDummy`.
  `GD_TargetDummyBot` (a different pawn) serves the Corrosive variant. `FireDamage.OnTakeDamage` completes the Fire
  objective on damage of the right type; no kill is required by this data.
- **Lent weapon:** `GD_Z1_RockPaperGenocideData.MW_RockPaper_Fire` (`MissionWeaponBalanceDefinition`) over base
  `GD_Weap_Pistol.A_Weapons_Elemental.Pistol_Maliwan_2_Fire`, tied to the `Fire` objective.
- **Original trace channel:** `tools/sdk_trace/openwillow_gametrace` (hooks MissionTracker, behaviors, Kismet
  remote events; probe channel for asking the real engine to run its own logic). Written against the SDK stubs,
  never executed.

## Host loop with stock world data (2026-10-01)

AI-assisted (Claude). Host behaviour only, **no original-game capture**: everything below is the UE5 host running the
installed data with the stand-ins listed at the end. Automated checks and visual checks are reported separately.

### What is now data-driven

Manifests are read at run time from ignored `local/` (`-owslice=local/slice/world.json`, `-ownpcs=local/slice/npc_assets.json`
plus its sibling `npc_identity.json`, `-owaudio=local/slice/audio.json`, with `-owmover=local/doors/mover.json`); no
coordinate, stat or name list is compiled in (`OpenWillowSliceData.*`). Positions use the manifests' `host` blocks; a run-time
check confirms that convention against the door, whose placement is already verified against the loaded map: the loaded door
lies 70.6 uu from the world.json walk segment between nodes 12 and 26 (18,117.7 uu if Y were mirrored).

- **Range trigger:** the `WillowWaypoint_9` cylinder (radius 357.81, half height 145.31) from data. It completes
  `RockPaper_GoToRange` when the player's capsule overlaps the cylinder while that objective is active. The 700 cm radius is gone.
- **Marcus** (`OpenWillowNpc.*`): placed at the `WillowAIPawn_13` pose with the imported mesh, the pawn's mesh translation and
  the idle clip. The use key (E) within 250 cm accepts the mission or turns it in.
  - `RocksPaper_MoveMarcusToRange` reaches `WillowSeqAct_AIScripted_2` in the installed sequence, and the host now runs it:
    Marcus walks nodes 12, 26, 35, 40 at GroundSpeed 294 with the walk clip, arriving within each node's
    PawnArrivalRadius (128).
  - Each arrival enters that node's `SeqEvent_ArrivedAtMoveNode_*` in the same sequence instance, so the **installed links**
    open (node 12) and close (node 26) the door.
  - At the last node the host fires the action's `Finished` output. That reaches `SeqAct_ApplyBehavior_6`
    (Flag_LookAtPlayer), and he turns to face the player.
- **Target dummy:**
  - When the `Fire` objective becomes active, the host spawns the imported `Pawn_TargetDummy` mesh (idle `Shield_Struggle_2`,
    death `Death_Fire_var1`) at `WillowPopulationPoint_40`.
  - The host enters the sequence events whose `Originator` is the den or the point (`SeqEvent_PopulatedActor_2`,
    `SeqEvent_PopulatedPoint_0`; new `Kismet::eventsForOriginator`) and binds their Instigator variables to the dummy.
    `SetPhysics_0 -> AttachToActor_4` then attaches it to the target carrier.
  - The dummy provider's own `OnSpawned` emits `RocksPaper_MoveTargetForward`. That runs through `IsTargetReady?` to
    `SeqAct_Interp_0` **Play**.
  - The second bound Matinee (`UOpenWillowMover::BindTrack`) moves `InterpActor_4` and the attached scene props
    `InterpActor_52/53/54` by the binding's keys (2 s, +664 key-space units); the dummy follows. Event-track keys fire
    `ChangeBool_FALSE`, `WheelBackward` and `ChangeBool_TRUE`.
  - After the fire hit, the provider's `RocksPaper_SendTargetBack` drives **Reverse**. At the reverse end, `ChangeBool_TRUE`
    reaches `SeqAct_Destroy_0`, whose Target variables include the dummy's, so the host destroys the dummy.
    `GearboxSeqAct_ResetPopulationCount_0` is logged, not run.
  - `AOpenWillowCombatTarget` keeps the engine shapes everywhere else (Phaselock and combat tests).
- **Health:** max(20, 80 x 1.13^L) at Maya's level (L1: 90.4). The 400 stand-in remains only without `-owquest`.
- **Respawn:** `GetBestPlayerPlacementPoint`'s nearest `CanResurrectHere` station, then that station's exit points in round
  robin from 0. With no station activated, a death at the range picks FastTravelStation_0 exit 0 (8109.24, 2929.86, 3716.16).
  This matches world.json's own `fresh_state_choice`.
- **Dialog hook:** each mission dialog effect is looked up as `dialog:<event>` in audio.json, and its source id and state are
  logged. Every entry is `extracted_undecoded`, so nothing plays.
- **Dummy world behaviors** (`boundaryCalls`: Behavior_Transform EAIT_Fire, RegisterTargetable, IntMath,
  ChangeInstanceDataSwitch) are logged with their decoded fields and **not run**.
- **Damage:** the host maps its fire damage class to `GD_Incendiary.DamageType.DmgType_Incendiary_Impact` for `damageDummy`,
  and plain damage to None. Only the quest's own dummy counts.

### Automated checks (2026-10-01 15:05, CMake Release and UE module rebuilt first)

- **`tools/test_quest.ps1`: first run 37/37 PASS, resume run 7/7 PASS**, exit 0 (`local/quest/run-first-20261001-150538.log`,
  `run-resume-20261001-150849.log`). Checks per replaced stand-in:
  - Convention: the world.json convention matches the loaded door.
  - Marcus: placed at the stock pose with the imported mesh and idle clip. The use key out of reach is ignored; in reach it
    accepts. The installed Kismet starts the walk, the walk clip plays and the walk reaches the last node.
  - Door: opened then closed by the installed arrival events. Motions [+1, -1], one open start, one close end, door closed at
    the end. **The close request arrives at t=1.44 of the 1.5 s opening, and the host turns the door around** (UNVERIFIED).
  - End of walk: the `Finished` output sets look-at, and Marcus faces the player.
  - Range: outside the stock cylinder (but inside the old 700 cm) does not trigger. Touching it advances the set and lends the
    weapon.
  - Dummy: spawned at the population point with the stock mesh and attached to the carrier by the installed Kismet. The
    provider starts the Matinee, which reaches its forward end (world offset magnitude = key delta 664), and the dummy moves
    with the carrier. A visibility trace from the trigger centre hits the dummy at 439 uu.
  - Damage: non-fire damage does not complete; fire damage completes through the provider and the weapon is removed. The
    provider sends the target back, and the installed Kismet destroys the dummy at the reverse end.
  - Turn-in: the use key turns in, the reward is granted once and cannot be repeated.
  - Death: triggers a respawn at the decoded station exit point, restores formula health, keeps the mission state, and the
    respawn point is standable (grounded).
  - Dialog hook: found 6/6 lines, played 0. Save file written.
  - Resume run: status, reward and respawn count kept; Marcus at the stock pose; the mission cannot be re-accepted; health from
    the formula; no dummy.
- **`tools/test_mover.ps1` (door suite): 16/16 PASS** (`local/doors/run-20261001-150928.log`). The door composition is now the
  shared `MatineePose`, which reduces to the previous formula when the first rotation key is zero.
- **CTest:** 10/10. **`tools/verify_packages.py`:** 9/9 packages match. **`tests/kismet_test.py`:** passes, including the new
  case 13 (`--originator`).
- **Earlier failing runs** on this pass, kept as evidence:
  - `run-first-20261001-120535`, 35/37: the door's close request was dropped while it was opening, and the dummy was out of the
    line of fire.
  - `run-first-20261001-121729`, 36/37: position delta along world axes. The dummy ended behind a pillar in the next lane; the
    trace hit `StaticMeshActor_SMC_1635` at 1016 uu (`OWQuest_2_RangeDummy-20261001-121729.png`).

### Visual checks (one still each, by eye; host presentation only)

- `local/quest/OWQuest_1_MarcusStockPose-20261001-150538.png`: Marcus, textured, behind his shop counter bars, facing the
  camera.
- `OWQuest_2a_DummySpawned-*.png`: the dummy and a dartboard at the far end of the range's centre lane.
- `OWQuest_2_RangeDummy-20261001-150538.png`: after the forward move, the trolley (dartboard and kneeling dummy) has come down
  the centre lane toward the player.
- `OWQuest_3_RangeMarcusAndDummy-20261001-150538.png`: Marcus at the end of his walk, standing on the range floor, with the lane
  and dartboard behind him.

### Remaining stand-ins and UNVERIFIED

- **Target Matinee frame:** pose = Key(t) x Key(0)^-1 x placed pose. The key-space delta (+664 X) is un-rotated by the first
  rotation key (-180, 0, -90), then rotated by the placed rotation, which gives -664 world Y, down the lane.
  - Why this reading: it reduces exactly to the door's convention (checked by eye) and keeps the target in the lane.
  - The other readings are rejected only visually: world axes put the target behind a pillar, and the actor frame moves it
    664 uu down. Not compared with UE3.
- **Door turn-around:** a Play/Reverse request while the door moves the other way reverses it at its current position.
  `InterpActor.InterpolationChanged` is not run.
- **Marcus walk:**
  - The first leg (stock pose to node 12) is a straight line; the original uses native navmesh pathing.
  - Host choices: straight lines between nodes, planar arrival radius, yaw turned at the class rate. No SlowDownDist easing, no
    collision, no floor trace. Look-at is yaw only (no head or eye control).
  - Talk reach of 250 cm. No mission menu and no barks: the use key accepts or turns in directly.
- **Dummy:**
  - Spawn trigger: "the `Fire` objective became active" (MissionPopulationAspect is native).
  - Order: populated events first, then OnSpawned. SetPhysics is not changed (class default not resolved).
  - Attachment: the dummy keeps its spawn pose on the carrier. The holder's bone `Target` offset is not applied, and the holder
    has no prepared mesh.
  - Hit volume: a hidden capsule sized from the imported mesh bounds. Health 20000 (host value).
  - Not run: Transform, RegisterTargetable and the cosmetic switches. `RocksPaper_TargetKilled_Reset` (BodyDeath provider) is
    not bound.
  - Matinee details: event-key boundary firing; `bRewindOnPlay` read as "every Play rewinds". The two Ak tracks are listed, not
    played.
- **Touch rule:** capsule-against-cylinder overlap stands in for UE3 touch semantics.
- **Health:** the 94-constant reading is still open. Skills, class mods and relics are not applied. A level change after start
  is not reapplied.
- **Respawn:**
  - Station activation is not traced, so the host never activates a station: the active-checkpoint and active-station rules
    never fire.
  - Occupied and vehicle exit skipping are not modelled.
  - With a CoopPlayerStart near `ResurrectTravelStation_0`, the original may pick that station instead; not established.
- **Unchanged from the first host loop:**
  - The lent weapon (host fire-typed shots).
  - The XP amount (counted only).
  - The dependency fixture.
  - Save/resume covers the mission state only; Marcus returns to his stock pose on reload.
- **Hand play:** in these runs Maya's first-person arms have no skeletal mesh and she is "Unarmed". The log shows
  `GetSocketInfoByName(R_Weapon_Bone): No SkeletalMesh for Component(FirstPersonArms)` and `M_OW_FxAdditive` not found. Weapon
  assets are seeded by another lane. Without an equipped weapon, the Fire objective cannot be completed by hand.

### Hand play

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/run_quest.ps1 -Fresh    # optional: -Level N, -Save <file>, -Seconds N
```

The script waits for and holds `local/ue_run.lock`, saves to `local/quest/manual-save.json` and logs to
`local/quest/manual-*.log`. An on-screen line shows the objective, plus "E: talk to Marcus" when in reach (otherwise his
distance). The log prints `OWQUEST hint: press E (use) to talk to Marcus`.

Keys:

| Key | Action |
|---|---|
| WASD | Move |
| Mouse | Look |
| Space | Jump |
| Left Shift | Sprint |
| **E** | **Use: talk to Marcus (accept / turn in)**; also the developer door toggle and pickups |
| Left mouse | Fire |
| R | Reload |
| 1-4 | Weapon slots |
| 0 | Holster |
| F | Phaselock |
| I | Inventory |
| K | Skills |

Route:
1. Go to Marcus behind his shop counter and press E to accept.
2. Follow him to the range; the door opens and closes on his walk.
3. Step into the start of the centre lane (the waypoint cylinder). The dummy rolls toward you.
4. Shoot it with a fire weapon. It rolls back and is removed.
5. Press E at Marcus to turn in.
