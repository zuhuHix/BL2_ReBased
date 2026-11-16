## 2026-11-16: Engine crates wgpu and winit in ow-host, host skeleton (H1)

AI-assisted (Claude Code). Lane H1, the first lane of the host track (`docs/rust-port/ARCHITECTURE.md`, section 8,
"Wave 7 and the host track"), on the route of
[the 2026-11-15 decision](2026-11-15-01-engine-route-own-engine-on-wgpu-and-winit.md).

**Maintainer approval and its scope.** The maintainer approved in chat on 2026-11-16 a new `THIRD_PARTY.md` entry for
**wgpu 30.0.1 and winit 0.30.13**, the versions the Phase 0R benchmark built on Rust 1.96.0, and the start of the host
lanes. The approval covers exactly these two direct dependencies and their transitive crates. It does not cover glam,
bytemuck, pollster, serde, image, gltf, rapier3d or Ruffle, which the benchmark prototype also used; `ow-host` does
without them (a std-only `block_on` for wgpu's async calls, and nothing yet that needs vector maths or packed
buffers), and each would need its own entry and the maintainer's yes.

**What changed.**
- `THIRD_PARTY.md`: a new section, "Engine crates: wgpu and winit (maintainer approval 2026-11-16)", and the Rust
  toolchain section's "Crates" and "Engine candidates" lines now point at it.
- `crates/ow-host/Cargo.toml`: `wgpu = "=30.0.1"` and `winit = "=0.30.13"`, default features (no other feature was
  needed). Engine crates live only in `ow-host` (architecture section 2.2).
- `Cargo.toml` (workspace): its header comment only (lines 4-6). It said "No third-party crates", which these two
  crates make false; it now says the only ones are wgpu and winit, in `ow-host` alone, and that any other still
  needs its `THIRD_PARTY.md` entry first. Members, resolver, package fields and lints are unchanged.
- `Cargo.lock`: 254 third-party packages added, all from crates.io, every one at the version the benchmark's own lock
  holds (compared name by name).
- `tests/crate_edges_test.py`: allows these two crates only in `ow-host`'s `[dependencies]`, pinned to exactly the
  approved versions and from crates.io; refuses them in any other crate or table, any other third-party crate in any
  crate (`ow-host` included), and any `Cargo.lock` package with a source that the engine crates do not pull in or
  that comes from anywhere but crates.io. Six new cases (19 tests, 13 before).
- `crates/ow-host/src/`: the host skeleton. `args.rs` (command line), `app.rs` (a winit event loop on the main thread,
  a window titled "BL2_ReBased", one simulation step and one frame per redraw, resize and close handled, a 10 ms
  back-off while no frame can be drawn, and a `--frames` deadline that wakes the loop by itself, so a minimised
  window that gets no redraw cannot hang the run), `gpu.rs` (instance, adapter, device, surface; a frame that clears and
  presents; the uncaptured-error handler and device-lost callback replaced by ones that record the first fault, so a
  device or surface error ends the run with a message instead of a panic), `sim.rs` (the host boundary of D18 and
  architecture 3.9: `Send` message types, `HostCommand` in and `Snapshot` out, behind a `Simulation` trait, and a
  `StubWorld` that counts steps; the simulation runs on the event-loop thread for now), `executor.rs` (the std-only
  `block_on`), `stats.rs` (frame times in fixed memory: a running count, minimum and maximum, and at most 100,000
  intervals kept for the median, none in a `--window` run; printed through `ow_core::fmt`).

**Commands.** `ow-host --game <install>` checks the install and stops, as before (no GPU needed). `--window` adds the
window until it is closed; `--frames N` renders N presented frames and exits 0, printing the adapter and frame times;
`--no-vsync` asks for `AutoNoVsync` instead of `AutoVsync`. A `--frames N` run fails if its frames are not
presented within 60 s plus 1 s per frame (360 s for 300; the slowest frames seen, 252 ms, fit four times over);
`--time-limit S` replaces that limit. Every window run needs `--game` (AGENTS.md rule 11), even
though nothing is read from the install yet. `ow-host --headless-probe` creates an instance and adapter with no window
and prints the adapter's name, backend, type and driver; it exits 4 when no adapter exists, so CI can tell "no GPU"
from a failure. Exit codes: 0 done, 1 refused or failed, 2 bad command line, 4 probe found no adapter.

**Licences** (the full list is in `THIRD_PARTY.md`): wgpu MIT OR Apache-2.0 and winit Apache-2.0, read from the
licence files of the published crates at those versions. The 254 transitive packages carry 20 distinct `license`
expressions, all permissive or permissive-with-a-choice; none is copyleft-only. `r-efi` 5.3.0 and 6.0.0 (`MIT OR
Apache-2.0 OR LGPL-2.1-or-later`, reached only through `getrandom` on the UEFI target) is the one with a copyleft
option, and the permissive side is taken. Transitive licences are as each crate's Cargo field states them; their
licence files were not read.

**Verified (zuhu desktop, i9-9900K, RTX 4070, Rust 1.96.0).**
- `cargo build --workspace --locked` with no warnings; `cargo test --workspace --locked` passes, `ow-host` 23 unit
  tests among them (18 before review r1, 22 before review r2); `python tests/crate_edges_test.py` 19 tests OK.
- `ow-host --headless-probe`: "NVIDIA GeForce RTX 4070", backend Vulkan, DiscreteGpu, driver NVIDIA 621.01, exit 0.
- `ow-host --game <install> --frames 300`, release build, under the machine run lock: window opened, 300 frames
  cleared and presented, 0 skipped, exit 0. With `--no-vsync`: median 0.171 ms over 299 intervals (min 0.164, max
  0.508), 642 ms wall for the whole run. With vsync (the default): median 252 ms, which is not the 165 Hz monitor's
  6.1 ms. The same 250 ms came on Dx12 and Vulkan. The desktop had had no input for about 18 hours, so the displays
  were very likely asleep, and DWM probably paced presents at about 4 Hz. **UNVERIFIED**: the vsync run needs a
  repeat with the screen on.
- Minimised window (after review r1, same desktop): a `--frames 48` run minimised at 2.0 s and restored at 6.1 s
  exited 0 with 48 presented and 319 skipped; a `--frames 100000 --time-limit 15` run kept minimised from 2.0 s
  exited 1 at 15.8 s, at its limit, with 15 presented. Details in `docs/verification/GATES.md`, "Host (Rust, H1)".
  The window still got redraws while minimised, so a minimised window with no redraw at all, which the deadline's
  own wake-up covers, was not seen: **UNVERIFIED**.
- A queued redraw after a stop draws nothing, and the first recorded outcome is final, so a deadline failure cannot
  become exit 0 (review r2; a unit test covers the outcome rule, not the event order).

**Not verified.** Resizing and closing by hand were not tried (the runs were `--frames` runs on an unattended
screen; the code paths are written, not exercised). The device-lost, surface-lost and validation paths were not
provoked. Nothing was run on pc-hummer, the Linux laptop or CI. Whether CI's Windows runner has an adapter for the
probe is not known. The host draws nothing from the game and the stub world is no `World`: H2 wires scene loading.

**Sensitive files touched:** `Cargo.lock` and `crates/ow-host/Cargo.toml` (the lane brief granted the edit, which
the files' headers otherwise forbid), and the workspace `Cargo.toml`, whose header comment changed and nothing else
(see "What changed"). `rust-toolchain.toml` is unchanged.
