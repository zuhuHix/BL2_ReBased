## 2026-09-25: Archives floor collision for the Sanctuary walker

The visible `Sanctuary_P` placements of
`Env_Sanctuary.Meshes.RolandsArchivesFloors` (`StaticMeshActor_44` component
228) and `RolandsArchivesCrushRoom` (`StaticMeshActor_222` component 439) have
host collision enabled but neither mesh has an `RB_BodySetup`. The first has
three render sections with horizontal triangles at z 3680 and 4256; the
adjoining room has floor triangles at z 4256. Their actor placement translates
Y by -384. The separate `Sanctuary_Px` copies of the main floor remain
source-hidden with collision disabled.

For these two exact mesh identities, `prepare_level.py` now records a
`triangle_mesh` fallback when the source body is absent. UE5 uses each
section's render triangles as complex collision and retains each placement's
source collision switch. This is a host walking approximation, not a claim
about the original game's blocking volumes or collision parity. UModel build
1590 is available at `%USERPROFILE%/Tools/UEViewer/umodel.exe` (SHA-256
`13502E5A4D8F6B5F32252AFEBD6360F7302CCFACCF6B8DDA65BEFF0BE2D364A0`),
but this collision decision uses the project's package identities and local
render geometry rather than an external collision export.

Fresh Sanctuary static-mesh, terrain and BSP preparation completed: 4,430
base placements, eight terrains with 15 components, and 228 root BSP polygons.
The previous local scene included the baked centre pillar. Its preparation
step could not rerun because this Python 3.14 installation lacks NumPy, so
the prior ignored scene was restored with only the two freshly prepared
collision records inserted; a comparison found those were the only changed
shared mesh records. UE5 reimport and saved scene, collision and UV checks
passed. The collision check reports 619 mesh sections, 3,246 enabled
components, 131 enabled triangle components and zero errors. A live
`-owwalk -owmaya -owspawnprobe` drop at (12357.3, 857.3, 4456) settled at
z 4338.15, `grounded=1`, directly on `StaticMeshActor_44` component 228.
Two adjoining-room drops grounded on overlapping props, so direct runtime
ground contact on the CrushRoom triangles and a walked route through the
building remain unverified. CTest passed 6/6, `tests/level_test.py` passed
31/31 and installed-package verification matched all nine code packages.

