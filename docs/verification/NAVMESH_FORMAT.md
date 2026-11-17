# Sanctuary's navigation mesh: format and what the Rust port reads (2026-11-16)

AI-assisted (Claude Code). Research lane NAV-MESH, feeding the host's AI lane (H-AI-HOST). Package-format work only:
the installed packages were read with `ow-package` (Rust, `--exports`, `--properties`, `--payload`) and checked with
structural oracles; no native code, decompiler output or analysis database was opened, and there was no real-game
session. Raw exports, payloads and the exploration scripts' output stay under ignored `local/p2/navmesh/`. Everything
below is **UNVERIFIED** as what the running game does with the data; what is proven is how the bytes divide.

Review r1 fixes: AI-assisted (Codex / GPT-6.1 Sol), continuing the partial Claude Code fix round.

## 1. What is there

Sanctuary's navigation is Gearbox's own mesh class, `Engine.GBXNavMesh`. There is no stock UE3 `NavigationMeshBase`
inside the pylons, and no `PathNode` or `ReachSpec` network. Class census of the ten levels `Scene::load` reads:

| Level | `GBXNavMesh` (export bytes) | `Pylon` | `PylonSeed` | `PathTargetPoint` | `PathBlockingVolume` | cover components |
|---|---|---|---|---|---|---|
| `Sanctuary_P` | 1 (344,324) | 0 | 1 | 0 | 1 | 0 |
| `Sanctuary_Land` | 1 (6,946) | 0 | 0 | 0 | 0 | 0 |
| `Sanctuary_Light` | 1 (5,076) | 0 | 0 | 0 | 0 | 0 |
| `Sanctuary_Dynamic` | 1 (2,460) | 7 | 8 | 9 | 3 | 19 `WillowCoverComponent` |
| `_Combat`, `_Side` | 0 | 0 | 0 | 0 | 0 | 2 and 1 |
| `_Outer`, `_FX`, `_Px`, `_Audio` | 0 | 0 | 0 | 0 | 0 | 0 |

Each `GBXNavMesh` is a placed actor (outer: the level) with one `GBXNavMeshRenderingComponent` (an editor drawing aid).
The pylons, seeds, target points and blocking volumes hold tags only (section 4): they are build inputs and markers,
not the mesh.

## 2. Where the data is, and its layout

A `GBXNavMesh`'s tags start 26 bytes in (the placed-actor prefix) and parse completely: `BuildVersion` (4 in all
four), `BuildGUID` (the same in all four), `MeshID` (1, 2, 3; absent, so the class default, in `Sanctuary_P`),
`ConnectedMeshes` (an array of `GBXConnectedNavMesh` {`Mesh`, `MeshID`}, left untyped by the walker),
`RenderComponent`, `Tag` and `EditorQuat`. The navigation data is the untagged tail after the tags:
343,886 bytes in `Sanctuary_P`, 6,608 in `_Land`, 4,738 in `_Light`, 2,122 in `_Dynamic`.

The class in `Engine.upk` names that data. It declares `Vertices`, `Polys`, `PolyData`, `PathSizes`, `Obstacles` and
`SpecialMovers` as arrays with the native flag (`CPF_Native`, 0x1000, so serialized by the class's own code, not as
tags), and `PolyLookup` as a native const struct. It also gives the field types and the constants
`NUM_EDGES` 3, `WORDSIZE` 2, `NUM_EDGE_BYTES` 6, `MAX_POLY_OBSTACLES` 6, `MAX_POLY_OBSTACLE_BYTES` 12,
`MAX_POLY_POINT_CHECKS` 10 and `DESIRED_NAVMESH_LOOKUP_NODE_SIZE` 1024.0. Within each struct the fields are in
declaration order, the reverse of their export order, which the triangle layout confirmed first.

The tail is a run of arrays, each an `i32` count then its elements:

| # | Part | Element | Bytes per element |
|---|---|---|---|
| 1 | vertices | 3 `f32` (uu) | 12 |
| 2 | triangles | `Verts` 3 `u16`, `ConnectedPolys` 3 `u16`, `PolyDataIdx` `u16`; `0xFFFF` is none | 14 |
| 3 | per-triangle data | `CrossLevelConnections` (count, then `EdgeIdx`, `ConnectedNavmeshIdx`, `PolyIdx`, 3 `u16`), `SpecialMoves` (count), `Obstacles` 6 `u16` (`0xFFFF` empty) | 20 + 6 per link |
| 4 | path sizes | `Radius` `f32`, `Height` `f32`, `DrawColor` 4 bytes, `ObstacleDrawColor` 4 bytes, `StartIndex` `i32` | 20 |
| 5 | lookup grid | `XCells` `i32`, `YCells` `i32`, `Origin` 3 `f32`, `CellSize` 3 `f32`, `CellIndices` (count, `i32` each, -1 empty), `Cells` (count, then per cell a count of `u16` triangle indices) | (one struct) |
| 6 | obstacles | an object reference `i32`, `MinAffectedPathSize` `i32` | 8 |
| 7 | eight bytes | zero in all four meshes; meaning not assigned | |

**Polygons are triangles** (`NUM_EDGES` 3, and the 16-bit indices of `WORDSIZE` 2 make `NUM_EDGE_BYTES` 6).

**The `SpecialMoves` element is not known.** No Sanctuary triangle has one, so its size was never seen. A reader must
stop at a positive count; the Rust reader answers `NotYet`. A negative count is malformed (`BadCount`).

## 3. What parses, and the structural oracles

The walk above, made independently in a Python script and in the Rust reader (`ow_assets::navmesh`), gives the same
counts on all four meshes:

| Mesh | Vertices | Triangles | Per-triangle data (cross-level links) | Path sizes | Grid | Listed cells | Obstacles |
|---|---|---|---|---|---|---|---|
| `Sanctuary_P` | 10,395 | 12,092 | 205 (61) | 3 | 33 x 37 | 511 | 15 |
| `Sanctuary_Land` | 202 | 191 | 33 (36) | 1 | 6 x 3 | 7 | 0 |
| `Sanctuary_Light` | 144 | 148 | 6 (6) | 2 | 4 x 4 | 14 | 0 |
| `Sanctuary_Dynamic` | 55 | 46 | 23 (19) | 2 | 2 x 2 | 3 | 2 |

Checks that hold on all four:
- The arrays meet exactly: the walk ends eight bytes before the export's end, and those eight bytes are zero.
- Every vertex is finite. Every vertex index is below the vertex count.
- Every neighbour is below the triangle count or `0xFFFF`.
- Every per-triangle data index is in range or `0xFFFF`, and every data entry is used by some triangle.
- Every cross-level edge is 0 to 2. Every obstacle slot is below the obstacle count or `0xFFFF`.
- The grid's index count is `XCells x YCells`. Every listed cell is indexed exactly once (the -1s are empty cells),
  every cell's triangles are in range, and every triangle is in some cell.
- Every vertex lies inside the grid's extent (`Origin` plus cells times `CellSize`, in x and y), with an inclusive
  1 uu margin on each side. This is a reader tolerance for float rounding, not a game rule. No z extent is
  inferred from the planar grid.
- Every path size's `StartIndex` is nonnegative and at most the triangle count.

**Neighbour slots are edges.** For every link between two triangles that share two vertex indices (26,276 in
`Sanctuary_P`), the shared pair is the edge from vertex `k` to vertex `k + 1` of slot `k`. In the three small meshes
every link is mutual. In `Sanctuary_P`, 4 of 30,928 links are one-way, each between triangles that share an edge.

**World space, one point checked, UNVERIFIED.** The lowest vertex of `Sanctuary_P` is at z 2720.0, and the `_Land` triangle under
the end point of H4's scripted walk (decision 2026-11-16-52) is at z 2720.0 there. That is the floor height H4's own
collision gives (the pawn's centre at 2800.03, half height 80). Taking the vertices as world uu without
`CachedLocalToWorld` fits this one point. This is a comparison to the host collision, not a running-game height
check or proof of world space; a mesh on a moving base (`MeshBase`) may differ.

**Path sizes split the triangles into runs.** `Sanctuary_P` has three sizes: radius 22, 40 and 65, all of height 220,
starting at triangles 0, 8,433 and 11,961. `_Dynamic` and `_Light` have the first two, `_Land` only the first. Reading
`StartIndex` as the first triangle of a size's own mesh fits the counts. But about 370 links in `Sanctuary_P` cross
from one run to another, and about 4,650 linked pairs share fewer than two vertex indices. Most of those still touch
the edge geometrically (median gap 0.14 uu), some do not (95th percentile 17 uu). What the game does with those links
is **UNVERIFIED**.

## 4. What does not parse, or is not assigned

- The `SpecialMoves` element (none present).
- The eight trailing bytes of every mesh (zero; kept as read, any non-zero byte refused).
- SpecialMovers (declared native) is not located; the 8 trailing bytes are a candidate (an empty array's count is
  four zero bytes), UNVERIFIED. The reader does not assign them.
- `ConnectedMeshes`: its elements are tagged structs the walker leaves untyped, so which mesh a cross-level link's
  `ConnectedNavmeshIdx` names is not resolved here. The array is 196 bytes in `_P` and 68 in the others.
- What the obstacles are at run time: they reference objects of the level, and whether each blocks is game state.
- The pylons: their tags parse completely (`PathSizes`, `NextPylon`, `NavGuid`, the cylinder and more), and each
  leaves 8 untyped bytes. No mesh data is in them.
- Coverage as read (vertices taken as world uu, one point checked): no Sanctuary navmesh triangle lies under the
  player start, or under the fence H4's walk stopped at. Only the walk's end point is on a mesh (`_Land`). Coverage
  of the whole walkable town remains UNVERIFIED until the coordinate interpretation is confirmed.

## 5. What the Rust port has now

`ow_assets::navmesh::read_navmesh(package, export)` reads a `GBXNavMesh` (decision 2026-11-16-75):
- the vertices, triangles with neighbours, per-triangle data, path sizes, lookup grid and obstacles;
- the structural oracles in section 3: local index bounds, used per-triangle data, complete triangle and cell
  coverage, grid dimensions and planar extent (inclusive 1 uu margin), and neighbour edge slots where two distinct
  vertex indices are shared. Cross-level mesh and triangle targets remain unresolved;
- exactly eight zero bytes after the obstacles, with a `TrailingNotZero` error for any non-zero byte;
- a `NotYet` error on a positive special-move count, `BadCount` on a negative count;
- finite floats and counts bounded by the remaining bytes. The height comparison and path-size run interpretation
  above remain observations, not additional reader checks.

Four tests on invented meshes cover field round-tripping, refusal cases, each of the eight trailing bytes, and the
inclusive planar margin at all four boundaries. An ignored driver
(`sanctuary_navmeshes_on_the_installed_game`, `OW_BL2_ROOT`) prints the counts above.

## 6. Recommended next step

**A reader lane is not needed: the reader exists.** What H-AI-HOST needs next is a path on it. Two options, for the
hub:

1. **Recommended: a labelled host path-finder over this mesh.** A* over the triangle adjacency of the run whose radius
   fits the pawn, with the lookup grid to find the start and end triangles. Then string-pull the triangle path through
   the shared edges. It is the host's own algorithm, a stand-in for the game's `GBXNavMeshPathFinder.FindPathToLocation`
   (a native whose rules are not read) and its two path modifiers (`GBXNavMeshPathModifier_Simplify`,
   `GBXNavMeshPathModifier_SmoothTurns`; their class defaults are readable). It moves pawns only on the game's own
   walkable triangles. Open items it would carry:
   - cross-level links, which need `ConnectedMeshes` typed;
   - the links that cross path-size runs;
   - obstacles' run-time state;
   - special moves (none in Sanctuary).
2. **A straight-line stand-in** (move toward the target, stop at collision). It is simpler, but it ignores the mesh the
   game ships and will walk pawns where the game's AI cannot go. Only for a first frame of motion, and labelled as such.

For parity rather than a stand-in, the game's path-finder and modifiers would need an analyst lane under the
clean-room rule (`docs/LANE_BRIEF.md`). Before any of this, check that the bullymong's map has a `GBXNavMesh` covering
its area; Sanctuary's does not cover the player start as read (vertices taken as world uu, one point checked).
