# Slice world placement and values (Fire mission, Sanctuary), 2026-10-01

AI-assisted (Claude). Extraction and structural checks only: **no original-game capture, no host run**.
Every behavioural statement about native code is `UNVERIFIED`. Numbers below are decoded from the
installed packages by the project's reader; they are quoted here as evidence for this record and are
regenerated, never committed, by the command at the end. Clean room: no GPL tool code, no executable
disassembly; script functions named below were read with `research/script_disasm.py` (package bytecode).

Replaces these host stand-ins listed in `SANCTUARY_RPG_MISSION.md`: 700 cm range radius, Marcus as a
method call with no placement/walk, the dummy at a host position, the unbound target mover, respawn at
session start, Maya's 400 health, the uncounted XP amount, the undocumented dependency fixture.

## Coordinates

`ue3` = serialized Unreal units and rotator units. `host` = the conversion `tools/prepare_level.py`
(`transform`) already uses for the prepared scene and the door mover: location unchanged (1 uu = 1 UE5
unit as placed today), rotation degrees = units x 360 / 65536 as `[Pitch, Yaw, Roll]`.

## 1a. Range trigger (objective `RockPaper_GoToRange`) — recovered

- **Object:** `Sanctuary_Dynamic:TheWorld.PersistentLevel.WillowWaypoint_9` (WillowGame.WillowWaypoint),
  `WaypointInfo.LinkedObjective = GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire.RockPaper_GoToRange`,
  `bUpdateObjectiveOnPlayerTouch = true`, no `ObjectiveSetRestrictions`, no `TouchVolumes`.
- **Shape:** `CylinderComponent_3166`, radius **357.81**, CollisionHeight (half height) **145.31**, centre
  **(9595.84, 9384.23, 3404.0)**, yaw -32768 (-180 deg). Not a 700 cm sphere.
- **Completion rule (script, disassembled):** `WillowWaypoint.Touch`: if `bUpdateObjectiveOnPlayerTouch` and
  `Other.IsA('Pawn')` and `Other.IsPlayerOwned()` -> `ProcessPlayerTouch`: if
  `MissionTracker.IsMissionObjectiveActive(LinkedObjective)` and (no restriction or one restricted set
  active) -> `MissionTracker.UpdateObjective(LinkedObjective)`. `MissionTracker` itself is native.
- **How found:** the mission's `InitialObjectiveSet` names one objective; of the 10 levels of `Sanctuary_P`,
  only `Sanctuary_Dynamic` imports it, and of the 15 `WillowWaypoint` exports in the map exactly one links it.
  A wider class scan of `Sanctuary_Dynamic` (UpdateMissionObjective behaviours, triggers, touch events,
  waypoints, enable conditions; not every export) found no other reference to that objective.
  `Episode_4.SeqEvent_Touch_3` (TriggerVolume_2 near the shop) is enabled while this mission is *not
  started*; it is an Episode 4 gate, not the range trigger.
- **Oracles:** exactly one linked waypoint; Marcus's scripted walk ends inside this cylinder (1b).
- `UNVERIFIED`: UE3 cylinder touch semantics in the host (planar radius, +-CollisionHeight), and that the
  player pawn's collision (not its origin) is what touches.

## 1b. Marcus: placement and walk — recovered (navigation leg UNVERIFIED)

- **Marcus is a placed pawn, not population-spawned** (corrects the earlier census note):
  `Sanctuary_Dynamic:TheWorld.PersistentLevel.WillowAIPawn_13`, archetype `GD_Marcus.Character.Pawn_Marcus`,
  location **(10915.29, 8926.62, 3321.0)**, yaw -16448 (-90.35 deg).
  Found through the Kismet data: `WillowSeqEvent_MissionRemoteEvent_16` (`RocksPaper_MoveMarcusToRange`, this
  mission) -> `WillowSeqAct_AIScripted_2` ("MarcusGO") `Target` = `SeqVar_Named_1` (`KV_Ep4_Marcus01`) ->
  the single `SeqVar_Object` with that `VarName` (`Main_Sequence.SeqVar_Object_5`) -> `ObjValue`.
- **Movement data:** `CharClass_Marcus`: GroundSpeed **294**, WalkingPct 1, `PHYS_NavMeshWalking`, yaw rotation
  rate 40000 units/s, SlowDownDist 200, SlowDownMinPct 0.2. The action: FocusStyle `ESF_Path`, Stance
  `STANCE_None` (class default). `WillowSeqAct_AIScripted` declares no speed property; move nodes use class
  defaults PawnArrivalRadius **128**, AISpeedPercentageHere **1.0**, bFuzzyArrival false.
- **Walk to the range** (Destination `WillowAIMoveNode_12`, then `NextNodes` until none):

  | # | node | ue3 location | yaw deg | on arrival |
  |---|---|---|---|---|
  | start | placed pawn | (10915.29, 8926.62, 3321) | -90.35 | native navigation to node 12 |
  | 1 | WillowAIMoveNode_12 | (10752.94, 9021.71, 3291) | 177.89 | `SeqEvent_ArrivedAtMoveNode_0` -> door `SeqAct_Interp_2` **Play** |
  | 2 | WillowAIMoveNode_26 | (10328.84, 9030.23, 3291) | 177.89 | `SeqEvent_ArrivedAtMoveNode_2` -> door **Reverse** |
  | 3 | WillowAIMoveNode_35 | (9950.12, 9289.85, 3291) | 177.89 | none |
  | 4 | WillowAIMoveNode_40 | (9782.78, 9286.58, 3291) | -142.38 | none (end) |

  `Finished` -> `SeqAct_ApplyBehavior_6` -> `Behavior_SetAIFlag` `GD_AI_Flags.Misc.Flag_LookAtPlayer = true`.
- **Return walk (other mission, recorded):** `WillowSeqAct_AIScripted_0` ("Moving to shop"), entered from the
  *Amp* mission's `RocksPaper_MoveMarcusToShop` through `SeqAct_ApplyBehavior_5` (LookAtPlayer flag, value
  default false): node 39 (door Play) -> node 18 (door Reverse), then `MarcusStartPatrol`.
- **Oracles (all pass):** named variable resolves to exactly one object whose archetype is `Pawn_Marcus`; each
  node's predecessor is listed in its `PreviousNodes`; the door events along the walk are Play then Reverse
  on one Matinee; the door actor `InterpActor_13` lies 79.2 uu from the segment between the opening and
  closing nodes; the walk's last node lies inside the 1a cylinder (planar 210.9 uu, dz -113); the return walk
  ends 5.4 uu from the placed pawn with the same yaw (so node 18 is his shop spot).
- `UNVERIFIED`: the leg from the placed pawn to node 12 is native navmesh pathing (the manifest gives a
  straight line); that the AI walks the `NextNodes` chain after reaching its destination and fires `Finished`
  at the chain end (inferred from the data layout, not from code); speed scaling by AISpeedPercentageHere.

## 1c. Target dummy — recovered (spawn trigger UNVERIFIED)

- **Den serving Fire:** `PopulationOpportunityDen_13` (Description "Rocks, Paper, Genocide: Fire"),
  `MissionPopulationAspect_1.MissionObjective = ...M_RockPaperGenocide_Fire.Fire`, WaypointSetting
  `PWS_MissionObjective`, MaxTotalActors 1, SpawnRadius 30000, region `GD_GameStages.Zone1.Sanctuary`
  min/max game stage 7/9, enabled (class default).
- **Chain:** `GD_Population_Psycho.Population.PopDef_TargetDummy` -> `PopulationFactoryBalancedAIPawn_0` ->
  `GD_Population_Psycho.Balance.PawnBalance_TargetDummy` -> archetype `GD_TargetDummy.Character.Pawn_TargetDummy`,
  DefaultExpLevel attribute `GD_Balance.EnemyLevel.EnemyLevel_GameStage_Exact`.
- **Spawn point:** `WillowPopulationPoint_40` at **(9607.43, 10427.84, 3247.0)**, yaw -90 deg. Shared with the
  Shock (Den_27) and Amp (Den_11, initially disabled) dens; Corrosive (Den_26, `PopDef_TargetDummyBot`) uses
  point 39 (2.3 uu away).
- **After spawn (Kismet):** `SeqEvent_PopulatedPoint_0` (point 40) -> `SeqAct_SetPhysics_0` on the spawned actor
  (no tagged NewPhysics: class default) -> `SeqAct_AttachToActor_4`: attach to
  `WillowInteractiveObject_1108` (`IO_RockPaperGenocide_TargetHolder`) bone `Target`. The holder is at
  (9611.59, 10469.03, 3253.19), hard-attached (`Base`) to `InterpActor_4`.
- **Oracles (pass):** one den names one of this mission's objectives; its chain resolves to the expected pawn and
  balance; the spawn point is 41.9 uu from the holder; the holder's `Base` is the target Matinee's group actor
  and is in that actor's `Attached` list.
- `UNVERIFIED`: that the den spawns when the `Fire` objective becomes active (MissionPopulationAspect rule is
  native); SetPhysics' default enum value; the `TargetDummy` sequence (Den_28, enabled after the Amp mission
  completes) is post-mission target practice and not part of this route.

## 1d. Target mover and the other Kismet world ops — binding recovered, frame UNVERIFIED

`tools/prepare_mover.py --binding` (new, data-only mode; the door mode is unchanged) on
`RocksPaperGenocide.SeqAct_Interp_0`: duration **2 s**, bRewindOnPlay, inputs Play/Reverse/Stop/Pause/Change
Dir/Last Frame (class default). Group `Target` -> `SeqVar_Object_23` -> `InterpActor_4` (hidden carrier,
DrawScale 0.25, at (9568, 10480, 3344), pitch -90 deg) carrying `WillowInteractiveObject_1108` and
`InterpActor_52/53/54`. Move track `IMF_RelativeToInitial`, 2 keys, `CIM_CurveAutoClamped`, zero tangents:
position (-472.84, 5780.82, 166.88) -> (191.16, 5780.82, 166.88), i.e. a **+664.0 uu X** delta; Euler
constant (-180, 0, -90). Event tracks: forward-only `ChangeBool_FALSE` at 0; reverse-only `ChangeBool_TRUE` at
0 and `WheelBackward` at 2. Two Ak tracks (Claptrap run start/end) are listed, not played.

Who drives it (data, exact names): the mission's own remote events `RocksPaper_TargetForward`,
`RocksPaper_TargetBack`, `RocksPaper_SetFireTargetBool`, `RocksPaper_FireCompleted` have **no Kismet listener
anywhere in the map**. The Matinee is driven by the dummy's behaviours instead:
`CharClass_TargetDummy` `FireDamage.OnSpawned` -> `Behavior_RemoteEvent_60` `RocksPaper_MoveTargetForward`
-> `SeqEvent_RemoteEvent_5` -> `SeqCond_CompareBool_0` (`IsTargetReady?`, initially true) -> **Play** (false ->
`SeqAct_Delay_0` -> `SeqAct_Switch_3` -> re-check); the `FireDamage` chain that completes `Fire` also enables
the dummy's `ObjectiveComplete` sequence, whose `OnBehaviorSequenceEnabled` link (2 s delay) runs
`Behavior_RemoteEvent_44` `RocksPaper_SendTargetBack` -> **Reverse**;
the dummy's death `RocksPaper_TargetKilled_Reset` -> Reverse + dialog. Reverse end fires `ChangeBool_TRUE` ->
`IsTargetReady? = true`, `SeqAct_Destroy_0` (the four populated dummies) -> `GearboxSeqAct_ResetPopulationCount_0`.

- `SeqAct_Toggle_0`: target `PopulationOpportunityDen_11` (Amp den); reached only by the Amp mission's
  `RocksPaper_SpawnSlagTarget` (input 0) and `RocksPaper_AmpCompleted` (input 1). Not on the Fire route.
- `GearboxSeqAct_TriggerDialogName_1`: group `GD_VOSQ_RockPaperGeno.Groups.DialogGroups_Side_RockPaperGeno`,
  event `GD_VOSQ_RockPaperGeno.Events.VOSQ_RockPaperGeno_17_EchoX_Marcus`, name `GD_Dialog_NPC.Names.DialogName_Marcus`;
  reached by `RocksPaper_TargetKilled_Reset`.
- `UNVERIFIED`: the world frame of the +664 X delta (door convention: first-key delta added to the placed
  actor, giving forward end (10232, 10480, 3344); the carrier's -90 deg pitch makes a rotated-frame reading
  possible); whether native code routes mission remote events to plain remote events of another name (no
  data says so); CompareBool/Delay/Switch timing (Delay duration is the class default).

## 1e. Respawn — rule recovered, runtime state UNVERIFIED

Stations in the map (all four `TeleportDest.Owner`/exit-point `Owner` back-references check):

| station | ue3 location | CanResurrectHere | destination exit points |
|---|---|---|---|
| `Sanctuary_P` FastTravelStation_0 (`GD_FastTravelStations.Sanctuary.Sanctuary`) | (8194.22, 2782.63, 3628.26) | yes (`bInitiallyActive`) | 4, first (8109.24, 2929.86, 3716.16) yaw 90 |
| `Sanctuary_P` LevelTravelStation_1 (SanctuaryToIce) | (-4584.70, -10814.21, 2423.07) | no | 4 |
| `Sanctuary_P` ResurrectTravelStation_0 (touch radius 5000, height 240) | (-2257.61, -9872.50, 2621.44) | yes | 4 |
| `Sanctuary_Dynamic` ResurrectTravelStation_1 (touch radius 7500, height 1200) | (-21972.31, -29238.36, 1715.63) | yes | 4 |

Selection, from script (`WillowPlayerPawn.GetBestPlayerPlacementPoint`): the GRI's
`ActiveRespawnCheckpointTeleportActor` (set by `TravelStation.ReplacePreviouslyActivatedStation` when a station
is activated) -> else the first station with `bIsCurrentlyActive || bShouldBeActive` -> else the **nearest**
station whose `CanResurrectHere(false)` is true -> else the nearest other. Exit point:
`TeleporterDestination.GetNextExitPoint` round robin from `ExitPointsCounter` (0), skipping vehicle exit points
and occupied ones. With no activated station, a death at Marcus's shop or at the range picks
**FastTravelStation_0 -> StationTeleporterDestination_0 -> StationTeleporterExitPoint_0** (6.7 km away, the
nearest capable station). `UNVERIFIED`: what activates a station in play (touch radius vs. interaction; the
caller of `SetStationActivatedState` was not traced). Note: `WillowCoopPlayerStart_0` (655.6, -6348.9, 2800) lies
4571 uu (planar) and 179 uu (vertical) from `ResurrectTravelStation_0`, inside its 5000/240 touch values; if
those values are the activation rule, a player entering at the start would activate it and it would win over
the nearest-station rule. Not established.

## 2a. XP reward — inputs recovered, amount UNVERIFIED (native)

`Reward.ExperienceRewardPercentage` -> `GD_MissionRewardBalance.XP.XPReward_02_Small`: resolver chain
`ConstantAttributeValueResolver` **0.05** x `SimpleMathValueResolver` Mul by `Init_MissionXPPlaythroughMultiplier`
(conditional: default 1; the single conditional branch `Playthrough2_XPReward_Scale` = 1). So **0.05 on both
playthroughs**. Experience to reach level L: `Init_ExperienceRequiredForLevel` = **60 x L^2.8 + 7.33** (level from
a global attribute). `MissionDefinition.GetExperienceReward` and `GetRegionGameStage` are **native** (no script).
The manifest's candidate amount, percentage x (required(L+1) - required(L)), gives e.g. L7 316, L8 396, L9 484 —
**a hypothesis**, to be checked against the quest-accept/turn-in XP number in an original-game capture. Mission
level: `GameStageRegion = GD_GameStages.Zone1.Sanctuary`, which carries no stage data; the only data hint is the
mission dens' 7..9 game-stage bounds.

## 2b. Maya's health — recovered as a formula (one convention UNVERIFIED)

`CharClass_Siren.HealthPoolDefinition = D_Resourcepools.PlayerPools.HealthPool`, `StartWithMaxValue`, BaseMaxValue
-> `GD_Balance_HealthAndDamage.HealthAndDamage.Init_PlayerHealth`: Multiplier = attribute
`Att_UniversalBalanceMultiplier_HealthShields` (**80**), Level = `Att_UniversalBalanceScaler` (**1.13**), Power =
`PlayerExperienceLevel`, min 20: **health(L) = 80 x 1.13^L** -> L1 90.4, L5 147.4, L7 188.2, L10 271.6.
`UNVERIFIED`: the Multiplier term also carries BaseValueConstant 94; the attribute is used (the existing
`weapon_recipe.attribute_value` convention). If the constant were used instead, health would be 94 x 1.13^L.
Skills, class mods and relics are not applied. **Shield:** `ShieldPool` has no BaseMaxValue — base capacity 0,
capacity comes from an equipped shield item.

## 2c. Dependency fixture — what it stands for

`M_RockPaperGenocide_Fire.Dependencies = [GD_Episode03.M_Ep3_CatchARide]` ("The Road to Sanctuary"; plot
critical; depends on `GD_Episode02.M_Ep2c_Henchman`; region `GD_GameStages.Zone1.Ice_A`; turned in at the
Sanctuary fast-travel station; next in chain `GD_Episode04.M_Ep4_WelcomeToSanctuary`). The fixture stands for
player progress "CatchARide complete". Sanctuary access implies more: the level travel station into Sanctuary
(`IceToSanctuary`) requires CatchARide active with `ContactRoland` complete, and the Sanctuary fast-travel
station requires `M_Ep4_WelcomeToSanctuary` active with `GotoSanctuaryGate` complete. Which status the
dependency check accepts (complete vs. turned in) is MissionTracker (native): `UNVERIFIED`.

## Manifest

`local/slice/world.json`, schema `ow-slice-world-v1` (ignored; game-derived):

- `levels`, `coordinates`, `reader_sha256`, `generated`.
- `range_trigger`: `object`, `ue3`/`host`, `shape {type: cylinder, radius, half_height, component}`,
  `linked_objective`, `objective_set_restrictions`, `update_objective_on_player_touch`, `enabled`,
  `touch_volumes`, `completion_rule`, `how_found`.
- `marcus`: `pawn` (object, archetype, pose), `ai_class` (speeds), `entry_event`, `walk_to_range`
  {action, focus_style, stance, target, destination, `start_leg`, `path[]` (node, pose, pawn_arrival_radius,
  ai_speed_percentage, fuzzy_arrival, previous_nodes, `on_arrival[]` with resolved Kismet outputs), finished},
  `other_walks[]` (same shape + entry_events), `oracles[]`.
- `target_dummy`: `den` (den, event, description, pose, population chain `archetypes[]`, `spawn_points[]`,
  game_stage, mission_objective), `other_dens[]`, `after_spawn[]` (Kismet ops), `holder`, `oracles[]`.
- `target_mover`: `binding` (`ow-mover-binding-v1`: duration, inputs, outputs, `groups[]` with actor, attached,
  initial, position/rotation curves, event_tracks, omitted_tracks), `driven_by[]`, `upstream_events[]`,
  `remote_event_sources[]`, `toggles[]`, `dialogs[]`.
- `mission_remote_event_listeners`: mission `SupportedRemoteEvents` -> Kismet listeners found in the map.
- `respawn`: `stations[]`, `fresh_state_choice` (for Marcus's spot and the range), `selection_rule`,
  `runtime_state`, `oracles[]`.
- `scene_bounds` (when `local/sanctuary/scene.json` exists): planar distance from each recovered position to the
  nearest prepared static-mesh placement origin (an AABB is useless: backdrop meshes span hundreds of km). On
  this run: dummy spawn 41.5, walk nodes 168-841, Marcus 356, range trigger 1046, fast-travel station 580,
  `ResurrectTravelStation_0` 2633, level travel station 3203, `ResurrectTravelStation_1` 5738 uu. Prepared mesh
  origins are sparse (BSP/terrain are not counted), so a large distance does not mean off-map; this is a weak
  check that shows positions are among the map's geometry, not that they stand on a floor.
- `values`: `xp`, `health`, `shield`, `dependency` (from `tools/slice_values.py`).

## Reproduce

```powershell
cmake --build build --config Release
python tools/prepare_slice_world.py --game "$env:OPENWILLOW_BL2"     # writes local/slice/world.json (~4 min)
python tools/slice_values.py --game "$env:OPENWILLOW_BL2"            # values only: local/slice/values.json
python tools/prepare_mover.py --game "$env:OPENWILLOW_BL2" --package Sanctuary_Dynamic --binding `
  --action TheWorld.PersistentLevel.Main_Sequence.RocksPaperGenocide.SeqAct_Interp_0 --output local/slice/target_mover.json
python tests/slice_world_test.py
```

## Not done

No host wiring (host files belong to another lane), no original-game capture, no navmesh path for Marcus's first
leg, no decoding of the native population, MissionTracker, region game-stage or experience-reward code.
