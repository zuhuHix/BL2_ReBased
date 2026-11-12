## 2026-09-23: no walker collision for the BoxLrg on the Sanctuary player start

A maintainer walking the host scene was stuck at spawn and identified the
blocker in the UE editor as `Sanctuary_Outer` `InterpActor_34`
`StaticMeshComponent_20`. It is a `Prop_Garbage.Meshes.BoxLrg` at
(2251, -4296, 2733), yaw -61.875 degrees, with DrawScale 20 and DrawScale3D
(1.5, 4, 0.5). That makes a box roughly 19 x 34 x 4.6 m spanning z 2503-2963.
It was already hidden visually on 2026-09-22, with its collision kept.
`Sanctuary_P`'s own `WillowCoopPlayerStart_0` is at (655.6, -6348.9, 2800),
inside that box, and the host spawns 100 units above it. A blocking volume on
the game's own player start cannot be active when the player is in the landed
town. The component serializes no collision overrides, so it blocks under
UE3 defaults. So something the host does not run (Kismet level visibility of
`_Outer` versus `_Land`, or `_Outer`'s `Main_Sequence` Matinee) must remove or
move it in the original. Which one is not known; we found no record-level
reference to the actor in `_Outer`'s scene records, but the record schema does
not cover Kismet variables.

`tools/prepare_level.py` now leaves exactly this (level, source) placement
without walker collision. The other two hidden BoxLrg placements (`_26`,
`_33`) keep their collision, since there is no comparable evidence for them.
`docs/TOOLING.md` previously placed `_34` in `Sanctuary_P`; the manifest records
it in `Sanctuary_Outer`, and the doc is corrected. Not verified: whether other
`_Outer` content should be inactive in landed play.

