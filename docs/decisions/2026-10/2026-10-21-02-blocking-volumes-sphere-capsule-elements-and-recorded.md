## 2026-10-21: Blocking volumes, sphere/capsule elements and recorded walking routes (Sanctuary)

AI-assisted (Claude), lane B. Tools (`collision_geometry.py`, new `collision_volumes.py`, `refresh_collision.py`,
new `walking_routes.py`, `real_game/scripts/walking.py`, `real_game/walking_capture.ps1`, `test_ue_viewer.ps1 -Route`)
and host (`OpenWillowCollision.*`, `WalkingTest.cpp`, `apply_collision.py`, `verify_collision.py`). No change to `src/`,
`CMakeLists.txt`, `prepare_level.py` or `OpenWillowWalker.*`. Details: `COLLISION_WALKING_VERIFICATION.md`, 2026-10-21.

- Blocking volumes are read from `BrushComponent.BrushAggGeom` (tagged `KAggregateGeom`) with the existing decoder;
  blocking comes from `bCollideActors`/`bBlockActors` with class defaults read from the installed `Engine` package.
  Rotated, scaled or pre-pivoted volumes are refused rather than guessed. Dynamic volumes enter the host only in the
  state observed in the running game. **Confirmed in game:** decoded geometry equals the live bounds for all seven
  brush volumes; brush components do not block zero-extent traces; path-blocking volumes do not collide.
- `KSphereElem`/`KSphylElem` are decoded (scaled frames refused); the capsule axis convention is `UNVERIFIED` and no
  placed Sanctuary mesh uses either.
- `refresh_collision.py` now keeps `prepare_level`'s triangle floors and `placement_collision()` exception (it had
  re-enabled one landed placement) and skips BSP/terrain entries.
- Routes are compared on ground height under the centre, not feet height: the running game's cylinder stands 35-42 cm
  above a 33 degree ramp hull (rim contact), a UE5 capsule lower. Its floor normal matches the hull, so stairs use the
  simple hull in the original as in the host. Results: 29/29 of the probes the original finished, volumes 5/5.
  Read but not observed: `JumpZ` 630 (the walker keeps its 420 estimate). The host walks two probes the original
  could not; unexplained.

