## 2026-09-15: Sanctuary root BSP polygons as a labeled approximation

This supersedes the "keep BSP unimplemented" conclusion above for the two
persistent-level root Models of Sanctuary only. `tools/bsp_decode.py`
consumes the root `Model` native tail as 28 zero bytes, bulk vector, point
and 64-byte node arrays, a self-reference, 60-byte surface and 24-byte vertex
records, and each root `ModelComponent` as material elements with node
membership lists. A polygon is accepted only when its node plane, its
surface plane and its surface normal vector agree, all of its points lie on
that plane within 0.02 cm, it is convex and consistently ordered, and its
component and element memberships back-reference each other and cover every
node exactly once. Unlike the earlier rejected hypothesis, polygon points
come from the node and vertex arrays; the point-like fields of the 60-byte
records are left opaque. Both Sanctuary Models pass every gate (228
polygons, 571 triangles, 24 components, 105 sections).

`tools/prepare_bsp.py` adds those polygons to a frozen Sanctuary scene with
native material assignments, a 128 cm world-planar UV placeholder and opt-in
host triangle collision. Surface texture-axis fields, element lighting
blocks, the 42,068-byte Model remainder and native `PolyFlags` are retained
as digests and remain `UNVERIFIED`; volume-owned Models are still rejected
structurally; other maps are refused. `OpenWillow.BspWalking` stands on and
walks an unobstructed upward-facing polygon per Model; that is host behaviour
on the recovered geometry, not original-game parity, and no matched view has
been produced. No `src/` bounds check or layout changed.

Inspecting the import showed the outdoor start-area floor as UE's default
`WorldGridMaterial` checkerboard. The cause was not BSP: sections the
preparers leave with `material: None` (two terrains labeled
`neutral_constant`, ten static-mesh sections whose native material never
resolved) were skipped by the importer. `import_level.py` now binds a lit
0.5 gray `M_OpenWillowNeutralFallback` to those sections and
`verify_level.py` asserts it (15 mesh sections, 20 placements on Sanctuary).
This makes the gap visible as a labeled flat gray instead of a misleading
pattern; it does not resolve the terrain alpha decode or the missing
materials. Record: docs/verification/SANCTUARY_BSP_POLYGONS.md.

