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

Superseded for the player side by "Player side with stock data" below (lent stock pistol, XP, Phaselock, loot).

## Player side with stock data (2026-10-01)

AI-assisted (Claude). Host behaviour only. **No original-game capture**: every rule below that the game decides in
native code is a host choice and is labelled UNVERIFIED here and in the code. Automated checks, visual checks and one
hand-play session are reported separately.

### What changed

- **Slice gear instead of the level-30 demo set.** `run_quest.ps1` and `test_quest.ps1` pass
  `-owitems=local/items/slice` (tools/weapon_slice_gear.py) and start Maya at the slice gear level from
  `slice_manifest.json` (8; that level is itself an UNVERIFIED slice choice inside Sanctuary's 7-9 band, printed at
  launch). She carries the four pool-rolled slice guns and draws the first. The demo items in `local/items` are not
  loaded in these sessions. `-Level N` still overrides.
- **Lent stock pistol.** The mission executor's `MissionWeaponGranted` effect (lent while the objective named by
  `MissionWeapon.MissionObjective` is active, the rule recorded earlier, UNVERIFIED) now gives Maya the recipe whose
  provenance is `mission_weapon` and whose balance equals the mission's own `MissionWeapon`
  (`GD_Z1_RockPaperGenocideData.MW_RockPaper_Fire`). The recipe is `slice_mission_pistol_fire` ("Inflammatory
  Torment", level 8, 49.1 damage, 1.72/s, magazine 7.26, card damage type
  `GD_Incendiary.DamageType.DmgType_Incendiary_Impact`). It is shown with the imported
  `Weapons/MaliwanPistol/SK_Pistol_Maliwan_2_Fire_seed1`, the rolled seed-1 sample from `npc_assets.json`.
  - Placement (host rule, UNVERIFIED): the first empty slot, else the last slot. It is drawn at once and its level is
    not checked.
  - `MissionWeaponRemoved` takes it back and draws the weapon held before. A save made while it is lent re-lends it on
    the next launch (not exercised by the suite).
  - Mission weapons and the loot stand-in are no longer loaded into the backpack at start.
- **Damage type from the item.** A shot carries the held item's card `damage_type` path into the dummy's
  `OnTakeDamage` (`vm::FireMissionSlice::damageDummy`). The host fire-damage class `UOpenWillowFireDamageType` is
  removed. `DamageSource` is still passed empty: its stock value is not decoded.
- **Arms.** The arms are hidden while no pose clip set is loaded and shown once a weapon is drawn.
  - Why an unarmed Maya showed no arms: the arms mesh was set at start, but the clips were set only in `SelectSlot`.
    With no weapon the animation instance outputs the bind pose, which lacks the clips' root correction. That the bind
    pose lies out of view is an inference; it was not checked by screenshot.
  - No `Unarmed` clips are imported, so holstering also hides the arms.
  - Whether the original shows arms with no weapon is not observed (UNVERIFIED).
- **`M_OW_FxAdditive`** (tracers, muzzle and impact flashes, the Phaselock shell) and `M_OW_BulletHole` were missing
  because `Weapons/InfinityProxy` had never been seeded in this worktree. `tools/seed_slice_player_assets.ps1` step
  `fx` seeded it with the existing `import_infinity_proxy.py`, from `local/items/pistol_vladof_5_infinity_3.gltf` and
  the 2026-09-30 UModel texture export.
- **XP into skills.** Turn-in adds `MissionXp(L)` to the skills component: percentage 0.05 (playthrough 1) x
  (required(L+1) - required(L)), with required(L) = 60 L^2.8 + 7.33 read from world.json. That is the CANDIDATE rule
  from `tools/slice_values.py`; `GetExperienceReward` is native, so it is UNVERIFIED.
  - The mission level is the slice gear level (8), so the reward is 396 XP. The C++ value equals the tool's own
    candidate table.
  - Level-up uses the existing threshold curve (60 L^2.8 - 60, which equals required(L) - required(1)).
  - At level 8 one reward does not level Maya up (7,918 XP to level 9). Points she already has are spent on the
    Skills page (K).
  - The canvas HUD now shows her real level, XP progress and health (it showed a fixed "1" before).
  - Experience and skill grades are not saved.
- **Stock Phaselock** from `local/character/action_skill_siren.json` (`-owactionskill=`). No number is compiled in;
  without the manifest the skill is unavailable. Row by row against the record's host-versus-stock table:

  | Row | Status |
  |---|---|
  | Lock length | Done: release at LiftDuration + Att_Phaselock_Duration x target PhaselockTimeScale (5.7 s base) |
  | Fade | Done: the host shell fades over LockFadeOutTime before the release (the shell is host presentation, not the stock bubble effect) |
  | Lift height | Partly: 200 uu above the target's origin, half of it in the first half of the lift. Ground trace, collision height and ceiling clamp are not applied, and the curve shapes are host choices |
  | Hover | Done: 30 uu x sin(0.5 pi t) from the end of the lift, no rotation. The bob's time origin is UNVERIFIED and "smoothed" is not modelled |
  | Cooldown | Done: a 13 s pool refilled at the cast, drained at base rate + CooldownManager PreAdd (0) while held, then at 1/s, so ready about 18.7 s after a base cast (semantics UNVERIFIED) |
  | Miss | Done: no lift, and the cooldown resets after ReleaseBufferTime (1 s). The miss impact effect is not drawn |
  | Targeting | Not done: native auto-aim. Host: view ray, then a 30 cm sweep to 2500 cm; a sweep that starts inside geometry is ignored |
  | Valid target | Partly: not already locked, not dead. Friendliness, vehicles, `Flag_Skills_CanPhaseLock` and blocked-target damage are not done |
  | Re-lock same target | Done: x0.6 time scale (MT_Scale -0.4) for 25 s after release, so 3.7 s |
  | Cast gate | Not done beyond "action skill bought" (weapon action, on foot, healthy) |
  | Upgrades | Done for Suspension: the manifest's PostAdd per grade on Att_Phaselock_Duration. Other skills are not applied |
  | Skill points | Unchanged (same rule as the data) |
  | Target state | Partly: host flag only. No IsPhaselocked attribute, AI flag or AIProvoke |
  | Drop | Host shape: DropTime 0.5 s quadratic ease-in |

- **Loot.** The stock data gives no item drop for this mission. `PawnBalance_TargetDummy` has no item pools, and the
  mission's `RewardData` tags only `ExperienceRewardPercentage` and `CreditRewardMultiplier` 0 (read with
  `--properties` on Startup export 24569). Two consequences:
  - The dummy does not drop anything.
  - A **turn-in loot stand-in** sits behind the mission reward. `weapon_slice_gear.py --reward-only` rolls the slice
    fallback list (`StandardEnemyGunsAndGear`) from seed 1 upward with the existing `loot_pools.roll` and keeps the
    first seed whose roll drops a weapon. That is seed 31 of 31: Pool_GunsAndGear > Pool_Weapons_All > ..._01_Common
    > Shotguns_01_Common > `SG_Bandit`, parts rolled with the same seed. Seeds 1-30 dropped nothing or money/eridium.
  - The host drops that recipe as a pickup in front of the player at turn-in. The use key collects it: with nothing
    to accept or turn in, E near Marcus is no longer consumed.
  - This is a demonstration of the pickup path, **not stock behaviour**. The seed choice is deliberate and labelled.
- **Pistol paint.** `prepare_weapon_paint.py` now accepts MIC chains with no pattern texture (as the thumbnail
  renderer already did) and a `--mesh` target. `Mati_MaliwanUncommon` -> `MasterMati_MaliwanUncommon` provides
  masks, the packed detail atlas (blue channel for pistols), the normal map and nine A/B/C zone colours.
  - The pistol now shows pale white and blue-grey zones instead of the raw composite.
  - Its `p_Decal` (`Pattern_MaliwanUncommon`, with `p_DecalScalePosition`/`p_DecalRotate`/`p_DecalChannel`) is not
    reproduced.
  - The Master_Gun graph is stripped, so the channel reading stays UNVERIFIED.
  - The four pool-rolled slice guns and the loot stand-in have the grey stand-in material: their MICs have no local
    UModel export.

### Automated checks (2026-10-01, CMake Release and UE module rebuilt first)

- **`tools/test_quest.ps1`: first run 57/57 PASS, resume run 7/7 PASS**, exit 0 (`local/quest/run-first-20261001-155229.log`,
  `run-resume-20261001-155326.log`). New checks: `mission_weapon_recipe_and_mesh_found`,
  `mission_weapon_not_carried_before_lend`, `maya_starts_armed_with_arms_shown`,
  `lent_pistol_drawn_with_recipe_identity_and_stats`, `lent_pistol_shows_imported_mesh_with_arms`,
  `wrong_element_shot_reaches_dummy_and_does_not_complete` (a real shot from the Jakobs slice pistol,
  `DmgType_Normal`), `lent_pistol_shot_carries_its_damage_type_to_dummy`,
  `incendiary_shot_completes_fire_objective_via_dummy_provider`, `mission_weapon_removed_after_objective`,
  `xp_amount_is_candidate_formula_at_mission_level` (396 = the tool's table), `xp_reward_levels_up_when_requirement_met`
  (test fixture: experience topped up by 7,522 first; level 8 -> 9, points 4 -> 5), `turn_in_drops_loot_stand_in_pickup`,
  `use_key_collects_loot_pickup_into_backpack`, `skill_point_buys_phaselock`, `phaselock_timelines_match_manifest_table`
  (60 values, grades 0-5, first lock and re-lock; agreement between two readings of the same data, not a game check),
  `phaselock_miss_lifts_nothing_and_holds_skill`, `phaselock_miss_resets_cooldown_after_release_buffer`,
  `phaselock_hit_uses_manifest_timeline`, `phaselock_lifts_to_stock_height` (224 uu at 1.30 s, within 200 +- 30),
  `phaselock_releases_at_manifest_time` (5.73 s against 5.70; release on the first tick after the time, one frame
  allowed), `phaselock_cooldown_paused_while_target_held` (12.97 of 13 left at release),
  `phaselock_diminishing_returns_on_released_target` (0.6), `suspension_point_adds_manifest_lock_time` (test fixture:
  level 11; Ward x5 then Suspension 1 gives 5.5). The old host-damage checks were replaced by the real-shot checks.
- **Earlier failing runs, kept as evidence:**
  - `run-first-20261001-153640` (50/55): the Phaselock sphere sweep started inside the range's ceiling beam and
    reported a hit at 0 uu. Fixed by the view-ray-first targeting.
  - The next run (54/55): the release was seen 0.05 s late, against a 0.05 s tolerance. The check now allows one frame.
- **`tools/test_mover.ps1`: 16/16 PASS** (`local/doors/run-20261001-160822.log`).
- **`tools/test_inventory_actions.ps1`: 47 PASS, 0 FAIL, 0 NOT_RUN, 2 KNOWN_DIVERGENCE** (sort order; exit 3)
  (`local/inventory-actions/run-20261001-160333.log`). The gear manifest is absent in this worktree; the shield
  steps use the suite's synthetic shield and passed, so no step failed for missing data.
- **CTest 10/10; `tools/verify_packages.py` 9/9 packages match.**

### Visual checks (by eye; host presentation only)

- `local/quest/OWQuest_2_RangeDummy-20261001-154542.png` (same view in `-155229`): Maya's arms hold the lent Maliwan
  pistol in pale white and blue-grey paint. The card shows "Inflammatory Torment / Maliwan". The stock dummy kneels in
  front of the dartboard.
- `OWQuest_1_MarcusStockPose-*`: Maya holds the grey Jakobs slice pistol at the start; the arms are visible.
- `OWQuest_4_Phaselock-*`: the host target lifted into the range's ceiling beams inside the violet shell (now drawn
  with `M_OW_FxAdditive`). The lift has no ceiling clamp; see the table.
- `local/quest/OWHandSmoke_fresh_40s-20261001-155537.png` (desktop capture of the hand-play window): Sanctuary at the
  session start. Maya holds the slice pistol with her arms visible, and the objective line reads "talk to Marcus (E)".
- Earlier captures (`-153640`) show the unpainted composite noise on the same pistol, for comparison.

### One hand-play session (2026-10-01 15:55-15:57, `local/quest/manual-20261001-155458.log`)

The agent started a `run_quest.ps1 -Fresh` smoke window. A person then played it: sprint, reload, slot changes, accept
at Marcus, the range touch, the dummy, one incendiary shot from the lent pistol completing Fire, the pistol taken back,
turn-in, +396 XP (level 8 stays 8), and the loot stand-in picked up with E. Phaselock was not used in that session.
The agent then stopped that window about six minutes after the last input; the mission state had already been saved
(`manual-save.json`, status Complete). This is the only hand-play evidence. It is not a game comparison.

### Still not done / UNVERIFIED

- Where the game puts a lent weapon, its level, and whether the original shows arms with no weapon.
- The XP amount rule, the mission level, the experience and skill save, and health after a level-up.
- `DamageSource`, the Incendiary status effect, the dummy's hit volume and health (20000).
- Phaselock rows marked not done or partly above. Its sweep is a host stand-in.
- Loot is a labelled stand-in. Ammo and money drops and pickups are not hosted.
- Paint: the decal, the Master_Gun lighting, and paint for the other slice guns.
- Audio: unchanged (lookup only).
- The inventory page's 3D preview still looks only in `Weapons/Items`, so slice guns show no preview mesh there.

### Hand play (updated)

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/run_quest.ps1 -Fresh    # Maya at the slice gear level (8)
```

Keys are unchanged from the table above. E near Marcus now falls through to pickups when there is nothing to accept or
turn in. Route:
1. Maya starts holding the Jakobs slice pistol. Walk to Marcus's shop and press E to accept.
2. Follow him to the range and step into the centre lane's waypoint cylinder. The lent "Inflammatory Torment" is put
   in a slot and drawn, and the dummy rolls forward.
3. Shoot the dummy with it. Another slice gun does not complete the objective. Fire completes, the pistol is taken
   back, and the dummy rolls back and is removed.
4. Press E at Marcus to turn in: +396 XP (bar at bottom centre). Press E again by the dropped shotgun to pick it up.
5. K opens Skills: buy Phaselock (Maya has 4 points at level 8 from the start). F casts it. The only liftable target
   in this session is the stock dummy while it is on the range, so buy Phaselock before shooting it if you want to
   lift it. After it is removed, F only shows the miss.

## Progression, Phaselock rules and dummy behaviours (2026-10-01, second machine)

AI-assisted (Claude, with subagents). Host behaviour only. **No original-game capture**: every rule the game decides in
native code is a host choice labelled UNVERIFIED here and in the code. This pass ran on a second machine whose ignored
data was regenerated from the installed game with the documented tools (`docs/TOOLING.md`), not copied; hand-patched
content from the first machine (for example floor meshes) is not present there. Before any code change the regenerated
data reproduced the recorded baseline: quest suite 57/57 and resume 7/7, door suite 16/16, CTest 10/10, packages 9/9.

### What changed

- **Progression is saved.** The quest save has an optional `progression` block (level, experience, action grade, skill
  grades). On load it replaces the `-owlevel` start level before weapons are equipped; a save without it behaves as
  before; an invalid block fails the quest. `Save()` already ran every tick, so XP, level-ups and skill purchases reach
  the file within a frame. `tools/run_quest.ps1` without `-Fresh` therefore resumes progression too (its "Maya level N"
  launch line still prints the gear level, which the save then overrides).
- **Health follows the level.** Max health is recomputed from the same formula whenever the level changes, and a
  level-up refills current health. The refill is read from data: `OnExpLevelChange` (script) runs the class's
  `OnLevelUp` behaviours, whose skill definition adds `HealthMaxValue` to `HealthCurrentValue`. That the pool caps the
  sum at the maximum, and that the effect acts once, are UNVERIFIED. The same definition also touches the action-skill
  cooldown; that is not modelled.
- **Phaselock**, row by row (replaces the rows of the same name in the table above):

  | Row | Status |
  |---|---|
  | Lift height | Done from script: `BeginLifting`'s ground trace, collision half height and ceiling clamp on the centre's path, with the `GetLiftLocation` curve. UNVERIFIED: the 1 uu box on `ECC_Visibility`, bounds standing in for the cylinder, the bob's time origin and smoothing; the bob is not clamped |
  | Targeting | Partly: range from the auto-aim data (`GD_Autoaim.Default` `MaxTargetDistance`); the view ray then the 30 cm sweep stand in for the native `GetPreferredTarget` (UNVERIFIED) |
  | Valid target | Partly: alive, not already locked, and a host property standing for `Flag_Skills_CanPhaseLock`; a blocked target is not lifted and its damage is not applied (amount not recovered). Friendliness, vehicles and Resurrect are not modelled |
  | Cast gate | Partly: bought, cooldown, and the activation constraints (weapon action = not reloading, healthy = health above 0, on foot = always). The evaluators are native, so each mapping is UNVERIFIED; while-active constraints, ladders and rider seats are not modelled |

  The earlier statement that Phaselock lifts the target into the range's ceiling beams was a misreading of the
  screenshot: in the lane the nearest surface is about 460 uu above the target's centre and the lifted target's top
  stays about 160 uu below it; a beam nearer the camera hides the upper part of the view (the 220 cm presentation shell
  is a separate host effect). The manifest format is now `openwillow.action_skill/2`.
- **Dummy behaviours.** `Behavior_Transform` (EAIT_Fire) sets the dummy's transform type and its target name becomes
  the balance's playthrough-1 transformed name (`GetTransformedDisplayName` is native: UNVERIFIED).
  `Behavior_RegisterTargetable` puts the dummy in a host targetable list; no host targeting reads it yet. IntMath and
  ChangeInstanceDataSwitch remain logged, not run.
- **Attach point.** The `AttachToActor` name `Target` is a `SocketComponent` in the holder's body composition, not a
  skeletal bone. `world.json` now carries its pose and the host puts the dummy's origin on it at attach (UNVERIFIED:
  `Activated` is native, `bUseConstructAttachment` is not interpreted). The earlier "kneeling" dummy was the mesh hanging
  below an origin placed at floor level.
- **Dependency fixture.** `GD_Episode03.M_Ep3_CatchARide` cannot be satisfied from installed data (a plain dependency;
  Sanctuary is reachable while it is still active; the bulk fast-forward is native). It remains a fixture standing in
  for save state, now filled from the mission's declared `Dependencies`, labelled, and logged at start.
- **Inventory previews** resolve meshes through `UOpenWillowInventory::LoadWeaponMesh`, so slice guns and the lent
  pistol are found. Not looked at by eye in the inventory page.
- **Weapon paint.** The five slice guns and the mission pistol are painted from their MIC chains plus Master_Gun's own
  parameter defaults, with a decal layer by an UNVERIFIED reading (`docs/TOOLING.md`, DECISIONS 2026-10-01).

### Automated checks (2026-10-01 22:23-22:48, CMake Release and UE module rebuilt first)

- **`tools/test_quest.ps1`: first run 73/73 PASS, resume run 10/10 PASS**, exit 0 (`run-first-20261001-224649.log`,
  `run-resume-20261001-224755.log`; also `-223529` before the paint import). New first-run checks:
  `level_up_sets_formula_max_health`, `level_up_refills_current_health` (fixture: half of max health removed before
  turn-in), `max_health_follows_level_after_fixtures`, `save_holds_final_progression`,
  `phaselock_lift_stays_below_ceiling` (under a test fixture box, because the lane has no low surface),
  `phaselock_height_is_lift_end_plus_bob`, `phaselock_cast_gate_open_when_constraints_met`,
  `phaselock_weapon_action_constraint_refuses_cast`, `phaselock_health_constraint_refuses_cast`,
  `phaselock_blocked_target_is_not_lifted`, `phaselock_blocked_cast_keeps_cooldown`,
  `dependency_fixture_is_the_missions_declared_dependencies`, `dummy_attached_at_holder_socket_from_manifest`,
  `dummy_transform_type_and_target_name_from_provider`, `dummy_not_targetable_before_register_behavior`,
  `dummy_registered_targetable_by_provider`. Changed: `phaselock_lifts_to_stock_height` now asserts the lift rule gives
  `HeightFromGround` (+-2) over open ground; `dummy_moves_with_carrier` measures from the attached location. New resume
  checks: `resume_level_and_experience_from_save_over_owlevel`, `resume_skill_points_and_phaselock_from_save`,
  `resume_skill_grades_from_save` (all compared against the loaded block, not fixed numbers).
- **Earlier failing run, kept as evidence:** `run-first-20261001-222312` (67/68): `phaselock_lift_stays_below_ceiling`
  found no surface within `HeightFromGround` of the target's centre anywhere in the lane.
- **`tools/test_mover.ps1`: 16/16 PASS.** **CTest 10/10; `tools/verify_packages.py` 9/9.** Python: `slice_world_test`
  15, `prepare_action_skill_test` 9, `weapon_paint_test` 9, `skill_stats_test` 5 passed.
- **`tools/test_inventory_actions.ps1`: 45 PASS, 0 FAIL, 2 NOT_RUN, 2 KNOWN_DIVERGENCE** (exit 1) on this machine. The
  two NOT_RUN steps are the backpack scroll steps: the locally seeded backpack has fewer than nine rows. This is
  missing local data, not a code result; on the first machine the recorded result was 47 PASS.

### Visual checks (host presentation only)

- By eye: `OWQuest_2_RangeDummy-20261001-224649.png` shows the dummy standing on the lane floor in front of the
  dartboard, and the lent pistol with orange bands over the earlier blue-grey and white patches.
  `OWQuest_1_MarcusStockPose-*` shows the Jakobs slice pistol with a pale marbled surface instead of flat grey.
- Independent critic agent (it could not fetch real first-person images and judged from text descriptions of the
  skins plus one unrelated local reference; low to medium confidence): Jakobs pistol **2/10**, Maliwan fire pistol
  **4/10** (2/10 before the decal). Both show irregular blotchy patches where the real skins have clean zones.

### Still not done / UNVERIFIED

- Everything native listed above; no rule here has been compared with the running game.
- Weapon paint: the zone mask reading (blotches), `p_DecalRotate`, static switches; no real-image comparison yet.
- The parking-lot floor in front of Scooter's garage was not checked: it needs a hand run on the machine that holds
  the hand-patched content.
- Audio is unchanged (lookup only; the decoder choice is the maintainer's).
