# Collision and walking slice (2026-09-14)

Checkout: t3code-9935cb53. This is an opt-in placeholder walking mode for the
frozen map viewer. It is not UE3 movement parity or completed M1.

## Verified

- C++ Release and UE5.8 OpenWillowEditor builds passed.
- CTest: 5/5 passed, including fixed Box/Plane/Matrix values, every truncated
  fixed-struct payload, invalid Box validity and existing parser regressions.
- Scene tests: 10/10 passed. Collision geometry tests: 3/3 passed, covering
  asymmetric box transforms, bounds/indices, degeneracy and unsupported shapes.
- All nine installed code packages still match the independent Python reader.
- Existing Sanctuary geometry was copied locally from t3code-df92eae1; all 423
  source mesh OBJs were freshly extracted and compared byte-for-byte before
  attaching collision metadata. Materials and actor placements were preserved.
- Sanctuary: 304 supported mesh bodies, 117 absent, two unsupported; 509 hulls.
  Collision applies to 3,116 components. The 495 reusable mesh sections reopened
  with matching hull vertices and component switches, zero errors/warnings.
- Existing saved-scene verification also passed for all 4,768 section actors
  and four material channels, zero errors/warnings.
- OpenWillow.Walking passed: the real Sanctuary start settles on imported
  collision at Z=2810.150 cm. An isolated synthetic fixture checks gravity,
  standing height, movement stopped by a wall, jump ascent/landing, falling
  off an edge, standing on and climbing a 20 degree slope.
- OpenWillow.Viewer free-flight regression passed on the updated Sanctuary.
- The final walking run is viewer-ad4a6dd250e247da8c1c104d4009aaf7.log.
  OpenWillowWalkingStart.png was visually inspected: the standing camera shows
  textured Sanctuary architecture, with the existing black sky, mirrored sign
  and fallback surfaces. An earlier black capture was rejected because it was
  taken after teleporting to the isolated test fixture; capture now precedes it.

## Limits

Only Sanctuary received the current collision refresh/import and runtime checks.
Ash source bodies were inspected but its saved scene has not been refreshed.
The two unsupported Sanctuary meshes are Engine:EngineMeshes.Cube (body class
identity outside the accepted qualified name) and WaterPlaneVertex300 (invalid
box dimensions). Other shape types, terrain/BSP, blocking volumes, CDO defaults,
physics simulation and gameplay remain outside this slice. Passing at the spawn
does not prove every route is walkable.

Tests supply engine movement input. Physical WASD/mouse/Space feel, stairs and
matched original-game viewpoints remain user checks. Existing sky, materials,
mirrored signage and performance limitations remain. The automation readiness
threshold is 1 FPS; a pass is not a performance certification.

## Reproduce

```powershell
$engine = 'C:/Program Files/Epic Games/UE_5.8'
$game = 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2'
python tests/collision_test.py
python tools/refresh_collision.py --reader build/Release/ow-package.exe --game $game --scene local/sanctuary
$env:OPENWILLOW_BL2 = $game
$env:OPENWILLOW_SCENE = (Resolve-Path local/sanctuary).Path
$project = (Resolve-Path host/ue5/OpenWillow/OpenWillow.uproject).Path
& "$engine/Engine/Binaries/Win64/UnrealEditor-Cmd.exe" $project -run=pythonscript "-script=$((Resolve-Path host/ue5/apply_collision.py).Path)" -unattended -nullrhi
& "$engine/Engine/Binaries/Win64/UnrealEditor-Cmd.exe" $project -run=pythonscript "-script=$((Resolve-Path host/ue5/verify_collision.py).Path)" -unattended -nullrhi
./tools/test_ue_viewer.ps1 -Engine $engine -Game $game -Scene local/sanctuary -Walk
./tools/test_ue_viewer.ps1 -Engine $engine -Game $game -Scene local/sanctuary
./tools/run_ue_level.ps1 -Engine $engine -Game $game -Scene local/sanctuary -ViewOnly -SkipBuild -Walk
```

Build this checkout's UE module before running these commands. Close its editor
before modifying its generated assets. `-SkipBuild` requires the current binary.
Walking controls: WASD/mouse and Space to jump. Omit `-Walk` for free flight.

Evidence stays ignored under local/sanctuary (collision-report.json,
ue-collision.json, ue-collision-verify.json, ue-verify.json and viewer logs),
local/level-verify-console.log and the UE project's Saved/Screenshots directory.

# Routes, stairs and blocking volumes (2026-10-21)

AI-assisted (Claude), lane B. Branch `feature/walking-collision`. Sanctuary only; Ash and Southpaw Factory were not
refreshed (no prepared scene of either exists on this machine). Ground truth comes from the running original game
through the community mod SDK (`tools/real_game/`); everything it recorded stays under ignored `local/realgame/walking/`.
This section records rules, counts and pass/fail only.

## What changed

- **Blocking volumes** (`tools/collision_volumes.py`). A UE3 `BlockingVolume` keeps its collision in its
  `BrushComponent`'s tagged `BrushAggGeom`, the same `KAggregateGeom` layout as `RB_BodySetup.AggGeom`, so the existing
  property decoder reads it (actor prefix 26 bytes, component prefix 8, the same values `--scene-records` uses; no
  change to `src/`). Whether a volume blocks comes from the actor's `bCollideActors` and `bBlockActors`, with unset
  values read from the class defaults (`Default__<Class>` and its archetype chain) in the installed `Engine` package:
  `Volume` sets `bCollideActors=True`, `BlockingVolume` `bBlockActors=True`, `PathBlockingVolume` `bCollideActors=False`.
  Rotation, DrawScale, DrawScale3D and PrePivot are refused, not guessed (none occurs in Sanctuary). A
  `DynamicBlockingVolume` keeps its brush whatever its serialized flags say and is left out until its state is
  observed in the running game, because Kismet may toggle it (review round 1: a serialized-off volume that the
  running game shows on is now imported).
  The host builds blocking volumes at play time from `volumes-runtime.json` through one `AOpenWillowVolumes` actor
  (hidden, collision-only, blocks pawns and physics bodies, ignores the Visibility and Camera channels).
- **Sphere and capsule elements** (`tools/collision_geometry.py` `shapes()`, `UOpenWillowCollision::SetShapes`).
  `KSphereElem` and `KSphylElem` are decoded from their `TM`, `Radius` and `Length`; a scaled or sheared `TM` is
  refused. That the capsule's long axis is the TM's Z axis with `Length` between the hemisphere centres (UE5's
  `FKSphylElem` convention) is `UNVERIFIED`. `hulls()`, which `prepare_level.py` uses, still refuses them.
- **Refresh** (`tools/refresh_collision.py`): writes `volumes` and `volume_policy` into `scene.json`, keeps
  `prepare_level`'s triangle floors and its `placement_collision()` rule (before, a refresh re-enabled the one landed
  placement that rule switches off), skips `prepare_bsp`/`prepare_terrain` entries (before, it would have stopped on
  them), and accepts `--live-volumes` with the live capture.
- **Route probes** (`tools/walking_routes.py`, `tools/real_game/scripts/walking.py`, `tools/real_game/walking_capture.ps1`):
  one straight probe per placed stair ramp (start 80 cm before the low edge, end 80 cm past the high edge) plus five
  multi-ramp flights and switchbacks and one open-ground walk from the player start, 35 in all. The running game walks
  each with a steering autopilot (view yaw set every frame, forward key held) and records the pawn every frame.
  `OpenWillow.RouteWalking` replays the same probes in the host with the same steering rule. A probe without a
  usable trace, or a trace older than the capture's `captured_at`, stops `walking_routes.py routes`; so does a probe
  list that differs from the committed count for the map (`EXPECTED`: Sanctuary 35 routes, 5 blocking volumes). The
  capture clears old traces first. The test requires those committed counts and fails on zero volumes (review
  rounds 1-2). The 2026-10-21 capture predates `captured_at`; it was backfilled from the session log's start time,
  73 s before the first trace.

## Observed in the running game (2026-10-21, one session, Save0008 level-8 Maya in Sanctuary)

- Player pawn (`WillowPlayerPawn`): cylinder radius 42, half-height 80; `MaxStepHeight` 35; `WalkableFloorZ` 0.70;
  `GroundSpeed` 440; `AccelRate` 2048; `AirSpeed` 500; `AirControl` 0.11; `MaxFallSpeed` 1500; `JumpZ` 630
  (`JumpZBaseValue` 630); `BaseEyeHeight` 70; crouch height 50; gravity -500. These are read values. A recorded
  jump failed (the jump key did not register), so the jump arc itself is still not observed.
- Volumes: all seven Sanctuary volumes that have a brush match their live `CollisionComponent.Bounds` exactly
  (0.00 cm on origin and extent), which confirms the decoded geometry and the Location-only placement there. Every
  live `BrushComponent` has `BlockZeroExtent=False`, `BlockNonZeroExtent=True`. The four `PathBlockingVolume`s do
  not collide (as their class default says) and have no brush component in the cooked level.
  `DynamicBlockingVolume_1` and `_2` collide; `_0` (a 23 m wide box around the town centre) and `_3` do not, at this
  save's plot point. Result for the host: 5 blocking volumes (3 static, 2 dynamic as observed), 6 non-blocking.
- Stairs: the original pawn's floor normal on the ramps (Z 0.80) equals the ramp hulls' (0.802), so player movement
  uses the convex ramp hull, not the steps' render triangles. On a slope its feet stand 35-42 cm above the hull
  surface under its centre at radius 42 and 33 degrees, which fits a cylinder resting on its uphill rim
  (r·tanθ = 31 cm) plus a few cm of floor gap. A UE5 capsule rests lower, so routes are compared on the ground height
  under the centre, not on feet height.
- Routes: 29 of 35 probes reached their end. The six that did not (`stair_00`, `stair_04`, `stair_10`-`12`,
  `flight_hq`) start above a drop or against a wall in the original too; they are reported, not asserted.
- Method note: the capture moves the pawn by writing its Location. The running game left it hovering at the start
  point until it moved, then it dropped onto the floor. Heights therefore count from the first landing.

Saves were blocked during the session; the one `.bak` written at character selection was restored from the
pre-session backup and the save folder compared afterwards.

## Host results (UE 5.8, this machine)

Automated checks:

- `python tests/collision_test.py`: 10/10 (box, convex, sphere, capsule, refused scale, volume flags and defaults,
  refused transforms, dynamic volumes including serialized-off/live-on, live observations).
  `python tests/walking_routes_test.py`: 9/9 (including truncated probe lists and stale traces).
- CTest 10/10; `tools/verify_packages.py` 9/9 packages. No file under `src/` and no `CMakeLists.txt` change.
- `refresh_collision.py` on Sanctuary: 304 supported bodies, 113 absent, 2 unsupported (unchanged), 509 hulls,
  0 spheres and 0 capsules on placed meshes (the 7 capsules and 1 sphere outside `PhAT` assets in the Sanctuary
  packages are not used by any placed mesh), 4 triangle floors kept; volumes 5 blocking, 6 non-blocking with the
  live state.
- `apply_collision.py` / `verify_collision.py`: 495 mesh sections, 3,126 enabled components (3,116 on 2026-09-14;
  the difference fits the 11 triangle-floor components minus the landed placement kept off), one
  `OpenWillow_Volumes` actor, 0 errors after reopening.

In-engine (`tools/test_ue_viewer.ps1`):

- `-Route` (`OpenWillow.RouteWalking` + `OpenWillow.CollisionShapes`): pass. Probes the original finished: 29/29
  reached by the host walker with the original's cylinder (42/80), step height, walkable Z and speed applied for the
  run; lowest ground under the centre at most 23.8 cm below the original's (`stair_03`), end ground within 22.4 cm
  (`stair_06`); margins 30 cm. Blocking volumes: 5/5 stop a pawn capsule swept onto them and ignore line traces.
  Of the six probes the original did not finish, the host also stops on four; it **walks `stair_04` and `stair_10`
  where the original did not** (on `stair_10` the original pawn did not move at all, which suggests a blocker the
  host lacks there). Not explained yet.
- `-Walk` (`OpenWillow.Walking`): pass. `-Bsp`: pass, stood and walked 2/2 models.
- `-Terrain`: **fails one assertion**, stand 8/8, seam 1/1, hole endpoint assertions 23/24 (`Terrain_2`: a hole
  endpoint rests on terrain at the recorded surface). The same failure repeats with the volumes switched off (an A/B
  run against a scene copy without `volumes-runtime.json`), so it does not come from this change. This machine's
  `terrain-runtime.json` was copied from an earlier worktree and lists 24 spaced hole candidates; the last recorded
  run had 8. Left for the terrain owner.

The host walker itself still uses its placeholder 34/88 capsule and 450 cm/s unless `-owmaya` is given; it was not
changed here (`OpenWillowWalker.*` is shared). The read `JumpZ` 630 differs from the walker's estimated 420, but no
jump was observed, so the walker keeps 420.

## Still open

- Routes cover the 29 straight Sanctuary ramps, five flights and one open-ground walk; town-wide routes (doors,
  rooftops, the HQ interior), crouching and jumping between ledges are not recorded.
- `stair_04` and `stair_10` divergences; the two unsupported meshes from 2026-09-14.
- Kismet toggling of `DynamicBlockingVolume`s (the host has the state of one save's plot point).
- Sphere and capsule axis convention, unexercised on Sanctuary's placed meshes; Ash and Southpaw Factory refreshes.

## Reproduce

```powershell
$game = 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2'
python tools/walking_routes.py candidates --scene local/sanctuary --output local/collision/probes.json
powershell -File tools/real_game/walking_capture.ps1 -Probes local/collision/probes.json   # takes the screen and keyboard
python tools/walking_routes.py routes --probes local/collision/probes.json --traces local/realgame/walking `
    --capture local/realgame/walking/walk_info.json --output local/sanctuary/walk-routes.json
python tools/refresh_collision.py --reader build/Release/ow-package.exe --game $game --scene local/sanctuary `
    --live-volumes local/realgame/walking/walk_info.json
# then apply_collision.py / verify_collision.py as above, and
./tools/test_ue_viewer.ps1 -Engine $engine -Game $game -Scene local/sanctuary -Route
```

The hand-made flights, switchback and open-ground walk used here were appended to the candidate list by hand;
`walking_capture.ps1` loads `Save0008.sav` through the installed `quick_startup` mod's `-Character` option by default.
