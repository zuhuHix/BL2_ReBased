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
- **The change.** The store keeps, per candidate file it has read (by scan position, until the next `scan`), a 64-bit
  hash of each folded export path, up to the first export whose path cannot be built (`find_export` fails there unless
  an earlier export matched), and the file's size and modification time when it was read (`FileExports`). A later
  lookup skips a file whose hashes do not hold its path, without loading it.
  - A hash that holds the path is confirmed by loading the file afresh (`try_file`), and that load is returned, as the
    C++ returns its fresh candidate; a hash collision therefore costs one load and cannot change an answer. On the
    first read of a file, the load just made is searched with `find_export` itself. Only hashes are kept (review r1 of
    PR #362, M2: the first round kept every path as a string).
  - A file that fails to load is not remembered: it is tried again on the next lookup, as the C++ tries it on every
    lookup (review r1, M1).
  - A file whose size or modification time differs from when it was read is read again (review r1, M1).
  - The C++ check oracle (`ResolveCheck`, the full scan on every lookup) is unchanged and still runs `try_file` on
    every file.
- **What could differ from the C++.** The candidate lists, the scan order and the order of files tried are unchanged.
  The C++ loads every candidate afresh on every lookup, so it sees any change to a file at once, and since the review
  fix of PR #299 (`the_global_answer_is_the_package_it_matched`) so does this store's answer. One case remains: a file
  rewritten during the store's life with its size and modification time both unchanged is skipped by its old paths,
  so a path it gained is not found there. (The first round said the store's path cache already assumes files do not
  change; that was wrong as a defence, since the fallback was made robust to rewrites, and is withdrawn.)
- **Verified (automated).** Invented packages, the check oracle on in every test, each lookup's answer agreeing with
  its full scan:
  - `the_global_fallback_reads_each_candidate_file_once`: five candidate files under one root, one unreadable; seven
    lookups (three matches in three files, a miss, a path held by a file read before, a second miss, a cached match).
    Loads after each lookup 2, 4, 6, 7, 8, 9, 9, per file: each readable file once, the one with a later match once
    more (its fresh load), and the unreadable file on each of the four lookups that reach it. `scan` then empties the
    cache.
  - `a_failed_or_rewritten_candidate_is_read_again`: a file that failed to load is found holding a path once it
    loads, and a file read before is found holding a path it gained by a rewrite (checked to fail with the size and
    time check removed).
  - `the_summary_follows_find_export_past_a_broken_path_and_keeps_scan_order`: a path after an export whose path
    cannot be built is not found there, one before it is; of two files whose summaries are both cached, the first in
    scan order answers.
  - The existing global-fallback tests and `tests/global_resolve_test.py` (ctest) pass unchanged.
- **Verified (local, installed game, pc-hummer; outputs under ignored `local/p2/fix-pkg-lookup/` only).** A local
  driver ran `Scene::load` on `Sanctuary_P` (Off) and then H8's `MaterialLibrary::read`. It was built twice on H8's
  tree (`feature/rust-h8-materials` cb81a1d, whose `ow-package` equals `main`'s): once as it is, once with this
  `store.rs`. Each was run twice, alternating.
  - Scene load: 41.2 / 42.0 s before, 41.5 / 41.7 s after.
  - Material read: 86.2 / 88.3 s before, 5.7 / 5.5 s after.
  - Both read 381 materials and 136 textures that only cooked lists name, over 4,446 placements. The full `Debug`
    dump of the library (materials and textures, 1,385,899 bytes) is byte-identical before and after.
  - **Memory** (review r1, M2; one run each, the same driver on `origin/main` 60413ba, H8 landed, with three
    `store.rs`; peak working set polled every 250 ms): before, 3,227 MiB (material read 86.6 s); the first round's
    string summaries, 3,393 MiB (5.7 s); the hashes, 3,143 MiB (5.5 s). The three libraries are byte-identical.
- **Gates** (pc-hummer, on `origin/main` 60413ba, with H8-MATERIALS landed):
  - `cargo build --workspace --locked`: clean.
  - `cargo test --workspace --locked`: 1,690 passed, 0 failed, 25 ignored; 1,692, 0, 25 after review r1 (two tests
    added, one changed).
  - `cargo build --release -p ow-cli --locked`: clean.
  - `tests/crate_edges_test.py`: OK.
  - `tools/decisions_index.py --check --baseline origin/main`: OK, 1 added.
  - `tools/verify_packages.py --reader target/release/ow-package.exe`: 9 packages, decoded bytes, counts and export
    fields match, 3,339/3,339 class bodies exact, unchanged.
  - ctest: 173/173 with the release binary present.
- **UNVERIFIED.** Nothing here concerns the game: the fallback's rule (first file in scan order) is the C++'s and
  stays UNVERIFIED as the game's lookup (`PACKAGE_GLOBAL_INDEX.md`).
