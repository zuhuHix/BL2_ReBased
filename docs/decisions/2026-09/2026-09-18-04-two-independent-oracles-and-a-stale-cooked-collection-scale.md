## 2026-09-18: two independent oracles, and a stale cooked collection scale

Two oracles were run against the existing decode, with nothing copied from
either. umodel (UE Viewer, MIT) is a second reader of the same bytes.
OpenBLCMM's Borderlands 2 dumps are the output of the game's own `obj dump`
console command, so they report what the running engine concluded rather than
what another decoder reads. Both stay on the user's machine; reports go to
ignored `local/`.

`tools/crosscheck_umodel.py` compared umodel's `-list` with our `--exports` on
2006 packages (base plus DLC): 4,750,427 exports, no offset, size or class
disagreement. The 7 name-only differences in 5 packages are umodel-side
normalization — it rewrites names containing a control or non-ASCII byte to
`__name_N__` and trims one trailing space, while our reader keeps the raw
bytes — and are bucketed separately so the exit status reflects byte-range
agreement. `tools/crosscheck_umodel_assets.py` compared a prepared Sanctuary
scene with umodel's glTF/PNG exports: 421 of 462 meshes agree on sections,
triangles, vertex counts, positions and UVs with none disagreeing (41 have no
umodel counterpart), and 275 of 288 textures agree, 271 of them within the
maximum per-channel difference of 1 that DXT decoder rounding produces. The
glTF axis mapping and the UV V flip were recovered by search over the data,
not assumed.

`tools/crosscheck_blcmm_dumps.py` compared the same scene with the game's
dumps. All 15 `TerrainComponent`s agree on section base and size, and mapping
our decoded vertices through the reported `_LocalToWorld` reproduces the
reported `Bounds` to 0.003 cm on origin and to the constant one-unit extent
expansion — corroborating the height convention and cell scale against the
engine. All 24 `ModelComponent`s agree on node count, element count and owning
`Model`; the dumps print empty `Nodes(N)=`/`Elements(N)=` values, so contents,
BSP UVs and `PolyFlags` remain untestable and `UNVERIFIED`. 4209 actor
placements reproduce the engine's `_LocalToWorld` within 1e-3 on rotation and
0.05 cm on translation, which confirms the rotator decode; 35 `InterpActor`
placements are reported as movers rather than disagreements, because a dump
shows where a matinee-driven actor had moved to. 353 material texture picks
match the parameters the game reports and none are contradicted, but 348
channels — concentrated in emissive (197) and normal (149) — have no
corresponding parameter, so the oracle is silent on them; 16 parameter names
are listed as unrecognised rather than guessed at.

That pass found one real defect. Our placement of
`Sanctuary_P … StaticMeshCollectionActor_10.StaticMeshActor_SMC_1802` was
unscaled while the engine reports an X row scaled by 0.97. The component export
carries `Scale3D=(0.97,1,1)` as an ordinary property; the collection actor's
cooked per-entry tail records `(1,1,1)`. Of the 3153 collection children in
`Sanctuary_P`, 2037 declare their own `Scale3D`/`Scale` and the tail agrees with
the property in 2036, so the tail is a cache that is stale in exactly this case.
`prepare_level.py` now prefers the component's own property where it exists and
falls back to the tail otherwise (`collection_scale`). No `src/` layout, bounds
check or terminator check changed in this pass; the only parsing behaviour
change is which of two already-decoded scale sources wins.

Records: docs/verification/UMODEL_CROSSCHECK.md and
docs/verification/BLCMM_DUMP_CROSSCHECK.md.

