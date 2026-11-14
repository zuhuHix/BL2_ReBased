## 2026-11-14: Rust replaces the UE5 host; engine route by benchmark

AI-assisted (Claude Code). Maintainer decision (zuhu, 2026-11-14), made in chat after a hand-play of the slice on main
08bcf3e. It reverses the host-engine choice of `docs/OPENWILLOW_ENGINE_PLAN.md` section 2.1 (Unreal Engine 5), which
AGENTS.md had marked as made. The maintainer chose the four options below from a list that also offered keeping the
UE5 host frozen, keeping the C++ core behind a C interface, a docs-only pass and a full code port in one run.

- **Language.** New runtime code is Rust: the engine-agnostic core (today `src/`: LZO container, package reader, tagged
  properties, script disassembly and VM, natives, items, weapons, missions, combat) and the host. Python stays the
  language of tooling and analysis (`tools/`, `research/`). The overlay pages (`tools/hud_overlay/*.js`, Ruffle in a web
  page) stay until the host has its own way to show BL2's movies; Ruffle is itself written in Rust, so embedding it
  natively is one of the benchmark questions.
- **The C++ core is ported, not wrapped.** `src/` keeps building with CMake and keeps its ctest suite until each Rust
  crate matches it. Matching means: the same synthetic fixtures give the same results, and
  `tools/verify_packages.py --reader <the Rust reader>` gives the same counts and the same exact class bodies on the real
  packages as the C++ reader. A part of `src/` is retired only when its Rust crate passes both. Until then the C++ reader
  stays the reference, and nothing about the 832/46 format is re-derived: the Rust port reuses the offsets the C++ code
  already recovered and checked (rule 4), and the port is our own code being rewritten, not new analysis.
- **The UE5 host is removed now.** `host/ue5/` (the OpenWillow UE module, its `Config/`, its `.uproject` and the editor
  Python scripts), the scripts that build, seed, import into or launch Unreal, the UE suites (`tools/suites.txt`,
  `tools/run_suites.ps1`, the `tools/test_*.ps1` that start the editor) and the tests that cover only those scripts leave
  the tree in this change. The last commit that holds them is main 08bcf3e; `git show 08bcf3e:<path>` recovers any of
  them, and the host's code and evidence stay useful as a behaviour reference. Ignored local data under
  `host/ue5/OpenWillow/Content` is not deleted by this change; it is game-derived, belongs to the maintainer, and is
  removed by hand when no longer wanted.
- **Engine route: benchmark first (Phase 0R).** No Rust engine is chosen yet. A timeboxed benchmark builds the same small
  scene in each candidate and the maintainer picks from the measurements. Candidates: **Bevy** (MIT or Apache-2.0) and
  **an own engine on wgpu + winit** (MIT or Apache-2.0) are required; **Fyrox** (MIT) is optional. The scene: Sanctuary's
  static meshes from the existing local exports, Maya's skeletal mesh playing one animation, a capsule that walks on the
  collision, and one BL2 movie shown through Ruffle. Measured: clean and incremental build time, frame time at 1280x720
  and 1920x1080 on each project PC, memory, binary size, lines of our own code, and what was missing and had to be
  written. Prototypes live under ignored `local/bench/` because each candidate is a new dependency; adding the chosen one
  to the tree needs its `THIRD_PARTY.md` entry first (rule 6). The record is `docs/verification/ENGINE_BENCHMARK.md`.
- **Cargo workspace.** The repository root gains a Cargo workspace (`Cargo.toml`, `rust-toolchain.toml` pinning Rust
  1.96.0, crates under `crates/`). It has no third-party crates; each one added later needs a `THIRD_PARTY.md` entry.
  The crates start as skeletons with their scope written down; `ow-host` only checks that it was given a Borderlands 2
  install (rule 11) and says that no engine route is chosen. CI gains a job that runs `cargo build` and `cargo test` on
  the workspace.
- **What the evidence means now.** Records in `docs/verification/` that describe runs of the UE5 host keep their text:
  they say what that host did, which is still true and still useful as a target. They no longer describe a host that
  exists, so each such record opens with a note saying so. Nothing the UE5 host showed counts toward a Rust host's
  gate. Evidence that does not depend on the host stays as it was: real-game captures, the own-words native notes, the
  script disassembly, the package reader's checks, and the core's VM, combat, mission and item results.
- **Status.** The host half of Phases 1 to 4 (Sanctuary walkable and seen, Maya moving, shooting and casting Phaselock
  in a host, the slice gate) goes back to not started for the Rust host. The engine-agnostic half (package reader,
  census, VM prototype, Fire event order, combat and item rules) keeps its status. The vertical-slice priority is
  unchanged: Sanctuary and Maya first, then the Phase 4 gate, then breadth.
- **The run lock stays.** `tools/ue_lock.ps1` still serialises the real-game helpers on each machine, and a Rust host
  run will take the same lock. Renaming it is left for a later change, because the real-game tools dot-source it.

Not verified: everything about the Rust route. No Rust code reads a package yet, no candidate engine has been tried,
and no measurement exists. What this change does verify is listed in its PR: the C++ build and ctest after the removal,
`verify_packages.py`, the new workspace's `cargo build` and `cargo test`, and the decision-index check.
