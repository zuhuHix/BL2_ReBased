## 2026-09-14: Glacier primary-layer approximation

The inspected `Mat_Glacier` and `Mati_Glacier2x` now have an explicit, narrowly
scoped Material v1 recipe. It requires the exact four-texture resource set,
Texture2D classes, and a finite retained `P_TexScalar_RGMain_BASnow` vector.
It binds GlacierFront_Dif and GlacierFront_Nrm and applies the vector's RG
tiling to UV0. The base uses (1,1); the inspected instance overrides (3,3).
Explicit diffuse parameters, including null, prevent this fallback. Existing
normal parameters are preserved. Unknown family members and changed/ambiguous
texture sets are not covered.

This is **not reconstruction of the original layered shader**. UV0 and the
primary-layer interpretation are recorded assumptions; native static
permutation data is not decoded. Snow blending, reflection and glow are
omitted. PNG alpha is fully opaque in both diffuse sources, so it does not
provide a snow blend mask. Both affected source meshes have two UV sets;
the current OBJ path still carries only UV0. No native binary-layout parser,
offset, or bounds check changed.

The manifest records `surface_approximation` and per-channel `channel_uv`;
the audit reports partial surfaces separately from missing diffuse. Thirty-three
placed sections receive this partial recipe. Remaining opaque diffuse gaps:
14 definitions / 43 sections; fully untextured opaque gaps: 11 / 21. These
counts do not mean the original glacier appearance is complete.

Host import creates TextureCoordinate nodes, and saved-scene verification
checks their channel and tiling. See the
[glacier verification record](docs/verification/GLACIER_PRIMARY_LAYER.md).

