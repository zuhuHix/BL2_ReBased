## 2026-11-16: Rust scene load: definition loads without a heap walk, package indexes on worker threads

AI-assisted (Claude Code). Lane FIX-SCENE-LOAD. Sanctuary's `Scene::load` (H2, decision 2026-11-16-46) took about
41 s on pc-hummer. This lane profiled it and removed the three largest costs; the load now takes about 11 to 12 s. The
answers are byte-identical: the whole `Debug` output of the scene, the material library and the lights has the same
digest before and after. No dependency was added, no bounds check was loosened, and no parsing behaviour changed.
There was no real-game session.

- **Where the time went** (temporary timers, removed before the commit; Sanctuary_P, release build, about 41 s):
  - **19 s: freeing objects.** Each definition the store loads is built as a new object and then moved into its
    handle with `ObjectSpace::free`. `free` walks the whole heap to clear references to the freed object. Over 6,052
    loads into a growing heap, that walk was quadratic.
  - **About 5 s: the package store's global index.** The first holder lookup builds it by decoding every package of
    the tree, 914 files one after another: 3.9 s of LZO decoding (2,837 MB decoded from 1,128 MB), 0.7 s of table
    parsing, 0.3 s of reading.
  - **About 5.5 to 7 s: the definition store's file indexes.** It decoded 767 candidate files one by one to list
    their export paths.
  - **About 3 s: holder files decoded twice.** A holder's file was decoded for its index, dropped, then loaded
    again by the package store (61 files).
  - **About 5 s: `PackageStore::resolve`'s global fallback** for 434 mesh imports. That is FIX-PKG-LOOKUP's
    function (PR #362), and this lane does not touch it.

- **What changed.**
  - **`ow-object/src/heap.rs`: `Heap::free_newest` and `ObjectSpace::free_newest`.** These free an object that no
    older object refers to.
    - References to it are cleared only in the objects created after it (found from the end of the creation order)
      and in the class table's struct defaults.
    - Everything else is `free`'s: the slot, its generation, the free list, the count and the creation order. So
      every later handle is the one `free` would give.
    - In debug builds they check that no older object refers to the freed one; the whole workspace suite ran with
      that check on.
    - `free` and `free_many` are unchanged; they now share two private helpers with `free_newest`.
  - **`ow-defs/src/store.rs`.**
    - `load_unloaded` and `archetype_slots` move a newly built object's slots into its handle with `free_newest`.
      Such an object is made by the decode, and the decode's linker never hands it out (references resolve to the
      store's handles by path), so only objects made after it, during its own decode, can hold it.
    - A path's candidate files are asked of the package store once per first segment. The segment rule
      (`candidate_segment`) is the store's `first_segment`, and a test checks the two give the same lists.
    - On a segment's first lookup, all its candidates' export paths are read together on worker threads (at most 16,
      `std::thread::scope`). Each file's index is a pure function of its bytes, so reading early changes no answer.
    - A holder file read that way, or read by the lazy path just before, goes to the package store with
      `PackageStore::adopt` instead of being decoded again. `adopt` keys it by the same normalised path and gives the
      id `load_path` would. A holder's id is then kept.
    - The module documentation's "Cost" paragraph says this.
  - **`ow-defs/src/identity.rs`.** `FileIndex::read_with_package` replaces `read`: it also hands back the package it
    loaded.
  - **`ow-package/src/store.rs` (sensitive).** `build_global_index` reads each file's roots on worker threads (at most
    16) through a new private `file_roots`, then folds the results into the index in scan order, as the single pass
    did. A worker that panics has its panic raised again, as the single pass would have panicked (review r1).
    - No check changed, and nothing outside `build_global_index` changed. `find_global` and `resolve_import`, which
      PR #362 edits, are untouched. The branch is rebased onto main 37ca667, which holds #362, without a conflict,
      and the module documentation gains a "Global index build" paragraph next to #362's "Candidate files".

- **Proof.**
  - A new installed driver, `scene/tests.rs` `sanctuary_load_digest_on_the_installed_game` (ignored by default), times
    the store open, `Scene::load`, `MaterialLibrary::read` and `SceneLights::read`. It writes FNV-1a 64 digests of
    their full `Debug` output to ignored `local/p2/scene-load/`.
  - Results (pc-hummer, 28 logical cores, release build):

    | Run | `Scene::load` | `MaterialLibrary::read` |
    |---|---|---|
    | before 1 (main 1075dc2) | 40,868 ms | 87,387 ms |
    | before 2 (main 1075dc2) | 40,030 ms | 89,418 ms |
    | after A | 10,612 ms | 87,954 ms |
    | after B | 10,940 ms | 92,812 ms |
    | with #362 merged locally, timers still in | 11,132 ms and 11,431 ms | 5,341 ms and 5,561 ms |
    | before 1 (main 37ca667, with #362 and H9) | 42,137 ms | 5,768 ms |
    | before 2 (main 37ca667) | 44,374 ms | 5,754 ms |
    | after 1 (this branch rebased on 37ca667) | 11,927 ms | 5,852 ms |
    | after 2 (this branch rebased on 37ca667) | 12,811 ms | 6,250 ms |

  - The digests were the same in every run on the same main:
    - scene `22de5b99211d1eba` over 46,200,323 bytes (all runs);
    - materials `8b4cfb67c8b06c18` over 792,049 bytes (all runs);
    - lights `fdb2c9336baac762` over 717 bytes on main 1075dc2. On main 37ca667 they are `a0f0f075a6699088` over
      734 bytes, before and after: H9 (decision 65) added the sun's `LightGuid` to the lights.
  - **What remains after this lane:**
    - about 5 s of `PackageStore::resolve` in the scene load;
    - with #362, the material read's 5.3 s, of which 5.55 s on the timed run is `PackageStore::resolve` for 571
      cooked texture references (about 5.8 s on main 37ca667, before and after this lane).
    - Both are `find_global` reading each candidate file once, one at a time. Reading those candidates on worker
      threads, as this lane does for the indexes, is the follow-up now that #362 has landed.
  - Also remaining: the level packages and the rest of the decode, about 4 s.
  - **Synthetic tests:**
    - `ow-object`: `free_newest_clears_the_newer_references_and_reuses_the_slot_as_free_does` (two heaps freed both
      ways give the same next handle and count), and, in debug builds,
      `free_newest_checks_in_debug_builds_that_no_older_object_refers_to_it`;
    - `ow-package`: `the_index_read_on_worker_threads_keeps_the_scan_order` (more files than threads; a fold in the
      wrong order fails it, checked by a mutation);
    - `ow-defs`: `candidate_lists_are_the_store_s_and_a_prefetched_holder_is_adopted_once`.

- **UNVERIFIED and limits.**
  - The speed-ups are measured on one machine.
  - The byte-identical claim covers Sanctuary's load and the synthetic suites.
  - `free_newest`'s precondition rests on how the decode allocates, argued above and checked in debug builds, not
    proven for every caller. Only the definition store calls it. The debug check ran on the synthetic suites only:
    the installed Sanctuary driver runs in release, and its digests cover the scene, material library and lights,
    not the object space, so the precondition was not checked on Sanctuary (review r1 of PR #371). A breach there
    would leave a stale handle in an older object, not corrupt memory.
  - Worker threads: up to 16 whole-package decodes run at once in each pool (peak memory), capped by
    `available_parallelism`.
  - Worker threads read packages concurrently, which only changes when the reads happen, not what they produce.
