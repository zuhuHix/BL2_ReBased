# Sanctuary lower surfaces and HLS fallback

AI-assisted source-data investigation, 2026-09-15. Evidence comes from the
user's installed game through the existing reader; no binary layouts changed.
Game-derived reports and textures remain under ignored `local/`.

## Classification

| Surface | Evidence | Treatment |
| --- | --- | --- |
| Lower IcePlate | Four placements in Sanctuary_Land; StaticMesh with 322 vertices, 560 triangles, a recovered 16-vertex convex hull and an ice physical material | Keep visible and retain collision. Use observed FrozenLake color and Ice_Nrm textures as an explicit approximation. |
| WorldTransition | Two BasePlane_256x128 placements, in Outer and Land; translucent Unlit material, stripped opacity connection, ExitPlane preview mesh and transition enable parameters | Retain the existing exact mesh/material hiding rule. Native transition visuals remain unsupported. |
| Missing ground/floors | Eight Terrain actors, 15 TerrainComponents and 24 ModelComponents across P and Land are omitted by the static-mesh pipeline | Separate geometry work. Their spatial coverage and how many holes they fill remain unverified. Model counts also include volume shapes. |
| Gate road | One SanctuaryRoad_01 placement in Sanctuary_Land; Mat_IceRoadSanctuary has a stripped DiffuseColor expression, a p_Normal parameter with no texture, concrete physics and seven cooked textures including BrokenRoad_Dif, Snow_Dif, GrasslandsRock_Dif, BrokenRoad_Alpha and noise/splatter overlays | Use the inspected BrokenRoad_Dif as an explicit UV0 color approximation. The alpha-driven snow/rock blend, overlays and native normal are omitted; no cooked normal exists. |
| Green sidewalk/HLS | Optimized Mati_SancBuild4a directly parents Sanc_HLS_Master, with Color and Luminosity textures and separate UV transforms | Keep the same-package regular concrete atlas fallback; validate its parent and exact texture, preserve other supported channels and report the approximation. |

IcePlate placement origins have Z values approximately -3968, -5264, -4798
and -324 cm, with large nonuniform scales. They are authored mesh surfaces,
not generated void-fill planes. Frozen sublevel loading still does not prove
their native activation state.

## Material findings

The generic sole-`_Dif` heuristic selected `Snow_Dif` for Mat_FrozenLake even
though its cooked texture list also contains `FrozenLake`, ice normals, noise
and a reflection texture. The surviving parameters include vertex-paint noise,
UV modulation, reflection scale and glow color. The stripped connections do
not establish the layer blend. The bounded replacement uses the inspected
FrozenLake Texture2D as color and Ice_Nrm as a normal-map approximation on
unchanged UV0. It explicitly omits the snow blend, native normal graph,
reflection, glow and native UV modulation. Missing or ambiguous required
texture references fail to a recorded material gap. Explicit normal overrides,
including null overrides, retain priority.

The sidewalk fallback already existed before this pass. It uses the regular
Mati_SancBuild4a's SancBuild4a_Dif atlas rather than treating the packed HLS
inputs as RGB. This pass validates the exact HLS parent and concrete atlas,
retains supported non-diffuse parameters, and records texture/UV provenance.
It covers three effective sections: the parking-lot sidewalk, broken bridge
and landed bridge. The sidewalk has 139 vertices and 82 triangles; its UV0
extends outside 0..1. Atlas compatibility and native HLS reconstruction remain
unverified; no HLS-to-RGB or UV-transform formula was guessed.

## Reproduction

After preparing Sanctuary, run:

```powershell
python tools/refresh_materials.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --scene local/sanctuary --reuse-textures
python tools/audit_sanctuary_artifacts.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --scene local/sanctuary --output local/artifact-pass.json
```

The audit records effective actor overrides, component identities, transforms,
visibility/collision states, material provenance, source properties and omitted
class counts. It does not infer terrain/BSP geometry from class names.

## Verification

- Release reader and UE5 host builds passed.
- Scene regression tests: 20 passed, including scoped ice selection, HLS
  parent/atlas rejection, retained normal channels and helper visibility.
  Material refresh now reapplies the accepted Unlit dome's two-sided policy;
  a regression test excludes floor-overridden domes.
- CTest: 5/5 passed. Nine installed code packages matched decoded bytes,
  counts and export fields against the research reader.
- Fresh scene: 4,430 placements, 423 meshes and 372 material definitions.
- Refreshed audit: 44 missing diffuse definitions, 12 opaque gaps (42
  sections), nine opaque materials with no channels (20 sections), and four
  explicit surface approximations (40 sections). The later gate-road fallback
  moves one opaque section from the gap list to a fifth explicit approximation;
  the ice-road regression test covers scope, missing-resource failure and the
  omitted normal.
- Terrain footprints from the generic tagged-property decode: the void inside
  the gate lies within Sanctuary_P Terrain_2 (origin -8064, -13568, 0; 384 cm
  per patch; 21x52 patches) by an unverified NumPatches x DrawScale3D extent
  estimate. PersistentLevel.Model_3 BSP may also contribute floors there; both
  formats remain undecoded.
- UE5 reopened all 4,768 section actors; final material verification (including
  normal texture binding and two-sided state) passed with zero errors/warnings.
  Collision verification covered 495 mesh sections and 3,116 enabled components;
  UV verification covered 512,418 corners / 170,806 triangles.
- Final viewer run passed with 90 samples, 49.82 ms mean / 53.44 ms p95 and
  5,172.27 cm pawn movement. `Sanctuary_P_start00001.png` shows blue/teal lower
  ice, with substantial street-level floor gaps still present. This is a
  runtime material check, not original-game parity or a completed walking route.
  Other white areas and floating/opaque artifacts remain in the scene.
- Screenshots are in `host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor/`;
  import/reopen logs, `artifact-pass.json` and the final viewer log are under
  `local/`. Original-game matched views remain outstanding.

## Follow-up (2026-09-15): terrain floors, BSP, HLS, parity, gaps

Status of the five open items after the terrain pass recorded in
[SANCTUARY_TERRAIN_BSP_HANDOFF.md](SANCTUARY_TERRAIN_BSP_HANDOFF.md#gate-status-2026-09-15).

1. **Terrain floor topology: completed through all five gates on the host.**
   Hole/diagonal bits are corroborated by two independent native structures
   (component index strip and bounds-tree leaves) for 15/15 components; the
   scene now carries 15 terrain component meshes (5,195 cells) with labeled
   material approximations and triangle collision. Reopen verification passed
   with zero errors; the runtime walking test stood on 8/8 terrains, found no
   terrain floor in 8/8 flagged hole cells and crossed 1/1 walkable seam (two
   runs). Native face orientation for flipped cells, layer blending and the
   retained tail words remain **UNVERIFIED**; nothing here is original-game
   parity.
2. **Root BSP Model/Polys tails: investigated, not emitted.** Model_3's bulk
   arrays (43 vectors, 360 points bracketing the start area, 216 nodes) were
   read under `local/`; the trailing record layout failed internal checks, so
   no floor exists and no reader route was added. Volume-owned Models stay
   structurally rejected. See the handoff record's root Model findings.
3. **Native HLS-to-RGB: not derivable from the data; fallback kept.** The
   surviving Sanc_HLS_Master expressions are parameters only: Color
   (SancBuilding_Roofs_HS), Luminosity (SancBuilding_Roofs_L), Mask
   (SancBuilding_Masks), Diffuse_Modulation (1, .5, .4, 1), Color/Mask/
   Luminosity UV scale-and-offset vectors, a StaticSwitch `Use_Modulation`
   with null A/B inputs and two StaticComponentMask records that the package
   truncates. No combine formula survives, so none was invented; the
   validated regular-concrete atlas fallback stays.
4. **Original-game matched-view screenshots: not done.** This session cannot
   drive the original game; the comparison still needs someone with both
   builds open (ROADMAP "Matched-viewpoint screenshots").
5. **Street-level floor gaps: rechecked, unchanged by terrain.**
   `Sanctuary_P_start00002.png` (after the terrain import) shows the same
   gap in front of the start camera as `Sanctuary_P_start00001.png`. The
   camera (655, -6349, 2900) lies just outside Terrain_2's 22-column grid
   and Terrain_2's nearby samples sit at Z 500..2730, below street level;
   Terrain_7 begins 4.6 km north at Z 3175+. So no imported terrain covers
   that spot, and Model_3's point cloud (z up to 4288 over x -4864..7008,
   y -6960..4864) remains the only candidate, **UNVERIFIED** until BSP is
   decoded and gated. No further material fallback was added. The 00002
   capture shows flatter, lower-detail surfaces than 00001; the run rebuilt
   DDC textures at load, so this is read as capture timing, not a scene
   change, and was not re-verified.

Automated checks for this follow-up (host build 2026-09-15): CTest 5/5;
`verify_packages.py` matched all nine code packages; `terrain_test.py` 6,
`terrain_geometry_test.py` 7, `terrain_probe_test.py` 2 and `level_test.py`
21 tests passed. In-game: `OpenWillow.TerrainWalking` and `OpenWillow.Viewer`
passed; the viewer reported 90 samples, 136.88 ms mean / 181.48 ms p95 with
the terrain meshes present and 11,616 cm pawn movement. That run overlapped
the CTest suite on the same machine, so the frame time is not comparable
with the earlier 49.82 ms figure; a terrain performance cost is neither
shown nor excluded.

## Continuation (2026-09-15, Codex)

- **A, matched original views: blocked by available controls.** This session
  has no native desktop-control API to operate Borderlands 2. No original
  capture or parity claim was produced. Fresh host captures
  `Sanctuary_P_{start,moved,overview}00003.png` remain under UE Saved. The
  start image was visually inspected: the broad street-floor gap persists,
  with dark sky and overhead geometry artifacts also visible. Source objects
  for those overhead artifacts were not identified in this continuation.
- **B, BSP: advanced, stopped before implementation.** Corrected candidate
  record starts (18720/1332) yield 107/12 matching vector/plane normals, but
  the candidate point association fails 94/107 and 12/12 plane checks.
  Native topology remains **UNVERIFIED**; no floor or reader route was added.
- **C, blending: negative result within the tested mapping.** Of 99
  alpha-array/weightmap pairs across six terrains, no nonconstant array
  matches any fitting integer crop exactly. Two zero arrays match one zero
  texture. This does not resolve filtering/composition or AlphaMapIndex
  semantics; the existing dominant-layer approximation stays.
- **D, hole drift: host cause diagnosed.** Per-frame traces show Land
  Terrain_3's drop starts inside StaticMeshActor_SMC_1281, undergoes roughly
  9.4 m lateral correction with zero horizontal velocity, then slides to
  neighbouring TerrainComponent_5. Total drift is 11.5 m. The unchanged
  hole assertion passes at its endpoint; it is not direct runtime proof at
  the original hole location. See the
  [detailed diagnostic record](SANCTUARY_TERRAIN_BSP_HANDOFF.md#follow-up-diagnostics-2026-09-15-codex).

Validation: UE5 Development build succeeded; CTest 5/5, nine installed package
comparisons and Python terrain/geometry/probe/level suites (6/7/2/21) passed.
Separately, host `OpenWillow.TerrainWalking` passed with the existing
8/8 stand, 8/8 hole endpoint and 1/1 seam counts (five hole endpoints on other
geometry; two seams skipped). Host `OpenWillow.Viewer` passed with 90 samples,
50.85 ms mean / 55.16 ms p95 and 5282.08 cm movement. That viewer run did not
overlap CTest; one sample run does not isolate terrain's performance cost.
Logs: `local/sanctuary/viewer-4d9dca0662e14a4a8eec885561a08b2a.log` (terrain)
and `viewer-12d59ec7962f480bbb5060e8028a1ce6.log` (viewer). These are host
runtime results, not original-game checks. No parsing behavior changed.
