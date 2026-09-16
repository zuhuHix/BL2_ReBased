# Sanctuary terrain/BSP diagnostic handoff (2026-09-15)

AI-assisted, original diagnostics against the user's installed packages in
checkout `t3code-85ec0714`. No external implementation, decompiled executable,
or recovered engine source was consulted. Game-derived diagnostics remain in
ignored `local/`. The initial prefix diagnostic now feeds a bounded terrain
floor importer; root BSP recovery remains incomplete.

## Implemented scope

`tools/terrain_decode.py` consumes existing reader JSON and payload output.
`decode_terrain(payload, data)` returns grid dimensions, uint16 samples, raw
byte flags, actor transform, layer/component properties, and opaque remainder
offset/length/SHA-256. `decode_component(payload, data)` returns the observed
37-byte bounds-tree prefix and opaque remainder metadata. Neither emits meshes,
enables collision, nor assigns hole or triangle-flip semantics.

The separate `--terrain-records` route in the package CLI applies these property
prefixes: Terrain 26 bytes; TerrainComponent/ModelComponent/BrushComponent 8;
TerrainLayerSetup/TerrainMaterial/TerrainWeightMapTexture/Model/Polys 4.
This is scoped independently of the normal scene-records material schema.

`tools/terrain-arrays.schema` contains exactly:

```text
Layers=StructProperty:TerrainLayer
TerrainComponents=ObjectProperty
Materials=StructProperty:TerrainFilteredMaterial
```

The two struct labels select generic tagged-property decoding. Their labels
are diagnostic names, not evidence of native C++ type identity. Actual nested
tag streams and terminators validate under the existing bounded decoder.
All 13 TerrainLayerSetup property streams consume exactly. The normal schema's
`Materials=ObjectProperty` must not be reused: an observed one-element setup
Materials array occupies 745 bytes and is a tagged struct array, not an object
reference array. No existing bounds check was relaxed.

Layers expose observed Name, Setup, AlphaMapIndex, Hidden and editor highlight
properties. Setup Materials expose material references and tagged noise/height/
slope filter properties. TerrainMaterial exposes Material, LocalToMapping and
mapping properties. Their values are retained; native layer weighting, filtering,
UV behavior and material graph semantics remain **UNVERIFIED**.

## Observed actor prefixes

Every native Terrain tail starts exactly at property_offset + consumed_bytes:
u32 N, N uint16 samples, u32 N, N bytes. Both counts equal the patch-grid vertex
count. Remaining bytes are deliberately opaque and reported, not silently
accepted as a fully decoded object.

| Package / actor | Export | Patches | Samples |
|---|---:|---:|---:|
| Sanctuary_P / Terrain_0 | 13596 | 16 x 18 | 323 |
| Sanctuary_P / Terrain_10 | 13597 | 20 x 25 | 546 |
| Sanctuary_P / Terrain_2 | 13598 | 21 x 52 | 1166 |
| Sanctuary_P / Terrain_3 | 13599 | 50 x 55 | 2856 |
| Sanctuary_P / Terrain_6 | 13600 | 36 x 40 | 1517 |
| Sanctuary_P / Terrain_7 | 13601 | 36 x 40 | 1517 |
| Sanctuary_Land / Terrain_3 | 2154 | 32 x 26 | 891 |
| Sanctuary_Land / Terrain_8 | 2155 | 27 x 32 | 924 |

Root-agent independent bounds diagnostics corroborate row-major +X/+Y samples
and local Z = (sample - 32768) / 128, followed by actor scale and rotation.
All 15 component bounds match corresponding sample-grid extents padded by
1/64 local units before transformation. Reported discrepancies are below
0.005 world units, including the four rotated Terrain_8 components.
This is geometric corroboration, not proof of native triangle topology.
Evidence: `local/terrain-root/bounds-proof.json`, `local/terrain_bounds.py`,
`local/terrain_rotation.py`.

## Component bounds prefix

The component native prefix is u32 node count followed by count * 37 bytes:
six float bounds, byte 1, u32 branch/leaf value 0 or 1, four uint16 words.
Observed branches contain forward child indices or 65535 for absent slots;
leaves contain grid base X/Y and size X/Y. Bounds and counts decode for all 15
components. The diagnostic validates finite ordered bounds, fixed valid byte,
branch/leaf range, forward/in-range non-sentinel child references and nonzero
leaf dimensions. Full tree coverage, native collision buffers and the remainder
are **UNVERIFIED**. A decoded tree does not enable or prove walking collision.

## First visible slice and acceptance gates

Start with `Sanctuary_P:Terrain_10` (13597), sole component 13604:
20 x 25 patches, actor location approximately (9496.35, 3566.93, 3606.59),
scale (128,128,256), 546 samples in 32752..32776. Its native tail begins at
byte 1819 and contains 8222 bytes. Five owned PF_G8 weightmaps, exports
13635..13639, are 24 x 28. This near-city grid is a smaller alignment target
than remote Terrain_0, whose DrawScale additionally multiplies by eight.

An isolated render-only diagnostic may display the recovered height grid with
fixed diagonals and raw-flag colors, but must label holes/diagonals UNVERIFIED
and keep collision disabled. It must not replace the main Sanctuary scene or
be reported as native floor recovery. Before production floor output:

1. Corroborate hole and triangle-diagonal bits against native data or runtime
   observations; do not turn every cell into a solid floor by assumption.
2. Validate component subgrids, transforms, section coverage and winding.
3. Preserve source layer/setup/material identities and raw weightmaps; explicitly
   label conservative material approximations rather than invent graph behavior.
4. Validate saved-scene reopen and visible alignment at matched camera positions.
5. Enable collision only after topology is supported; verify standing, walking,
   edges/holes and crossing component seams in runtime, separately from tests.

## Gate status (2026-09-15)

All five gates were exercised on the installed packages; the evidence is
recorded here so that later readers can tell what is corroborated from what is
inferred.

1. **Hole/diagonal bits: corroborated from native data.** Each component's
   remaining tail (after the decoded bounds tree) was walked with bounded
   counts: `(2, N)` u16 records with an opaque 14-byte stride, an opaque
   72-byte block ending in `(1, own export index)`, `(8, V)` vertices of
   `<BBHhh>` (grid X, Y, height sample, two retained int16), two retained
   words, then a `(2, M)` u16 triangle strip with degenerate restarts. For all
   15 components the vertex X/Y/height values equal the terrain samples, the
   strip decodes to exactly the non-hole cells (flag bit 0 clear) with the
   diagonal chosen by flag bit 1, and the bounds-tree leaves tile the same
   cells. Two independent native structures therefore agree with the bit
   reading. The 14-byte records, the 72-byte block and the retained words are
   kept opaque with hashes; meaning **UNVERIFIED**.
2. **Subgrids/transforms/coverage/winding: validated structurally.** Component
   sections tile each terrain's patch grid exactly; actor location/yaw/scale
   place the vertices; the emitted OBJ bounds match the host mesh bounds.
   Winding is chosen geometrically (visible from above, as the terrain probe
   did). The native strip's parity orientation differs for flipped cells, so
   native face orientation stays **UNVERIFIED**.
3. **Identities preserved, approximation labeled.** Layer names, setups,
   TerrainMaterials, MappingScale/LocalToMapping and the PF_G8 weightmap paths
   are retained in the scene's `terrain` block. The visible material is a
   labeled approximation (`terrain_dominant_alpha_layer_v1`, largest mean
   per-vertex alpha by AlphaMapIndex where the actor-tail alpha arrays decode
   exactly; otherwise `neutral_constant`). Those arrays do not reproduce the
   weightmaps; native blending is **UNVERIFIED**. Chosen layers: Terrain_0
   Rocky (Mat_PatchySnow, no channels), Terrain_10/3/6/7 SandTransition
   (Mat_DesertRockTransition, sole cooked `_Dif`), Terrain_2 and Land
   Terrain_3 neutral, Land Terrain_8 solid (Mat_SolidSnow, no channels).
4. **Saved-scene reopen: passed.** 4,783 section actors reopened; collision
   verification 510 sections / 3,131 enabled / 15 triangle-mesh; UV
   verification 510 sections, 543,588 corners, 181,196 triangles; zero
   errors. Visible alignment was checked only against the host's own mesh
   bounds, not against the original game.
5. **Runtime, separately from tests: passed on the host.** With
   `prepare_terrain.py --collision` the 15 component meshes use host
   complex-as-simple triangle collision (no hulls). `test_ue_viewer.ps1
   -Terrain` runs `OpenWillow.TerrainWalking` from `terrain-runtime.json`
   (two consecutive runs, same result):
   `stand=8/8 occluded_stands=0 hole=8/8 hole_on_other=5 seam=1/1
   skipped_seams=2`. Terrain_2's first stand candidate was covered by a
   building floor (`StaticMeshActor_SMC_117`, 6 m above the cell); the
   second candidate stood on `TerrainComponent_12`. Five hole probes came to
   rest on unrelated static meshes above the hole cell, which only shows the
   terrain carried nothing there; two fell freely; the Land Terrain_3 probe
   ended 11.5 m from its drop point on `TerrainComponent_5`, 65 cm above the
   hole cell's highest corner. Its path was not recorded, so the cause of that
   drift (sliding along neighbouring slopes or other geometry) is
   **UNVERIFIED**. Seams on Terrain_0 (45 m) and Land Terrain_8 (6 m) were
   skipped as unwalkable by height; Land Terrain_3's seam was crossed from
   component 4 to 6.

None of this is original-game parity: the reference is the decoded topology,
and the host's own collision confirms it behaves as decoded.

## Rejection contract and tests

Reject wrong property prefix, invalid consumption or payload extent mismatch,
missing/nonpositive dimensions, more than 4,000,000 samples, unequal array
counts, truncation at either array, nonfinite transforms, invalid/truncated node
counts, nonfinite or inverted bounds, unknown tree flags, invalid child indices,
and zero-size leaves. No offset scanning or retry at alternate positions.
Raw terrain byte flags are preserved even when unfamiliar because their meaning
is not interpreted. Opaque tails retain explicit UNVERIFIED status and hashes.

Synthetic tests cover every truncation position inside the array/tree prefixes,
count mismatches, strict property start/extent, sample budget, nonfinite values,
sparse branches and invalid node flags/references. They use no game-derived bytes.

```powershell
python tests/terrain_test.py
python tools/terrain_decode.py --reader build/Release/ow-package.exe --game "$env:OPENWILLOW_BL2" --output local/terrain-diagnostic
```

```powershell
python tests/terrain_geometry_test.py
python tools/prepare_terrain.py --reader build/Release/ow-package.exe --game "$env:OPENWILLOW_BL2" --scene local/sanctuary --collision
./tools/test_ue_viewer.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $env:OPENWILLOW_BL2 -Scene local/sanctuary -Terrain
```

`terrain_geometry_test.py` (7 tests) covers the synthetic vertex/strip walk:
self-reference, record count, vertex height, covered hole, wrong diagonal,
missing cell and cell-spanning index rejections, truncation at every byte,
leaf-coverage rejections, OBJ winding/UV, spread runtime stand candidates and the axis-aligned mapping scale.

Current results: 6 + 7 synthetic tests pass; both installed packages produce
zero diagnostic errors, with 8 terrain prefixes, 15 component prefixes and
15 component geometries (5,195 cells, 10,390 triangles, none rejected) decoded.
Output: `local/terrain-diagnostic/terrain_diagnostics.json`. Earlier explicit
property diagnostics: `local/terrain-research/`, reproduced by the ignored
`local/terrain_research.py` and `local/terrain_tails.py` scripts. Full repository
CTest/package checks are recorded by the integrating root task.

## BSP limitation and next investigation

| Package | Root Model export | Native tail bytes | Root Polys export | Native tail bytes |
|---|---:|---:|---:|---:|
| Sanctuary_P | 2921 (Model_3) | 148216 | 8222 (Polys_7) | 12 |
| Sanctuary_Land | 443 (Model_4) | 6500 | 600 (Polys_76) | 12 |

Root Models and 18/6 ModelComponents are directly owned by PersistentLevel
(1842 in P, 201 in Land). Volume-owned Models have a different owner chain;
their Polys contain larger native tails and must not become visible floor meshes.
The root Polys' 12-byte tails do not establish surviving editable polygons.
Investigate cooked root Model render buffers separately, preserving material
assignments and rejecting volume ownership structurally. No BSP native reader
or visual/collision recovery is implemented by this diagnostic.

### Root Model findings (2026-09-15, local only)

Sanctuary_P Model_3's 148,216-byte tail was inspected under ignored `local/`
without adding a reader route: 28 zero bytes, then bulk arrays of 12 x 43 unit
vectors, 12 x 360 points (x -4864..7008, y -6960..4864, z -528..4288, which
brackets the start area and the street-level gap in
`Sanctuary_P_start00002.png`), 64 x 216 nodes (a 4-float plane and 12 words),
then 60-byte-period records at offset 18724 whose fields failed internal
consistency checks (vector indices stay below 43, but a presumed point index
of 3584 exceeds 360). No field meaning is asserted, no floor is emitted, and
volume-owned Models remain rejected structurally by `bsp_scope`. The remaining
bytes and the render-buffer layout are **UNVERIFIED**; a visible BSP floor
would need the same five gates as terrain.

## Follow-up diagnostics (2026-09-15, Codex)

### Hole-probe displacement

`TerrainWalkingTest.cpp` now records every hole-probe frame, including the
initial teleport: position, velocity, last movement input, movement mode,
current floor and a downward trace with source identity, normal and penetration
state. These are diagnostics only; pass criteria are unchanged.

The rebuilt host reproduced Land Terrain_3's displacement in
`local/sanctuary/viewer-4d9dca0662e14a4a8eec885561a08b2a.log`:

- At t=0, the pawn is at (-15936, -29504, 2012), with zero input/velocity.
  The trace starts penetrating `StaticMeshActor_SMC_1281` in
  `StaticMeshCollectionActor_33`.
- At t=0.0466 s, it has moved to (-16441.151, -28709.026, 2347.919),
  approximately 942 cm sideways, while horizontal velocity is still zero.
  The trace still starts penetrating the same mesh. This supports initial
  collision penetration correction, rather than ordinary walking input.
- Subsequent frames follow that mesh's slope, normal approximately
  (-0.505, 0.794, 0.338), with increasing downhill velocity and zero input.
  At t=2.511 s it rests on TerrainComponent_5 at
  (-16553.121, -28532.815, 2015.481), 1151 cm from the drop point.

Thus the observed host path is initial penetration correction followed by
sliding on unrelated geometry and landing on neighbouring terrain. The exact
internal movement-solver corrections were not instrumented. The existing
8/8 hole endpoint assertions still pass, but this displaced probe does **not**
directly verify the original hole location at runtime. The decoded native
strip/tree evidence for omitted hole cells is separate and unchanged.
Host summary: stand=8/8, occluded_stands=0, hole=8/8, hole_on_other=5,
seam=1/1, skipped_seams=2. This is not original-game verification.

### Root Model record alignment

The local diagnostic `local/check_bsp_records.py` advances the rejected
record hypothesis without adding a reader route. After the three bulk arrays,
both Models contain their own export index and a count: 2921/107 and 443/12.
The candidate 60-byte records therefore begin at native-tail offsets 18720
and 1332, respectively, four bytes before the earlier Model_3 hypothesis.
For 107/107 and 12/12 records, a candidate vector reference matches the
record's plane normal within 0.001. However, the candidate point association
fails the plane equation by more than 0.1 world units in 94/107 and 12/12
records. This rejects that association; it does not establish topology,
transforms, material assignments or render buffers. No BSP floor is emitted.
Evidence: `local/bsp-record-followup.json`. Remaining field meanings and
native topology are **UNVERIFIED**.

### Terrain alpha/weightmap comparison

`local/check_layer_correspondence.py` compares each decoded alpha array to
each owned PF_G8 weightmap at every integer crop offset that fits the terrain
vertex grid. Across the six terrains with alpha arrays, 99 array/texture pairs
were checked. Only two pairs match exactly: two constant-zero Terrain_10
arrays match the same zero texture. No nonconstant exact match was found.
The other two terrains still lack a decoded alpha-array sequence. Evidence:
`local/terrain-weightmaps/layer-correspondence.json`.

This rules out direct equality under the tested crop mapping, not resampling,
filtering, reordered axes or native composition. AlphaMapIndex and setup
identities alone do not establish those operations. Blending remains
**UNVERIFIED**; `terrain_dominant_alpha_layer_v1` is unchanged.

## Runtime refresh (2026-09-16)

The current prepared/imported Sanctuary scene was rebuilt with terrain and
root-BSP geometry present. Reopen verification passed for 4,888 section actors
from 4,469 source placements, including 15 terrain components, one native sky
dome, 106 explicitly hidden visual placements and 75 unsupported translucent
sections. Collision verification reports 615 mesh sections, 3,236 enabled
components and 120 triangle components; UV verification reports 545,301
corners and 181,767 triangles with matching source winding.

Fresh standalone host checks passed:

- `OpenWillow.TerrainWalking`: `stand=8/8`, `route=4/4`, `hole=4/4`,
  `hole_on_other=1`, `seam=1/1`, `skipped_seams=2`.
- `OpenWillow.BspWalking`: `stood_and_walked=2/2`,
  `rejected_candidates=0`.

The helper policy now carries `OpenWillow_HiddenVisual` and
`OpenWillow_UnsupportedTranslucent` tags and reasserts visibility at standalone
restart while retaining the saved collision state. The visible start view is
cleaner of the known helper/checkerboard class, but a white rectangular
placement, broad street-level voids and other white terrain/material gaps still
remain. The remaining rectangle was not assigned to the hidden translucent
water actor by the runtime trace, so no additional actor was hidden by guess.
These are host visual findings; original-game comparison and native material
parity remain open.
