## 2026-11-19: Rust slice gate link 2: a headless Sanctuary walk through the shared mover

AI-assisted (Claude Code). Lane LINK-2-HEADLESS-MOVER; the hub gave number 66. Slice link 2 (`reach_fight_area`) had
no headless driver, because H4's mover (decision 2026-11-16-52) lived in `ow-host`, which `ow-cli` does not depend on.
This lane moves the renderer-free scene loader, game-frame maths and mover down to a crate both binaries depend on.
It then gives `ow-package --slice-gate` a link-2 driver that walks Sanctuary through that same code. The movement
rules are not copied, and none changes. Parsing does not change. Nothing here was checked in the running game.

- **Why `ow-assets`.** The code reads `ow_assets` (meshes, textures and body setups), so its home must reach
  `ow-assets`.
  - **`ow-world`, the hub's first pick, cannot reach it.** `ow-world` may depend only on `ow-vm` and `ow-balance`
    (architecture section 2.2, `tests/crate_edges_test.py`). An `ow-world` to `ow-assets` edge would change the
    architecture gate, and that is the maintainer's call.
  - **A new crate would change the gate too.** It would be a 23rd crate, and the test counts 22.
  - **`ow-assets` takes no manifest change.** It already depends on exactly what the code reads (`ow-core`,
    `ow-package`, `ow-object` and `ow-defs`), and both `ow-cli` and `ow-host` already depend on it. Its scope already
    names "level placement".
  - **The grant.** The hub granted the three `pub mod` lines in `crates/ow-assets/src/lib.rs`, whose header says
    lanes do not edit it. Neither manifest nor `crate_edges_test.py` changes.
  - **Later.** If the maintainer ever adds an `ow-world` to `ow-assets` edge, the modules can move to `ow-world`
    whole. The three host re-exports keep their paths either way.
- **What moved** (with `git mv`; the bodies are unchanged except where these points say):
  - `ow_assets::scene`, from `ow-host/src/scene.rs`. Its crate-private helpers (`Scene::empty`, `read`,
    `read_object` and the like) are now `pub`, because `ow-host`'s material, lighting and draw code uses them. Its 28
    `#[non_exhaustive]` attributes are removed, so that `ow-host` can still build the scene's structs. Inside one
    crate the attribute restricts nothing, so the host keeps what it had.
  - `ow_assets::movement`, from `ow-host/src/movement*` (collision, geometry, level collision, mover, walk and the
    Sanctuary runner). `sanctuary::load`, `resolve_map` and `run` take the install's cooked folder instead of
    `ow-host`'s `Install`, and the host passes `&install.cooked`.
  - `ow_assets::placement`, new: the game-frame half of `ow-host/src/frame.rs` (rotator wrap and axes, `Affine`,
    `Ue3Matrix`, `Placement`, `component_to_world`, `rotator_forward`, `direction_to_rotator`).
- **What stayed in `ow-host` (R9).**
  - **`frame.rs`, the engine frame.** The engine matrix, the view and projection matrices, positions to and from
    the engine, and a matrix back to a rotator. `Affine::to_engine` is now the `frame::ToEngine` trait, because
    `Affine` lives in another crate. `frame.rs` re-exports `ow_assets::placement`, so every `crate::frame::` path is
    unchanged.
  - **`movement.rs` and `scene`** re-export the moved modules whole, under their old paths.
  - **The scene loader's two installed-game drivers.** They use the host's install, material and lighting modules,
    so they moved to `ow-host/src/scene_install_tests.rs`.
  - **The tests.** Both crates hold 263 `#[test]` functions together, before and after.
- **The link-2 driver** (`crates/ow-cli/src/cmd/slice_gate.rs`, `sanctuary-walk`).
  - **The steps.** `load` (`sanctuary::load`, Sanctuary_P and its collision), `spawn` (`sanctuary::spawn`, Maya at
    the first player start), and `walk` (`walk::run` of `SANCTUARY_SCRIPT` at 1/60 s, as `ow-host --walk` does).
  - **The evidence.** It is the walk's own two checks (no fall through the world, no stuck segment), in the pure
    `walk_graded`. A walk that fails them is an `error` at `walk`.
  - **The fight area.** A walk that passes goes on to `fight_area`, which answers the named `NotYet`
    `fight_area_not_yet`: "needs a fight area placed in the map world" (SLICE_RUN_PLAN.md section 3.2). The gate needs
    enemies placed where Maya can reach them, and whether that is Sanctuary's own population or a fixture is not
    chosen. The walk is not taken as a stand-in for it, so the row is not `ok`.
  - **The harness.** The row's `harness` holds the spawn's own lines.
  - **The checker.** `tools/check_slice_run_off.py` accepts `sanctuary-walk` and ties it to link 2.
- **Verified on invented data:** the `ow-cli` gate tests pass 17 of 17, one of them new: link 2's driver and steps;
  a passing walk is evidence and stops at `fight_area` as a `NotYet`; a failing one is an `error` at `walk`.
  `tests/check_slice_run_off_test.py` passes 15 of 15, one of them new: the walk driver drives link 2 only.
  `tests/crate_edges_test.py` passes 19 of 19 with neither manifest touched. `cargo test --workspace` passes 2,074,
  with 0 failed and 45 ignored.
- **Formatting and lints.** CI runs neither. On `ow-assets`, `ow-host` and `ow-cli`, `cargo fmt --check` reports 561
  hunks after against 562 before, so the tree's own drift is unchanged and this lane adds none. `cargo clippy
  --workspace --all-targets` reports 48 warning sites, none of them on a line this lane added.
- **Installed game, Off** (desktop, 2026-11-19, on `origin/main` 7e80d7b; outputs under ignored `local/p2/link2/`):
  - **`--slice-gate`, link 2 before (strict):** `notyet`, driver `none`, no steps, stopped with "no headless
    driver: the only mover is H4's in ow-host ...".
  - **After, strict and past-errors alike:** `notyet`, driver `sanctuary-walk`, steps `load`, `spawn`, `walk` and
    `fight_area`. The walk passed its own checks, and the link stops at `fight_area` on "the fight area of link 2:
    needs a fight area placed in the map world" (record SLICE_RUN_PLAN.md section 3.2). The harness lines are the
    spawn's: `WillowCoopPlayerStart_0`, lifted 1 uu out of solid.
  - **The other 13 rows** are byte-identical before and after (strict). The counts stay 0 ok, 14 notyet, 0 error,
    and `tools/check_slice_run_off.py` reads the run as well-formed (gate not passed).
  - **`ow-host --game <install> --walk`**, built from `origin/main` and from this branch (release), run under the
    machine lock. The two 98-line reports are identical except the scene's load time: every segment digest, the
    result line (PASS, 1050 ticks, travelled 3562.2 uu) and the path digest `fd6551c9598c8ec7`.
- **UNVERIFIED** in the running game: every movement rule (decision 2026-11-16-52), and where the fight area is.
