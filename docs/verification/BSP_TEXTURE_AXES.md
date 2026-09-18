# BSP surface texture axes

AI-assisted decode and verification pass, 2026-09-18. Evidence comes from the
user's installed game through our own reader; no game bytes, dump text or
report files enter the repo (reports stay under ignored `local/`). The field
roles follow the long-published Unreal BSP surface model (a base point and two
texture-axis vectors per surface, texture coordinates by projection); the
*positions* of those fields in the 832/46 60-byte surface record were tested
against the data, not asserted from memory.

## What changed

`tools/bsp_decode.py` now dereferences three ints of each 60-byte surface
record that were previously kept only inside the `opaque_surface_records`
digest: `s[2]` as a point-pool index (texture base) and `s[4]`, `s[5]` as
vector-pool indices (texture U and V axes). Each is range-checked and an
out-of-range value rejects the Model. No bounds check was loosened and no new
offset was introduced; the record was already unpacked as `<8i4f3i>` within
validated array framing.

`tools/prepare_bsp.py` writes BSP texture coordinates as

    u = ((P - Base) . TextureU) / texel_scale
    v = ((P - Base) . TextureV) / texel_scale

with `texel_scale = 128` by default (policy `bsp_surface_axes_v1`). The
earlier 128 cm world-planar placeholder remains available as `--uv planar`.

## Evidence for the field roles

### In-data invariants (Sanctuary root Models, 119 used surfaces)

Read-only survey over both persistent-level root Models (`Sanctuary_P`: 107
surfaces, `Sanctuary_Land`: 12), testing every unidentified int slot of the
record:

- `s[4]` is a valid vector index on 119/119 and perpendicular to the surface
  normal on 119/119.
- `s[5]` is a valid vector index on 119/119 and perpendicular to the normal on
  115/119. The four exceptions are 45-degree slopes whose axes are
  `TU = ±X, TV = -Z`: the world-axis default the editor applies to sloped
  faces, which is exactly when an in-plane V axis is not expected.
- `vectors[s[4]] . vectors[s[5]] = 0` on 119/119.
- `s[2]` is a valid point index on 119/119. It lies on the surface plane on
  only 13 of them, which is why "base on plane" was **dropped as an invariant**
  before implementation: volume brushes store their base in brush space (see
  below), and `(P - Base) . Axis` is unaffected by an offset along the normal
  whenever the axis is in-plane.
- Axis magnitudes cluster at 1, 0.5, 0.333, 0.25 and 0.0625 — the reciprocal
  texture scales an editor applies — with a handful of non-uniform pairs.
- No other int slot behaves like a point or vector index: `s[1]` takes two
  values (0 and 3584), `s[6]` runs 0..5, `s[7]` and `s[14]` are 0, `s[13]` is 3
  and `s[12]` decodes as the floats 32, 256 and 65504.

### Editor Polys exports as a second serialization

Cooked packages strip the root Model's `Polys` child to an empty array, but
volume-owned Models keep theirs. An FPoly stores `Base`, `Normal`, `TextureU`
and `TextureV` as explicit vectors beside its vertex list, written by different
engine code than the surface records. `tools/crosscheck_bsp_polys.py` reads
each volume Model's pools with the same array framing, parses its Polys
(twelve floats, a vertex count, the vertices, then a fixed opaque tail whose
size is recovered from the data — 84 bytes on every one of the 2392 Models),
matches each surface to the single FPoly whose vertices lie on the surface's
plane, and compares.

Across every non-`_SF` package of the base game:

| | Count |
| --- | --- |
| Packages with a non-empty Polys export | 161 |
| Volume Models compared | 2392 |
| Surfaces with a unique coplanar FPoly: **agree** | 15,393 |
| **differ** | 0 |
| No unique coplanar FPoly (not evidence either way) | 892 |
| ... of which exactly one FPoly carries identical Base/TextureU/TextureV | 736 |
| ... of which several coplanar FPolys share those identical values | 156 |
| ... of which none does | 0 |
| FPolys whose stored normal disagrees with their vertices | 440 |

Maximum deviation on agreeing surfaces: base point 0.0004 cm, axes 0.0004 —
float32 pool merging, not a decode difference.

Negative controls: for each compared surface the tool also asks whether any
*other* int slot would have reproduced the FPoly's base or an axis. The
constant slots coincide by chance (`s[7]`/`s[14]` = 0 hit point 0 on 4,704;
`s[13]` = 3 hits vector 3 on 4,523) but no other slot reaches the 15,393 of
`s[2]`, `s[4]` and `s[5]`.

Two hazards were found and handled while getting there, and they are the
reason the unmatched bucket exists:

- 440 FPolys carry a stored normal that disagrees with their own vertices by up
  to 184 cm (hand-edited brush faces). The tool therefore matches on the plane
  computed from the vertices, and accepts either normal for facing, because one
  Model in `Grass_Audio`/`Outwash_Audio` also has a poly with a reversed
  winding but a correct stored normal. Before that fix those two showed as
  `differ` against the wrong coplanar neighbour; after it they are
  `no_unique_poly`.
- In several sound-volume Models the FPoly vertex lists are stale against the
  compiled Model by up to 680 cm, so no poly is coplanar with the surface, and
  a few Models carry coplanar duplicate polys. Every one of the 892 unmatched
  surfaces still has an FPoly with the identical base and axes (736 exactly
  one, 156 several duplicates); the tool reports that count for context but
  does not count it as agreement, since matching by the values under test
  would be circular.

## What this does not establish

- **The texel scale.** `(P - Base) . Axis` is a distance in world units; the
  divisor that turns it into texture repeats is not stored in the package.
  umodel does not export BSP and the game's object dumps print `Nodes(N)=`
  empty, so neither oracle can see it. The default of 128 is a prior, chosen
  to match the earlier placeholder's tiling density; it stays `UNVERIFIED`
  until a tiled BSP surface is compared against the original game in a
  matched view. `--texel-scale` exists for that comparison.
- The V flip on OBJ export follows the static-mesh writer in `src/assets.cpp`
  (`1 - v`), for which umodel's glTF established the convention; the same
  flip on BSP is an assumption of consistency, not a separate observation.
- `PolyFlags` (`s[1]`), `iBrushPoly` (`s[6]`), the shadow-map scale float,
  lighting channels, the vertex records' trailing floats and the Model
  remainder remain opaque and `UNVERIFIED`.
- Nothing here is rendering parity. No in-game comparison was made in this pass.

## Automated checks

`tests/bsp_test.py` (8 tests) covers the axis decode, both UV policies with a
custom texel scale, and rejection of every out-of-range texture reference.
`tests/crosscheck_bsp_polys_test.py` (7 tests) covers tail-size recovery,
header and truncation rejection, agree/differ/invalid/ambiguous statuses, the
negative controls, and matching on the vertex plane rather than a stale normal.
Synthetic fixtures only.

## Reproduction

```powershell
python tools/crosscheck_bsp_polys.py --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --output local/bsp/polys-all.json
python tools/prepare_bsp.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 `
  --scene local/sanctuary --collision            # add --uv planar for the placeholder
```

The cross-check exits non-zero on any `differ`, invalid reference or parse
error; `no_unique_poly` is not a disagreement.
