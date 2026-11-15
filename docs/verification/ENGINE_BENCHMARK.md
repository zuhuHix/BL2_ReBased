# Engine benchmark (Phase 0R)

AI-assisted (Claude Code). Record created 2026-11-14 with the decision that Rust replaces the UE5 host
([decision](../decisions/2026-11/2026-11-14-01-rust-replaces-the-ue5-host-engine-route-by-benchmark.md)).

**Status: RUN, route picked (2026-11-15).** Two candidates were built and measured on 2026-11-14: Bevy (lane B1, on
pc-hummer) and an own engine on wgpu + winit (lane B2, on the zuhu desktop). Fyrox, the optional one, was not built.
The maintainer picked **the own engine on wgpu + winit**
([decision](../decisions/2026-11/2026-11-15-01-engine-route-own-engine-on-wgpu-and-winit.md)). The wgpu numbers
below are as that lane's results file and logs give them (`local/bench/wgpu/`, ignored, on the zuhu desktop). The
Bevy numbers come from the B1 lane's report; its results file (`local/bench/bevy/results.md`) is on pc-hummer, not in
this checkout. Nothing was measured beyond them.

## Purpose

This benchmark built the same small scene in each candidate so the maintainer could pick the host engine route from
measurements instead of from reputation. It was timeboxed: it answers "which route do we build the slice on", not "is
the slice done".

## Read this first

- **Not like for like.** The two prototypes ran on different PCs (B2: i9-9900K with an RTX 4070; B1: i7-14700K with an
  RTX 5070 Ti) and drew the scene differently (B2: 784 instanced draws; B1: about 4,500 un-instanced meshes with
  culling off). Their frame times do not compare directly. No cross-machine run was made, and zuhu-laptop (Linux) ran
  neither.
- Both are far under a 16.7 ms frame on this scene.
- Neither is BL2's look: one diffuse texture per material, a sun and ambient, no normal maps, lightmaps, fog or
  shadows. Nothing here is a parity result.

## Candidates

| Candidate | Required | Version built | Lane, machine | Prototype folder |
|---|---|---|---|---|
| Bevy | yes | 0.20.1 (needs Rust 1.97.1 or newer; the lane used a local 1.99.0, the repository pins 1.96.0) | B1, pc-hummer | `local/bench/bevy/` |
| Own engine on wgpu + winit | yes | wgpu 30.0.1, winit 0.30.13, Rust 1.96.0 | B2, zuhu desktop | `local/bench/wgpu/` |
| Fyrox | optional | not built | none | none |

## The scene

The same in every candidate:

1. Sanctuary's static meshes, from the existing local exports (game-derived, read from ignored `local/`, never copied
   into the prototype's tracked files or into this record).
2. Maya's skeletal mesh playing one animation.
3. A capsule that walks on Sanctuary's collision.
4. One BL2 movie shown through Ruffle (Ruffle is itself written in Rust, so whether it embeds natively is one of the
   questions).

The scene reads the player's own install and local exports only (rule 11); nothing game-derived is packaged with a
prototype. B2 wrote the shared camera path (a 60 s orbit over the plaza, 5 s warm-up, vsync off, no MSAA, shadows or
post pass); frame time is the interval between successive frame starts, reported as median and 95th percentile.

## What each prototype built

- **wgpu + winit (B2).** Sanctuary: 4163 of 4470 placements drawn (the `hidden_visual` ones skipped), 1,280,463
  triangles, 270 textures (0 failed), 784 instanced draws, no frustum culling. Maya: the body only, playing
  `Idle_var1` with GPU skinning; head and hands were not exported locally, and that the clip's axis mapping matches
  the game is `UNVERIFIED`. Capsule: rapier3d's kinematic character controller on 4209 convex hulls and a
  10,961-triangle terrain and BSP mesh; falls, stairs and blocking volumes not checked. Ruffle: embedded natively,
  sharing the engine's wgpu instance, device and queue (both wgpu 30.0.1), measured with
  `UI_CharacterPortraits/Siren.swf`; `HUD.swf` and the Sanctuary tactical map load and render but draw nothing without
  game script driving them.
- **Bevy (B1).** Sanctuary with textures, Maya playing `Idle_var1`, a capsule walking on the collision through
  avian3d. Ruffle **not** embedded: its crates come from git, outside that lane's crates.io-only brief. Maya kept
  Bevy's default culling, textures had no mipmaps, and the light strengths were the lane's choice.

## Measurements

| Measurement | Bevy 0.20.1 (B1, pc-hummer) | wgpu + winit (B2, zuhu desktop) | Fyrox (optional) |
|---|---|---|---|
| Clean build time | release 2 min 41 s, dev 4 min 24 s (`-j 16`) | release 3 min 04 s with Ruffle, 1 min 52 s without; dev 6 min 55 s with Ruffle (`-j 8`, 1 run each, downloads excluded) | not built |
| Incremental build time (one-line change in our code) | release 5.3-5.7 s, dev 8-11 s | release 8.72 / 8.81 s, dev 7.68 / 6.77 s (2 runs each) | not built |
| Frame time, 1280x720 | median 1.33 ms, p95 1.80 ms (3 runs of 60 s pooled, release) | median 0.527 / 0.535 / 0.529 ms, p95 0.607 / 0.976 / 0.599 ms (3 runs of 60 s, release) | not built |
| Frame time, 1920x1080 | median 1.35 ms, p95 2.10 ms (3 runs pooled); one hitch of about 538 ms in each third run, cause not found | median 0.528 / 0.573 / 0.679 / 0.604 ms, p95 0.701 / 1.200 / 1.420 / 1.471 ms (4 runs) | not built |
| Frame time on the other PCs | not measured on the zuhu desktop or zuhu-laptop | not measured on pc-hummer or zuhu-laptop | not built |
| Draws | about 4,500 meshes, culling off (B1's count; that this makes the frame CPU-bound is the lane's unprofiled guess) | 784 instanced draws, no culling | not built |
| Memory (process, scene loaded) | process RAM 2.4-3.0 GiB; GPU not measured | peak working set 2053-2101 MiB, peak private bytes 4385-4423 MiB, GPU +2298-2448 MiB | not built |
| Binary size (release) | 96.8 MiB, no Ruffle | 21.5 MiB (22,562,304 bytes) with Ruffle, 11.1 MiB (11,591,168 bytes) without | not built |
| Lines of our own code | 754 | 1821 (1724 Rust, 97 WGSL; 103 of the Rust lines embed Ruffle) | not built |
| What was missing and had to be written | not summarised here (B1's results file is on pc-hummer) | renderer (pipelines, instancing, reverse-Z depth, mips, sRGB), scene and OBJ loading, glTF skin and clip player with GPU skinning, rapier collider glue, Ruffle render target, benchmark harness | not built |
| Ruffle movie shown natively, or how | not embedded (git dependency, outside the brief) | yes, on the engine's own wgpu device; `ruffle_core` needs a Java JDK at build time | not built |
| Hosting the `!Send` simulation (D18) | tested on the main thread: a 43-line test held it as a non-send resource and stepped it from host messages, 300 steps; systems touching it are pinned to the main thread while others still run in parallel; its state cannot live in components, ordinary resources or `Send + Sync` plugins; a separate thread with channels was described, not tried | on the event-loop thread, or on a simulation thread with channels; neither ruled out | not built |

The B2 numbers were taken with the release binary while holding the machine run lock: frame times with `Instant` at
each redraw (throughput, not GPU time), memory by polling the started PID every 250 ms, and the GPU with
`nvidia-smi`. The run-to-run spread at 1080p (0.53 to 0.68 ms median) is noise: desktop apps were open and GPU clocks
were not pinned.

## Dependencies seen

Direct dependencies of the wgpu + winit prototype, with licences as its results file lists them (each crate's licence
file, or its Cargo `license` field where the published crate has none). **To be confirmed in `THIRD_PARTY.md` before
any of them lands** in the tree (`AGENTS.md` rule 6). Nothing has been added.

| Crate | Version | Source | Licence (as seen) |
|---|---|---|---|
| wgpu | 30.0.1 | crates.io | MIT OR Apache-2.0 |
| winit | 0.30.13 | crates.io | Apache-2.0 |
| glam | 0.33.12 | crates.io | MIT OR Apache-2.0 |
| bytemuck | 1.25.2 | crates.io | Zlib OR Apache-2.0 OR MIT |
| pollster | 1.0.1 | crates.io | Apache-2.0/MIT |
| serde, serde_json | 1.0.229, 1.0.154 | crates.io | MIT OR Apache-2.0 |
| image (PNG only) | 0.25.10 | crates.io | MIT OR Apache-2.0 |
| gltf | 1.4.1 | crates.io | MIT OR Apache-2.0 |
| rapier3d | 0.36.1 | crates.io | Apache-2.0 (Cargo field; no licence file in the published crate; upstream LICENSE is Apache-2.0) |
| ruffle_core, ruffle_render, ruffle_render_wgpu | 0.7.1, git rev `aa1deca6522ed748f22bfe1e5385ea33e133ecf1` | github.com/ruffle-rs/ruffle (not on crates.io) | MIT OR Apache-2.0 (repository LICENSE.md) |

- The resolved graph with Ruffle has 473 packages. Transitive licences seen include Unicode-3.0, Zlib, BSD-3-Clause,
  BSD-2-Clause and ISC. Two crates are dual-licensed with a GPL or LGPL option (`self_cell`, `r-efi`), where the
  permissive side would be taken; that choice is part of the `THIRD_PARTY.md` review.
- Build-time only: a Java JDK for `ruffle_core`'s build script (B2 used the Temurin JDK 21 already listed for Ghidra).
- serde, serde_json, image and gltf read the benchmark's local exports; whether the host needs them is open.

## How to run

- Prototypes live under ignored `local/bench/<candidate>/`, outside the Cargo workspace, because each candidate is a
  new dependency that is not yet in `THIRD_PARTY.md`. This section is kept for a re-run.
- One run per machine at a time. On the Windows PCs a run takes the machine-wide run lock from `tools/ue_lock.ps1`
  (`Enter-OwUeLock -Owner <name>`), the same lock the real-game helpers use, and releases it when the prototype has
  exited. How a run is serialised on the Linux laptop is not settled yet; settle it before the first run there.
- Machines: the zuhu desktop (Windows), pc-hummer (Windows) and zuhu-laptop (Linux). A candidate that does not build
  or run on one of them is a result; record it, do not drop the machine.
- Process safety: never kill a process by image name (`taskkill /IM`, `Stop-Process -Name`, `pkill`, `killall`);
  stop only a PID your own run started, after checking its command line. Never delete a lock you do not hold.
- Evidence for anything about BL2 itself still comes from the usual tools: bl-sdk live reads of the running game
  first (the RTSE way, `tools/real_game/`), REA (the `rea` MCP server) for decoding, Ghidra for `Borderlands2.exe`.
  The benchmark does not need any of them; it reuses exports that already exist.

## Decision

The maintainer picked the own engine on wgpu + winit on 2026-11-15
([decision](../decisions/2026-11/2026-11-15-01-engine-route-own-engine-on-wgpu-and-winit.md)). The reasons given
there: Ruffle embeds natively on the same wgpu device (BL2's UI is Flash); an about 4.5-9 times smaller binary (21.5 MiB with Ruffle, 11.1 MiB without, against 96.8 MiB); no ECS
scheduler imposing `Send` rules on the simulation; no forced toolchain bump; and Bevy's renderer, animation and scenes
would not match BL2's AnimTree, materials or Kismet anyway. The cost: about 2.4 times as much own code in this scene,
and an animation and material pipeline written by hand. Each engine crate's `THIRD_PARTY.md` entry comes before any
of its code or crates land in the tree.

## What this benchmark does not prove

- Parity with the original game: a scene that renders, animates and walks is not BL2's look, movement or feel.
- Gameplay: no VM, natives, combat, missions, UI or saves run in the benchmark scene.
- That the Rust core matches the C++ core: that is the separate port check (same synthetic fixtures, and
  `tools/verify_packages.py --reader` giving the same results), not part of this record.
- Long-term cost: build times and code size on a small scene say little about the full slice.
