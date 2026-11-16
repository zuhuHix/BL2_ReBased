## 2026-11-16: Host movement and collision (H4)

AI-assisted (Claude Code). Lane H4, host track (architecture section 8, "Wave 7 and the host track"): player movement
and collision per `docs/verification/NATIVE_PLAYER_MOVEMENT.md`. Number 52 was reserved for this lane by the hub.
There was no real-game session. Every movement rule here is **UNVERIFIED** in the running game unless a line says
how it was confirmed.

- **What changed.**
  - `crates/ow-assets/src/collision.rs` (new; ow-assets additions were granted by the hub) and its tests, plus one
    `pub mod` line in `crates/ow-assets/src/lib.rs`.
  - `crates/ow-host/src/movement.rs` and `crates/ow-host/src/movement/` (new: `geom`, `collide`, `mover`, `level`,
    `walk`, `sanctuary`, `tests`), plus one `pub mod` line in `crates/ow-host/src/lib.rs`.
  - `ow-host --game <install> --walk` in `args.rs` and `main.rs`.
  - The H4 check in `docs/verification/GATES.md`.
  - No dependency, no physics crate (hub decision: we write our own collision), no external tool, no bounds check and
    no reader offset changed.
- **Where movement lives: `ow-host`.** Architecture section 2.1 gives no crate below the host a movement rule. A
  library crate would be a 23rd crate with new section 2.2 edges, an architecture change this lane does not make. The
  module reads only `ow-core`, `ow-assets` and the scene, and stays in uu, Z-up, so it can move down whole when a game
  crate needs it.
- **Collision data: a mesh's `RB_BodySetup`.** `ow_assets::collision::read_body_setup` ports
  `tools/collision_geometry.py` `hulls()`, the reader `tools/prepare_level.py` used for Sanctuary.
  - It walks the body's tags from 4 with the collision lines of `tools/level-arrays.schema`. It returns
    `AggGeom.ConvexElems` (vertices, and `FaceTriData` triangles when tagged, checked against `ElemBox`) and
    `BoxElems` (eight corners through `TM`) in mesh space.
  - Spheres, capsules, `bNoRBCollision`, `bPerPolyShape`, untyped aggregate fields, flat hulls and non-finite values
    refuse the whole body, as in the tool.
  - No offset is new: the tag walker and the fixed `Vector`/`Box`/`Plane`/`Matrix` layouts are `ow_package::props`'s
    (decision 2026-09-14-04).
- **Which placements collide** (`movement/level.rs`).
  - Every placement of the H2 scene whose component has `CollideActors` and `BlockActors` true. Hidden placements
    are included, as the tool keeps a hidden helper's collision.
  - The flags are read from the definition store's object, so an untagged flag takes the class default. The tool
    takes it as true. On Sanctuary the two agree: no placement is non-blocking by a class default alone.
  - Owner actor flags are not read, by either.
  - Each hull goes through the placement's world transform (`frame::component_to_world`, or a collection entry's
    matrix with scale `Scale3D` x `Scale`) and becomes a convex brush.
  - Not applied: the tool's two named triangle stand-ins (archives floor, scooter frontage) and its
    `LANDED_INACTIVE_COLLISION` exclusion of `Sanctuary_Outer` `InterpActor_34` (decision 2026-09-23-03). That
    decision says the box encloses the player start. With the scene's poses, composed as the BLCMM cross-check
    confirmed against the running engine's matrices (`BLCMM_DUMP_CROSSCHECK.md`, the rule H3 adopted), it does not:
    the box's brush spans y -5906.8 to -2685.4, and the start's y is -6348.9, about 440 uu outside. So nothing here
    supports dropping it. Whether
    `_Outer` content blocks in landed play is Kismet and streaming state, and stays open.
  - Typed `NotYet` lines in the census:
    - terrain (`tools/terrain_decode.py` not ported);
    - BSP (`tools/bsp_decode.py` not ported);
    - blocking volumes (`tools/collision_volumes.py` not ported);
    - per-polygon (kDOP) collision of a mesh with no body (not read).
- **The sweep** (`movement/collide.rs`, our own, a stand-in for the game's contact rule, which was not read).
  - A brush is a convex solid's planes: hull faces from `FaceTriData` or a search over point triples, plus the six
    bounding-box bevels. A triangle is a thin brush of two faces and three edge planes.
  - A swept shape is a point against the brush grown by the shape's support distance along each plane normal: exact
    on faces, up to `r (1 / cos(a / 2) - 1)` early near an edge of angle `a` (17 uu at a right angle, radius 42).
  - The trace stops 1/32 uu short of a surface. A move that keeps half of that away counts as clear, so slides along
    ramps and walls are not taken for entries by rounding. A start inside a brush may leave it, never go deeper.
  - A uniform 512 uu grid in X and Y; candidates in ascending brush order, the nearest hit wins, the lower index on a
    tie.
- **The shape is a cylinder, not a capsule.** The pawn collides as its `CylinderComponent` (radius 42, half height
  80, read live), and the one live slope observation fits a flat-bottomed cylinder. On the stair ramp hulls (normal
  Z 0.80) the original's feet stood 35-42 uu above the surface under its centre. `r tan(a)` gives 31.5 uu for a
  cylinder; a capsule would give 10.5 uu (`COLLISION_WALKING_VERIFICATION.md`, 2026-10-21). The sweep supports both;
  Maya uses the cylinder. The brief asked for a capsule; this follows the recorded evidence instead.
- **The mover** (`movement/mover.rs`).
  - Read from native code or script (UNVERIFIED in game):
    - input to acceleration (section 9);
    - the ground velocity step and sub-stepped braking with the 10 uu/s stop (section 4);
    - the falling step: air-control limit, start kick, at most 8 sub-steps of 0.05 s, fluid friction, terminal
      velocity (section 7);
    - the jump (section 13), and landing on a floor of normal Z at least `WalkableFloorZ` (section 7.5).
  - **Host stand-ins**, because the game's walking step, floor finding and step-up are "Not read yet":
    - a walkable ramp ahead turns the move along its plane;
    - a wall or a too-steep slope first tries a step: up `MaxStepHeight`, across, back down onto a walkable floor;
    - otherwise the move slides along the horizontal wall normal; a slide back into an earlier wall is a corner and
      stops;
    - after the move, the pawn snaps down by up to `MaxStepHeight` onto a walkable floor, or it falls;
    - a falling sub-step moves by its new velocity, and a landing drops the rest of the sub-step;
    - a spawn inside a brush is lifted up to 160 uu, 1 uu at a time. Sanctuary's start is exactly half the cylinder's
      height above its floor, so the cylinder touches it and is lifted 1 uu.
  - Numbers (`MovementParams::maya`):
    - read live on 2026-10-21 (bl-sdk property reads, not measured motion): radius 42, half height 80, step 35,
      walkable 0.70, `GroundSpeed` 440, `AccelRate` 2048, `AirControl` 0.11, `JumpZ` 630, gravity -500,
      `BaseEyeHeight` 70;
    - cooked or native, UNVERIFIED: `GroundFriction` 8, `FluidFriction` 0.3, `TerminalVelocity` 4000, braking
      factor 2 with 0.03 s sub-steps, pitch limits +-16384.
  - Not here: sprint, crouch, the walk key, ladders, swimming, falling damage (the landing speed is reported), eye
    easing and bob, moving bases, pawn-to-pawn collision.
  - Determinism: `f64` state, a pure step; the same inputs at the same fixed step give the same bits (a test).
- **Verified (automated, synthetic geometry only).** Six `ow-assets` tests cover a convex element with triangles, a
  box element through its `TM`, an empty body, ten refusals, the bounds tolerance and the wrong class. Fifteen
  `ow-host` tests (fourteen movement tests and one argument test) cover:
  - a flat floor and the run to 440;
  - braking to a stop;
  - steps of 20 and 35 climbed, 36 and 60 not;
  - ramps of normal Z 0.80 and 0.707 walked, 0.69 and 0.55 not;
  - a wall slide, and a corner that holds still;
  - a fall and landing (811 uu/s after 1,000 uu);
  - the jump;
  - walking off a ledge;
  - two triangles as a floor;
  - the cylinder and capsule resting heights on a 0.8 slope (31.5 and 10.5 uu above the surface, plus the skin);
  - face planes from triangles equal to the searched ones;
  - the spawn lift;
  - the same path twice, bit for bit;
  - the walk checks;
  - `--walk` parsing.

  The jump at the 1/60 s step rises 312.7 uu and lands after 2.25 s at -499 uu/s. Without the falling drag the arc
  would be 396.9 uu and 2.52 s. Which one the game does is open, and it is the first real-game measurement to ask for.
- **Verified (local, installed game, zuhu desktop, release build; output under ignored `local/p2/h4-movement/`).**
  - **Collision counts.** Of 4,446 placements, 3,855 block and 591 do not; no flag was unreadable. Of the blocking
    placements:
    - 3,124 are placed with hulls;
    - 681 have a mesh without a body (110 distinct meshes);
    - 50 have a body with no hulls;
    - none was refused.
  - 299 distinct meshes have a decoded body, with 494 hulls. They gave 4,219 brushes, none refused. By level:
    `Sanctuary_P` 3,114, `_Land` 930, `_Dynamic` 98, `_Outer` 33, `_Light` 25, `_FX` 8, `_Px` 7, `_Combat` 2,
    `_Side` 2.
  - For comparison, `prepare_level.py` on 2026-09-14 reported 304 supported bodies, 509 hulls and 3,116 components
    with collision. It counted every mesh, and it kept the untagged-flag rule. The counts are close; they are not
    expected to be equal.
  - **The walk.** `ow-host --game <install> --walk` exits 0 with result PASS:
    - spawn at `WillowCoopPlayerStart_0` (655.6, -6348.9, 2800), yaw 8192, lifted 1 uu;
    - landed and walked at z 2800.03, the floor's top at 2720;
    - 1,050 ticks, 3,562 uu travelled, no fall through, no stuck segment, no start-solid tick.
  - Two segments stalled at walls, both named by the run:
    - the jump ran into `Sanctuary_P` `StaticMeshActor_SMC_26` (`ChainLinkFenceShortNoWire`, top 323 uu above the
      floor, above the 312.7 uu jump);
    - the strafe ran into `Sanctuary_Outer` `InterpActor_34` (`BoxLrg`).
  - The scene load took 40-52 s on this desktop (24.6 s on pc-hummer in H2), and building the collision well under a
    second.
- **Open.**
  - A real-game jump trace (the pawn's Z per tick through one standing jump) to settle the falling drag.
  - A real-game walk along the same path to compare ground heights and the two walls.
  - The game's walking step, floor finding, step-up and spawn search (natives not read).
  - Terrain, BSP and blocking-volume collision (ports of the Python decoders).
  - Per-polygon collision for meshes without a body.
  - The `_Outer` level's state in landed play.
  - Window wiring for WASD and mouse look, and the camera following the pawn, once H2 stage 2 draws. The input type
    (`MoveInput`) and the eye location exist; the window's key and mouse events do not reach the simulation yet.
