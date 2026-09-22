# Sanctuary terrain/BSP evidence refresh, 2026-09-22

AI-assisted checkpoint and host verification. Existing work was committed as
`c740cc2`; this pass changes calibration fixtures and documentation, not
production parsing or geometry. Game-derived evidence remains ignored under
`local/`. Original-game screenshots and item 6 calibration were explicitly
deferred by the maintainer until a later comparison pass.

## Automated checks

CTest passed 6/6 and installed-package verification matched all nine packages.
Python suites passed: terrain 6, terrain geometry 7, BSP 8, weightmap 3,
terrain material 7, material audit 2, and BSP calibration 6.

The calibration suite initially failed four of five tests. Its fixtures and
documented example still supplied a single `pixel` field, while the tool
already required `pixel_original_game` and `pixel_ue5` for each anchor.
Fixtures and example now match that requirement. An added regression rejects
missing per-view anchors and legacy single-pixel inputs. Production acceptance
was not loosened. Synthetic acceptance does not calibrate the installed map.

The current scene audit preserves 8 terrains, 15 components, 5,195 cells,
10,390 terrain triangles, and 228 root-Model BSP polygons / 571 triangles /
105 sections. The scene records 31 decoded weightmaps, eight weighted
materials and zero dominant-layer fallbacks. This audit did not repeat the
raw cooked-byte comparison. The 15 unassigned prop sections remain 6 banners,
4 blocking cubes, 3 vending icons and 2 building-trim sections; none is terrain
or BSP. Full identities are in `local/evidence/current-audit.json`.

## Fresh host runtime

The existing imported map was tested without regeneration or reimport.
`terrain-runtime.json` and `bsp-runtime.json` are prepared probe inputs, not
runtime result files; freshness is established by the new harness logs.

| Harness | Result | Log under local/sanctuary |
| --- | --- | --- |
| TerrainWalking | Success; stand 8/8, direct holes 3/8, seam 1/1 | viewer-b120535e0ce14dcd92f65830b7b61cce.log |
| BspWalking | Success; stood and walked 2/2, rejected candidates 0 | viewer-529b617e5e9a4d5d985c588ecbc2378e.log |
| Inspection | Success; seven camera views captured | viewer-2c5a8574c11c47fd8a25f207ee2e806f.log |

Terrain endpoint assertions pass 8/8, but five endpoints land on other
geometry, five original probes are obstructed and two endpoints are displaced.
Three original probes report penetration; two seams are skipped. These
limitations remain open and do not prove all original holes.

## Visual review

All seven fresh images were inspected and preserved under
`local/evidence/current-screenshots/`, with suffix `00001.png`. The indices
below refer to the existing `inspection-views.json` camera order, not to a
pixel-level source-object identification.

| View | Observation | Acceptance limit |
| --- | --- | --- |
| 00, spawn | Large opening below Scooter's street exposes lower rock/ice; a patterned box is visible at lower right | Missing-surface cause and box identity unresolved |
| 01, Terrain_10 camera | Nearby textured geometry fills most of the image | Obstructed view cannot establish terrain blend quality |
| 02, persistent Terrain_3 camera | Road shows cracked/dirt texture transitions | Host blend appearance observed; native match unverified |
| 03, Land Terrain_3 camera | Textured cracked roadway visible | Does not identify or validate the original hole |
| 04, Terrain_8 camera | Snow surface visible, with a bright yellow surface at the upper edge | Bright surface identity and native appearance unresolved |
| 05, persistent Model_3 camera | Textured floor visible around vehicles and workshop props | BSP repeat density and orientation remain unverified |
| 06, Land Model_4 camera | Beams, a cylindrical support and lower ice visible | Camera alone does not isolate the intended BSP surface |

This is a fresh visual review with unresolved findings, not visual acceptance.
The exact four named classification buckets from the frozen DoD were not
found in the repository records, so this pass does not claim that classification
gate is complete. Source-object attribution is still needed for the unresolved
artifacts. `scene.json` retains `visual_validation: pending`.

## Deferred original-game evidence

`local/evidence/calibration-current.json` inventories the existing BSP scene
but considers zero measurements. Scale and V orientation remain `UNVERIFIED`.
The maintainer will supply original-game versus UE5 screenshots later; neither
item 6 nor matched original-game parity is claimed complete by this refresh.
