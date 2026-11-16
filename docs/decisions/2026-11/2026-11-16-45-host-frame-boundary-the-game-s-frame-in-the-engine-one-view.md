## 2026-11-16: Host frame boundary: the game's frame in the engine, one view basis into wgpu clip space

AI-assisted (Claude Code). Lane H3 of the host track (architecture section 8, "Wave 7 and the host track"; rule R9,
decision D11), branch `feature/rust-h3-frame`. New module `crates/ow-host/src/frame.rs` (tests in
`crates/ow-host/src/frame/tests.rs`), one `pub mod frame;` line in `crates/ow-host/src/lib.rs`. Pure maths on std and
`ow-core`; no dependency added. No parsing behaviour changes.

**The frame.** The engine's world frame is the game's: X forward, Y right, Z up, left-handed, one engine unit = one
uu, `f32`. Positions cross unchanged, and an actor's engine matrix is its UE3 matrix stored column-major (column `i` =
UE3 row `i`, so the 16 floats are in `XPlane`..`WPlane` order). The only change of axes is the view: camera right,
up and forward go to wgpu view x, y and z, the cyclic swap (Y, Z, X), determinant +1. Clip space is wgpu's (NDC x
right, y up, depth 0..1) with reverse-Z depth (near 1, far 0, `Greater`), as the B2 wgpu benchmark used
(`docs/verification/ENGINE_BENCHMARK.md`; `local/bench/` was not on pc-hummer, so only the record was read).

**Why.** A right-handed Y-up world (glTF's) needs a reflection somewhere, and a reflection flips triangle winding,
cross products and the sign of roll. Keeping the game's frame leaves package vertices, normals and placements as
stored and puts the one axis change in a 3x3 permutation the tests pin. uu stays the length unit: the gameplay crates
and the recorded evidence are all in uu, and `f32` holds Sanctuary's coordinates (about 10,000 uu) to about 0.001 uu.

**Conventions used, with their sources.**
- Rotator to axes: `src/natives_core.cpp:437-443` (`GetAxes`, rows X forward, Y right, Z up), the same matrix as
  `tools/crosscheck_blcmm_dumps.py` `rotation_matrix`.
- Placement: scale = `DrawScale3D` x `DrawScale` (component: `Scale3D` x `Scale`), `tools/prepare_level.py:198-203`;
  each rotation row times its axis' scale, points as rows, translation last, component first and then its actor
  (`crosscheck_blcmm_dumps.py` `transform_matrix`, `compose`).
- Collection entries: the cooked `Matrix` rows times the kept scale, translation from `WPlane`
  (`crosscheck_blcmm_dumps.py` `actor_matrix`); a non-finite or non-affine matrix is refused, as
  `prepare_level.py` `collection_transforms` refuses it.
- Rotator wrap: `src/natives_core.cpp:51-55`. Direction to rotator: `src/interp.cpp:242-251` (truncation, roll 0).

**Verified, and how.** The placement order, rotator decode and draw-scale rule are the ones that reproduced the
running engine's own `_LocalToWorld` (its `obj dump`) on 4209 Sanctuary placements to 1e-3 on rotation entries and
0.05 uu on translation (`docs/verification/BLCMM_DUMP_CROSSCHECK.md`, "Actor placement"). This module's tests pin
four invented placements to that Python composer's output (run on pc-hummer, Python 3.12.10), within one `f32` step;
the C++ binary has no command that prints a placement matrix, so the Python composer is the pin. The other expected
values (quarter turns, wrap, order, view and depth) are derived by hand in the tests.

**UNVERIFIED.**
- The game's own precision for these matrices (single precision, or a sine table): the module works in `f64` and
  rounds once to `f32`, as the C++ `GetAxes` and the Python composer do.
- `rotator_from_engine` (matrix to rotator, for input) has no oracle: it rounds to the nearest unit so a rotator
  survives the round trip; roll is derived from the `GetAxes` rows.
- The wgpu `FrontFace` value: the frame is proper, and the retired UE5 host's OBJ note predicts counter-clockwise in
  NDC; the renderer lane settles it with a drawn test.
- Which FOV the host passes to `perspective_reverse_z` (it takes the vertical angle): the camera lane's, from
  `NATIVE_PLAYER_MOVEMENT.md` section 18 and `WEAPON_VISUALS.md` section 24.

**Not handled.** `PrePivot` and the component flags `AbsoluteTranslation`, `AbsoluteRotation`, `AbsoluteScale`: the
project's tools refuse or skip them and their place in the game's matrix is not read. `component_to_world` must not be
used for a component that sets one.
