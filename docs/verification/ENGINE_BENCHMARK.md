# Engine benchmark (Phase 0R)

AI-assisted (Claude Code). Record created 2026-11-14 with the decision that Rust replaces the UE5 host
([decision](../decisions/2026-11/2026-11-14-01-rust-replaces-the-ue5-host-engine-route-by-benchmark.md)).

**Status: NOT RUN.** No candidate has been built, no prototype exists and no number below has been measured. Every
cell says "not measured" until a run fills it, and a run fills only what it saw.

## Purpose

No Rust engine is chosen yet. This benchmark builds the same small scene in each candidate so the maintainer can pick
the host engine route from measurements instead of from reputation. It is timeboxed: it answers "which route do we
build the slice on", not "is the slice done".

## Candidates

Licences are as stated in the decision. Each is **to be confirmed in `THIRD_PARTY.md` before any of it lands** in the
tree (`AGENTS.md` rule 6); until then the prototypes stay under ignored `local/bench/` and pull their crates there.

| Candidate | Required | Licence (as stated in the decision) | Prototype folder |
|---|---|---|---|
| Bevy | yes | MIT or Apache-2.0 | `local/bench/bevy/` |
| Own engine on wgpu + winit | yes | MIT or Apache-2.0 | `local/bench/wgpu/` |
| Fyrox | optional | MIT | `local/bench/fyrox/` |

Versions, source URLs and the exact crate list of each prototype are recorded here when it is built.

## The scene

The same in every candidate:

1. Sanctuary's static meshes, from the existing local exports (game-derived, read from ignored `local/`, never copied
   into the prototype's tracked files or into this record).
2. Maya's skeletal mesh playing one animation.
3. A capsule that walks on Sanctuary's collision.
4. One BL2 movie shown through Ruffle (Ruffle is itself written in Rust, so whether it embeds natively is one of the
   questions).

The scene reads the player's own install and local exports only (rule 11); nothing game-derived is packaged with a
prototype.

## Measurements

All cells: not measured.

| Measurement | Bevy | wgpu + winit | Fyrox (optional) |
|---|---|---|---|
| Clean build time | not measured | not measured | not measured |
| Incremental build time (one-line change in our code) | not measured | not measured | not measured |
| Frame time, 1280x720, zuhu desktop | not measured | not measured | not measured |
| Frame time, 1920x1080, zuhu desktop | not measured | not measured | not measured |
| Frame time, 1280x720, pc-hummer | not measured | not measured | not measured |
| Frame time, 1920x1080, pc-hummer | not measured | not measured | not measured |
| Frame time, 1280x720, zuhu-laptop (Linux) | not measured | not measured | not measured |
| Frame time, 1920x1080, zuhu-laptop (Linux) | not measured | not measured | not measured |
| Memory (process, scene loaded) | not measured | not measured | not measured |
| Binary size (release) | not measured | not measured | not measured |
| Lines of our own code | not measured | not measured | not measured |
| What was missing and had to be written | not measured | not measured | not measured |
| Ruffle movie shown natively, or how | not measured | not measured | not measured |

Each filled cell names the commit or prototype state, the machine, the build profile and how the number was taken
(tool, number of runs, spread).

## How to run

- Prototypes live under ignored `local/bench/<candidate>/`, outside the Cargo workspace, because each candidate is a
  new dependency that is not yet in `THIRD_PARTY.md`.
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

## Decision rule

The maintainer picks the route from this record. The pick is recorded as a new decision file under
`docs/decisions/`, saying which candidate, why, and which measurements it rests on. The chosen engine's
`THIRD_PARTY.md` entry comes before any of its code or crates land in the tree.

## What this benchmark does not prove

- Parity with the original game: a scene that renders, animates and walks is not BL2's look, movement or feel.
- Gameplay: no VM, natives, combat, missions, UI or saves run in the benchmark scene.
- That the Rust core matches the C++ core: that is the separate port check (same synthetic fixtures, and
  `tools/verify_packages.py --reader` giving the same results), not part of this record.
- Long-term cost: build times and code size on a small scene say little about the full slice.
