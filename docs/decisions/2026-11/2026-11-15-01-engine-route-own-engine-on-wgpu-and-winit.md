## 2026-11-15: Engine route: own engine on wgpu and winit

AI-assisted (Claude Code). Maintainer decision (zuhu, 2026-11-15), made in chat from the Phase 0R benchmark: the Rust
host is built as **our own engine on wgpu + winit**, not on Bevy. Fyrox, the optional candidate, was not built. The
pick follows the hub's recommendation, which the maintainer accepted. It closes the open engine question of
[the 2026-11-14 decision](2026-11-14-01-rust-replaces-the-ue5-host-engine-route-by-benchmark.md); the language choice
(Rust) is unchanged. Record: [ENGINE_BENCHMARK.md](../../verification/ENGINE_BENCHMARK.md).

**What the benchmark measured** (both prototypes under ignored `local/bench/`, outside the workspace). The wgpu
figures are from that lane's results file and logs on the zuhu desktop; the Bevy figures come from the B1 lane's
report on pc-hummer, whose results file is not in this checkout.

- **B2, own engine on wgpu + winit**, zuhu desktop (i9-9900K, RTX 4070, Vulkan backend, Immediate present). Built:
  Sanctuary's static meshes (4163 placements, 1.28 M triangles, 784 instanced draws, 270 textures); Maya's body
  playing `Idle_var1` (head and hands not exported); a rapier3d capsule walking on 4209 convex hulls plus the terrain
  and BSP triangles; Ruffle embedded natively on the engine's own wgpu device (both on wgpu 30.0.1), measured with
  `UI_CharacterPortraits/Siren.swf` (`HUD.swf` and the tactical map draw nothing without game script driving them).
  Frame time over a 60 s camera path, release build: 1280x720 median 0.53 ms, p95 0.60-0.98 ms (3 runs); 1920x1080
  median 0.53-0.68 ms, p95 0.70-1.47 ms (4 runs). Clean release build 3 min 04 s with Ruffle, 1 min 52 s without;
  clean dev build 6 min 55 s; one-line incremental 8.7 s release, 6.8-7.7 s dev. Peak working set 2053-2101 MiB,
  peak private bytes 4385-4423 MiB, GPU +2298-2448 MiB. Release binary 21.5 MiB with Ruffle, 11.1 MiB without. Own
  code 1821 lines (1724 Rust, 97 WGSL).
- **B1, Bevy 0.20.1**, pc-hummer (i7-14700K, RTX 5070 Ti). Built: Sanctuary with textures, Maya playing `Idle_var1`, a
  capsule walking on the collision through avian3d. Ruffle was **not** embedded (its crates come from git, outside
  that lane's crates.io-only brief). Frame time, 3 runs of 60 s pooled, release: 1280x720 median 1.33 ms, p95 1.80
  ms; 1920x1080 median 1.35 ms, p95 2.10 ms; about 4,500 meshes drawn with culling off (B1's count; that
  this makes the frame CPU-bound is the lane's own guess, not profiled); one hitch of about 538 ms in
  each third run, cause not found. Clean build with `-j 16`: release 2 min 41 s, dev 4 min 24 s; incremental release
  5.3-5.7 s, dev 8-11 s. Process RAM 2.4-3.0 GiB (GPU memory not measured). Release binary 96.8 MiB. Own code 754
  lines. Bevy 0.20.1 needs Rust 1.97.1 or newer; the repository pins 1.96.0, and the lane used a local 1.99.0. Maya
  kept Bevy's default culling, textures had no mipmaps, and the light strengths were the lane's choice.
- **Hosting the single-threaded simulation (D18 in `docs/rust-port/ARCHITECTURE.md`).** In Bevy, B1 tested the
  main-thread option: a 43-line test held the simulation as a non-send resource and stepped it from host messages, 300
  steps, all on the main thread. Bevy systems that touch it are pinned to the main thread while other systems still run
  in parallel, and its state cannot live in components, ordinary resources or `Send + Sync` plugins. The
  separate-thread-with-channels option was described, not tried. The own engine can run it on the event-loop thread or
  on a simulation thread talking through channels; the prototype rules out neither.

**Caveats.** The two prototypes ran on different PCs and with different draw strategies (784 instanced draws against
about 4,500 un-instanced meshes with culling off), so their frame times are not a like-for-like comparison. No
cross-machine run was made, and neither candidate ran on zuhu-laptop (Linux). Both are far under a 16.7 ms frame.
Neither prototype is BL2's look: simple diffuse-plus-sun shading, no normal maps, lightmaps, fog or shadows. The B2
lane deviated from its crates.io-only brief to embed Ruffle (git rev `aa1deca6522ed748f22bfe1e5385ea33e133ecf1`), and
`ruffle_core`'s build script needs a Java JDK (the lane used the Temurin JDK already listed for Ghidra). Nothing here
is a parity result, and every BL2 behaviour stays `UNVERIFIED` until checked against the running game.

**Why this route:**

- BL2's UI is Flash, and Ruffle embeds natively on the same wgpu device; that was shown in B2 and not attempted in B1.
- The binary is about 4.5-9 times smaller (21.5 MiB with Ruffle, 11.1 MiB without, against Bevy's 96.8 MiB without
  Ruffle).
- No ECS scheduler imposes `Send` rules on the deliberately `!Send` simulation (in Bevy its state would be kept out
  of components, ordinary resources and `Send + Sync` plugins, and the systems touching it pinned to the main
  thread).
- No forced toolchain bump: wgpu 30 builds on the pinned Rust 1.96.0; Bevy 0.20.1 would need 1.97.1 or newer.
- Bevy's renderer, animation and scene system would not match BL2's AnimTree, materials or Kismet anyway, so the
  project would replace much of them.

**What it costs:** more of our own code (about 2.4 times as much in the benchmark scene: 1821 lines against 754),
and the animation and material pipelines are written by hand.

**What follows:**

- Nothing is added to the tree by this decision. The engine crates (wgpu, winit, glam, bytemuck, pollster, rapier3d,
  the Ruffle crates `ruffle_core`, `ruffle_render` and `ruffle_render_wgpu` from git, and a Java JDK as a Ruffle build
  dependency) enter the workspace only with `THIRD_PARTY.md` entries the maintainer approves (AGENTS.md rule 6), when
  the host-track lane needs them. The licences the benchmark saw are listed in the record and are to be confirmed
  there first.
- `crates/ow-host` stays a skeleton that checks the install; its messages now say the route is chosen and the host is
  not written yet.
- The host's gates in `docs/verification/GATES.md` are set when the host-track lane starts, from this route.
- The route is not reopened (AGENTS.md rule 6); that is the maintainer's call.
