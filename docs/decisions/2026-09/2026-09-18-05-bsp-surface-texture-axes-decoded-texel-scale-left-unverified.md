## 2026-09-18: BSP surface texture axes decoded; texel scale left UNVERIFIED

This narrows the "surface texture-axis fields ... remain UNVERIFIED" line of
the 2026-09-15 BSP entry. `tools/bsp_decode.py` now reads three ints of the
60-byte surface record it already unpacked: `s[2]` as the texture base point
index and `s[4]`, `s[5]` as the texture U/V vector indices, each range-checked
against the pools (an out-of-range value rejects the Model, as every other
reference does). No bounds check, array framing or offset changed; the
identification is of fields that were already inside validated bytes.

Two kinds of evidence, both from the installed game through our own reader,
support the roles. First, in-data invariants on the 119 surfaces the Sanctuary
root Models use: `s[4]` is perpendicular to the surface normal on all 119,
`s[5]` on 115 (the four exceptions are 45-degree slopes carrying the
world-axis default `TU = ±X, TV = -Z`), the two axes are mutually
perpendicular throughout, and no other int slot behaves like an index. Second,
volume-owned Models keep an editor `Polys` export whose FPoly records store
`Base`/`TextureU`/`TextureV` as explicit vectors; `tools/crosscheck_bsp_polys.py`
matches each surface to the FPoly on its plane and finds 15,393 agree, 0
differ across 2392 Models in 161 packages, with negative controls showing no
other slot reproduces those vectors. 892 surfaces have no unique coplanar
FPoly (stale editor vertex lists, stale or reversed normals, duplicate polys)
and are reported, not counted.

One planned invariant was dropped on evidence before implementation: the base
point is *not* on the surface plane for most surfaces (volume brushes keep it
in brush space), and the projection formula does not need it to be.

`tools/prepare_bsp.py` now defaults to `--uv surface_axes`:
`((P - Base) . Axis) / texel_scale`, written with the same V flip as the
static-mesh OBJ writer. The divisor is the part no oracle can see — umodel
exports no BSP and the object dumps print empty node arrays — so
`texel_scale = 128` is a prior matching the earlier placeholder's density,
recorded in the manifest as `texel_scale_status: UNVERIFIED`, adjustable with
`--texel-scale`, and waiting on a matched in-game view of a tiled BSP surface.
`--uv planar` keeps the previous placeholder. `PolyFlags`, `iBrushPoly`, the
shadow-map scale, lighting channels and the Model remainder stay opaque.
Record: docs/verification/BSP_TEXTURE_AXES.md.

