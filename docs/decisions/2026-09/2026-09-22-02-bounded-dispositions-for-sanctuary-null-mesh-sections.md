## 2026-09-22: bounded dispositions for Sanctuary null mesh sections

The installed Sanctuary payload assigns material index zero (`None`) to 15
placed sections: six `ResistanceBanner_03` sections, four `Blocking_Cube`
sections, three `VendingIcon` sections and two `SancBuild1_Trim` sections.
`tools/prepare_level.py` now records exact, package-scoped dispositions rather
than treating these as unresolved path lookups. The six banner companion
sections use the observed `Mati_ResistanceBanners_Static` from the sibling
`ResistanceBannerFrame_02` mesh; the two trim sections reuse the observed
`Mati_SancBuild1a_04` slot-0 material, corroborated by `SancBuild1Base_Trim`.
The three vending icons retain a named host neutral fallback. The four
unassigned blocking helpers retain their observed collision body and the
existing render-only hide policy for the exact `InterpActor_19/55/56/57`
placements. `refresh_materials.py` reapplies these rules to older manifests.

These are host-side material/visibility dispositions, not native bindings or
shader reconstruction. `audit_scene_materials.py` reports all 15 as explicit
policy rows with `visual_status=UNVERIFIED` and zero unresolved null sections;
matched original-game screenshots remain required for visual acceptance. See
`docs/verification/SANCTUARY_SECTION_MATERIAL_ASSIGNMENTS.md` and
`tests/material_assignment_test.py`.

