# Temporary UE5 sky fallback and scene-gap audit (2026-09-15)

This slice changes only the UE5 host lighting rig and the manifest audit. It
does not change UE3 package parsing, cooked Material serialization, or native
skybox extraction.

## Host fallback

`host/ue5/import_level.py` now creates a labelled
`OpenWillow_SkyAtmosphere` actor beside the existing inspection lights and
registers `OpenWillow_Sun` as atmosphere light index 0. The atmosphere remains
the ambient-light fallback, while a two-sided, non-colliding blue shell named
`OpenWillow_SkyFallback` covers the unresolved visual background.
`ue-import.json` and `ue-verify.json` identify the combined fallback as
`temporary_sky_fallback: UE5_SkyAtmosphere+OpenWillow_SkyFallback`.

The shell uses UE5's engine sphere with a host-created Unlit constant color and
reverse culling because the inspection camera is inside it. It is centered on
the deterministic start position and scaled to 10,000, so the automated
movement sample remains inside the shell. This is a visual inspection aid; it
does not claim recovery of Sanctuary's native sky graph, clouds, time of day,
or lighting.

The fallback is not a claim that Sanctuary's sky texture, sky dome, clouds, or
lighting graph have been recovered. A visible game-window result still
requires a UE5.8 import/reopen and human inspection.

## Runtime evidence

- Fresh preparation still produced 4,430 placements, 423 meshes, 372
  materials, and 178 bounded recovery issues.
- Fresh UE5 import and saved-scene, collision, and UV verification passed with
  4,768 section actors, one native dome placement, and 106 hidden visual
  helpers. The lighting verifier also checked the fallback sphere, material,
  reverse culling, and `NoCollision` profile.
- `OpenWillow.Viewer` passed after that import. The new start capture is
  `host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor/Sanctuary_P_start00029.png`;
  it shows a blue upper background while the recovered city textures remain
  visible. The lower white regions are still the separately observed
  `IcePlate` geometry and are not hidden or reclassified by this slice.

## Audit improvements

`tools/audit_scene_materials.py` remains read-only and now reports:

- `gap_status_counts`: partial channels, cooked-resource candidates,
  non-opaque gaps and no-supported-channel fallbacks;
- `issue_counts` and bounded `issue_examples`, grouped into unsupported
  component owners, invalid color streams, collision, approximations,
  material fallbacks and other issues;
- a `status` field on each diffuse gap; and
- `--all-gaps` to print non-opaque gaps in addition to the default opaque
  priority list.

The report still follows effective actor material overrides and keeps
unassigned placed sections explicit. Counts describe manifest coverage, not
visual fidelity or shader reconstruction.

## Checks

- `python -m py_compile host/ue5/import_level.py host/ue5/verify_level.py tools/audit_scene_materials.py tests/material_audit_test.py`
- `python tests/material_audit_test.py`
- `python tests/level_test.py`

The UE5.8 commandlet import, saved-scene verification, collision verification,
UV verification, and viewer harness all passed in this checkout. The latest
fresh captures are `Sanctuary_P_start00031.png`,
`Sanctuary_P_moved00031.png`, and `Sanctuary_P_overview00031.png`; the
post-diagnostic restore import also passed and left no `IcePlate` placements
hidden. The open UE5 game window is the final human-inspection step. The
captures confirm a readable blue upper background and textured city geometry;
they do not establish native sky parity or resolve the separate lower
`IcePlate` surfaces.
