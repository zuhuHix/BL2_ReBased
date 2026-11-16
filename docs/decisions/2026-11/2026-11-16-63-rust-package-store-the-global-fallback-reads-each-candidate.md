## 2026-11-16: Rust package store: the global fallback reads each candidate file once

AI-assisted (Claude Code). Lane FIX-PKG-LOOKUP changes how `PackageStore`'s global fallback (`find_global`,
`crates/ow-package/src/store.rs`, a sensitive file) visits its candidate files. The answers are unchanged; no bounds
check, decoder or reader changed, and no dependency was added.

- **The cause.** H8-MATERIALS (branch `feature/rust-h8-materials`, since landed as decision 2026-11-16-58) found that
  reading Sanctuary's materials added about 87 s on top of the 44 s scene load. Nearly all of it was
  `PackageStore::resolve` for 214 distinct cooked texture imports, of which only 2 are in the scene's levels. On every
  lookup that reaches the fallback, the store loaded each candidate file afresh (`try_file`: `Package::load`, then
  `find_export`), as the C++ `findGlobal` does. The misses are cached per path, but many different paths share the same
  candidate files, so the same files were loaded again and again. A cache in `ow-host` could not help, because the paths
  differ.
- **The change.** The store keeps, per candidate file it has read (by scan position, until the next `scan`), the file's
  export paths, folded, each with the first export that has it (`FileExports`). Building stops at the first export
  whose path cannot be built, so a lookup answers exactly where `Package::find_export` gives `Ok(Some)`. An error past
  that point, and no match, are both "not here", as `try_file` counts them. A later lookup skips a file whose paths do
  not hold its path, without loading it.
  - A file that failed to load is remembered as no candidate.
  - The file that holds the path is still returned as a fresh load, as the C++ returns its fresh candidate: on the
    first read the load just made, later a new `try_file`.
  - The C++ check oracle (`ResolveCheck`, the full scan on every lookup) is unchanged and still runs `try_file` on
    every file.
- **What could differ from the C++.** Only a file that changes on disk during the store's life: the store would skip
  it by its old paths. The store's path cache (`load_path`) already assumes files do not change. The candidate lists,
  the scan order and the order of files tried are unchanged.
- **Verified (automated).** New test `the_global_fallback_reads_each_candidate_file_once` (invented packages, five
  candidate files under one root, one of them unreadable): seven lookups (three matches in three files, a miss, a path
  held by a file read before, a second miss, a cached match). Each answer agrees with the check oracle's full scan
  (six `OWRESOLVECHECK same` lines, no disagreement), and the fallback's file loads are counted after each lookup:
  2, 4, 5, 5, 6, 6, 6. That is each file read once (five reads), plus one fresh load of a file that was read before
  and holds a later path. The old fallback reloaded every candidate on each lookup. The existing global-fallback tests
  and `tests/global_resolve_test.py` (ctest) pass unchanged.
- **Verified (local, installed game, pc-hummer; outputs under ignored `local/p2/fix-pkg-lookup/` only).** A local
  driver ran `Scene::load` on `Sanctuary_P` (Off) and then H8's `MaterialLibrary::read`. It was built twice on H8's
  tree (`feature/rust-h8-materials` cb81a1d, whose `ow-package` equals `main`'s): once as it is, once with this
  `store.rs`. Each was run twice, alternating.
  - Scene load: 41.2 / 42.0 s before, 41.5 / 41.7 s after.
  - Material read: 86.2 / 88.3 s before, 5.7 / 5.5 s after.
  - Both read 381 materials and 136 textures that only cooked lists name, over 4,446 placements. The full `Debug`
    dump of the library (materials and textures, 1,385,899 bytes) is byte-identical before and after.
- **Gates** (pc-hummer, on `origin/main` 60413ba, with H8-MATERIALS landed):
  - `cargo build --workspace --locked`: clean.
  - `cargo test --workspace --locked`: 1,690 passed, 0 failed, 25 ignored.
  - `cargo build --release -p ow-cli --locked`: clean.
  - `tests/crate_edges_test.py`: OK.
  - `tools/decisions_index.py --check --baseline origin/main`: OK, 1 added.
  - `tools/verify_packages.py --reader target/release/ow-package.exe`: 9 packages, decoded bytes, counts and export
    fields match, 3,339/3,339 class bodies exact, unchanged.
  - ctest: 173/173 with the release binary present.
- **UNVERIFIED.** Nothing here concerns the game: the fallback's rule (first file in scan order) is the C++'s and
  stays UNVERIFIED as the game's lookup (`PACKAGE_GLOBAL_INDEX.md`).
