## 2026-11-16: Rust navmesh reader: GBXNavMesh triangles, neighbours and lookup grid (NAV-MESH)

AI-assisted (Claude Code). Research lane NAV-MESH; the hub reserved number 75. The research record is
`docs/verification/NAVMESH_FORMAT.md`; this file records the code it added.

- **What changed.**
  - New `crates/ow-assets/src/navmesh.rs` and `navmesh/tests.rs`, plus one `pub mod navmesh;` line in
    `crates/ow-assets/src/lib.rs` (ow-assets additions are within the lane's brief).
  - The record `docs/verification/NAVMESH_FORMAT.md`.
  - No dependency, no tool, no reader or bounds-check change, no host change.
- **What the reader reads.** `read_navmesh(package, export)` reads an `Engine.GBXNavMesh`:
  - its tags from the placed-actor prefix (26), only to find where they end;
  - then the native tail: vertices, triangles (three vertex and three neighbour `u16`, a data index), per-triangle
    data (cross-level links, special moves, six obstacle slots), path sizes, the lookup grid, obstacles, and eight
    trailing bytes.
  - The field names, types and array sizes come from the class's declarations and constants in the installed
    `Engine.upk`. The order of the parts and of each struct's fields was fitted to the four Sanctuary meshes; within
    a struct, declaration order is the reverse of export order.
- **Why it can be trusted that far: structural oracles** (all four Sanctuary meshes; the record, section 3).
  - The parts meet exactly and end eight zero bytes before the export's end.
  - Every index is in range, and every data entry and grid cell is used.
  - The grid's index count is `XCells x YCells`, and every triangle is in some cell.
  - Neighbour slot `k` is the edge from vertex `k` to `k + 1` for all 26,276 two-shared-vertex links.
  - A Python walk and the Rust reader give the same counts.
  - So the layout is proven for these meshes only, and **UNVERIFIED** for others. The reader enforces every check
    and refuses what was not seen:
    - a non-zero special-move count (`NotYet`: its element layout is unknown);
    - a tail that does not end eight bytes after the obstacles;
    - any index out of range;
    - a non-finite float;
    - a count larger than the bytes left.
- **Not assigned or not read.** The eight trailing bytes (kept as read); `ConnectedMeshes`' elements (tagged,
  untyped); the obstacles' run-time state. The meaning of the links that cross path-size runs, and of linked
  triangles that share fewer than two vertex indices, stays UNVERIFIED.
- **Verified (automated, synthetic).** Two `ow-assets` tests on an invented two-triangle mesh in a testkit package:
  - it reads back field by field;
  - 15 refusals: a bad vertex, neighbour, edge, obstacle or lookup-triangle index; a special move; three bad grids;
    a bad path-size start; a short and a long tail; a non-finite vertex; an over-large count; another class.
- **Verified (local, installed game, zuhu desktop).** The ignored driver `sanctuary_navmeshes_on_the_installed_game`
  reads all four Sanctuary meshes:
  - `_P`: 10,395 vertices, 12,092 triangles;
  - `_Land`: 202 and 191;
  - `_Light`: 144 and 148;
  - `_Dynamic`: 55 and 46;
  - trailing bytes zero in all four.
- **Open.**
  - A path-finder on the mesh for H-AI-HOST: the record recommends a labelled host A* stand-in.
  - The game's `GBXNavMeshPathFinder`, which would need a clean-room analyst lane.
  - `ConnectedMeshes` typing.
  - Navmesh coverage of the bullymong's area; Sanctuary's meshes do not cover the player start.
