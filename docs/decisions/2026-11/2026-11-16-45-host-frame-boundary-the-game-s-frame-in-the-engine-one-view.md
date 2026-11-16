## 2026-11-16: Host frame boundary: the game's frame in the engine, one view basis into wgpu clip space

AI-assisted (Claude Code). Lane H3 of the host track (architecture section 8, "Wave 7 and the host track"; rule R9,
decision D11), branch `feature/rust-h3-frame`. New module `crates/ow-host/src/frame.rs` (tests in
`crates/ow-host/src/frame/tests.rs`), one `pub mod frame;` line in `crates/ow-host/src/lib.rs`. Pure maths on std and
`ow-core`; no dependency added. No parsing behaviour changes.

**The frame.** The engine's world frame is the game's: X forward, Y right, Z up, left-handed, one engine unit = one
uu, `f32`. Positions cross unchanged, and an actor's engine matrix is its UE3 matrix stored column-major (column `i` =
UE3 row `i`, so the 16 floats are in `XPlane`..`WPlane` order). The only change of axes is the view: camera right,
up and forward go to wgpu view x, y and z, the cyclic swap (Y, Z, X), determinant +1, so the view mirrors nothing.
Clip space is wgpu's (NDC x right, y up, depth 0..1) with reverse-Z depth (near 1, `Greater`, clear to 0), in two
forms: `perspective_reverse_z_infinite` (no far plane, depth = near / z) and `perspective_reverse_z` (finite far
plane, depth 0 at far). The B2 wgpu benchmark used the infinite form (`perspective_infinite_reverse_lh`, read by the
review of PR #343 in the benchmark source under the desktop's `local/bench/`; `ENGINE_BENCHMARK.md` names only
reverse-Z, and `local/bench/` is not on pc-hummer).

**Mirrors.** A placement with an odd number of negative scale components (`DrawScale3D`, `Scale3D` or `DrawScale`)
has a negative determinant and flips winding. `Affine::determinant` and `Affine::mirrors` report it, and the renderer
flips the front face for such a draw, as B2 did (`FrontFace::Cw` for det < 0).

**Bad input.** Everything built from game data returns `Option`, `None` unless every output entry is a finite `f32`:
`Placement::affine`, `component_to_world`, `Affine::from_ue3_matrix`, `view_matrix`, both perspective forms and
`rotator_from_engine`. `Affine::then` and `Affine::to_engine` take any `Affine`; a caller that composes one by hand
checks it with `Affine::is_finite` first.

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
- `ow-vm`'s `natives_core.rs` holds faithful ports of the same wrap (`normalize_axis`, on `i64`) and of `GetAxes`,
  which, like the C++, does not wrap before the trigonometry. This module wraps first on purpose, so rotators a turn
  apart give the same bits; the two agree exactly for axes in -32768..=32767. Moving both into `ow_core::units` is a
  follow-up.

**Verified, and how.** The placement order, rotator decode and draw-scale rule are the ones that reproduced the
running engine's own `_LocalToWorld` (its `obj dump`) on 4209 Sanctuary placements to 1e-3 on rotation entries and
0.05 cm on translation, the record's unit, which is one uu as the scene is placed
(`docs/verification/BLCMM_DUMP_CROSSCHECK.md`, "Actor placement"). This module's tests pin four invented placements
to that Python composer's output (run on pc-hummer, Python 3.12.10): all 48 entries equal the pinned `f32` exactly,
except that a mathematically zero entry holding a 1e-16 rounding residue only has to stay below 1e-15 (P4's roll
32768 is wrapped to -32768 here, so its residue has the other sign). The C++ binary has no command that prints a
placement matrix, so the Python composer is the pin. The other expected values (quarter turns, wrap, order, view,
both depth forms, mirrors, refusals, poles) are derived by hand in the tests.

**UNVERIFIED.**
- The game's own precision for these matrices (single precision, or a sine table): the module works in `f64` and
  rounds once to `f32`, as the C++ `GetAxes` and the Python composer do.
- `rotator_from_engine` (matrix to rotator, for input) has no oracle: it rounds to the nearest unit so a rotator
  survives the round trip; roll is derived from the `GetAxes` rows. At the poles (pitch +-16384) it may return
  another rotator with the same axes; the tests check the axes there, not the rotator.
- The wgpu `FrontFace` value for an unmirrored draw: the view is proper, and the retired UE5 host's OBJ note predicts
  counter-clockwise in NDC; the renderer lane settles it with a drawn test.
- Which FOV the host passes to `perspective_reverse_z` (it takes the vertical angle): the camera lane's, from
  `NATIVE_PLAYER_MOVEMENT.md` section 18 and `WEAPON_VISUALS.md` section 24.

**Not handled.** `PrePivot` and the component flags `AbsoluteTranslation`, `AbsoluteRotation`, `AbsoluteScale`: the
project's tools refuse or skip them and their place in the game's matrix is not read. `component_to_world` must not be
used for a component that sets one.
