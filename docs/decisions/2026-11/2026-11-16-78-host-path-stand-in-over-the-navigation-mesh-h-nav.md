## 2026-11-16: Host path stand-in over the navigation mesh (H-NAV)

AI-assisted (Claude Code; completion and verification: Codex / GPT-6.1 Sol). The unpublished H-NAV checkpoint and
its reserved decision number 78 are retained. This uses the reader landed with PR #373, including its review fixes.

`ow-host::nav` provides an original project-owned HOST STAND-IN query, `NavMap::find_path(start, end, radius)`.
Inputs and waypoints are game units, Z-up; `frame.rs` remains the only conversion point and is unchanged.
The external path is the existing project NavMesh reader and the user's installed packages. Extraction, tools,
dependencies and licences are unchanged. No game payload or native-analysis output enters the tracked change.

The query chooses the smallest path-size radius at least as large as the pawn radius. Triangle runs use
`StartIndex` through the next start. The lookup grid places each endpoint in a triangle, using cell index
`y * XCells + x`, barycentric containment, and the highest floor within 35 uu above the input and the run's height
below it. These are stand-in choices; the world's coordinate interpretation and the game's selection rules remain
UNVERIFIED. There is no search for a nearby off-mesh endpoint.

A* uses centroid distances and triangle-index tie breaking. The estimate targets the destination triangle's
centroid. It follows only neighbours in the selected run with the exact shared edge identified by their slot.
Geometric links without shared vertex indices are excluded. Obstacle-bearing and degenerate triangles are
excluded; starting or ending on one returns `NotYet`. Cross-level links and links into another size run are not
followed. `NoPath` means no route in this supported subgraph, not proof that the game cannot make a path.

Waypoints visit triangle centroids and shared-edge midpoints. Each segment stays on one planar triangle, including
its height. The earlier unpublished planar simplification could skip changes in floor height and was removed.
These routes can zigzag and are not shortest walking paths. Run selection alone does not prove pawn clearance.
Invalid inputs, off-mesh endpoints, no fitting run, no supported path, and cross-mesh requests have typed errors;
there is no straight-line fallback.

`NavPath::check_collision` is a separate conservative H4 check. It samples support at intervals no larger than
10 uu, requires a floor within 1 uu with normal Z at least 0.70, and sweeps a cylinder with the requested radius
and half height just above the path. Missing or mismatched floors, blocked sweeps and start-solid placements are
typed `CollisionUnsupported` refusals. This is sampled support plus continuous segment clearance, not a proof
of continuous support between samples. Slopes requiring another cylinder lift may be refused. Terrain, BSP,
blocking volumes and per-polygon collision missing from H4 cannot be certified by this check.

`NavLocomotion` implements `ow_ai::awareness::actions::Locomotion`. The host registers a handle's pawn and radius;
the adapter reads its VM `Location`, stores successful routes for `path_of`, clears a previous route before a
replacement attempt, handles validity/invalidation and keeps the requested stance byte. Ordinary routing
refusals answer false at the boolean seam; unsupported operations retain `NotYet`. Arrival uses the selected
mesh's floor, within the run radius horizontally and 220 uu vertically, a host choice. Reference-mode calls
refuse with `NotYet`: this adapter supplies no native parity. Following routes, applying speed and handling
latent moves remain H-AI-HOST work. A follower must check collision before walking a route.

Synthetic tests write invented meshes through the existing package testkit and read them back through the
unchanged reader. They cover routing around a hole, deterministic paths, stacked runs, invalid inputs,
off-mesh/no-path refusals, non-shared geometric links, changing floor heights, collision support and clearance,
arrival, and the VM-backed Locomotion seam with Reference refusal. They establish host behavior only.

The ignored installed driver loads Sanctuary headlessly, seeks three short routes on the small Land mesh's
neighbourhood, and reports lengths and waypoint counts only after the H4 check passes. It asserts three supported
routes and reports all attempted queries and refusals. Output stays under ignored `local/p2/h-nav/`.
The old broad six-query experiment is preserved there: only 135 of 998 floor samples matched within 40 uu and
4 of 77 sweeps were blocked. Its passing test status did not establish walk-on-floor safety. It is not reused
as acceptance evidence for this change.

The game's `GBXNavMeshPathFinder.FindPathToLocation` and its modifiers have not been implemented or checked in a
running game. Every pathfinding rule here remains UNVERIFIED as the game's. A later clean-room native pathfinder
is required for parity. No real-game, window, GPU, benchmark or gameplay host session is part of this lane.
