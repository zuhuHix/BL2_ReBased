## 2026-09-22: hide CollisionCube placements by their observed hidden flags

The 2026-09-14 render-only policy hid all 94 `Common_Meshes.CollisionCube`
placements by mesh name. That removed the street in front of Scooter's
garage: six `Sanctuary_Land` `StaticMeshCollectionActor_38` placements
(`SMC_544/603/608/615/617/618`) are 1536x1536 uu `Mati_FloorConcrete01`
slabs with top z = 2720, and 8 of the 11 floating liquid decals in the
scene sit 8 uu above them. Nothing else in the manifest covers that area.

`prepare_level.py` now records `source_hidden` for each placement: whether
the component serializes `HiddenGame`, or its owner actor serializes
`bHidden`. A CollisionCube is hidden when `source_hidden` is set, or when none
of its effective materials resolves to a known source other than the cube's
own `Common_Meshes.Collision.Mat_Collision`. In the regenerated Sanctuary
manifest, 81 cubes stay hidden: 65 set `HiddenGame` (one also has
`Mati_FogsheetBlack`) and 16 belong to `bHidden` `InterpActor`s. Those
`InterpActor`s include the 13 `Mati_SlateRock8xTileWarm` cubes that are
parked about 350,000 uu away. 13 cubes render: eight `Mati_FloorConcrete01`,
three `Mati_SancBaseConcrete_tile02`, one `Master_Black` sheet
(1000x70x0.05 scale, shadows and lighting disabled) and one
`Mat_RoadIceSkybox` piece. `refresh_materials.py` treats a manifest without
`source_hidden` as hidden, which keeps the earlier result for older
manifests.

Verified: decoded flags and resolved materials for all 94 placements, and
synthetic tests of the rule. Comparing the regenerated manifest with the
previous one: exactly 13 placements change, all CollisionCube, from hidden
to rendered. The placement count (4469) and issue count (185) are unchanged.
The UE5 import and its saved-scene, collision and UV verifiers pass. An
ad-hoc Inspection capture from (1500, -6400, 2900) toward the
`ScootersGarageSign` shows a continuous concrete street where the sky showed
through before. The run reported one camera-rotation tolerance error for the
fractional ad-hoc pitch; the screenshots were still captured. The layout
matches a maintainer's original-game capture of the same spot. The dark puddle
stain visible in that capture does not appear in the host; the liquid decal
planes above the slabs were not investigated in this pass. Not verified: in UE3, `bHidden` on an owner
that Kismet or Matinee toggles at runtime is only the saved state. The owner
flag is applied only to CollisionCube; other meshes with a `bHidden` owner
are unchanged by this entry. Whether `Master_Black` and `Mat_RoadIceSkybox`
look right in the original game has not been checked against a capture.

