## 2026-11-15: Tools read packages with the Rust ow-package by default

AI-assisted (Claude Code). Lane W3-SWITCH (`docs/rust-port/ARCHITECTURE.md` section 8, wave 3). The change affects the
`--reader` defaults of `tools/*.py`, `tools/repo_paths.py` and a header comment in `src/package.hpp` / `src/package.cpp`.
No parser changes; what changes is which parser a tool runs when no `--reader` is given.

- **What changed.**
  - `tools/repo_paths.py` gains `default_reader(waits_for=None)`, with `RUST_READER` (`target/release/ow-package`) and
    `CXX_READER` (`build/Release/ow-package`), both relative to the checkout.
  - The 37 tools with a built-in `--reader` default now take it from the helper:
    - 32 default to the Rust binary: the 31 whose default line changed, and `item_roll.py`, which took
      `item_registry.DEFAULT_READER` and now calls the helper too;
    - 5 keep the C++ binary and name the flag they wait for:

      | Tool | Waits for |
      |---|---|
      | `audio_slice_chain.py` | `--mission-run` |
      | `prepare_range.py` | `--combat-run` |
      | `slice_native_census.py` | `--native-census` |
      | `verify_fire_event_order.py` | `--slice-run` |
      | `prepare_phaselock_target.py` | `--object-dump`, below |
  - Tools whose `--reader` is required are unchanged, and so is `tools/rust_parity.py`, whose `--cxx` is the C++
    reference by design. A path given with `--reader` always wins.
  - When `target/release/ow-package` is missing, a tool run with the Rust default and no `--reader` stops with one line
    that names the path and `cargo build --release -p ow-cli --locked`. It never falls back to C++. The check is skipped
    when the tool is only imported (a test calling its `main()`).
  - `audio_slice_chain.py` and `prepare_range.py` also call `--object-dump`. A comment at each call says that they wait
    for its parity too, once their first flag is ported.
- **Rule for moving a tool.** A tool moves when every `ow-package` flag it calls, directly or through the tools it
  imports or runs, is ported (`crates/ow-cli/src/cmd/*.rs` no longer answers "not ported", exit 3). The tools reached
  through `--object-dump`, where decision 2026-11-15-08 accepts two output differences, moved only after a rerun on both
  binaries over the installed game gave the same result.
- **Reruns** (pc-hummer, installed game, outputs under ignored `local/p2/W3-SWITCH/`; provenance fields such as time and
  reader hash left out of the comparison):
  - `export_index.py`: 914 packages, 2,058,664 exports on both; the files are byte-identical.
  - `mission_closure.py` (Fire mission): closure and Kismet graph equal; the path index has 2,058,664 rows on both,
    with the same content hash.
  - Equal:
    - `census_ambient_npcs.py` (both of its files);
    - `slice_npc_assets.py identity`;
    - `ambient_npc_assets.py identity`;
    - `prepare_mission_log.py`.
  - `prepare_ambient_world.py`: on both binaries the full run stops for want of the UModel-made `ambient_assets.json`.
    Its only reader call, the patrol speed scale, gives equal values for both NPCs.
  - `prepare_phaselock_target.py identity`: **differs**, so it keeps the C++ default. In the default parity mode (Off),
    the Rust `--object-dump` logs `defaults: skipped StaticMeshComponent.StaticMesh (ObjectProperty)` and leaves
    `staticmesh` out of the dump. That holds for all 8 worn-attachment components of the two targets. The C++ dump has
    it, and so does the Rust one under `OW_PARITY=reference`. The tool then stops at the first attachment (exit 1).
    This is a third difference, beyond the two that decision 2026-11-15-08 accepts. It is recorded here for the
    `ow-object` owner and not fixed in this lane, which edits no crate.
  - `tests/behavior_test.py` and the kept tools call unported flags. On the Rust binary the behaviour test stops at
    `--behavior-dump` with exit 3.
  - `tools/creature_zone.py` runs no reader. It reads what `slice_npc_assets.py` (equal) and `prepare_phaselock_target.py`
    (kept on C++) write.
- **Line endings.** On Windows the C++ binary prints CRLF and the Rust one LF. Every reader call under `tools/` runs in
  text mode, where Python reads CRLF and LF alike, so no tool parses output that depends on line endings. The
  bytes-mode `subprocess` calls run git, UModel or Python, not the reader.
- **Frozen.** `src/package.hpp` and `src/package.cpp` open with a FROZEN comment: no new feature and no new consumer;
  they stay the differential reference until W7-RETIRE. `src/container.*` freezes at its own gate.
- **Not verified.** Nothing here was checked in the running game. Equal output between two readers says the readers
  agree, not that either matches the game. Tools that need UModel output or a host step were rerun only as far as their
  reader calls (listed above).
- **Follow-up.** When a lane ports a flag, `grep -n "waits_for='--flag'" tools/*.py` lists the tools to move. Rerun
  each of them on both binaries before you drop the argument.
