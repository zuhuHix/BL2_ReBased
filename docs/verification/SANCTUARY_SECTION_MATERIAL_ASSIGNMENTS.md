# Sanctuary null-section material dispositions

This record covers the 15 placed sections that had no effective material in
the frozen Sanctuary manifest. The source observations came from the bounded
`ow-package --mesh` reports and the placed component identities in the scene
manifest. No original-game screenshot was used, and no native shader graph is
claimed recovered here.

## Source observations

| Placed family | Source mesh section | Native observation | Bounded disposition | Placed sections |
| --- | ---: | --- | --- | ---: |
| Resistance banners | `ResistanceBanner_03`, slot 1 | `material_index=0` / `None`; eight triangles form the two end strips. `ResistanceBannerFrame_02` observes `Mati_ResistanceBanners_Static`. | Bind `Mati_ResistanceBanners_Static` as an explicit companion-material approximation. | 6 |
| Building trim | `SancBuild1_Trim`, slot 1 | `material_index=0` / `None`; slot 0 observes `Mati_SancBuild1a_04`. `SancBuild1Base_Trim` uses that material for its only render section. | Reuse the same mesh's observed slot-0 material as an explicit approximation. | 2 |
| Vending icons | `VendingIcon`, slot 0 | `material_index=0` / `None`; 32-triangle icon has no observed mesh material. | Preserve the null source binding and record the existing host `M_OpenWillowNeutralFallback` (`RGB 0.5`). | 3 |
| Blocking helpers | `Blocking_Cube`, slot 0 | `material_index=0` / `None`; `RB_BodySetup_1` is observed. The four unassigned placements are `Sanctuary_Dynamic` `InterpActor_19`, `55`, `56`, and `57`. | Preserve null visual binding, retain observed collision, and apply the existing exact collision-helper hide policy. | 4 |

The banner and trim rules are exact Sanctuary package/object matches. They do
not select a nearest material for other maps or meshes. Every disposition is
stored in the section's `material_policy` with `native_material_index=0` and
status `partial_unverified`, `explicit_host_fallback`, or
`observed_helper_policy` as appropriate.

## Manifest and audit behavior

`tools/prepare_level.py` applies the policies immediately after reading mesh
sections. `tools/refresh_materials.py` reapplies them while refreshing an old
manifest, including loading the banner companion material when needed. The
read-only material audit reports `resolved_policy_sections=15`, the four
method counts (`observed_companion_material_v1=6`,
`same_mesh_observed_material_v1=2`, `host_neutral_fallback_v1=3`,
`helper_hidden_collision_only_v1=4`), `native_null_sections=33` and
`unassigned_placed_sections=0` for a fresh Sanctuary scene.

`native_null_sections` counts every *placed* section whose source mesh section
has material index zero, so it is deliberately larger than the 15 dispositions
above. The other 18 are placements where the actor's own material array
supplies the binding and no host policy is needed: `Blocking_Plane` (7),
`SkyboxMountains_low` (5), `WaterPlaneVertex300` (2), and one each of
`SanctuarySidewalk_ParkingLot_Low`, `Blocking_Cube` (the fifth placement,
assigned unlike the four helpers above), `SantuaryDome_Smesh` and
`SanctuaryRoad_01_Low`.

The section rows still carry `visual_status=UNVERIFIED`; this is assignment
coverage, not original-game visual parity.

## Verification

```powershell
python tests/material_assignment_test.py
python tests/material_audit_test.py
python tools/audit_scene_materials.py --scene local/sanctuary --output local/sanctuary/material-audit.json
```

The generated scene and audit output remain under ignored `local/`. Matched
original-game screenshots are still required for visual acceptance.
