## 2026-09-22: spaced hole probes and target-aware inspection candidates

`prepare_terrain.py` now emits three spaced flagged-hole candidates per
terrain (`hole_probe_policy=three_spaced_adjacent_cells_v1`,
`hole_candidates`) instead of a single one, so one prop or building floor
sitting over the chosen cell cannot hide the whole runtime check. The legacy
`hole` object is still emitted as the first candidate, and regenerating
Sanctuary's `terrain-runtime.json` left all eight first candidates identical
to the previously recorded cells, so this is a strict superset rather than a
change to existing evidence. `TerrainWalkingTest.cpp` walks the candidate
list and falls back to the single `hole` object for scenes prepared before
this change. Endpoint assertions are still reported separately from the
original-point band trace and are explicitly not treated as original-hole
proof; whether any of these cells is a hole in the original game remains
UNVERIFIED pending matched screenshots.

`prepare_inspection_views.py` adds host-side candidate poses that look back
at a recorded terrain stand point for the obstructed Terrain_10 view
(`target_trace_candidate_v1`), and `InspectionTest.cpp` selects the first
candidate whose target trace reaches the requested actor, warning and
advancing when one is obstructed. The offsets are deliberately broad and
symmetric host inspection candidates, not recovered original-game camera
coordinates.

`tools/diagnose_sanctuary_geometry.py` is a new read-only diagnostic that
resolves scene ownership, per-section effective material, terrain layer
provenance and camera-ray coverage gaps from the prepared manifest. It
reports; it does not modify host geometry. It has not yet produced a
confirmed cause for the Scooter-street opening or the bright snow-view
surface.

Verification: UE5 `OpenWillowEditor` compiles with both test changes; the
runtime automation tests could not be run in this worktree because the
Sanctuary map is not imported here. ctest 6/6 and 154 focused Python tests
passed.

