# Sanctuary root BSP polygons (2026-09-15)

AI-assisted (Claude Opus 5), original observation against the user's installed
packages. No external implementation, decompiled executable or recovered
engine source was consulted. Game-derived outputs stay in ignored `local/`.
This record covers the decode gates, the host import and the automated
runtime check. It does not claim original-game parity.

## Scope

Persistent-level root `Engine.Model` and `Engine.ModelComponent` exports of
`Sanctuary_P` and `Sanctuary_Land` only. Volume-owned Models are rejected
structurally by `require_root` (owner must be `TheWorld.PersistentLevel`).
Other maps have not been decoded.

## Decode gates (`tools/bsp_decode.py`)

Every gate must pass before `tools/prepare_bsp.py` writes any file:

- Root Model prefix: 28 zero bytes, then bulk arrays of 12-byte vectors,
  12-byte points and 64-byte nodes, a self-reference equal to the Model's
  export index, 60-byte surface records and 24-byte vertex records.
- Each node's polygon range must lie inside the vertex array and reference a
  valid surface; every vertex must index the point pool.
- The surface's normal-vector index must agree with its stored normal within
  0.001; the node plane and surface plane must agree (dot > 0.999) and every
  polygon point must lie on both planes within 0.02 cm.
- Polygons must be convex, ordered consistently with the plane and free of
  duplicate points, zero-length edges and reversed fan triangles.
- Each ModelComponent must reference its Model, each element must reference
  the component, element node lists must be unique and carry the element's
  material, the component node list must equal the union of its elements,
  and every node's stored component ordinal / position must back-reference
  the component that lists it.
- Across all components, node ids must cover `0..N-1` exactly once.

Unused native fields (remaining Model bytes, surface and vertex records,
element lighting blocks and auxiliary arrays) are retained as SHA-256
digests labeled `UNVERIFIED`; no meaning is asserted for them.

## Observed result

| Level | Components | Polygons | Triangles | Sections | Materials |
|---|---:|---:|---:|---:|---:|
| Sanctuary_P (`Model_3`) | 18 | 216 | 547 | 98 | 11 |
| Sanctuary_Land (`Model_4`) | 6 | 12 | 24 | 7 | 1 |
| Total | 24 | 228 | 571 | 105 | 12 |

All gates passed on both Models. The `Sanctuary_P` Model keeps a 42,068-byte
opaque remainder after the consumed arrays. Materials resolve through the
existing Material v1 path (`Mati_SancBuild1a`, `Mati_SancWall01`,
`Mati_DiggerPanels`, `Mati_FloorConcrete01`, `Mati_CatwalkMount`, ...); all
105 imported BSP actors carry their native material in the saved map.

## Approximations, labeled

- UVs: at the time of this record, a world-planar projection on the
  polygon's dominant axis, 128 cm per tile. Superseded on 2026-09-18: the
  surface records' base-point and texture-axis fields are now decoded and
  confirmed against editor Polys exports, and the projection uses them; the
  texel scale remains `UNVERIFIED`. See [BSP_TEXTURE_AXES.md](BSP_TEXTURE_AXES.md).
- Lighting: host inspection rig only; element lighting blocks are hashed,
  not interpreted.
- Collision: opt-in host triangle collision on every recovered polygon.
  Native `PolyFlags` (non-solid, portal, invisible, ...) are `UNVERIFIED`, so
  some recovered surfaces may block where the original game does not, and
  vice versa.

## Neutral fallback for unresolved section materials

While inspecting the result, the outdoor floor near the start camera rendered
as a gray/black checkerboard. This was not a BSP surface: the five terrain
sections of `Sanctuary_P.Terrain_2` and `Sanctuary_Land.Terrain_3` whose alpha
maps do not decode are labeled `neutral_constant` by `prepare_terrain.py`
but were emitted with `material: None`, and the importer skipped binding, so
UE substituted its default `WorldGridMaterial`. The importer now binds an
explicit lit 0.5 gray `M_OpenWillowNeutralFallback` in that case and
`verify_level.py` asserts it; both report `neutral_fallback_sections`.
The terrain alpha decode itself is unchanged and still open.

## Automated checks (this worktree, 2026-09-15)

```text
ctest --test-dir build -C Release --output-on-failure
100% tests passed, 0 tests failed out of 5

python tools/verify_packages.py --reader build/Release/ow-package.exe
GearboxFramework ... AkAudio: decoded bytes, counts and export fields match

python tests/bsp_test.py
Ran 6 tests in 0.003s  OK

run_ue_level.ps1 -Scene local/sanctuary -ImportOnly -SkipBuild
ue-import.json:   section_actors 4888, source_placements 4469, issues 188,
                  neutral_fallback_sections 15
ue-verify.json:   verified_section_actors 4888, hidden_visual 106,
                  native_skybox 1, verified_neutral_fallback_sections 20,
                  geometry_bounds matches source OBJ
ue-collision-verify.json: verified_mesh_sections 615, enabled_components
                  3236, enabled_triangle_components 120, errors 0
ue-uv-verify.json: verified_sections 615, verified_corners 545301,
                  verified_triangles 181767, winding matches source order

test_ue_viewer.ps1 -Scene local/sanctuary -Bsp
Test Completed. Result={Success} Name={BspWalking}
BSP stand: ModelComponent_0  node=23 valid=1 pawn=(4679.6, -5216.0, 2874.2)
BSP walk:  ModelComponent_0  valid=1 pawn=(4883.6, -5216.0, 2874.2)
BSP stand: ModelComponent_19 node=5  valid=1 pawn=(19580.1, -61287.7, 2688.9)
BSP walk:  ModelComponent_19 valid=1 pawn=(19770.1, -61230.1, 2688.6)
BSP runtime summary: stood_and_walked=2/2 rejected_candidates=0
```

The 120 triangle-collision components are the 15 terrain and 105 BSP
sections; the walk asserts a 200 cm displacement on the identified polygon
and nothing about routes, stairs or the original game's blocking.

## In-game checks

Before the fallback change the user inspected a 1280x720 viewer frame of
the outdoor area beyond the town wall: checkerboard floor, surrounding
static meshes textured normally.

A post-fallback 1280x720 viewer frame at the saved start pose (inside town,
Scooter's garage) shows textured buildings and no checkerboard;
`local/sanctuary/bsp-neutral-start.png`. The outdoor floor that previously
showed the checkerboard has not yet been re-inspected by eye after the
fallback change; the saved-map verifier confirms its material binding only.

## Not verified

- Original-game matched views of any BSP surface.
- The BSP texel scale (axes since decoded, see above), lightmaps,
  `PolyFlags` and the opaque Model remainder.
- Any map other than Sanctuary; `prepare_bsp.py` refuses other maps.
