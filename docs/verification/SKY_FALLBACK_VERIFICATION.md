# Temporary UE5 sky fallback and scene-gap audit (2026-09-14)

This slice changes only the UE5 host lighting rig and the manifest audit. It
does not change UE3 package parsing, cooked Material serialization, or native
skybox extraction.

## Host fallback

`host/ue5/import_level.py` now creates a labelled
`OpenWillow_SkyAtmosphere` actor beside the existing inspection lights and
registers `OpenWillow_Sun` as atmosphere light index 0. The atmosphere uses
restrained blue-gray scattering values intended to keep the inspection view
visible while native `_Skybox` translation remains open. `ue-import.json` and
`ue-verify.json` identify this as `temporary_sky_fallback: UE5_SkyAtmosphere`.

The fallback is not a claim that Sanctuary's sky texture, sky dome, clouds,
or lighting graph have been recovered. A visible game-window result still
requires a UE5.8 import/reopen and human inspection.

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

The UE5 editor build/import and a visible non-black screenshot are pending in
this checkout because generated host binaries and game-derived local scenes
are not present. The next runtime check should reopen Sanctuary, run saved
verification, then inspect `Sanctuary_P_start00000.png` before treating the
fallback as visually confirmed.
