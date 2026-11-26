# Contributing

Thanks for looking. Read the [Is this legal?](README.md#is-this-legal) section of the README
and [docs/LEGAL.md](docs/LEGAL.md) first. The legal and working rules are
non-negotiable and every pull request certifies compliance with them.

**License.** The project is [MIT](LICENSE). By opening a pull request you
agree your contribution is licensed under the same terms (inbound = outbound;
see [docs/LEGAL.md](docs/LEGAL.md#license)). There is no CLA.

## Ways to help

### 1. Verify with your own copy of the game

This is the most valuable thing a non-programmer can do, and it is the
project's primary correctness method. Build the reader (README, *Build and run it*) and:

- Run `python tools/verify_packages.py --reader build/Release/ow-package.exe`
  and `python tools/census.py ...` against your install. Report the totals,
  your platform (Steam/Epic), which DLC you own, and whether the UHD pack is
  installed. Different installs are different test cases.
- In-host map comparison waits for the Rust host: the UE5 host that could
  open a prepared map left the tree on 2026-11-14. The Rust host (`ow-host`,
  an own engine on wgpu + winit, picked 2026-11-15,
  [decision](docs/decisions/2026-11/2026-11-15-01-engine-route-own-engine-on-wgpu-and-winit.md))
  draws and walks Sanctuary, but nothing in it is checked against the real
  game yet. Until it is, `cargo build --workspace` and
  `cargo test --workspace` on your machine are a useful report, with
  `cargo --version` and your platform.

Use the **Verification report** issue template. Never attach package files or
extracted assets; screenshots are fine.

### 2. Contribute format findings

If you know something about version 832 / licensee 46 serialization (an
object prefix, a struct layout, a bulk-data quirk, a native-tail size), open
a **Format finding**. State how you observed it (which tool, which object,
which counts) and where it came from (your own observation, a public document,
a reference implementation and its license). A finding with provenance is
usable; one without is a rumour.

### 3. Code

Open items are listed in [ROADMAP.md](ROADMAP.md#now--next). Before starting
anything larger than a bounded task, open an issue so the approach can be
agreed. This matters most for anything that touches the sensitive areas below.

## Working rules

- **One observable capability per change.** Describe the input provenance,
  the expected behaviour, and a reproducible check.
- **Synthetic fixtures only.** Tests generate their own data with the
  existing `package()` / `tag()` helpers in `tests/`. Never commit game
  files, decompressed data, or extracted assets. Everything the tools write
  goes under `local/`, which is ignored; never move their output into the
  tree.
- **Report checks honestly and separately.** Automated checks (CTest,
  `cargo test`, `tests/level_test.py`, `tools/verify_packages.py`) are one thing; in-game
  or visual checks are another. Say which you ran and paste the output. Do
  not present count agreement as full compatibility.
- **Label guesses.** Behaviour inferred rather than verified against the
  real game is marked `UNVERIFIED` in code and docs.
- **Record decisions.** Any change to parsing behaviour gets a dated decision file
  under `docs/decisions/` (named as [DECISIONS.md](DECISIONS.md) says) stating what
  is verified and what is not.
- **Record provenance.** Any reference implementation consulted goes in
  [THIRD_PARTY.md](THIRD_PARTY.md) with its license, before code is written.
  Changing anything under `third_party/` means re-verifying the recorded
  hashes.
- **Use external exporters as measured inputs.** Community tools may be used
  outside the repository to accelerate local extraction. Record the tool
  version, command, output inventory, failures, unsupported types, duplicates,
  elapsed time and license. Do not commit their binaries or game-derived
  output, and do not treat a successful export as proof that a host renders it
  correctly or of gameplay compatibility.
- **Executable analysis stays local.** You may disassemble or decompile
  `Borderlands2.exe` on your own machine to understand a native function
  ([docs/NATIVE_ANALYSIS.md](docs/NATIVE_ANALYSIS.md)). Never paste,
  transcribe or auto-convert decompiler output into a contribution, and never
  attach listings or analysis databases to an issue or pull request; describe
  the behaviour in your own words and write the code from that.
- **Match the tone.** The docs are hedged and evidence-first on purpose.

## Sensitive areas

Changes here need explicit discussion first and a decision file (see DECISIONS.md):

- `src/package.cpp`: the `Reader` struct, `require()`, `limit`, size and
  terminator checks. Never loosen a bounds check to make something work.
- `src/container.cpp` / `.hpp`: LZO container validation.
- Their Rust ports, `crates/ow-package/` and `crates/ow-container/`, on the
  same terms, and `Cargo.toml` / `rust-toolchain.toml` (a new crate is a new
  dependency).
- Struct/array property decoding, `Texture2D` and `StaticMesh` serialization,
  the cooked `Material` resource reader: the 832/46 layout has no public
  spec; do not invent offsets.
- `CMakeLists.txt`, `THIRD_PARTY.md`, any `LICENSE` file: dependency and
  license decisions are the maintainer's.
- The language choice (Rust, replacing the UE5 host on 2026-11-14) is made
  and is not reopened; the engine route is the maintainer's pick from the
  Phase 0R benchmark.

Safe to work on without ceremony: census and CLI output, synthetic tests,
JSON output flags, tooling scripts, documentation.

## Every pull request

The PR template asks for:

1. What changed and why, in one paragraph.
2. Output of `ctest --test-dir build -C Release --output-on-failure`,
   `python tests/level_test.py` and `cargo test --workspace`.
3. For reader changes: output of `tools/verify_packages.py` against your own
   install. For census/property/asset/level changes: the relevant tool's
   numbers against your install.
4. In-game or visual checks, reported separately, or "none".
5. Confirmation that no game-derived data (including decompiler output and
   analysis databases) is included and that the contributor certification in
   [docs/LEGAL.md](docs/LEGAL.md) holds.

`main` always builds; CI runs the synthetic suites on every PR.

CI and the `pre-push` hook run the leak gate (`python tools/leak_gate.py --range origin/main..HEAD`), which refuses game files, decompiler output, dumps and personal identifiers.
On a finding, fix the content or ask the maintainer for an allow-list entry ([docs/TOOLING.md](docs/TOOLING.md), "Leak gate").

## AI coding tools

This is a solo project and AI coding tools are part of its workflow; the
project rules require saying so. If you use them too, the same rules apply to
their output: [AGENTS.md](AGENTS.md) is the tool-neutral working brief and holds the rules,
[CLAUDE.md](CLAUDE.md) imports it and adds the Claude Code specifics, a pre-write hook
reminds on sensitive-area edits and prompts for license/provenance files, and you are responsible for what you
submit. Read what it writes.
