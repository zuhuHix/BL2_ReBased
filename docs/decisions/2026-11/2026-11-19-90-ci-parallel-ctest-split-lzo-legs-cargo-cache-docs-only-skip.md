## 2026-11-19: CI: parallel ctest, split LZO legs, cargo cache, docs-only skip

AI-assisted (Claude Code, Opus 5.5). Lane CI-OPTIMIZE. The maintainer approved on 2026-11-19 ("you have my yes to fully
optimize everything") a change to `.github/workflows/ci.yml` and the use of GitHub's first-party `actions/cache`
(`actions/cache@v4` and its `actions/cache/restore@v4`). No other action, tool or dependency was added. The leak gate
job, `tools/leak_gate.py`, its lists and the hooks are unchanged, and no test's assertions changed.

**Why.** Per pull request, the MSVC LZO=OFF job took about 17 min (ctest 11m39 serial), LZO=ON about 12 min (ctest
8m19), the Rust job about 5 min. Both MSVC legs ran the whole suite, though only the entries that run a C++ binary can
change with `OPENWILLOW_LZO`. Superseded runs kept their runners, docs-only pull requests ran all three Windows jobs,
and the 254 third-party crates in `Cargo.lock` were downloaded and built from scratch in three jobs.

**What changed.**
- **Concurrency.** Group `ci-<workflow>-<PR number or ref>`; `cancel-in-progress` only for `pull_request`, so a new
  push to a pull request cancels the run on its older head, and runs on main are never cancelled mid-way.
- **Docs-only skip.** A new `changes` job (ubuntu, `actions/checkout` with a blobless full fetch, plain
  `git diff --name-only base...head`) feeds `if:` on the three Windows jobs. A pull request skips them only when every
  changed path is under `docs/` or is a top-level `*.md`, and `docs/TOOLING.md` is not among them. That exception is
  there because one test reads a doc: `tests/real_game_migration_test.py` (ctest `py-real_game_migration`) reads
  `docs/TOOLING.md`. No other test, tool a test runs, or Rust test reads a file under `docs/` or a top-level `*.md`
  (searched in `tests/`, `tools/`, `research/`, `crates/` and both `CMakeLists.txt`; the other hits were comments,
  synthetic paths inside temporary repositories, or record names in constants). Pushes to main, manual runs and an
  empty diff run everything. The leak gate has no `needs:` and always runs.
- **Parallel ctest.** `ctest --parallel <runner cores>`. The slowest entries get a `COST` in `tests/CMakeLists.txt`
  so they start first (order only; no timeout changed).
- **The ignored `local/` folder** is created before ctest. Some tests make a temporary folder under it, and
  `item_pools_rust_test.py` skips its class without it. In the serial run an earlier test created it first; in a
  parallel run that order is gone. A working checkout always has `local/`, so CI now has it too.
- **Leg split.** `tests/CMakeLists.txt` labels `cxx-binary` every entry whose `add_test` command runs
  `$<TARGET_FILE:ow-package>`, `$<TARGET_FILE:ow-weapongen>` or `ow-weapon-firing-test`, read from its own
  `add_test` calls so a new entry needs no list. That gives 25 entries with LZO on (23 with it off): the C++ reader,
  container, assets, VM, mission, combat and weapon tests and the six Rust tests that compare against the C++ binary.
  LZO=ON runs all 173 entries; LZO=OFF runs `-L cxx-binary` only. `level_test.py` and `viewer_test.py` run in LZO=ON
  only; the "reader refuses to run without arguments" check stays in both. The `needs-*` case-count step lists the
  `needs-*` entries of its leg's share (repeated `-L` options must all match), so it checks all 35 in LZO=ON and none
  in LZO=OFF, which runs none.
- **Cargo cache.** `~/.cargo/registry`, `~/.cargo/git` and `target/`, keyed on runner OS, job and the hash of
  `Cargo.lock` and `rust-toolchain.toml`, with a prefix `restore-keys` fallback. The two MSVC legs share one key; the
  LZO=ON leg saves it, the LZO=OFF leg only restores. `--locked` stays on every cargo call. A checkout gives every
  source file a new time stamp, so the workspace crates are rebuilt each run anyway; the cache saves the third-party
  crates. The Rust job's comment no longer says nothing is downloaded. On the zuhu desktop a workspace build and test
  leaves 7.8 GB in `target/debug` (3.8 GB incremental data, 3.0 GB workspace artifacts, 1.1 GB third-party), more than
  the repository's 10 GB cache allowance can hold beside the rest. So the Rust job sets `CARGO_INCREMENTAL=0` and its
  last step deletes the workspace crates' artifacts (`ow_*`, `libow_*`, `ow-*`) before the cache saves; the release
  `target/` of the MSVC legs is 65 MB and is kept whole.
- The C++ build step passes `--parallel`.

**Measured locally** (zuhu desktop, shared with other lanes, LZO on, 173 entries). Before the change, `ctest -j 8`
three times: 173/173 each, 170.7 s, 129.7 s and 124.1 s, no failures. With the labels: `-LE cxx-binary` (148
entries) passed 148/148 with the three C++ binaries present and again with them renamed away, with the same output
and the same case and skip counts per entry, so none of those 148 reaches a C++ binary by a tool's default path.
After the change, `ctest -j 8`: 173/173 in 144.4 s, the twelve costliest entries started first. No entry
collided on a shared file or port in these four full parallel runs and the two runs of the 148-entry share, so none needed `RESOURCE_LOCK` or `RUN_SERIAL`.
The LZO=OFF share was not run locally; CI runs it.

**CI** (the pull request's own first run): see its description for the before and after table. Not verified: the
warm-cache time, which needs a second run on a key that exists; the first run of this change on main saves the caches.
