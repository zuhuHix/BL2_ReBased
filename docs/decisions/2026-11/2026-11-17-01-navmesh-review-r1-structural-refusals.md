## 2026-11-17: Navmesh review r1 structural refusals

AI-assisted (Codex / GPT-6.1 Sol), continuing partial fixes by Claude Code.

Edits: docs/decisions/2026-11/2026-11-16-75-rust-navmesh-reader-gbxnavmesh-triangles-neighbours-and.md

Reason: correct the original reader-enforcement claim after review r1 of PR #373, record the reader's planar
tolerance, and qualify coverage and height claims by their limited evidence.

- `ow-assets` now refuses any non-zero trailing byte, unused per-triangle data, a triangle in no lookup cell, a
  vertex outside the grid's x/y extent, and a neighbour slot on the wrong edge when two distinct indices are shared.
- The planar extent uses an inclusive 1 uu reader tolerance for float rounding. This is not a recovered game
  constant. The existing bounds and finite-value checks remain in force.
- Negative special-move counts are `BadCount`; positive counts remain `NotYet`. The element layout is unknown.
- `SpecialMovers` is not located. The eight trailing bytes could include an empty-array count, UNVERIFIED; no field
  is assigned to them. `NavPathSize::start_index`, `CrossLevelLink.mesh` and `NavObstacle.obstacle` remain fitted,
  UNVERIFIED interpretations.
- Synthetic fixtures exercise the five structural refusals, the requested malformed indices/counts/floats, all
  eight trailing-byte positions and both sides of the planar tolerance.
- World-space heights and missing player-start coverage remain UNVERIFIED: vertices are taken as world uu, and
  one point was compared to host collision only. No running-game or host session is part of this fix round.
- No new dependency, tool, payload or change to the package reader. Earlier Claude assistance remains credited.
