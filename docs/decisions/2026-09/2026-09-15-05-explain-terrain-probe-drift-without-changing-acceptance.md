## 2026-09-15: explain terrain probe drift without changing acceptance criteria

Codex added per-frame host hole-probe diagnostics (position, velocity, input,
floor and downward trace) without changing pass criteria or parsing behavior.
The Land Terrain_3 probe begins inside StaticMeshActor_SMC_1281, moves about
9.4 m laterally with zero horizontal velocity on its first frame, then slides
along that mesh and lands on neighbouring TerrainComponent_5, 11.5 m away.
This supports penetration correction followed by sliding; internal solver
steps remain uninstrumented. A passing displaced endpoint must not be treated
as runtime verification of the original hole location. Native strip/tree
topology corroboration is independent of this test limitation.

Local-only BSP record and terrain alpha/weightmap diagnostics did not meet
the evidence threshold for new rendering behavior. Candidate BSP normals
match but point association fails; alpha comparisons find only constant-zero
matches. Keep BSP unimplemented and terrain blending explicitly approximate.
Original-game matched views remain outstanding. Detailed evidence and host
runtime results are in the two Sanctuary verification records.

