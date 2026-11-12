## 2026-09-23: hide every static placement the source marks hidden

A maintainer reported roof textures on the street in front of Scooter's
garage and the gate square beside it. The original game shows plain concrete
there. A triangle probe of the manifest at the bollards (around
(3500, -3800)) found two stacked floors: `Sanctuary_Land`
`SanctuarySidewalk_ParkingLot` at z 2783 with its concrete materials, and 8 uu
above it `Sanctuary_P` `StaticMeshCollectionActor_24` `SMC_288`,
`SanctuarySidewalk_ParkingLot_Low`, drawn with
`Optimization.Mati_SancBuild4a`. That material's regular-atlas fallback
(2026-09-14) samples a building atlas that includes roof tiles. The component
serializes `HiddenGame = true`, so the game never draws it. The 2026-09-22
rule applied `HiddenGame`/`bHidden` only to `CollisionCube`.

In the manifest, 219 placements had `source_hidden` set but still rendered.
204 of them are in `Sanctuary_Px`, an always-loaded sublevel where 205 of 206
placements are `HiddenGame`. They include merged low-detail building shells
such as `SancBuildingGroup01`, whose bounds reach down to street level
(z 2780) around Scooter's, plus roof pieces and `_Low` sidewalks.
`hidden_visual_mesh` now hides any placement with `source_hidden` unless its
source is an `InterpActor`. A mover's flag is only its saved state because
Kismet or Matinee can toggle it, so the 12 hidden-flagged `InterpActor`
placements keep the earlier rules. Collision is unchanged: UE3 `HiddenGame`
does not disable collision, and the host keeps each placement's
`collision_enabled`. `refresh_materials.py` now presumes an unrecorded flag
hidden only for `CollisionCube`, so older manifests keep their earlier result.
`verify_level.py` accepts any mesh for a hidden-flagged static placement.

Verified: the decoded `HiddenGame` on `SMC_288`, and synthetic tests of the
rule and the legacy-manifest default. Re-applying the rule to the local
manifest changes exactly 207 placements from rendered to hidden (204
`Sanctuary_Px`, 2 `Sanctuary_P`, 1 `Sanctuary_Land`) plus one entry that had
no flag (the skeletal centre pillar; it stays rendered). The UE5 reimport
hides 307 placements and passes the saved-scene, collision and UV verifiers.
`verify_level.py`'s exact `BoxLrg` source list now applies only to placements
without a source hidden flag, because two hidden-flagged `Sanctuary_Px`
`BoxLrg` placements are also hidden. A maintainer confirmed in the editor that
the roof textures are gone from the street. Not verified: a matched
original-game capture comparison, and which earlier commit first made the
overlap visible.

