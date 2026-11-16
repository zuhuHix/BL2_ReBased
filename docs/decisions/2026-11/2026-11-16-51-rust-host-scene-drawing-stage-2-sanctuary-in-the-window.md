## 2026-11-16: Rust host scene drawing stage 2: Sanctuary in the window

AI-assisted (Claude Code). Lane H2-DRAW, stage 2 of the host track's H2 (architecture section 8). It draws the scene of
decision 2026-11-16-46 in H1's window (decision 2026-11-16-44), through H3's boundary module (decision 2026-11-16-45).
The scene is drawn at the B2 benchmark's level: one diffuse texture, plain diffuse light from one sun, an ambient
term. Nothing here claims BL2's look, which stays **UNVERIFIED**; a visual critic run comes later. No dependency was
added (wgpu and winit only), no bounds check was loosened and no decoder changed. There was no real-game session.

Edits: docs/decisions/2026-11/2026-11-16-46-rust-host-scene-loading-stage-1-sanctuary-as-plain-data.md
(two wording lows of review r2 of PR #346: the null-entry rule is "one null per level, the tool stops on a second",
and the claim that no Sanctuary mesh has a triangle-less section now says how it was checked.)

- **What was added.**
  - New files: `crates/ow-host/src/draw.rs` (the draw plan, no GPU), `render.rs` (wgpu: pipelines, upload, the frame,
    an offscreen capture, the `Drawer` hook) and `camera.rs` (a fly camera), each with its tests.
  - The seam into H1's files, kept minimal:
    - `app.rs`: `run_with(options, sim, drawer)`, which attaches a `Drawer` once the device exists, lets it draw each
      frame instead of the clear colour, passes it window events and raw mouse motion, and opens the window at the
      size it asks for. `run` is `run_with(.., None)`, unchanged.
    - `gpu.rs`: `device()`, `queue()`, `format()`, and `render_with(draw)`. The old `render(clear)` now calls
      `render_with` with the same clear pass. There is one new error variant, `GpuError::Draw`. The grant named
      `app.rs` and `main.rs`; `gpu.rs` is H1's too, and the owner should look at this change.
    - `args.rs`: `--map NAME`, `--size WxH`, `--screenshot PATH`, as a new `Command::Scene`. H1's variants and tests
      are untouched, and `--walk` (H4) refuses `--map`. This file is also outside the grant's two.
    - `main.rs`: `scene_window`, which loads the scene, builds the plan, places the camera and runs the window.
  - Changes in `scene.rs` (stage 1's, owned):
    - `export_class` maps only "class not found" and "package not found" to `None`; any other store error is now a
      `SceneError` (an r2 low).
    - `MaterialTexture.depth` (the entry's place in the parent chain) and `SceneMaterial.two_sided` (the base
      material's `TwoSided`) are new.
    - Base-class lookups are cached.
- **How the scene is drawn.**
  - **Transforms.** An attached placement uses `frame::component_to_world`; a collection placement uses
    `frame::Affine::from_ue3_matrix` with its rows times Scale3D times scale, in `f64`.
  - **Batches.** A placement hidden by its serialized flags is not drawn. Batches are keyed by mesh, section, diffuse
    texture, sidedness and mirroring; each is one instanced, indexed draw.
  - **Front faces.** `FrontFace::Ccw` for an unmirrored draw and `Cw` for a mirroring one (`Affine::mirrors`). This
    settles H3's open question in two parts:
    - A drawn test (below) shows that a triangle whose index order runs against its stored normal faces the camera
      with `Ccw`, through the boundary's view and projection.
    - On Sanctuary, 175,567 of 175,722 unique LOD 0 triangles (99.91%) run against their stored normals; 124 run
      along them and 31 are flat. Weighted by drawn instances: 1,279,080 against, 820 along, 200 flat, of 1,280,100.
  - **Culling.** One-sided materials cull back faces. Two-sided ones cull nothing and light the back face with the
    flipped normal. A material whose chain gives no `TwoSided` is drawn two-sided, so no guess hides anything.
  - **Depth.** Reverse-Z with no far plane (`perspective_reverse_z_infinite`), `Depth32Float`, compare `Greater`,
    cleared to 0. Near plane 10 uu; vertical field of view 70 degrees. Both are viewer choices, not the game's.
  - **Diffuse.** Picked by parameter name, with the rule of `tools/prepare_level.py`:
    - its parts: `material_parameters`, `CHANNELS` and `ALIASES`, `channel_for_parameter`, `parameter_priority`, and
      the channel loop of `Scene.material`;
    - a child's parameter replaces its parent's of the same folded name, and base-material sample parameters give
      the defaults;
    - among the diffuse names, a real texture beats a `Common_Textures.Stub` one, and higher priority beats lower.
    Three differences from the tool:
    - its inferences for materials with no named diffuse are not ported: a sole unnamed `_Dif` sample, cooked
      texture resources, inspected fallbacks and recipes;
    - a child's explicit null override does not hide its parent's texture here, because the scene records no null
      entries;
    - the `MaterialExpressionTextureSampleParameter2D_<n>` names are not kept; only those inferences use them.
    Textures are decoded on demand through `Scene::decode_texture` (`ow-assets`), one at a time during the upload.
    Each is uploaded as `Rgba8UnormSrgb` with every resident mip that halves the one before.
  - **Camera.** A fly camera (WASD, Q and E down and up, Shift faster, mouse look while the right button is held),
    starting at the first player start raised by 100 uu, as `prepare_level.py` raises its camera. Movement and
    collision are H4's; this camera does neither.
- **Verified (automated).** In the ordinary `ow-host` tests:
  - the draw plan on an invented scene: each mesh uploaded once, the vertex and index bytes, placements batched with
    their own matrices, the hidden, no-mesh and empty-section counts;
  - the mirrored batch key, and the collection scale;
  - the diffuse pick by parameter name: the child override, the stub rule, the priority and the aliases;
  - the camera's start, movement, turning and reverse depth;
  - the mip chain, and the frame-time quantile;
  - the new command-line forms.
  Two drawn tests run offscreen with no window but need a GPU adapter or the software fallback, so they are
  `#[ignore]`d. Both passed on this PC.
  - `a_known_triangle_renders_with_each_winding` checks a triangle in four cases: its stored order is drawn and its
    reverse culled, both unmirrored and mirrored, and two-sided draws both. With the two front-face constants swapped
    it fails ("the UE3-ordered triangle is culled"), so it does tell them apart.
  - `the_upload_counts_what_it_sends` covers bytes and counts, and a texture that fails is drawn grey.
- **Verified (local, installed game, pc-hummer).** Release build on main 7a27b2a plus this lane. NVIDIA GeForce RTX
  5070 Ti, Vulkan, driver 616.92. Four runs under the machine lock (`tools/ue_lock.ps1`), each 3,000 frames with no
  vsync. Outputs are under ignored `local/p2/h2-draw/` only.
  - **Load.** The scene loads in 42.1 to 43.1 s and the plan builds in 17 ms. The upload takes 1.68 to 1.70 s:
    - 418 meshes (6,485,184 vertex and 2,108,664 index bytes);
    - 284,480 instance bytes;
    - 195 textures (1,519,272,512 bytes of uncompressed mips), none drawn grey.
  - **What is drawn.** 4,141 placements drawn (305 hidden, none without a mesh, no refused transform, 374
    mirrored), as 4,445 section instances in 596 instanced draws:
    - 86 of the draws are mirrored, 35 two-sided, and 160 untextured (their materials have no named diffuse);
    - 249 of the 381 materials have a named diffuse.
  - **Frame time.** Between successive presents, nearest rank:

    | Size | Run | Median | p95 |
    |---|---|---|---|
    | 1280x720 | a | 0.461 ms | 0.589 ms |
    | 1280x720 | b | 0.447 ms | 0.519 ms |
    | 1920x1080 | a | 0.451 ms | 0.577 ms |
    | 1920x1080 | b | 0.453 ms | 0.575 ms |

    The near-equal times at both sizes suggest the frame is not fill-bound (not profiled). These are not comparable
    with B2's (another PC, another renderer).
  - **Screenshots** from the player start, at 1280x720 and 1920x1080, are under `local/p2/h2-draw/`. What they show
    is recorded as seen, UNVERIFIED as the game's look:
    - the plaza and buildings textured and lit, with no inside-out faces;
    - a grey sky, which is the untextured sky dome (its master graph is not in the cooked package);
    - white planes where the game's surfaces are not opaque (clouds, translucent and masked materials, all drawn
      opaque here);
    - a large brown box in front of the start, probably one of the `BoxLrg` blocking placements the tool hides by
      rule (not checked).
  - **The load took 42-43 s here, where stage 1 measured 23-25 s for the same scene code on this PC.** The lower
    crates did not change between the two bases. The stage-1 driver rerun today took 44.8 s, and its texture decode
    3.7 s against 1.8 s, which points at the machine's state, not the code; the cause was not found.
- **Against the benchmark record** (`docs/verification/ENGINE_BENCHMARK.md`: 784 instanced draws, 4,163 drawn
  placements).
  - **Drawn placements, 4,141 against 4,163.** Today's `prepare_level.py` rerun draws 4,139: it hides 307 where the
    scene hides 305, by drawing 13 hidden-flagged `InterpActor`s and hiding 15 unflagged helper meshes (decision 46).
    4,141 − 13 + 15 = 4,139 on the same 4,446 placements. The other 24 are B2's manifest: an inference from the
    record's numbers, not checked (decision 46).
  - **Instanced draws, 596 against 784.** B2's batch key is not in the record, and its source is on the zuhu desktop,
    so this is not explained. For comparison on today's set, other keys give:
    - (mesh, section, material, mirrored): 688;
    - (mesh, section): 493;
    - (mesh, material): 600.
    Fewer placements, merging by texture rather than material, and a different section split would each lower B2's
    count to the scene's; which of them applies is open.
- **Not verified.**
  - Everything about the game's look.
  - The fly camera's keys and mouse in a live session: they are covered by unit tests only, since no one used the
    window interactively.
  - Whether the game culls the same faces: the data check covers index order against stored normals, not the
    game's renderer.
