# Map selection and third-map checks — 2026-09-14

This work was performed in checkout `t3code-5be76186`, with UE5.8 and the
installed Steam game. Game-derived files remain in ignored local directories.

## Command-line selection

`tools/viewer.py --game <install> --list` discovers 37 base-game persistent
package names. It supports case-insensitive selection, an interactive numbered
prompt, JSON listing, preparation, verified UE import and reopening a saved map.
Ambiguous packages and mismatched prepared manifests fail before launch.
Four synthetic tests cover those failure paths, discovery and cache selection; CI
runs them alongside the existing level tests. `--include-dlc` discovers all
82 installed map names and enables DLC package/cache lookup in preparation.
Duplicate cache names use a package-local match or fail explicitly. This is
not an in-game menu, and discovery is not a compatibility claim.

## Near-vertical collection regression

The initial Southpaw Factory import saved successfully, but the existing
independent verifier rejected a collection forward axis: expected approximately
`[0.0007302, 0.0002344, 0.9999997]`, observed `[0, 0, 1]`.
An editor diagnostic confirmed the matrix produced pitch `89.956060` degrees;
`set_actor_transform` snapped that pitch to 90 degrees through its quaternion
conversion. Assigning the matrix-derived Euler rotation to the unattached root
component's `relative_rotation` preserved the source axes.

The importer now performs that assignment for collection placements. This is a
host transform fix; no package layout, parser offset or tolerance changed.
The synthetic material fixture now includes an 89.96-degree collection with
negative scale. Import and fresh-process saved verification pass for both
fixture actors, all four material channels, geometry bounds and lighting.
Local evidence: `local/material-smoke/rotation-import.log`, `ue-verify.json`;
the diagnostic is `local/rotation-diagnostic.json`.

## Southpaw Factory

Preparation follows five levels and produces 3,987 placements, 175 reusable
meshes, 185 reusable mesh sections and 145 materials. The initial UE import
created 4,178 section actors. Its 43 issues comprise 20 labelled diffuse
inferences, 13 neutral material fallbacks and 10 unsupported component owners.
Corrected import and fresh-process saved verification pass for all 4,178
section actors, geometry bounds, four material channels and lighting. The UE
host rebuild, including runtime diagnostics, succeeds.

`OpenWillow.Viewer` passes: pawn displacement is 4,848.43 cm and the camera
follows movement and control rotation. Its 90-frame movement sample records
mean 46.83 ms, p95 48.21 ms, process physical memory 3,221.5 MB and the same
peak memory. This short automated run is not a controlled benchmark.

The start and moved screenshots were inspected. Textured geometry renders;
large gaps, black voids and fallback surfaces remain visible. This does not
establish correct room completeness or visual parity with the original game.
The overview screenshot was captured but has not been inspected.

Local evidence: `local/southpawfactory/rotation-import.log`, `ue-import.json`,
`ue-verify.json`, `viewer-test.log`, and
`viewer-78271e245d1f4adea36fa60c5b813833.log`. Screenshots are in the host's
ignored `Saved/Screenshots/WindowsEditor/SouthpawFactory_P_*` files.

## Automated baseline

- Release reader build: passed.
- CTest: `100% tests passed out of 5`.
- Level unit tests: `Ran 13 tests` / `OK` (including winding and DLC lookup regressions).
- Map selector unit tests: `Ran 4 tests` / `OK`.
- Nine code packages: decoded bytes, counts and export fields match.

No matched-viewpoint comparison with Borderlands 2 or physical input check has
been performed in this run. Sky, terrain/BSP, full material translation,
walking collision and broader map compatibility remain open.

The later winding correction and refreshed Southpaw checks are recorded in
[UV/winding verification](UV_WINDING_VERIFICATION.md). It fixes back-face
visibility without changing the scene placement counts above.
