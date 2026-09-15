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
- UE5 render and saved-scene verification results are recorded below after
  import. Original-game matched views remain outstanding.
