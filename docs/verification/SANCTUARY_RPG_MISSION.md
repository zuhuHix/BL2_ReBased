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
- `OWQuest_2_RangeDummy-20261001-150538.png`: after the forward move, the trolley (dartboard and dummy; the dummy looked kneeling because its origin was placed at floor level, corrected in the
  2026-10-01 second-machine section below) has come down
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

Superseded for the player side by "Player side with stock data" below (lent stock pistol, XP, Phaselock).

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
  - Mission weapons are no longer loaded into the backpack at start (the mission lends them).
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
  - 2026-10-02, following [NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) (read from native code, UNVERIFIED in game):
    the amount is truncated on the integer curve R(n) = max(0, trunc(60 x (n^2.8 + 7.33)) - 499) in single precision
    (395 at stage 8, not 396); the mission level is Sanctuary's region game stage (clamp(level, 7, 9) from the
    decoded playthrough-1 table, 8..11 once WelcomeToSanctuary is complete), fixed when the walker sets Maya's level at
    session start and kept in the quest save (`region_stage`); level-up uses the same curve (one point higher than the
    old one at most levels) with the level-50 cap (DLC cap increments not modelled).
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
  - **The turn-in gives XP only, as the stock data says.** A turn-in loot stand-in used to sit behind the reward (`weapon_slice_gear.py
    --reward-only` rolled `StandardEnemyGunsAndGear` to an `SG_Bandit` at seed 31 and the host dropped it as a pickup). **It was removed on
    2026-10-07 by maintainer decision** ("that's not how missions are gonna look"): the tool flag, its recipe file and manifest key, the host's
    spawn, and the two suite checks that picked it up are gone. The general pickup and backpack code stays (the lent pistol and the inventory
    suite use it). The script's `GetItemRewardsForPlayer` returns no item for this mission and the host takes nothing from any other source.
- **Pistol paint.** `prepare_weapon_paint.py` now accepts MIC chains with no pattern texture (as the thumbnail
  renderer already did) and a `--mesh` target. `Mati_MaliwanUncommon` -> `MasterMati_MaliwanUncommon` provides
  masks, the packed detail atlas (blue channel for pistols), the normal map and nine A/B/C zone colours.
  - The pistol now shows pale white and blue-grey zones instead of the raw composite.
  - Its `p_Decal` (`Pattern_MaliwanUncommon`, with `p_DecalScalePosition`/`p_DecalRotate`/`p_DecalChannel`) is not
    reproduced.
  - The Master_Gun graph is stripped, so the channel reading stays UNVERIFIED.
  - The four pool-rolled slice guns have the grey stand-in material: their MICs have no local
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
  (test fixture: experience topped up by 7,522 first; level 8 -> 9, points 4 -> 5), `skill_point_buys_phaselock`, `phaselock_timelines_match_manifest_table`
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
  pistol in pale white and blue-grey paint. The card shows "Inflammatory Torment / Maliwan". The stock dummy looks kneeling in
  front of the dartboard (an origin-placement error, corrected in the second-machine section below; DECISIONS 2026-10-01,
  "dummy world behaviours run in the host").
- `OWQuest_1_MarcusStockPose-*`: Maya holds the grey Jakobs slice pistol at the start; the arms are visible.
- `OWQuest_4_Phaselock-*`: the host target lifted inside the violet shell (now drawn
  with `M_OW_FxAdditive`). Read at the time as "into the range's ceiling beams" with no ceiling clamp; that reading was
  wrong, see the correction in the second-machine section below (DECISIONS 2026-10-01, "Phaselock lift, target rule and
  cast gate").
- `local/quest/OWHandSmoke_fresh_40s-20261001-155537.png` (desktop capture of the hand-play window): Sanctuary at the
  session start. Maya holds the slice pistol with her arms visible, and the objective line reads "talk to Marcus (E)".
- Earlier captures (`-153640`) show the unpainted composite noise on the same pistol, for comparison.

### One hand-play session (2026-10-01 15:55-15:57, `local/quest/manual-20261001-155458.log`)

The agent started a `run_quest.ps1 -Fresh` smoke window. A person then played it: sprint, reload, slot changes, accept
at Marcus, the range touch, the dummy, one incendiary shot from the lent pistol completing Fire, the pistol taken back,
turn-in, +396 XP (level 8 stays 8), and the loot stand-in picked up with E (the stand-in was removed on 2026-10-07). Phaselock was not used in that session.
The agent then stopped that window about six minutes after the last input; the mission state had already been saved
(`manual-save.json`, status Complete). This is the only hand-play evidence. It is not a game comparison.

### Still not done / UNVERIFIED

- Where the game puts a lent weapon, its level, and whether the original shows arms with no weapon.
- The XP amount rule, the mission level, the experience and skill save, and health after a level-up.
- `DamageSource`, the Incendiary status effect, the dummy's hit volume and health (20000).
- Phaselock rows marked not done or partly above. Its sweep is a host stand-in.
- The turn-in gives XP only (stock data; the loot stand-in was removed on 2026-10-07). Ammo and money drops and pickups are not hosted.
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
4. Press E at Marcus to turn in: +395 XP since 2026-10-02 (396 before; bar at bottom centre). Press E again by the dropped shotgun to pick it up.
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

## Script swap 1: accept and turn-in run the game's script (2026-10-05)

AI-assisted (Claude), lane I1. First stand-in swap of the Phase 2 Fire-mission plan. Source of every rule below is the own-words
note [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) (read from native code, **UNVERIFIED in the running
game**); the script itself is the installed WillowGame bytecode. Nothing here was compared with the real game's behaviour.

### What now runs as script

- **Accept** (`FireMissionSlice::accept`): the installed `WillowPlayerController.AcceptMission(Mission, MissionDirector = None)` on a
  VM controller. It reads `Role`, `WorldInfo.GRI` (a `WillowGameReplicationInfo`) and its `MissionTracker` (a VM `MissionTracker`
  object) and calls the native `MissionTracker.ActivateMission`. Before, `MissionSystem::accept` was called directly and the
  kickoff was played at once (a labelled host stand-in).
- **Turn-in** (`FireMissionSlice::turnIn`): `WillowPlayerController.ServerCompleteMission(Mission, None)`, which calls the native
  `CompleteMission`, then `PlayTurnIn`. The slice still refuses a turn-in while the mission is not ReadyToTurnIn (the real UI
  offers it only when `CanEndMission` holds; the native `CompleteMission` itself would still chain and untrack).
- **Every status change** (also the ReadyToTurnIn that the host's objectives cause) calls the installed
  `UpdateMissionStatus(Mission, NewStatus)` on the controller from `MissionSystem::setStatus`, **before** the observers and the
  `Default` event, then `MissionTracker.TriggerMissionStatusChangedDelegates`. On Complete the script's old-status check passes
  (the controller's record went Active -> ReadyToTurnIn), so it runs `ServerGrantMissionRewards`: currency type and amount, then
  `GetExperienceReward` and `ExpEarn`, the reward struct, the reward UI path (`AcceptOrSaveUnclaimedReward`, which for the empty
  reward ends in `MissionRewardsReceived`, clearing `bNeedsRewards`).
- **Kickoff** is now the tracker's pending record: `setStatus(Active)` writes it (the `SetActiveMission` rule: kickoff not heard,
  none pending); the next `tick()` consumes it (`IsMissionMoviePlaying` runs on the controller, `Default` id 12, heard flag set,
  record cleared). The mission's first set therefore starts one tick after acceptance instead of inside `accept`. The explicit
  `MissionSystem::kickoff()` stays as the test path (`--mission-run accept kickoff ...`) and consumes the record too.
  `PlayTurnIn` fires `Default` id 14 (the Fire mission has nothing on that link).

### Natives that are ours (all UNVERIFIED; `src/mission_script.*`, scoped to the bridge's objects like `src/mover.cpp`)

| Native | Implementation | Note section |
|---|---|---|
| `MissionTracker.ActivateMission` | `MissionSystem::accept` (refuses silently: not NotStarted, dependencies unmet); role ignored | MissionTracker.ActivateMission |
| `MissionTracker.CompleteMission` | `MissionSystem::turnInMission` (Complete from ReadyToTurnIn). **Not modelled:** `NextMissionInChain`, untracking, the unlock queue, the fast-forward prompt | MissionTracker.CompleteMission |
| `MissionTracker.PlayTurnIn` | `Default` id 14 | Kickoff after acceptance |
| `MissionTracker.GetMissionStatus` | the status number (Active 1, ReadyToTurnIn 3, Complete 4); another mission: NotStarted | Availability queries |
| `MissionTracker.IsDataValid` | true (reading of the name; no native note) | none |
| `WillowPlayerController.NativeGetMissionIndex` | index of the record whose `MissionDef` is the mission in the controller's playthrough-0 list, else -1 (inferred from how the script uses it; no native note) | none |
| `WillowPlayerController.ExpEarn` | **records** `(amount, source, type)` and does nothing else | WillowPlayerController.ExpEarn |
| `MissionDefinition.GetExperienceReward` | **HOST STAND-IN (replaced in swap 2 below):** returned the amount the host set (`FireMissionSlice::setExperienceReward`); the formula (NATIVE_PROGRESSION section 2) is not in `src/` yet | MissionDefinition.GetExperienceReward |

XP is unchanged for the host: `OpenWillowQuest` still grants it from the `Reward` effect with its own amount. At turn-in the host
supplies that same amount to the stand-in native, so the script reaches `ExpEarn`; the host logs `script ExpEarn calls=1
amount=395 source=4 (host amount 395: same)`. The amounts agree by construction, so what this proves is the **path**: one
`ExpEarn` call, source 4 (SideMissionAward, because the Fire mission is not plot-critical), type omitted, after the status
callback. The XP swap (formula in `src/`, `ExpEarn` raising the pool) is the next step.

### Stubs the script hits (logged, never fail the quest; `FireMissionSlice::scriptStubs()`)

All return the zero value of their result. The host logs the list after accept and after turn-in
(`OWQUEST script stubs after ...`); `--slice-run` prints it in its `script.stubs` field. From the real-data run (`accept` to
`turnin`, 19 entries):

- status callbacks: `WillowPlayerController.GetCurrentPlaythrough` (0 is the right answer for one playthrough),
  `GetHUDMovie`, `Actor.SetTimer` (the contextual prompt's retry, because there is no HUD movie), `UpdateLcdMissionStatus`,
  `PlayerController.IsPrimaryPlayer`, `PlayUIAkEvent`, `WorldInfo.IsMenuLevel`,
  `WillowLeviathanService.RecordMissionStatusChangedEventForPlayer` (telemetry), `MissionDefinition.GetGameStage`,
  `MissionTracker.GetActivePrimaryObjectiveSet`, `GetObjectivesProgress`, `GetAllMissions`, `WillowGlobals.GetWillowGlobals`
  (a static call through a class constant, which step 0 made reach its native; not checked without step 0);
- reward path: `MissionDefinition.GetCurrencyRewardType`, `GetCurrencyReward`, `ShouldGrantAlternateReward`,
  `GetItemRewardsForPlayer` (their zero results happen to equal the Fire mission's data: no credits, normal reward, no items),
  `RefreshBalanceDataFromMissionCompletion`, `UnlockAchievementIfConditionsMet`.

Other VM notes (`script.notes`): reads of the reward struct's static arrays (`WeaponRewards[0..1]`, `ItemRewards[0..1]`) report
"accessed array out of bounds": a struct's zero value does not materialise its static-array fields. Reproduced with
`--run WillowGame.WillowPlayerController.GetNumRewardChoices` and no argument. It does not change the Fire result (no
items, 0 choices) but it is a VM gap for missions with item rewards.

### Not modelled yet (differences from the note that this step leaves)

`ClientReceiveMissionStatus` (remote players), the mission weapon at Active/Complete (the slice still lends it with the objective
set), timed and defend missions, `RequiredObjectivesComplete` and `Failed`, the `SetActiveMission` early-exit gate and tracked-mission
choice, the "Loader" level wait, the kickoff's dialog request, and the note's order **initial objective set before the `Default`
event** for `bActivateInitialObjectiveSet` missions (the Fire mission has it false; `MissionSystem::accept` keeps the old order for
the toy data). The controller's mission record is rebuilt from the restored status after `loadState` (a restored mission has
played its kickoff: an assumption).

### Checks (2026-10-05, CMake Release and UE module rebuilt first)

- `ctest`: **11/11** (new: `mission-script-synthetic`, invented `Engine`/`WillowGame`/mission packages with hand-assembled
  controller/tracker script: status hooks in order, kickoff consumed by the tick, refused second accept, reward call with the
  host amount, no stubs); `vm-synthetic` gained the class-constant case (step 0). `tools/verify_packages.py`: 9/9 match.
- Real-data CLI (`--slice-run ... accept tick:5 range spawn tick:6 damage:... turnin`): the 22 host events are identical to the run
  before the swap; the three kickoff events (dialog, set, remote event) moved from the `accept` step to the next `tick`.
  With `xp:395` before `turnin`: `exp_earned` = one call (395, source 4, type omitted), controller record status 4,
  `bNeedsRewards` false.
- **`tools/test_quest.ps1`: first run PASS checks=80 errors=0, resume PASS checks=11 errors=0** (before: 79 and 11). The one new
  check is `turn_in_script_calls_exp_earn_side_mission_award`; no existing check changed, including
  `installed_kismet_starts_marcus_walk` right after the use key (the kickoff now plays on the next tick of the same session).

## Script swap 2: experience (2026-10-05)

AI-assisted (Claude), lane I1. Second stand-in swap. The host no longer computes the mission XP: the amount, the pool and the level-up
come from the script path and the natives under it, written from [NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) (sections 1-3) and
[NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md). **Every rule stays UNVERIFIED in the running game except the one
amount confirmed on 2026-10-02: 395 at stage 8.**

### What runs now

1. `UpdateMissionStatus(Complete)` (script) reaches `ServerGrantMissionRewards` (script), which calls, in this order and all from
   `src/mission_script.cpp`: `GetCurrencyRewardType`, `GetCurrencyReward`, `GetExperienceReward`, `ExpEarn(amount, 4)`,
   `GetItemRewardsForPlayer`. Source 4 is SideMissionAward (the Fire mission is not plot-critical).
2. **`MissionDefinition.GetExperienceReward`** (native, ours): `trunc(float(span x percentage x m))`, `span = R(L+1) - R(L)` over the
   mission's game stage `L`, `m = 1` (first playthrough, player below level 50; other cases throw "not implemented" rather than
   guess). The percentage is read from the installed data: `Reward.ExperienceRewardPercentage` is an attribute whose value chain is a
   constant (0.05) times a conditional on `PlayThroughCount` (1 here), evaluated by `src/progression.*` (below). The curve is read
   from `GD_Balance_Experience.Formulas.Init_ExperienceRequiredForLevel` (`60 x (n ^ 2.8 + 7.33)`, the level through a global-slot
   attribute), `R(n) = max(0, trunc f(n) - trunc f(1))`, single precision.
3. **`MissionDefinition.GetGameStage`** (native, ours): the stage locked into the mission when it became Active (the status routine
   does that before the script hook), else the region's current stage. The region stage is the host's input
   (`FireMissionSlice::setRegionGameStage`; the host keeps fixing it per player and playthrough and saving it).
4. **`WillowPlayerController.ExpEarn`** (native, ours): `Exp x scale` added to a VM-side pool (`CurrentValue`, a float), clamped to
   `[0, R(max level)]`, only when it raises the pool; it does not level up. The scales (`ExpCombatPointsScale`,
   `ExpMissionPointsScale`, `ExpAllPointsScale`) are attributes whose base values the note did not decode: **taken as 1**. The call is
   still recorded (`expEarned()`).
5. **The pool update** (`ExperienceResourcePool.ApplyExpPointsToExpLevel(false)`, implemented as `MissionScript::updateExperiencePool`
   and run from `FireMissionSlice::tick`, standing in for the pool's per-frame update): while the pool has reached
   `ExpPointsNextLevelAt` (> 0) and the level is below `GetMaxExpLevel` (native, ours: 50), the installed script
   `WillowPlayerController.ExpLevelUp(false)` runs on the VM controller. It raises `PlayerReplicationInfo.ExpLevel`, then
   `OnExpLevelChange` (script) sets `ExpPointsNextLevelAt` through `GetExpPointsRequiredForLevel` (native, ours, the same curve).
   Skill points, health and the HUD stay stubbed in the script (the globals' skill-point formula is not evaluated; swap 7 below evaluates it
   and the new maximum health); `LevelUpCount` is not kept.
6. **Where the state lives:** the VM holds the pool value and the level (the controller's `PlayerReplicationInfo`), the host keeps
   Maya's `UOpenWillowSkills` (level and experience display, skill points, health, save). Simplest design that keeps the host's UI and
   save untouched: before accept and turn-in the host copies its inputs in (`setRegionGameStage`, `setPlayerExperience(level,
   experience)`), and the slice hands back two events, `Experience` (what ExpEarn added) and `Level` (the level the pool update
   reached). The host applies `Experience` with its own `AddExperience` and compares the `Level` event with its own level
   (`OWQUEST script level-up: VM level 9, host level 9 (same)`; a quest-suite check). `GrantExperience` and its use of the
   manifest's `MissionXp` are gone from the grant path; `MissionXp` and the manifest's `candidate_amount_by_mission_level` remain only
   as the suite's oracle.

### `src/progression.*`: a subset of the attribute evaluator (NATIVE_PROGRESSION section 1)

`AttributeEvaluator` evaluates an `AttributeInitializationData`: the constant, an attribute whose context chain is the no-context
resolver and whose value chain is constant / simple-math (Add, Sub, Mul, Div) / global-slot resolvers, a definition's `ValueFormula`
(offset added before the multiplier, power skipped at 1) or `ConditionalInitialization` (expressions on `PlayThroughCount`, all must
hold: UNVERIFIED), the base-value modes (numbers 1..3 as the note lists them, which name is which not checked), scale, range
restriction and rounding, in single precision. Random variance, other resolvers, other context resolvers and other condition
attributes throw `unsupported ...`: a mission that needs them stops with a script error instead of a guessed value.

### Other reward natives implemented from the bridge note

- `GetCurrencyRewardType`: `Reward.CurrencyRewardType` (or the alternative's).
- `GetCurrencyReward`: other currencies from `OtherCurrencyReward`; credits only when the multiplier is exactly 0 (the Fire mission;
  the note's `MissionCreditRewardFormula` attribute values were not decoded, so any other multiplier logs a "not implemented" entry in the
  stub list and returns 0). Optional-objective currency is likewise a "not implemented" entry.
- `ShouldGrantAlternateReward`: the last objective set of the `NextSet` chain, its objectives' indices in `ObjectiveDefs`, the passed
  progress; **exercised only with the empty progress that the stubbed `MissionTracker.GetObjectivesProgress` returns**, so it answers false;
  for a bit-mask objective the required progress is its full mask (UNVERIFIED).
- `GetItemRewardsForPlayer`: the empty case (the Fire mission). Non-empty `RewardItems` or `RewardItemPools` log a "not implemented"
  entry: item generation and pool rolls belong to the loot lane (`NATIVE_LOOT.md`).
- `MissionDefinition.GetGameStage` as above.

### VM fix: static-array fields of structs

A struct's default tags set a static array (`ArrayDim > 1`) one element per tag, selected by the tag's array index. The VM overwrote the
whole array with the last element, so `PendingMissionRewardData.WeaponRewards/ItemRewards` read "out of bounds" and, more
visibly, `ProviderDefinitionPathName.PathComponentNames` was a scalar holding the last component. Both now are two-element / name arrays.
`providerPathLeaf` (`src/behavior.*`) takes the provider's own name from that array for the two callers that used the scalar. The
dummy provider's sequence changes still route (22 host events identical to before). Synthetic check: `vm_test.py` `MakePair`.

### Stubs still hit on the Fire turn-in (real data, 14 up to the turn-in, 18 with a level-up)

`Actor.SetTimer`, `MissionTracker.GetActivePrimaryObjectiveSet` / `GetAllMissions` / `GetObjectivesProgress`,
`PlayerController.IsPrimaryPlayer`, `WillowGlobals.GetWillowGlobals`, `WillowLeviathanService.RecordMissionStatusChangedEventForPlayer`,
`WillowPlayerController.GetCurrentPlaythrough` / `GetHUDMovie` / `PlayUIAkEvent` / `RefreshBalanceDataFromMissionCompletion` /
`UnlockAchievementIfConditionsMet` / `UpdateLcdMissionStatus`, `WorldInfo.IsMenuLevel`; the level-up adds
`AttributeInitializationDefinition.EvaluateInitializationData` (the skill-point formula), `WillowLeviathanService.RecordPlayerCharacterGainedLevelEventForPlayer`,
`RecordPointsEarnedEventForPlayer` and `WillowPlayerController.RecalculateAttributeInitializedState`. Removed from the list by this swap:
`MissionDefinition.GetGameStage`, `GetCurrencyReward`, `GetCurrencyRewardType`, `ShouldGrantAlternateReward`, `GetItemRewardsForPlayer`.

### Not modelled / uncertain

Playthrough 2 and later and level 50+ (the playthrough multiplier m), optional objectives' own XP, the experience scales (1),
DLC level caps, `LevelUpCount`, HUD updates and telemetry in `ExpEarn`, the order in which the real pool updates relative to other
per-frame work (here: once per `tick`, after the mission tick), and everything the note calls open. The region stage is still the
host's table (`values.xp.region_stage`), not evaluated from `RegionBalanceData` in `src/`.

### Checks (2026-10-05, CMake Release and UE module rebuilt first)

- **Real-data CLI** (`--slice-run ... stage:8 player:8:20208 accept ... turnin tick:1`): one `Experience` event of **395**, `exp_earned`
  `(395, 4, -1)`, pool 20,603, no level change; with `player:8:27900` the pool reaches 28,295, the script `ExpLevelUp` runs and the
  `Level` event says 9. At `stage:7` the amount is 316 (the table in NATIVE_PROGRESSION).
- Synthetic `mission_script_test.py` (invented curve `2 x (n^2 + 1)`, percentage `0.5 x (conditional 3)`): reward `trunc(18 x 1.5) = 27`
  at stage 4, the later `stage:9` does not change it (locked at accept), pool 16 + 27 = 43 gives level 3 -> 4 by the script `ExpLevelUp`
  and stops, the pool clamps at R(50) = 4998 and the level at 50, no region stage gives 0.
- `ctest` **11/11**, `tools/verify_packages.py` 9/9, UE module `Result: Succeeded`.
- **`tools/test_quest.ps1`: first run PASS checks=81 errors=0 (80 before), resume PASS checks=11 errors=0.** New check
  `script_pool_update_levels_up_to_host_level`; all existing XP, level, skill-point and health checks unchanged and green.

## Script swap 3: the dummy's enable conditions and the controller helpers (2026-10-05)

AI-assisted (Claude), lane I1. Rules from [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) (sections A-C) and
[NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md), **all UNVERIFIED in the running game**.

### 1. The dummy's sequence enable conditions (`src/slice.*`, `src/mission.*`, `src/behavior.*`)

Replaced the old rule (a re-sync of every condition after each host call) with the note's:

- **When:** `MissionSystem` raises the tracker's observer notifications after it changed its own state: the status changed (after the
  script hook, before the `Default` event), the active set switched, an objective's progress written, an objective completed
  (before the set evaluation). `FireMissionSlice` applies **every** condition on **each** one (the notification's arguments are
  ignored); `setSequenceEnabled` changes nothing, and fires no event, for a sequence already in that state, so events come only from real
  transitions. `ObjectiveCleared` is not modelled; a loaded mission (`loadState`) is announced once (the LevelLoad kind).
- **The verdict:** not objective-specific: the mission status bit of `MissionStatesToLinkTo`. Objective-specific: no `LinkedObjective`
  gives false, else the objective is classified by `MissionSystem::objectiveState` (Complete when the mission is Active, ReadyToTurnIn
  or Complete and the progress equals the count; else Active when the mission is Active, the objective is in the active set and its
  progress is below the count; else NotStarted) and the verdict is that bit of `ObjectiveStatesToLinkTo` (the mission-state bits are
  not consulted). A non-empty `ObjectiveSetRestrictions` keeps a true verdict only while one listed set is the active set. Another
  mission counts as Complete when in the completed set, else NotStarted; its objectives NotStarted.
- **Registration** (`spawnDummy`): the provider is a consumer only from the spawn. `BehaviorProvider::registerConsumer` is pass 1 (every
  sequence disabled, then each `bEnabledOnSpawn` sequence enabled in order, its `OnBehaviorSequenceEnabled` firing); the conditions
  are then applied (pass 2, the immediate LevelLoad verdict) and only then `OnSpawned` fires. Before the spawn no condition observes
  anything (the old rule changed the unspawned provider's sequences from the accept on).
- **Kernel enable/disable:** `bSequenceEnabledMutex` (enabling a mutex sequence first disables one other enabled mutex sequence of the
  provider) and the event order (enabled event after the bit is set, disabled event before it is cleared). The dummy's data has the
  flag false on all nine sequences (read with `--object-dump`), so the mutex is exercised only by the synthetic test.
- **Synthetic test** (`tests/mission_script_test.py`, scenario D, `--slice-run` on invented `Startup` and `Sanctuary_Dynamic` packages
  with the stock provider path): no sequence event before the spawn; registration order `Idle`, then the conditions in sequence order;
  the progress notification already turns the objective's sequences off; set restrictions (one listed set active, one not);
  ReadyToTurnIn enables the second mutex sequence after disabling the first; Complete turns the mission-level condition on.
- **Real data** (`--slice-run ... accept tick:5 range spawn tick:6 damage:Shock damage:Incendiary tick:5 turnin`): the 22 host events are
  **identical** to the previous rule's, in the same order, and the enabled sequences at the end are the same (`Default`, `Idle`,
  `Targetable`). Differences, all from the note: (1) pass 1 now fires `Idle`'s `OnBehaviorSequenceEnabled`, which reaches
  `GearboxFramework.Behavior_SpecialMove` (an animation request; no animation system, so it is listed at the boundary and logged by the host
  as "not run by host"; it was never reached before and without the boundary entry it would have been an unsupported-class error);
  (2) the verdicts are applied at the notification points instead of after the whole host call (no host-visible effect here);
  (3) an objective-specific condition no longer also requires its mission-state bit (class default {Active}; equal for the Fire
  conditions); (4) the objective classification gates Complete on the mission status.
- Not modelled: per-instance objectives (`bRememberItemsWithinObjective` consumers), `bInstanced`, the waypoint hooks, the multi-condition class,
  the Kismet twin, `RequiredObjectivesComplete` and `Failed` statuses (MissionSystem has neither).

### 2. Controller helper natives (`src/mission_script.cpp`)

Implemented only those the slice's script reaches, per NATIVE_CONTROLLER_HELPERS.md:

| Native | VM behaviour |
|---|---|
| `MissionTracker.IsDataValid` / `ValidateData` | the tracker's `bDataValidated` flag; `ValidateData` is its only writer. **Trigger (a choice, UNVERIFIED):** the game sets it when the reply to a mission data request reaches `ClientValidateMissionData`; how a standalone run triggers that request was not read, so the installed script `ClientValidateMissionData` runs once on the controller when the VM graph is built (its effect is the game's: `ValidateData`, then a per-controller loop over an engine iterator, which the VM does not run (listed as a stub), and the script's two refresh calls) |
| `WillowPlayerController.GetCurrentPlaythrough` | `CurrentPlaythrough` of the VM replication info (0), also used by `NativeGetMissionIndex` (now range-checked against `MissionPlaythroughs`, first match) |
| `PlayerController.IsPrimaryPlayer` | true for the sole local controller |
| `WillowPlayerController.GetHUDMovie` | None (no HUD in the graph; the script then skips its HUD branches) |
| `WorldInfo.IsMenuLevel` | the VM world's `bIsMenuLevel`, false |
| `WillowGlobals.GetWillowGlobals`, `GearboxGlobals.GetGearboxGlobals` / `GetBehaviorKernel` | one VM `WillowGlobals` instance (the kernel field: None) |
| `WillowGlobals.GetGlobalsDefinition` | the installed `GD_Globals.General.Globals` object (named in the note's prose; no section of its own) |
| `UpdateLcdMissionStatus`, `PlayUIAkEvent` | documented no-ops (presentation only) |

**Turn-in stub count (real data, `accept` to turn-in): 14 before, 12 after.** Removed: `GetCurrentPlaythrough`, `IsPrimaryPlayer`,
`GetHUDMovie`, `PlayUIAkEvent`, `UpdateLcdMissionStatus`, `GetWillowGlobals`, `IsMenuLevel` (7). Newly reached because the script now gets real
answers (a real globals definition, a menu-level answer, the validation run): `Object.Localize` x3, `MissionTracker.AllExpansionSideMissionsComplete`,
`PlayerController.IsLocalPlayerController`, `function ?.SpawnPlayerMovie` (a call on a data-object stand-in without a class) and the engine
iterator (x2). Remaining from before: `Actor.SetTimer`, `MissionTracker.GetActivePrimaryObjectiveSet` / `GetAllMissions` / `GetObjectivesProgress`,
`WillowLeviathanService.RecordMissionStatusChangedEventForPlayer`, `RefreshBalanceDataFromMissionCompletion`, `UnlockAchievementIfConditionsMet`.

### Checks (2026-10-05, CMake Release and UE module rebuilt first)

- `ctest` 11/11 (`mission-script-synthetic` gained the helper probes and scenario D), `tools/verify_packages.py` 9/9.
- **`tools/test_quest.ps1`: first run PASS checks=81 errors=0, resume PASS checks=11 errors=0** (unchanged counts).
- **`tools/test_mover.ps1`: PASS checks=16 errors=0** (rerun because `src/behavior.*` changed).

## Script swap 4: Marcus's dialog (2026-10-05)

AI-assisted (Claude), lane I1. Rules from [NATIVE_DIALOG.md](NATIVE_DIALOG.md) and
[NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) (section G2, latent waits), **all UNVERIFIED in the running game**.
The stand-in in `MissionSystem` (every `Behavior_TriggerDialogEvent` selected Out and Finished at once and emitted a "dialog" effect
for the behavior's own tag) is gone. New `src/dialog.hpp/.cpp` (`DialogSystem`, owned by `MissionSystem`).

### The behavior and the kernel wait (`src/behavior.*`)
- A handler may now make its behavior **latent**: `BehaviorProvider::run().wait` (minimum 1/60 s) parks the thread with per-thread
  `state`; the behavior runs again later with `initialRun` false. The links selected in the latent call start new threads while this one waits.
  A waiting thread whose sequence was disabled ends without running (the note's per-behavior check inside a running thread is not applied).
- `tick()` now **advances time to each due thread** instead of jumping to the end of the call, so a long frame (the CLI's `tick:5`) passes
  through every wake and poll; `onTime` keeps the dialog components in step with it. (the behavior suite is unchanged; the host
  ticks at frame rate.)
- `Behavior_TriggerDialogEvent` (`DialogSystem::behavior`): first run: **Out (id 0) selected at once**, latent for 0.001 s (so one
  kernel wake, 1/60 s); the next run triggers the dialog; while the event's talk act is live it polls every **0.1 s**; **Finished (id 1)**
  on the first poll where it is no longer live (the same run when nothing started, or the pooled event data was reused);
  `bForcePlayImmediate` triggers in the first run and selects Finished, then Out; no `EventTag` selects Finished at once.

### The dialog (`DialogSystem`)
- **Event:** the group's **last enabled** `DialogEvents` entry for the tag. **Act:** its inline `OutputAction`, else the target of the group's
  link table for (the entry's 1-based id, link 0): a `TalkActs` template (ids after the events) or a node by `NodeID`. The Fire group's
  seven link-table events resolve to `TalkActs[0..6]` as the note's structural check says. Unsupported (throws "dialog: not implemented"):
  talker variables, chance/compare/switch/trigger/random-branch nodes, sound-effect events, output links on a talk act, talker-owned events (no group).
- **Talker:** an act with no audio in any TalkData passes through (nothing is chosen); `bInstigatorTalker` -> the instigator is the mission,
  never an actor, so **no talker** (reported as "no talker"); otherwise a random TalkData entry (seeded, deterministic) resolved by its **exact**
  name tag to a registered pawn (`registerTalker`), else, for an echo event, an **echo caller** (created when absent).
- **Audio device:** Talk does nothing without one. The host's line player is `DialogSystem::setLinePlayer` (told when a line starts,
  `lineEnded(id)` when it ends); `setTestLineLength(s)` is a test player (every line lasts `s`). **Default: no player = no device = no line
  starts**, so Finished follows on the wake that triggered the dialog (the note's "within one poll" is the same run here).
- **Priority arbitration** before a line starts: the tag's index in the globals' `Priorities` (`GD_Globals.Dialog.DialogGlobals`; lower = more
  important), floored for the tracked mission's `MissionDialogGroup` (`ActiveSideMissionMinPriority`, plot: `ActivePlotMissionMinPriority`,
  unless the base is already at least `ActiveMissionMinPriorityStart`); blocked when the group's current event (keyed by the root group) or the
  talker's own live line has an index `<=` the new one (`<` for an echo event without `bDoesNotOverrideSamePriority`). A started echo/group
  event silences the group (stops its live lines) and interrupts the talker's own line.
- **Line end** (the component update): audio end + `OutputDelay`, no audio -> the next update; the event data is then inactive.
- **What the host sees:** a `Dialog` effect/event per chosen line: a = event tag, b = group, c = the talker's name tag, detail =
  `act=...;ak=<AkEvent>;talker=echo|pawn;outcome=started|no audio device|blocked by priority|no audio event|no talker;line=<id>`.
  The host logs it, resolves the event in its audio manifest, **compares the VM's AkEvent with the manifest's** (new quest check
  `dialog_lines_name_the_manifest_ak_events`) and registers Marcus as a pawn talker (the most common talker name tag of the manifest: a host
  choice). CLI steps: `lines:<s>`, `talker:<name tag path>`; events print `detail`.

### Timeline on real data
- Host, no line player: the tracker tick plays the kickoff the frame after the accept, `Default` id 12 reaches `_1185` (Out), and one kernel wake
  (1/60 s) later the dialog is triggered ("no audio device") and **Finished** starts `GoToRange_ObjSet` and `RocksPaper_MoveMarcusToRange`:
  in the quest run 17 ms (3 frames) after `mission status -> Active`. Before this swap the set started inside the accept call.
- With a test line player (`lines:2.0`) the first objective set starts only **after the kickoff line ends** (the second `tick:1` step), as the
  note predicts; with the real 17.7 s kickoff audio that would be about 17.7 s plus up to 0.1 s.
- `--slice-run` (the same steps as before): the 16 non-dialog host events are identical and in the same order. Dialog lines: the six the stand-in
  listed (01, 02, 03b, 03a, 04, 05) are reported again: four "no audio device", and **03b and 03a as "no talker"** (their acts name the
  instigator as talker, and the mission context is no actor; the real game's context object for mission behaviors was not read, so whether those
  two lines play there is open).
- **Quest suite timing change, per the note:** `installed_kismet_starts_marcus_walk` and `marcus_walk_clip_playing` used to be checked in the frame
  after the use key; Marcus's walk now starts a few frames later (kickoff tick + kernel wake), so step 4 first records `use_key_accepts_mission`
  (unchanged, immediate) and then waits up to 2 s for the walk to start before the two checks, whose conditions are unchanged.

### Checks (2026-10-05, CMake Release and UE module rebuilt first)
- `ctest` 11/11 (`mission-script-synthetic` scenario E: Out first, dialog on the next wake, Finished at once without a player; with a 0.55 s test
  player Finished on the first poll after the end; blocked chatter; the last enabled entry; a template act through the link table; a pawn
  against the echo caller; `bForcePlayImmediate`), `tools/verify_packages.py` 9/9.
- **`tools/test_quest.ps1`: first run PASS checks=82 errors=0, resume PASS checks=11 errors=0** (81 before; the new check is the AkEvent comparison).
- **`tools/test_mover.ps1`: PASS checks=16 errors=0** (behavior and kernel code changed).

## Script swap 5: GoToRange through the stock waypoint script (2026-10-06)

AI-assisted (Claude), lane I1 then the orchestrator (both worked from [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) only; neither
opened analysis output). **All rules UNVERIFIED in the running game.** Replaces the stand-in where the host called `enterRange()` (a direct
`UpdateObjective(RockPaper_GoToRange)`) when the player capsule overlapped the cylinder while the objective was active.

- **What runs as script now:** the placed `WillowWaypoint_9` of `Sanctuary_Dynamic` is instantiated on the VM with authority and the world's
  replication info. Its own `PostBeginPlay` registers it with the tracker as a mission observer; its own `Touch` filters with `IsPlayerOwned`,
  asks `IsMissionObjectiveActive` and calls `UpdateObjective(LinkedObjective)`; its own "objective set changed" reaction re-checks every actor in
  its `Touching` list, so a player already standing in the range completes the objective when `GoToRange_ObjSet` becomes active.
- **New natives (src/mission_script.cpp):** `Actor.IsPlayerOwned` (owner chain to its root, the root's controller; true only for the player's
  controller), `MissionTracker.IsMissionObjectiveActive` / `IsMissionObjectiveComplete` / `IsObjectiveSetActive` (MissionSystem's own
  classification, the same predicate as its update gate), `UpdateObjective` (queued into MissionSystem), `RegisterMissionObserver` (VM
  observers get the `MissionReaction*` events; LevelLoad at registration). After each applied update the VM controller hears
  `UpdateMissionObjective(objective, bit)` and the tracker runs `TriggerMissionObjectivesChangedDelegates`, after the observers and before the
  objective's own events, as the note orders them.
- **VM:** interface casts (`EX_InterfaceCast`, `EX_DynamicCast` to an interface class) now succeed when the object's class defines every
  function the interface declares (`Runtime::implements`). The packages' implemented-interface tables are not decoded; this is a structural
  stand-in, UNVERIFIED, and an open item.
- **Host:** the host still owns the shape test (actor cylinder vs the waypoint cylinder, now for any actor via its simple collision cylinder) and
  reports begin/end of overlap for the player pawn and for Marcus (`touchWaypoint`); it no longer looks at the objective state. Marcus's touch
  reaches the script and is ignored there. `enterRange()` stays as a thin wrapper (a player touch).
- **Fire (second commit of the swap):** the dummy's `Behavior_UpdateMissionObjective` now runs its own `ApplyBehaviorToContext` on the VM
  (world role authority, `GetWorldInfo`, the cast of the context object to `IMissionObjective`, the world's tracker, `UpdateObjective`). New
  binding `BehaviorBase.GetWorldInfo` (the bridge's world, NATIVE_ENGINE_CORE.md); the VM world now has `Role` = authority. The dummy pawn has
  no VM object, so the context object is None: it casts to no interface exactly as a pawn does, and the bit is 0 (the note's reading). This
  path is covered by the quest suite and a real-data `--slice-run`; the toy package has no script behavior for it (open).
  `RequiredObjectivesComplete` is still not modelled (the note's gate also accepts it). New stubs reached at level start, all
  harmless for the slice: `Actor.AttachComponent`, `MissionTracker.RegisterWaypoint`, `Trigger.TriggerDetachSprites`.
- **CLI:** each `--slice-run` step record now carries the mission status after it; new steps `touch:` / `untouch:` `player` / `marcus`.

### Checks (2026-10-06, CMake Release and UE module rebuilt first)
- `ctest` 11/11 (`mission-script-synthetic` scenario F on a toy waypoint: Marcus ignored, one update for the player, a touch while not updatable
  does nothing, a player inside at set activation completes it without re-entry, the `range` wrapper; scenario A now also records the
  controller's `UpdateMissionObjective`), `tools/verify_packages.py` 9/9.
- **`tools/test_quest.ps1`: first run PASS checks=83 errors=0, resume PASS checks=11 errors=0** (82 before; new check
  `objective_completed_through_waypoint_script_once`). The log shows Marcus's touch delivered and ignored, then the player's touch.
- **`tools/test_mover.ps1`: PASS checks=16 errors=0.**
- After the Fire commit: CTest 11/11, packages 9/9, UE build Succeeded, quest PASS 83/83 and resume PASS 11/11, door PASS 16/16; a real-data
  `--slice-run` (accept, range, spawn, hit:other, hit:fire, turnin) reaches ReadyToTurnIn on the incendiary hit and Complete on turn-in with no errors.

## Script swap 6a: choosing the usable by the camera ray (2026-10-06)

Source: `NATIVE_USE_INTERACTION.md` ("Per-frame usable evaluation"). Every rule below is UNVERIFIED (read from native code, not confirmed in the
running game). Only step 6a is done; 6b and 6c (the use itself and the accept screen's logic as script) were sized and stopped, see below.

- **Rule implemented (host function):** the 250 cm sphere around the player is gone. `UOpenWillowQuest::InTalkReach()` now casts one ray from the
  player camera's position along the control rotation, `PlayerInteractionDistance` long, and Marcus is usable when the ray enters his hit volume and
  nothing else blocks the visibility channel in front of it. No cone and no radius, as the note says. The distance is not a host constant: the VM
  reads `GD_Globals.General.Globals.PlayerInteractionDistance` from the installed data (`MissionScript::playerInteractionDistance`, through
  `FireMissionSlice`; 350 uu on the real data, shown by `--slice-run` / `--mission-run` as `script.interaction_distance`). The globals loader the
  `WillowGlobals.GetGlobalsDefinition` native used is now one function (`MissionScript::globalsDefinition`).
- **Hit volume (host-chosen, UNVERIFIED):** Marcus's mesh has no collision, so `AOpenWillowNpc` gets a query-only capsule from the imported mesh's bounds
  (the combat targets' pattern), blocking only the visibility channel (it cannot block movement). The stock pawn's collision cylinder is not read.
- **View point:** the camera component's location and the pawn's current control rotation (the camera manager's cached view lags one frame after a
  teleport, which the automated run does).
- **Not modelled:** the 1/30 s evaluation throttle, the cinematic lock and other blocking flags, the trade proxy, icons and prompts, the Use key's own
  press path (`Walker->TryUse` still calls `TryUse`, which then accepts or turns in directly; that is 6b/6c).
- **Tests:** `mission-script-synthetic` scenario A reads an invented `PlayerInteractionDistance` (420) from a toy `GD_Globals.General.Globals`.
  Quest suite: three new checks, `use_ray_reaches_marcus_in_view`, `use_ray_misses_marcus_looking_away`, `use_ray_stops_at_interaction_distance`
  (at the data's distance plus 100); `use_key_out_of_reach_ignored`, `use_key_accepts_mission`, `use_key_turns_in_mission` and the resume
  "must not re-accept" check keep their names and pass.

### Why 6b and 6c were not started
The use itself as script needs, beyond the bridge's present objects, a VM Marcus `WillowAIPawn` with a mind, class and AI definition, the native
`AIClassDefinition.OnUsed` / `AIDefinition.OnUsed` (event with link filter on two providers) and Marcus's AI-definition provider running through the
kernel, `BehaviorBase.RunBehaviors`, the G4 cost natives, and the behaviors of his chain. One of those has no note: **`Behavior_IsSequenceEnabled`
(native, 227 instances, nine in Marcus's chain) has no description of what it checks or which output link means enabled**; the data shows only link
ids 0 and 1. Implementing it now would be guessing. The other pieces have notes. The orchestrator should decide whether to commission that note first.

### Checks (2026-10-06, CMake Release and UE module rebuilt first)
- `ctest` 11/11, `tools/verify_packages.py` 9/9, UE build Succeeded.
- **`tools/test_quest.ps1`: first run PASS checks=86 errors=0, resume PASS checks=11 errors=0** (83 before, three new use-ray checks).
- **`tools/test_mover.ps1`: PASS checks=16 errors=0.**

## Script swap 6c: the mission screen's logic as script (2026-10-06)

Sources: `NATIVE_USE_INTERACTION.md` ("Marcus's chain (data) and the lists (script)", the button section) and `NATIVE_MISSION_SCRIPT_BRIDGE.md`
("Availability queries", "IMissionDirector implementers"). Every rule is UNVERIFIED (read from native code, not confirmed in the running game).
The screen's movie is not hosted; 6b (the use itself, `OnUsed` through the kernel) waits for the `Behavior_IsSequenceEnabled` note.

- **A VM Marcus:** `MissionScript::placeMarcus` instantiates the stock archetype `GD_Marcus.Character.Pawn_Marcus` (a `WillowAIPawn`, so his
  `MissionDirectivesDefinition_1` table, seven entries, is the data's), with authority role and the VM world. The VM does not load references, so
  the table's reference is loaded in `placeMarcus`; the entries' missions stay references (imports into the mission's package). The same pawn is the
  waypoint's Marcus toucher (it replaces the bare `WillowAIPawn` of swap 5; identity only).
- **Bound natives (tracker):** `CanStartMission` (the `MissionSystem` predicate `canStart`, the one `accept()` now uses: NotStarted and dependencies
  met), `CanEndMission` (`canEnd`: ReadyToTurnIn), `GetCompletedBranch` (`completedBranch`, per the note: follow `NextSet` from the initial set to
  the last, 1 when all of its objectives are complete, 2 when not, 0 unless ReadyToTurnIn/Complete). A mission argument is matched by export path, so
  a directive entry (a reference) and the loaded definition are the same mission. Not modelled: Failed, repeatable Complete, the blocked-mission
  test, RequiredObjectivesComplete (`MissionSystem` has no such state). The tracker holds one mission record, so every other mission of Marcus's table
  (the other six entries) answers "no record": not startable, not endable, branch 0, status NotStarted (stand-in).
- **The lists are the stock script:** `WillowAIPawn.GetEligibleMissions`, `GetInProgressMissions`, `GetRedeemableMissions` run unchanged on the VM
  Marcus (`MissionScript::missionLists`; `FireMissionSlice::screen`; `--slice-run ... screen`, `--mission-run ... script:screen`). Real data:
  NotStarted -> eligible [Fire]; Active -> in progress [Fire]; the Complete / ReadyToTurnIn rows are covered by the toy scenario and by the
  quest suite (the real Fire mission cannot reach ReadyToTurnIn from the CLI without Kismet).
- **The confirm:** the host's use key asks the lists (`UOpenWillowQuest::MissionScreen`) and confirms the one offered entry (redeemable first, as
  `DetermineQuestEntries` lists them), a host stand-in for the button press: `slice.accept()` / `turnIn()` now check the lists, then run the stock
  `AcceptMission(Mission, Director = Marcus)` / `ServerCompleteMission(Mission, Marcus)` scripts. They stay as thin wrappers for the CLI and tests.
  A mission the lists do not offer is refused before the script (the button does not exist); the hint line and the key's "consumed" result use the
  same lists. With no Marcus in the data (the toy packages) the old direct path remains.
- **Director callbacks (script now):** `AcceptMission` calls `OnPlayerAcceptedMission(controller, mission)` and `ServerCompleteMission` calls
  `OnPlayerTurnedInMission` after `CompleteMission`, as stock script. Both are the stock `WillowAIPawn` functions; with nobody registered in
  `PawnsUsingMe` (the accept screen's `BeginUse` is part of opening the screen, 6b) they return at their first test, so they reach **no** further
  native here. When the screen is opened by the VM they will reach `MissionsAcceptedByPrimaryUser` / `PlayMissionTurnedInDialog` (not exercised).
- **Stubs:** no new ones: after accept 9 and after turn-in 15 in the quest log, the same lists as before the swap (CLI `--slice-run` the same).
- **Tests:** `mission-script-synthetic` scenario G (toy director with five invented entries: begins, begins+ends, other-mission end only, branch 1 and
  branch 2 endings; toy list functions in the stock structure over the bridge's natives): eligible only when NotStarted; in progress when Active
  (no dedup); redeemable by the completed branch when ready (branch-2 entry and the other mission not offered); nothing when Complete; an accept
  while Active is refused and the screen no longer offers it; the callbacks run (markers 120 / 121), the turn-in one before the reward. Quest suite:
  `mission_screen_offers_fire_from_marcus_directives` and `mission_screen_offers_turn_in_when_ready` added; all `use_key_*` names unchanged.

### Checks (2026-10-06, CMake Release and UE module rebuilt first)
- `ctest` 11/11, `tools/verify_packages.py` 9/9, UE build Succeeded.
- **`tools/test_quest.ps1`: first run PASS checks=88 errors=0, resume PASS checks=11 errors=0** (86 before, two new screen checks).
- **`tools/test_mover.ps1`: PASS checks=16 errors=0.**

## Script swap 6b: the use key runs Marcus's OnUsed chain (2026-10-06)

Source: `NATIVE_MARCUS_USE_CHAIN.md` (and `NATIVE_USE_INTERACTION.md`, `NATIVE_MISSION_DISPATCH.md` A2, `NATIVE_CONTROLLER_HELPERS.md`). Every rule is UNVERIFIED
(read from native code, not confirmed in the running game). The key path is now: key -> the host's use ray (6a) -> `FireMissionSlice::useMarcus` -> his
`OnUsed` chain on the VM -> the controller's `ClientGFxPlayMovie` (reported as a host event) -> the host confirms the offered entry (6c lists) -> the stock
`AcceptMission` / `ServerCompleteMission`. The host no longer decides on its own to open the screen.

- **The raise:** `MissionScript::useMarcus` raises `OnUsed` with link filter 2 (Generic) on Marcus's AI-definition provider
  (`GD_Marcus.Character.AIDef_Marcus.AIBehaviorProviderDefinition_0`, the existing `BehaviorProvider`, registered on his consumer at `placeMarcus`: PID 1 in his
  `ConsumerHandle`). The payload is the instigator (the player's VM pawn) and the used component (None). `BehaviorProvider::fireEvent` got an optional payload:
  an output-type variable link takes the element at its connection index into its object variable (a live VM object, new `Variable::live`), so the instigator
  lands in Brain's `PlayerWhoUsedMe`. The two later raises of `UseObject` (filters 0 and 1) reach no link and are not made. The VM Marcus `UseObject` script is not
  run (the bridge makes the raise itself).
- **The behaviors are script:** one handler runs each chain class's own `ApplyBehaviorToContext` on the VM (`Behavior_IsSequenceEnabled`,
  `Behavior_RemoteCustomEvent`, `Behavior_PlayAIMissionContextDialog`, `Behavior_HasMissions`, `Behavior_ShowMissionInterface`), once per context object
  (`BehaviorProvider::contexts`: a Context-type variable link resolves a named-variable reference to the first same-named non-reference variable, else the
  consumer's own pawn). `SelfObject` is Marcus. Behaviors now set `outer` to the provider definition (the resolver's Outer fallback), and a nested call into the
  provider (a remote event fired from script) keeps the calling behavior's `RunInfo`.
- **Natives (bound in `MissionScript::bindUse`, only when Marcus's provider exists):** `BehaviorKernel.IsBehaviorSequenceEnabled` (true only for the registered
  consumer + provider + an existing enabled sequence; None provider, unknown name, wrong handle give false), `BehaviorHelpers.ResolveBehaviorProviderDefinitionReference`
  (a non-empty path wins, joined with dots; only the bridge's own provider is found; then the reference, returned as is; then the source behavior's Outer; else None),
  `BehaviorKernel.ActivateBehaviorEventFromScript` (None provider fires nothing; an omitted filter is -1).
- **Needed beyond the three (each has a note except the last):** `BehaviorKernel.ActivateBehaviorOutputLink` (NATIVE_MISSION_DISPATCH A2: appends the id to the
  running behavior's list), `Object.QueryInterface` (NATIVE_CONTROLLER_HELPERS; a Core native, structural `Runtime::implements` stand-in; it worked for
  `IBehaviorConsumer` and `IMissionDirector` on Marcus), and `WillowPawn.GetBehaviorConsumerHandle` (a plain read of the pawn's own `ConsumerHandle` field, scoped to the VM Marcus; NATIVE_BEHAVIOR_CONTEXT.md later
  described it: the field must start at -1 and be assigned once at consumer registration, which swap 6d does). Also a small VM change: `Runtime::overrideScript` lets a host binding replace a script function; used only for
  the controller's `ClientGFxPlayMovie`, which is presentation and is reported as `interfaceOpened` (movie definition, director) instead of run.
- **Real data (`--slice-run ... use`):** the nine checks answer "not enabled" (outputs 1: three names have no sequence, six belong to other missions), the context
  dialog runs, `Behavior_HasMissions` selects 0 while the Fire mission is eligible, in progress or redeemable and 1 when the tracker has no such mission for
  Marcus (the Complete state: no link, nothing happens, the key is not consumed), `Behavior_ShowMissionInterface` opens the interface
  (`UI_Mission.MissionInterface_Definition`, director `GD_Marcus.Character.Pawn_Marcus`). In the quest suite the first press accepts, the last turns in.
- **Registration of his provider:** every sequence starts disabled and the `bEnabledOnSpawn` ones (`AI`, `Brain`) are enabled; the enable conditions of the other
  missions' sequences are not applied (those missions are not in the tracker, every verdict would be "not enabled"). The `OnBehaviorSequenceEnabled` events of his
  spawn reach four world operations that nothing here runs, listed at the boundary and in the script notes: `Behavior_AIHold`, `Behavior_SetPawnThrottleData`,
  `Behavior_ChangeUsability`, `Behavior_SetUsableIcon`.
- **Stubs newly reached:** one, `BehaviorBase.GetBehaviorContext(behaviorcontextdata,object,object,object,behaviorparameters)`, from
  `Behavior_PlayAIMissionContextDialog` (one call per press). It is the pure resolver described in NATIVE_BEHAVIOR_CONTEXT.md (not a kernel context lookup); as a stub it returned None, so the dialog behavior's script ended
  without calling `WillowAIPawn.PlayOnUseDialog` (the dialog on use was silent). The stub also needed the runner's struct fill (swap 6d).
- **Tests:** `mission-script-synthetic` scenario H (packages `UseChainOn` / `UseChainOff`): toy Marcus with a provider at the stock path; checks for a sequence
  that does not exist, one that is disabled, one that is enabled (fires its remote event; the cascade stops, the interface is not reached), a check whose
  path names no provider (selects nothing), the path winning over a `SequenceProvider` reference, and the payload variable reaching the Context-linked
  behavior (the interface opens when no check is enabled; the script RPC is replaced). Quest suite: `use_chain_reaches_mission_interface` and
  `use_chain_all_nine_checks_not_enabled` added; every earlier check name unchanged. CLI: `--slice-run ... use`, `--mission-run ... script:use` print the cascade.
- **Not covered:** the reference and Outer branches of the resolver on the real data (all nine instances use a path); the remote events of the six other
  missions (not enabled here); the Kismet named variables (`MarcusVendingProxy`, `MarcusStoreUser`) are not resolved.

### Checks (2026-10-06, CMake Release and UE module rebuilt first)
- `ctest` 11/11, `tools/verify_packages.py` 9/9, UE build Succeeded.
- **`tools/test_quest.ps1`: first run PASS checks=90 errors=0, resume PASS checks=11 errors=0** (88 before, two new use-chain checks).
- **`tools/test_mover.ps1`: PASS checks=16 errors=0.**

## Script swap 6d: Marcus's on-use dialog runs as script (2026-10-06)

Source: `NATIVE_BEHAVIOR_CONTEXT.md` (lane G21). Every rule is UNVERIFIED (read from native code, not confirmed in the running game). The stub that
left the on-use dialog silent (`BehaviorBase.GetBehaviorContext`) is gone; `Behavior_PlayAIMissionContextDialog` and `WillowAIPawn.PlayOnUseDialog` now run
their own script on the VM and pick the tag from Marcus's own lists. Playing the line is an open item (below).

- **`GetBehaviorContext` (pure resolver, `MissionScript::bindUse`):** the struct's `BehaviorContext` selector picks the base object: 0 `SelfObject`, 1 the
  instigator, 2 the other participant, 4 the struct's own `ContextObject`; 3 (EventData) and anything else give None. A non-empty `InstancedDataContextName`
  would ask the object's instance data; that path is not on Marcus's route and is listed as not implemented. The static and interface siblings are not bound
  (not on the route).
- **The thread runner's struct fill (`BehaviorProvider::bindContextInputs`):** before a behavior runs, an Input link onto a struct property of type
  `BehaviorContextData` stores the first resolved object in `ContextObject` (None when nothing resolves) and sets the selector to 4; after the run
  `ContextObject` is None again (the selector stays 4). Kernel-run behaviors get `SelfObject` = the consumer's pawn and no instigator, participant or event
  data. A Context link that resolves to nothing skips the behavior for that pass (no run, no output); the provider then follows the default link only when the
  behavior supports one. (A behavior with no Context link runs once on the consumer's own pawn, as before.)
- **The consumer handle:** the VM Marcus's `ConsumerHandle.PID` starts at -1 (invalid) and is assigned once when his provider registers (the emulation of
  `InitializeBehaviorProviders`, in `placeMarcus`): 0, the first slot (UNVERIFIED: the allocator's base). `GetBehaviorConsumerHandle` stays a plain field read;
  the kernel natives treat -1 (and any other value) that is not his registered handle as "unregistered".
- **What the guard needs, from his stock data (nothing invented):** `MyWillowMind` is a bare `WillowMind` VM object whose `AIClass` is the archetype's AI class
  (`GD_Marcus.Character.CharClass_Marcus`, loaded, which names his `AIDef`); his `DialogComponent` is the archetype's component (loaded). The mind carries no
  native mind state (nothing else reads it on this route). `PawnsUsingMe` is empty, the controller is the VM player controller. Building the mind was small.
- **The tag:** `PlayOnUseDialog` counts the director lists (redeemable, then eligible, then in progress, then none: `DET_OnUse_MissionComplete`,
  `..._MissionsAvailable`, `..._AllMissionsInProgress`, `..._NoMissions`, fetched through `WillowDialogGlobalsDefinition.Get`, bound to return
  `GD_Globals.Dialog.DialogGlobals`) and calls the pawn's `GearboxDialogComponent.TriggerEvent(tag, player)`. That call is bound on Marcus's component and reported
  (tag path, speaker = the component's owner, other object = the player pawn's class): `MarcusUse::onUseTag / onUseSpeaker / onUseTarget`, a host event
  `OnUseDialog` (`on_use_dialog` in the CLI), printed by `--slice-run ... use`.
  Real data: NotStarted gives `VO_NPC_OnUse_MissionsAvailable`, Active `VO_NPC_OnUse_AllMissionsInProgress`; in the quest suite the turn-in press gives
  `VO_NPC_OnUse_MissionComplete` and the press after completion `VO_NPC_OnUse_NoMissions`.
- **Playback: OPEN ITEM.** How the host plays dialog today: `DialogSystem` (src/dialog.*) plays a line for `Behavior_TriggerDialogEvent` with a group, a tag, Talk
  acts and a registered talker (NATIVE_DIALOG.md), and the host's line player (SLICE_AUDIO_CHAIN.md) plays the chosen audio event. The on-use tags are
  different: they sit in the generic group `GD_Dialog_NPC.Groups.DialogGroup_NPC`, whose Talk acts have no Marcus entry, so the **no-match output** runs a Trigger act
  that fires the implementation tag (`DET_NPC_OnUse_*`) in Marcus's own group `DialogGroup_NPC_Marcus`. `DialogSystem` has no component-level `TriggerEvent` group search,
  no no-match output and no Trigger acts, so this two-step dispatch is **not implemented** and the tag is reported, not played (the host logs it with "not played").
  The `MissionComplete` tag may be silent even then (note: its generic event has no output action).
- **Stubs:** none newly reached; `BehaviorBase.GetBehaviorContext` is gone. Quest log: 9 after accept, 15 after turn-in (the pre-6b counts).
  Newly bound natives needed by the route: `WillowDialogGlobalsDefinition.Get` (listed in NATIVE_DIALOG.md only as "see package", the globals object is what the note
  names) and `GearboxDialogComponent.TriggerEvent` (reported, scoped).
- **Tests:** `mission-script-synthetic` scenario H gained a `Probes` sequence in the toy provider: probe behaviors for selectors 0, 1, 2, 3, 4 (with and without
  an object), an unknown selector, an Input link that overwrites both the selector and the object in the data (the player: not Self), an Input link that resolves to
  nothing (None, the data's object is gone), and a behavior whose Context link is empty (skipped, its default link reaches the next probe). Quest suite:
  `use_chain_picks_on_use_dialog_tag` (the tag of the first press: missions available) and `use_chain_on_use_tag_follows_mission_state` (the turn-in press: mission
  complete) added; every earlier name unchanged. (The first name says "plays" as the coordinator named it; what the host does is log the tag, not play it.)
- **Not covered:** the runner's `ContextObject` reset after the run (no observer: it is rewritten before the next run); more than one object resolving into a
  `BehaviorContextData` input (only the first is stored, as the note says); `StaticGetBehaviorContext`, `StaticGetAllBehaviorContexts`, `GetBehaviorContextInterface`.

### Checks (2026-10-06, CMake Release and UE module rebuilt first)
- `ctest` 11/11, `tools/verify_packages.py` 9/9 (with class bodies exact), UE build Succeeded.
- **`tools/test_quest.ps1`: first run PASS checks=92 errors=0, resume PASS checks=11 errors=0** (90 before, two new on-use checks).
- **`tools/test_mover.ps1`: PASS checks=16 errors=0.**

## Script swap 6e: the on-use line plays through the stock dialog data (2026-10-06)

Source: `NATIVE_DIALOG.md` ("Component TriggerEvent / GetMatchingEvent", the Trigger act, the Talk act and its no-match output, the line rules),
`NATIVE_BEHAVIOR_CONTEXT.md` (the data finding), `SLICE_AUDIO_CHAIN.md` (how a chosen line reaches the host). Every rule is UNVERIFIED (read from native
code, not confirmed in the running game). `GearboxDialogComponent.TriggerEvent` is no longer "reported, not played": it runs in the existing `DialogSystem`
(`src/dialog.*`), and the line it chooses reaches the host through the mission's own dialog hook.

- **What `DialogSystem` gained (`triggerOnComponent`, no second engine):**
  - the **group search** of `GetMatchingEvent`: the speaker's groups in order, the first with an enabled entry for the tag wins, a group without one appends its
    `ParentGroup` to the search;
  - the event's **instigator** is the speaker (a registered talker with its own groups), so a Talk act with `bInstigatorTalker` takes the instigator when it has an
    entry for its own name tag (exact tag; the `ParentTag` ancestors are not modelled);
  - the **no-match output**: no valid talker and `bEnableNoMatch` takes output 1, which here leads to a **Trigger act**; output links other than a no-match
    link to a Trigger act still throw "not implemented";
  - the **Trigger act**: its Instigator variable gives the talkers, those with a group holding an enabled entry for its `DialogEvent` can talk it, and the event
    is triggered on that talker's own groups with the **same event data**. The act's own output 0 (after the line) has no link in the stock data and is not
    followed. Any other dialog variable class throws "not implemented".
- **How Marcus reaches it (bridge):** the component's speaker is read from his body class (`GD_Marcus.Character.BodyClass_Marcus`) by `DialogSystem::pawnDialog`: the
  name tag is its `DialogName`, and the group list follows the rule of `NATIVE_DIALOG_GROUPS.md` (UNVERIFIED, read from native code; replaces the earlier labelled
  stand-in "body class `DialogGroups` + globals `NPCDialogGroups`"): the body class's `DialogGroups`; then, **only if its `bNPCDialog` is set**, the name tag's
  `DlcExpansion.NPCDialogGroups` (if any) and the dialog globals' `NPCDialogGroups`; then **always** the globals' `DefaultTemplateGroup`; no de-duplication; an empty
  list with no body class. For Marcus that is **127 entries**: his own group `DialogGroup_NPC_Marcus`, the 125 generic NPC groups (first `GD_Dialog_NPC.Groups.DialogGroup_NPC`;
  `GD_VOSQ_ThisJustIn` appears twice, as in the data), then `GD_Dialog_Templates.Groups.DialogGroup_TemplateDefault` (checked on the real data: `--slice-run ... use`
  prints `on_use_groups` count 127 with that first and last entry, and entries 105 and 119 are `ThisJustIn`). Without the generic group the tag `VO_NPC_OnUse_*` is in no group of his and the
  line would stay silent. Marcus is registered as the talker when the component is triggered (so the mission's own lines, which the host registers him for, are unaffected).
- **What changed against the 6e code of the same day (following `NATIVE_DIALOG_GROUPS.md`):** (1) the list now ends with the default template group (and takes the
  name tag's expansion groups before the globals' ones); (2) `matchingGroup` appends a parent group **once, unique by identity** (it was appended unconditionally, with a
  32-entry cap on the search that a 127-group list would have hit); (3) template groups are skipped unless the caller allows them: `triggerOnComponent` (a fresh
  trigger, no event data reused) allows them, the Trigger act that fires on the same event data does not; a skipped template group adds no parent; (4) an import that
  no installed package file resolves (the body class's name tag, `GD_Dialog_NPC...`) is looked up by path among the dialog package's exports. No on-use line changes.
- **The four states (stock data, read with `ow-package --object-dump`):** `VO_NPC_OnUse_MissionsAvailable`, `..._AllMissionsInProgress` and `..._NoMissions` sit in the
  generic group `GD_Dialog_NPC.Groups.DialogGroup_NPC` with Talk acts (`Talk_24`, `Talk_33`, `Talk_4`) that have **no Marcus entry** and `bEnableNoMatch`; output 1 goes to
  Trigger acts (`Trigger_8`, `_7`, `_9`) whose `DialogEvent` is Marcus's `DET_NPC_OnUse_*` tag, whose talker variable is the event's Instigator; his group answers with
  Talk acts `115`, `113`, `116` and his own AkEvents `Ak_Play_VOCT_Marcus_Quest_New`, `..._Quest_During`, `..._Quest_No_New`. So NotStarted plays Quest_New, Active plays
  Quest_During, and the press after completion plays Quest_No_New (the `NoMissions` act is symmetric: confirmed, closing that open item of the note).
  **`VO_NPC_OnUse_MissionComplete` is silent** (finding, open question 4 closed): the generic event has no inline act; the link table points to the group's template talk
  act, whose only entry is Marcus's name tag with **no AkEvent**, so the act passes through (output 0, no link) and no line is chosen. Nothing was invented for it.
- **Reaching the host:** a chosen line is a dialog effect of the mission's `DialogSystem`, as for the mission's other lines: a host dialog event whose tag is the
  `DET_NPC_OnUse_*` tag, group `DialogGroup_NPC_Marcus`, talker `DialogName_Marcus` (a pawn). The host's hook (`K::Dialog`) looks the tag up in the audio manifest
  (`dialog:` entries, and now `marcus_group:` entries for Marcus's own group) and checks the AkEvent against the manifest's. **No audio is played: the host has no audio
  device** (its `played=0` check stays); "plays" means the line was chosen by the stock data and found in the manifest, exactly as for the mission's lines.
- **Real data (`--slice-run`):** NotStarted gives the Quest_New line (`outcome=started` with `lines:2`, `no audio device` without), Active gives the Quest_During line
  (blocked by priority if Marcus's previous line is still live: the stock arbitration, tick past it). In the quest suite: the turn-in press (MissionComplete) chooses no line;
  the press after completion chooses Quest_No_New.
- **Stubs:** none newly reached (quest log 9 after accept, 15 after turn-in).
- **Not modelled / open:** the `ParentTag` ancestor test of the talker validity; the manager `bEnabled` gate (assumed enabled); the priority gate is the existing
  one (the DET tags are `DialogPriority_20`); `WillowPawn.SetDialogNameTag` (the name tag override; the tag is `BodyClass.DialogName`); the manager's registered group list and `bEnabled` (the pawn's list is stored on the talker); sound-effect tags; Act_Chance / MissionSwitch / ObjectParameterSwitch in the
  generic group (not on the four on-use routes); a no-match output into anything but a Trigger act. A failed `no talker` line still consumes a line id.
- **Tests:** `mission-script-synthetic` scenario E2 (`--mission-run ... component:<speaker name tag>|<tag>|<groups>`; toy generic group, no-match output, Trigger act and a
  speaker group with a `ParentGroup`): the no-match chain plays the speaker's line (tag S, his AkEvent, a pawn talker); his own tag plays directly; a talker that cannot
  talk the event stays silent; an act without audio is silent; a tag in no group does nothing. Quest suite: `use_chain_plays_on_use_line`, `use_chain_plays_in_progress_line`,
  `use_chain_mission_complete_is_silent`, `use_chain_plays_no_missions_line` added; every earlier name unchanged.
  Scenario E3 (a pawn's dialog groups; `--mission-run ... pawn:<body class>|<event tag>`, invented body classes, groups, name tags and a template group): an NPC body gets
  [own, NPC group, template]; a non-NPC body [own, template] and no generic event; an NPC body whose name tag has an expansion [own, expansion group, NPC group, template]
  and the expansion group answers first, a non-NPC body with that name tag gets no expansion group; the template group is searched by a fresh trigger and skipped by the
  Trigger act on reused event data; a parent is searched after the template group and once although two groups name it.

### Checks (2026-10-06, CMake Release and UE module rebuilt first)
- `ctest` 11/11, `tools/verify_packages.py` 9/9 (class bodies exact), UE build Succeeded.
- **`tools/test_quest.ps1`: first run PASS checks=96 errors=0, resume PASS checks=11 errors=0** (92 before, four new on-use line checks).
- **`tools/test_mover.ps1`: PASS checks=16 errors=0.**

### Checks for the dialog-group rule (2026-10-06, CMake Release and UE module rebuilt first)
Rule: `NATIVE_DIALOG_GROUPS.md` (UNVERIFIED, not confirmed in the running game). Real data: Marcus's list is 127 entries (first `DialogGroup_NPC_Marcus`, last
`DialogGroup_TemplateDefault`); the on-use lines are unchanged (NotStarted Quest_New, Active Quest_During, after completion Quest_No_New, ReadyToTurnIn silent).
- `ctest` 11/11, `tools/verify_packages.py` 9/9 (class bodies exact), UE build Succeeded.
- **`tools/test_quest.ps1`: first run PASS checks=96 errors=0, resume PASS checks=11 errors=0** (unchanged); stubs unchanged (9 after accept, 15 after turn-in).
- **`tools/test_mover.ps1`: PASS checks=16 errors=0.**

## Script swap 7: the level-up's skill points and maximum health from the data (2026-10-06)

AI-assisted (Claude), lane I4. Rules from [NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) (sections 1, 3 and 4), [NATIVE_SKILLS.md](NATIVE_SKILLS.md) (section 1) and
[NATIVE_ATTRIBUTES.md](NATIVE_ATTRIBUTES.md) (sections 1, 5, 6, 8 and 11); the save rule of [NATIVE_SAVE_LOAD.md](NATIVE_SAVE_LOAD.md) is only the host's oracle
(`max(0, L - 4)` minus points spent). **Every rule stays UNVERIFIED in the running game** except what the notes say was confirmed on 2026-10-02: the points
`max(0, L - 4)` and the health pool's base maximum `80 x 1.13^L`.

### The traced path of one level-up (what is script, what is native, what is a stub)

`FireMissionSlice::tick` runs the pool update, which runs the installed script `WillowPlayerController.ExpLevelUp(false)` on the VM controller while the pool has
reached `ExpPointsNextLevelAt`. Traced with `research/script_disasm.py` and the VM's stub list (`--slice-run ... stage:8 player:8:27900 ... turnin tick:1`):

| Step of `ExpLevelUp` / `OnExpLevelChange` | Kind | Before swap 7 | Now |
|---|---|---|---|
| `GetWillowGlobals`, `GetGlobalsDefinition`, `GetMaxExpLevel` | native | implemented (swaps 2 and 3) | unchanged |
| `ExpLevel` + 1 | script | runs | unchanged |
| skill points: `GeneralSkillPoints += int(EvaluateInitializationData(GlobalsDefinition.GeneralSkillPointsPerLevelUp, Self))` | script + native | **stub** (returned 0: no points) | **implemented**: the `src/progression.*` evaluator on the installed `INI_SkillPointsPerLevelUp` (a conditional: 1 when `PlayerExperienceLevel >= 5`, default 0) |
| `SpecialistSkillPoints += int(EvaluateInitializationData(SpecialistSkillPointsPerLevelUp, Self))` | script + native | stub (0) | the same native; the data has no such definition, so an empty initialization (0) |
| `FireSkillPointsChangedDelegates` | script | runs (no delegates registered in the VM graph) | unchanged |
| first-skill-point stats, `BroadcastLocalizedMessage`, `RecordPointsEarnedEventForPlayer` | script / telemetry native | stat object absent; telemetry stub | unchanged: telemetry stays a stub (no rule to implement) |
| `OnExpLevelChange`: `ExpPointsNextLevelAt` from `GetExpPointsRequiredForLevel` | script + native | implemented (swap 2) | unchanged |
| `OnExpLevelChange`: `RecalculateAttributeInitializedState` | native thunk | **stub** | **host-boundary stand-in** (below) |
| `OnExpLevelChange`: the pawn branch (`SetGameStage`, intrinsic armour, the class's `OnLevelUp` behaviors) | script | skipped: the VM controller has no pawn | unchanged (so the health top-up of `PlayerBehavior_LevelUp` is not run) |
| `ClientOnExpLevelChange` (HUD queue, achievements, LCD, `RecordPlayerCharacterGainedLevelEventForPlayer`) | script | HUD absent; stubs | unchanged |

The health pool's maximum is **not** written anywhere by the level-up script. The notes do not say which native recomputes it: `RecalculateAttributeInitializedState`
is "a thin native thunk to a virtual method whose body was not reached" (NATIVE_ATTRIBUTES section 8) and `ResourcePoolManager.RecalculateBaseValues` is low
confidence. So that step is the one that needed a stand-in; it did not need the whole attribute and resource-pool system.

### What was implemented

- **`AttributeInitializationDefinition.EvaluateInitializationData`** (`src/mission_script.cpp`, static native, three arguments): evaluates the data with the existing
  `AttributeEvaluator`. The context source is the VM player controller, which resolves to its `PlayerReplicationInfo` (the player resolvers, NATIVE_ATTRIBUTES section 5).
  No source gives no player (a condition on the player's level is then false); any other source object, and a non-None override source, are listed as "not implemented"
  entries in the stub list.
- **`src/progression.*`** gained, inside the evaluator and for the two shapes the data needs: the context resolver `PlayerReplicationInfoAttributeContextResolver` (resolves
  when the context has a replication info, otherwise the attribute does not resolve), the value resolver `ObjectPropertyAttributeValueResolver` (get: the named property of the
  resolved context, `ExpLevel` for `PlayerExperienceLevel`), and the condition attribute `PlayerExperienceLevel` next to `PlayThroughCount`. A comparison whose operand does not
  resolve is false (NATIVE_ATTRIBUTES section 11). Any other resolver or condition attribute still throws "unsupported ..." rather than guess.
- **`WillowPlayerController.RecalculateAttributeInitializedState`: HOST-BOUNDARY STAND-IN.** It evaluates the health pool's base maximum for the new level
  (`GD_Siren.Character.CharClass_Siren.HealthPoolDefinition` -> `BaseMaxValue` -> `Init_PlayerHealth`, found by its stock path: Maya is the host's player class, the VM
  controller's own `PlayerClass` stays unset) and keeps it for the pool update. **What the real native recomputes, and that it is the one that does this, is not established;
  only the data and the formula (NATIVE_PROGRESSION section 4) are from the notes.** Missing class data is a "not implemented" stub entry, not an error.
- **The slice reports the result** (`MissionScript::updateExperiencePool`, after the loop): when the level changed, the events `Level` (as before), then `SkillPoints`
  (`GeneralSkillPoints` after minus before: the points the level-up awarded, sent even when 0) and `MaxHealth` (the pool's base maximum for the new level). Several levels in one
  update give one event each, with the summed points and the last level's health. `--slice-run` prints them as `skill_points` and `max_health` (`a` is the value).

### Host side (`OpenWillowQuest.cpp`)

The host's `UOpenWillowSkills` derives the unspent points from the level (`EarnedPointsAt`) and holds no counter, and its maximum health is `HealthForLevel` at the new
level, so there is nothing to apply for the two new events: they are **compared** with the host's own numbers, as the `Level` event is. Log lines:
`OWQUEST script level-up skill points: VM awarded 1, host 1 (same)` and `OWQUEST script level-up max health: VM 240.323 at level 9, host 240.323 (same)`. Two new suite
checks, `script_level_up_awards_skill_points` (one event, equal to the host's change of available points, a point at level 9) and `script_level_up_sets_max_health` (the VM's
number equals the walker's maximum health and `Data.HealthForLevel`); no existing check was renamed. The host's formulas (`EarnedPointsAt`, `HealthForLevel`) stay as the
oracle. The host's UI and save are untouched; the VM's numbers do not replace the host's displayed ones yet (open below).

### Real data and what it shows

`--slice-run ... stage:8 player:<level>:<experience> accept ... turnin tick:1` on the installed packages:

| Level-up | `skill_points` | `max_health` | Check |
|---|---|---|---|
| 8 -> 9 (pool 27,900 + 395) | 1 | 240.3233 | `80 x 1.13^9` |
| 4 -> 5 (pool 5,300 + 395) | 1 | 147.3948 | `80 x 1.13^5` (the note's 147.4) |
| 3 -> 4 -> 5 (pool 5,300; 4 happens in the first tick) | 0, then 1 | 130.4379, then 147.3948 | level 4 is below 5 |

Stubs removed from the real run: `AttributeInitializationDefinition.EvaluateInitializationData` (x2) and `WillowPlayerController.RecalculateAttributeInitializedState`. Still hit on
the level-up path, all with no rule to implement: `WillowLeviathanService.RecordPointsEarnedEventForPlayer`, `RecordPlayerCharacterGainedLevelEventForPlayer` (telemetry),
and the HUD side of `ClientOnExpLevelChange` (no HUD). No stub is newly reached.

### Not modelled / open

- The host still displays its own points and health; adopting the VM's numbers needs a points counter in the host (the save recomputes points from the level, NATIVE_SAVE_LOAD:
  `max(0, L - 4)` minus points spent, and must keep doing so) and a health setter, and is a UI/save change.
- Which native writes the health pool's base maximum in the game; whether a level-up refills current health (the host does; the class's `OnLevelUp` behavior is a pawn branch the VM skips).
- The VM's `GeneralSkillPoints` starts at 0 each run (the host owns the unspent count), so the script's "first skill point" stat branch (old 0, new above 0) is reached with no stats object
  and does nothing; the spent-points and respec paths are not run.
- Evaluation is single precision with `std::pow`; the game's `pow` may differ in the last bit (see NATIVE_PROGRESSION section 3 for the same question on the curve).
- Playthroughs above 1 and the skill-point definitions of DLC are not modelled (the evaluator throws on shapes it does not know).

### Checks (2026-10-06, CMake Release and UE module rebuilt first)

- Synthetic `tests/mission_script_test.py` scenario S7 (`--slice-run` on invented packages: a conditional `PlayerExperienceLevel >= 5` gives 1 point else 0, a health formula
  `10 x 1.5^level`; the toy `ExpLevelUp` mirrors the installed one's two extra steps): 4 -> 5 awards 1 point and 75.9375 health, 5 -> 6 awards 1 and 113.90625, 3 -> 4 awards 0 and
  50.625, 4 -> 6 in one update awards 2 once with the last level's health, a reward that does not cross a level sends neither event; no stubs and no notes. (The existing toy's
  `Expressions` array element lacked its `None` terminator, so its conditional list decoded as empty; fixed, the playthrough branch it models is still the default one.)
- `ctest` 11/11, `tools/verify_packages.py` 9/9 (class bodies exact), UE module `Result: Succeeded`.
- **`tools/test_quest.ps1`: first run PASS checks=98 errors=0 (96 before, two new: `script_level_up_awards_skill_points`, `script_level_up_sets_max_health`), resume PASS checks=11 errors=0.**
  The log lines: `OWQUEST script level-up skill points: VM awarded 1, host 1 (same)`, `OWQUEST script level-up max health: VM 240.323 at level 9, host 240.323 (same)`. Stubs unchanged
  at the points the suite logs them: 9 after accept, 15 after turn-in (the level-up runs in the next frame, after that line).
- **`tools/test_mover.ps1`: PASS checks=16 errors=0.**

## Turn-in loot stand-in removed (2026-10-07)

Maintainer decision, AI-assisted (Claude), lane I4: "that's not how missions are gonna look". The stock data gives the Fire mission no item reward (`RewardData` has only
`ExperienceRewardPercentage` and `CreditRewardMultiplier` 0; NATIVE_LOOT.md, `GetItemRewardPools` / `GetItemRewardsForPlayer`), so the turn-in is XP only.

- **Removed:** `tools/weapon_slice_gear.py --reward-only` and `--reward-seed-limit` (`build_reward`, the `slice_reward_roll` recipe, mesh and the manifest's `reward_roll` key; the
  rest of the tool, including the manifest's `loot` table and `build_mesh` used by `weapon_refresh_fragments.py`, is unchanged); in `OpenWillowQuest`: the `reward_roll` recipe lookup
  at start, `RewardItem` / `bHasReward` / `RewardPickup` / `DropReward`, the pickup spawned by the `Reward` effect; in `OpenWillowInventory`: the `reward_roll` provenance skip in
  `LoadRecipes` (only mission weapons are skipped now). The suite checks `turn_in_drops_loot_stand_in_pickup` and `use_key_collects_loot_pickup_into_backpack` are removed with it.
- **Kept:** `AOpenWillowInventoryPickup`, the walker's pickup path and the backpack code (the lent mission pistol, the inventory suite). The press of E at the end of turn-in now reaches
  Marcus (the "no missions" line, check `use_chain_plays_no_missions_line` unchanged).
- **New check `turn_in_gives_no_item_reward`:** after the turn-in the reward counter is 1, the script's stub list has no `GetItemRewardsForPlayer` entry (that native lists a "not
  implemented" entry for a non-empty item reward, so the script path found none), no `AOpenWillowInventoryPickup` exists in the world and the backpack count is unchanged.
- **Local files:** `local/items/slice/slice_reward_roll.*` and the manifest key were deleted on this machine (ignored, regenerable otherwise); a copy elsewhere is harmless but
  `host/ue5/import_slice_items.py` would still import a stale `slice_reward_roll.json` as an asset, so delete it there too.
