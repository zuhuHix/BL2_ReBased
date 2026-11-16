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
    conversion point. It holds each bone's skin matrix in model space. The mesh's placement in the world is applied
    as one more matrix after the weighted sum, so a weight sum a little off 1 (the reader allows 1e-3) is not
    scaled by the world position (review r1). The weights are used as stored, as in the CPU reference; nothing is
    renormalised.
  - Refused at upload: more than 256 bones, a vertex naming a missing bone, more than four influences, an index past
    the vertices. Refused when skinning: a palette of the wrong length, or a palette or placement with a non-finite
    entry.
  - **Normals** are built at the bind pose from the faces (an ActorX mesh has none). They are area-weighted, summed
    per welded point, and oriented against UE3's index order as `crate::render` settled it. A synthetic triangle
    checks the sign.
  - **Look:** one flat skin colour (a host choice), both faces drawn, lit by the scene's own `Lighting` (H8) with
    the scene shader's rule: the sun along its direction plus the sky's two hemispheres by the normal's height,
    times the exposure. Before review r1 it used stage 2's fixed sun and ambient, which made the hand look nearly
    white in H8's exposed scene. No texture or material: H8 owns them.
  - **Depth:** a second pass after the scene, with its own depth buffer cleared, so the arms are never hidden by a
    wall. The game's first-person depth handling is not read; this is a host choice.
  - **Their own projection (a host choice, UNVERIFIED):** the scene's vertical field of view with a 1 uu near plane
    (`anim::arms::ARMS_NEAR`), through `frame::perspective_reverse_z_infinite`. The scene's 10 uu near plane cut the
    forearms where they run back toward the eye (review r1). The arms clear their own depth, so the two planes do
    not meet.
- **Pose and placement** (`anim/arms.rs`).
  - The hands tree `GD_PlayerShared.Character.AnimTree_PlayerHands` is driven with no input set: its idle path,
    with the AnimSets of H7's report (the hands component's three, plus the pistol's for the weapon). Its two
    `NotYet` nodes are passed to their first child, as in that report.
  - The clock is the simulation's (`Snapshot::sim_time`). The pose advances in whole simulation steps (1/60 s), the
    remainder carried to the next frame, at most 240 steps a frame. So the pose after `n` steps is the same whatever
    the frame times; which step a frame shows still depends on when it is drawn. (Before review r1 the pose ticked by
    each frame's time, so it depended on how the frame times summed.)
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
    - a resumed driver matches an uninterrupted one;
    - review r1: a weight of 0.999 placed 3,600 uu out lands 0.01 uu from weight 1's spot (it would be 3.6 uu with
      the placement inside the palette); the step count is the same for one frame or five of the same total, a
      stall is capped, and bad times change nothing; a point 2 uu ahead is inside the arms' clip volume and
      outside the scene's.

    The GPU tests need an adapter and are `#[ignore]`d in the ordinary run, as `render::tests` are; on the desktop
    (RTX 4070) all pass.
  - **Installed game, headless** (`anim::arms::tests::installed_gpu_matches_cpu`; `local/p2/h6-skin/gpu_vs_cpu.txt`):
    - `Hands_Siren` (47 bones, 2,204 vertices, 3,692 triangles), posed by the hands tree at six times 0.5 s apart.
    - GPU against CPU: worst 3.05e-5 uu in model space.
    - Placed at an eye about 3,600 uu from the origin: worst 4.88e-4 uu, one `f32` step at that size; 2.44e-4 uu
      after review r1 (skinned in model space, then placed). The hands' positions in the view's frame (through the
      inverse of the posed `Camera` bone): the right hand about 47 uu ahead, 17 right and 22 below the eye, the
      left about 45 ahead, 9 right and 24 below.
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
  - The game's first-person placement and depth rules (the camera bone, `WillowAnimNode_AddCameraBone`).
  - The game's first-person near plane and field of view (the arms use 1 uu and the scene's field of view).
  - A comparison with a frame of the running game (a capture-session ask).
  - The arms' material (H8).
  - The body mesh in third person.
  - The weapon AnimSet joining the component at run time (H7's open item).
  - A GATES entry for the GPU tests and the arms window run (proposed in the lane handoff).
  - Debt: `anim::arms::capture_with` repeats `SceneRenderer::capture`'s readback; the two should become one helper.
  - A new `Driver` is built from the saved state each frame (the palette's vector is reused).
