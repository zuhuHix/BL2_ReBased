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
