# Cross-check against umodel

AI-assisted verification pass, 2026-09-18. umodel (UE Viewer, build 1590) is run
as an independent oracle on the user's installed game; its output tree is read,
never its code, and nothing was copied from it. See THIRD_PARTY.md. All exports
and reports stay under ignored `local/`.

Agreement here means two independent decoders produce the same answer from the
same bytes. It is evidence that our reader is not inventing structure. It is not
a specification, it does not make either decoder correct, and it says nothing
about how the original game renders any of it.

## Export tables: 4,750,427 exports, 0 byte-range disagreements

`tools/crosscheck_umodel.py` runs umodel's `-list` and our `--exports` on the
same package and compares, per export, the index, the serial offset, the serial
size, the class short name and the object name.

- 2006 packages: 914 base `CookedPCConsole` packages plus 1096 DLC packages.
- 4,750,427 exports compared.
- No offset, size or class disagreement anywhere.
- 7 name-only differences in 5 packages, all of them umodel-side normalization.

The 7 are:

| Package | Our bytes | umodel |
| --- | --- | --- |
| `GD_BTech_Streaming_SF`, `GD_Runner_Streaming_SF`, `TundraTrain_Dynamic` | `Part_Bandit\x1f_Technical_EngineFire` | `__name_N__` |
| `Startup` | `VO_Ep1_Pt3_09d_live_Jack ` (trailing space) | `VO_Ep1_Pt3_09d_live_Jack` |
| `Anemone_Startup_SF` (x3) | `Set_KillLtHoffman\x9c`, `SET_PickUpFirstMag\x9c`, `SET_SickAssJump\x9c` | `__name_N__` |

umodel rewrites a name containing a control or non-ASCII byte to `__name_N__`
and trims a single trailing space; our reader keeps the raw bytes. The tool
classifies those into a separate `name_normalized` bucket so the exit status
reflects byte-range agreement, which is the thing being tested. A name that
differs without a control byte or trailing space is still reported as a
mismatch — `tests/crosscheck_umodel_test.py` covers that distinction.

## Meshes, textures and materials

`tools/crosscheck_umodel_assets.py` compares a prepared Sanctuary scene against
umodel's `-export -gltf -png` output of the same packages.

### Geometry: 421 of 462 meshes agree, 0 disagree

Section count, triangles per section, referenced vertex count, vertex positions
and texture coordinates all match on 421 meshes. The remaining 41 have no
oracle: umodel does not export terrain components, BSP `ModelComponent`s or two
cross-package meshes as `StaticMesh`. Those are covered by the
[dump cross-check](BLCMM_DUMP_CROSSCHECK.md) instead.

The axis mapping was recovered from the data rather than assumed: searching all
48 permutation/sign combinations against per-mesh bounding boxes selects
permutation `[0,2,1]` with signs `[1,1,1]` and a factor of 100, i.e.
`ue[i] = gltf[perm[i]] * 100`, with a summed bounding-box error of 0.052 cm
across all meshes. Positions are then compared cell-wise at 0.01 cm, widened to
0.1 cm for skybox-sized meshes where float32 metres no longer carry that
precision beyond about 200 m.

UVs are exact on 493 of 493 sections after a V flip, and on 15 without one. That
establishes umodel's V convention relative to our OBJ output as an observation,
not an assumption.

### Textures: 275 of 288 compared, none contradicted

4 decode bit-identically. 271 agree within a maximum per-channel difference of
exactly 1, which is DXT decoder rounding rather than a decode disagreement; the
tool reports that as a distinct `agree_within_1` status instead of folding it
into either "same" or "different". 13 have no umodel counterpart.

### Material channel picks: informational

umodel's `.mat` summaries are a weaker oracle than the geometry, because umodel
classifies fewer channels than we do and falls back to `dummy_material_N` when
the material package was not loaded. Of 341 materials with an oracle: 350
channels agree, 399 are ours-only (mostly normal 204 and emissive 194, channels
umodel does not classify), 40 umodel-only, 2 where our pick appears in umodel's
unclassified list, and 5 differ.

The 5 differing picks are recorded rather than resolved: `Mat_SancBuild1` and
`Mat_SancBuild2` (umodel reports `MissingTexture`), `Mati_SancBuild4a` (umodel
picks an HLS atlas), `Mat_SnowBoulder` (umodel picks `Blood_Spatter_Tex`) and
`Mati_Moon` (umodel picks `MoonBase02_GRP`). The game's own object dumps confirm
353 of our channel picks and contradict none, which is the stronger evidence;
these five remain an open question about umodel's heuristic, our heuristic, or
both.

## Not established

Neither tool says anything about rendering. Shader graphs, terrain blending,
lightmaps, BSP UVs, `PolyFlags`, sky behaviour and in-game visual parity are all
outside this pass.

## Reproduction

```powershell
python tools/crosscheck_umodel.py --umodel $umodel --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --dlc --output local/umodel/crosscheck-all.json

python tools/crosscheck_umodel_assets.py --scene local/sanctuary/scene.json `
  --umodel-exports local/umodel --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --output local/umodel/crosscheck-assets.json
```

Both exit non-zero on a real disagreement; `name_normalized`, `oracle_missing`
and `agree_within_1` are not disagreements.
