## 2026-09-22: terrain layer weights read from the cooked WeightedMaterials block

`tools/prepare_terrain.py` draws each Sanctuary terrain as a host weighted
sum of its layers (`terrain_weighted_sum_v2`) when, and only when, the
weightmap/layer pairing can be read from the package itself. The terrain
actor's native tail, after the per-layer `AlphaMaps`, carries a
`WeightedMaterials` array (per entry: `Data[grid]` bytes, `SizeX`, `SizeY`,
a reference back to the terrain, a `TerrainMaterial` reference) followed by
a `WeightedTextureMaps` array of `TerrainWeightMapTexture` references, entry
`i` pairing with weight `i`. `grid` is `NumPatches * WeightmapTesselationLevel
+ 1` per axis; that property, not a decoding fault, is why `Terrain_2` and
`Sanctuary_Land:Terrain_3` looked undecodable at level 2. Decoded by
`decode_weighted_materials` in `tools/terrain_decode.py` and bounded like
the alpha-map decoder.

Verification: on all eight installed Sanctuary terrains, every cooked weight
array equals its paired `PF_G8` texture texel for texel (31 of 31), and
every `TerrainMaterial` reference belongs to a layer of that terrain. The
preparer repeats that byte comparison per terrain and falls back to the
single-layer approximation below if it fails. Weight sampling is
vertex-centred bilinear over patch coordinates (`scale = tessellation /
texture size`, `offset = 0.5 / texture size`); each layer's colour tiles by
its `TerrainMaterial`'s diagonal `LocalToMapping`. UE3 stores these weights
already stacked, so the host sums `weight_i * colour_i` without further
normalisation. Slope/noise filters are baked into the stored weights;
lightmaps, decorations and foliage are not reconstructed.

`Sanctuary_Land` layers reference setups and `TerrainMaterial`s imported
from `Sanctuary_P`; the preparer now resolves those through the scene
(`--properties ... --array-schema tools/terrain-arrays.schema`), which is
what made `Sanctuary_Land:Terrain_3` recoverable at all.

Codex's earlier revision of this change gated the weighted graph behind a
local evidence file that was never produced and replaced the previous
fallback with an untextured neutral material, regressing all 15 components
to white. When the cooked pairing cannot be trusted the preparer keeps the
2026-09-15 `terrain_dominant_alpha_layer_v1` approximation, or
`terrain_first_material_layer_v1` when no alpha ranking exists.

Five layer materials resolved with no texture channels because their cooked
lists carry several `_Dif` textures: `Mat_PatchySnow`, `Mat_SolidSnow`,
`Mat_DirtySnow`, `Mat_InterludeSandTracks`, `Mat_ColdGrass`. Their
extracted textures were inspected side by side and entries added to
`INSPECTED_COLOR_FALLBACKS` in `tools/prepare_level.py` naming the texture
that carries the surface colour (plus a native normal where one survives).
These are single-texture stand-ins for masked or macro-tinted blends and are
labeled as such. `tools/refresh_materials.py` now carries host terrain blend
materials across a refresh; re-run `prepare_terrain.py` afterwards.

Not established: visual match against the original game (pending in-game
comparison), the BSP texel scale and V orientation (`tools/calibrate_bsp_uv.py`
has no matched original-game/UE5 measurements; see
[BSP_TEXTURE_CALIBRATION.md](docs/verification/BSP_TEXTURE_CALIBRATION.md)).

