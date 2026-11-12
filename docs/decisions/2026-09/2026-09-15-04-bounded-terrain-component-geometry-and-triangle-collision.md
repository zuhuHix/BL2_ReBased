## 2026-09-15: bounded terrain component geometry and triangle collision

`tools/terrain_decode.py` now walks the remainder of each TerrainComponent
payload after the decoded bounds tree, in Python, without touching the C++
reader: a `(2, N)` u16 record array with an opaque 14-byte stride, an opaque
72-byte block that must end in `(1, own export index)`, an `(8, V)` array of
`<BBHhh>` vertices, two retained words, and a `(2, M)` u16 triangle strip. Every
count is bounded by the payload and by the terrain grid; vertex X/Y/height must
equal the terrain samples; the strip's decoded cells must equal exactly the
non-hole cells with the flag-bit-1 diagonal, and the bounds-tree leaves must
tile the same cells. Any disagreement rejects the component (no scanning, no
retry). The 14-byte records, the 72-byte block, the two int16 vertex words and
the two retained words are kept as opaque data with hashes; their meaning is
**UNVERIFIED**.

What this corroborates: for all 15 installed Sanctuary components the strip
and the leaf tree independently omit the same flag-bit-0 cells and the strip
parity reproduces flag-bit-1 diagonals, so hole and diagonal bits are used as
topology. What it does not establish: the native face orientation for flipped
cells (emitted geometrically, host-visible from above, **UNVERIFIED**), the
meaning of the retained words, and any renderer behaviour.

The Terrain actor tail is additionally probed for `u32 count == len(Layers)`
followed by `count * (u32 vertex_count, bytes)`; 6/8 terrains match. These
arrays are exposed only as a labeled visual approximation
(`terrain_dominant_alpha_layer_v1`: the layer with the largest mean alpha,
indexed by AlphaMapIndex); they do not reproduce the PF_G8 weightmaps and the
native blend is **UNVERIFIED**. Terrains without them use a neutral constant.

`tools/prepare_terrain.py` emits one static mesh per component and, with
`--collision`, marks it `triangle_mesh` (host complex-as-simple, never mixed
with hulls). The saved scene reopened with zero verification errors and the
new `OpenWillow.TerrainWalking` runtime test stood on all 8 terrains, found no
floor in 8 flagged hole cells and crossed the one walkable seam; this is host
behaviour on the decoded topology, not original-game parity. No `src/`
bounds check or layout changed.

