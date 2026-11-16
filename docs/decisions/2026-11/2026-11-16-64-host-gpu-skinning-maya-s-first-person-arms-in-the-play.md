## 2026-11-16: Host GPU skinning: Maya's first-person arms in the play window (H6 stage 2)

AI-assisted (Claude Code). Lane H6-SKIN, stage 2 of H6 (stage 1: decision 2026-11-16-53), on top of H2 stage 2
(decision 51), H4-PLAY (decision 57), H7's AnimTree evaluation (decision 56) and H8's materials (decision 58).
Number 64 was assigned by the hub (58, first taken here, is H8-MATERIALS'). There was no real-game session; nothing about how the game draws or
places its first-person arms has been seen in the running game, so everything below is **UNVERIFIED** there.
The checks are against stage 1's CPU reference and H4-PLAY's path digests.

- **What changed.**
  - New:
    - `crates/ow-host/src/anim/gpu.rs` and `anim/gpu/tests.rs`: GPU skinning;
    - `crates/ow-host/src/anim/arms.rs` and `anim/arms/tests.rs`: the arms in the play window.
  - Two `pub mod` lines in `anim/mod.rs`.
  - `anim/tree/eval.rs`: `DriverState`, `Driver::into_state` and `Driver::resume`, so an owner of the tree, clips
    and skeleton can keep the driver's state between frames; tested against an uninterrupted driver.
  - `anim/tree/report.rs`: `locate` and `library` made `pub(crate)`.
  - `--arms DIR` for `--play`, in `args.rs` and `main.rs` (with its parse tests).
  - `play.rs`: `PlayDrawer::with_arms`, plus four small hooks (attach, snapshot, encode, screenshot) that do nothing
    without arms.
  - `render.rs`, `draw.rs`, `frame.rs` and H8's material files are unchanged. Rebased on H8-MATERIALS: the play
    mode's scene is drawn with H8's materials, lights and `eye` as `main` sets it up, and the arms pass follows the
    lit, materialled scene in the same frame and screenshot. No dependency is added (wgpu and winit
    are the approved ones).
- **GPU skinning** (`anim/gpu.rs`, our own WGSL written from the wgpu 30.0.1 API).
  - A compute pass (`cs_main`, 64 vertices a group) skins each vertex's position and normal through its up to four
    bones' palette matrices, weighted as stored. This is the linear blend of stage 1's `anim::skin`, the CPU
    reference. The skinned vertices go to a buffer that the draw then binds as its vertex buffer, so the GPU test's
    readback is the drawn data itself.
  - The palette is one column-major matrix per bone from `frame::Affine::to_engine`, so `frame.rs` stays the one
    conversion point: each bone's skin matrix, then the mesh's placement.
  - Refused at upload: more than 256 bones, a vertex naming a missing bone, more than four influences, an index past
    the vertices. Refused when skinning: a palette of the wrong length or with a non-finite entry.
  - **Normals** are built at the bind pose from the faces (an ActorX mesh has none). They are area-weighted, summed
    per welded point, and oriented against UE3's index order as `crate::render` settled it. A synthetic triangle
    checks the sign.
  - **Look:** one flat skin colour (a host choice) under the scene's sun and ambient, both faces drawn. No texture
    or material: H8 owns them.
  - **Depth:** a second pass after the scene, with its own depth buffer cleared, so the arms are never hidden by a
    wall. The game's first-person depth handling is not read; this is a host choice.
- **Pose and placement** (`anim/arms.rs`).
  - The hands tree `GD_PlayerShared.Character.AnimTree_PlayerHands` is driven with no input set: its idle path,
    with the AnimSets of H7's report (the hands component's three, plus the pistol's for the weapon). Its two
    `NotYet` nodes are passed to their first child, as in that report.
  - The clock is the simulation's (`Snapshot::sim_time`): the pose advances by the simulated time since the last
    drawn frame, so a scripted run poses the same way whatever the frame times.
  - **Placement (a host choice, UNVERIFIED):** the posed `Camera` bone is put at the eye with its axes on the
    view's. The mesh's placement is the inverse of that bone's model matrix, then the eye's (`frame::Placement` of
    the eye and the view rotation, unit scale). The retired UE5 host anchored the arms on the same bone. The game's
    rule, which `WillowAnimNode_AddCameraBone` (`NotYet` in H7) takes part in, is not read.
- **Proof.**
  - **Synthetic** (`anim::gpu::tests`, `anim::tree::tests`):
    - bind normals face out of UE3's index order;
    - a 3-bone strip, at its bind pose and bent, skinned on the GPU equals the CPU reference within 1e-4 uu (placed
      100 uu away, within 1e-3);
    - bad meshes and palettes are refused;
    - the skinned strip is drawn where the CPU puts it (a 64 x 64 capture);
    - a resumed driver matches an uninterrupted one.

    The GPU tests need an adapter and are `#[ignore]`d in the ordinary run, as `render::tests` are; on the desktop
    (RTX 4070) all pass.
  - **Installed game, headless** (`anim::arms::tests::installed_gpu_matches_cpu`; `local/p2/h6-skin/gpu_vs_cpu.txt`):
    - `Hands_Siren` (47 bones, 2,204 vertices, 3,692 triangles), posed by the hands tree at six times 0.5 s apart.
    - GPU against CPU: worst 3.05e-5 uu in model space.
    - Placed at an eye about 3,600 uu from the origin: worst 4.88e-4 uu, one `f32` step at that size.
    - The test fails above 1e-4 uu (model space) and 1e-3 uu (placed).
  - **Installed game, in the window** (under `tools/ue_lock.ps1`):
    - Run: `ow-host --game <install> --map Sanctuary --play --play-script walk60.txt --arms <export folder>
      --size 1280x720 --screenshot <png> --no-vsync`, with H4-PLAY's 60 s script.
    - Exit 0. All twenty segment digests ran, and the end digest is `d7b95ebfe14df774`, H4-PLAY's baseline: drawing
      the arms changes nothing in the simulation.
    - 69,153 frame intervals, median 0.796 ms (H4-PLAY's baseline without arms: 0.680 ms), p95 1.120 ms.
    - The start and end screenshots (`local/p2/h6-skin/play_arms.png`, `play_arms_end.png`, kept local) show the
      right hand at the lower right of the view, shaded, through the whole walk.
    - Whether that matches where the game draws Maya's idle hands has not been compared with the game.
- **Not done (optional in the brief):** the third-person body, the weapon mesh, textures.
- **Open items.**
  - The game's first-person placement and depth rules (the camera bone, `WillowAnimNode_AddCameraBone`, the
    first-person field of view).
  - A comparison with a frame of the running game (a capture-session ask).
  - The arms' material (H8).
  - The body mesh in third person.
  - The weapon AnimSet joining the component at run time (H7's open item).
  - A GATES entry for the GPU tests and the arms window run (proposed in the lane handoff).
