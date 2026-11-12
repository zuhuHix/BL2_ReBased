## 2026-09-14: hide observed blocking helpers and render the dome interior

The Sanctuary source contains four `Common_Meshes.Blocking.Blocking_Cube`
placements with no effective material. They are collision helpers, not visible
level geometry; the host keeps their observed collision and hides only their
rendering. The native Sky_Dome faces are outward-oriented while the inspection
camera is inside the shell, so the accepted Unlit sky material is marked
two-sided by the bounded host policy. This does not infer the missing dynamic
sky graph or alter unrelated unassigned slots.

The artifact pass extends the same render-only policy to all five observed
`Common_Meshes.Blocking.Blocking_Cube` placements, all 94
`Common_Meshes.CollisionCube` placements, and the four `Mat_CloudLayer_Light`
`Blocking_Plane` placements. It also hides the exact `Sanctuary_P`
`InterpActor_34.StaticMeshComponent_20` `Prop_Garbage.Meshes.BoxLrg` placement
that blocked the start view. Source collision remains enabled where the
serialized mesh has a recovered collision hull; the policy does not claim
complete collision parity.

