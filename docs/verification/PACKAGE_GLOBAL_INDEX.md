# The store's global fallback: an index instead of a scan per path (2026-11-07)

AI-assisted (Claude), mission UI lane. A change to `PackageStore` (`src/package.cpp`, `src/package.hpp`). The answer it gives is meant to be unchanged; only the time differs. No bounds check was touched.

## The cost

A first E at Marcus froze the game for about 16 s on hummer and 26 s on the zuhu PC. The time was in `PackageStore::resolveLoaded`: when an import's root package is not a file on disk, it ran `Package::load` on every indexed file in turn until one had the path, and threw each one away. Four objects in the Fire slice each took a scan of 868 of the 914 files, 13 to 15 s each: two during the slice's setup and two on the first use.

## The rule that must not change

The fallback's answer is the first file in index order for which `findExport(globalPath)` is non-zero; a file that throws is no candidate. Packages hold copies of one path, so trying a likely file first could change the answer.

## What it does now

One pass over the tree, on the first fallback, maps a path's first segment to the ordered files that could hold it. A lookup walks only those files, in the same order, with the same `findExport`.

Why no file that matches can be left out. `Package::findExport` compares folded whole paths, after stripping the file's own stem if the path starts with it. So a match means one of two things:
- an export's path equals the path, and then that export's outermost object starts with the same first segment. The segment is cut from the name itself, because a name can hold a dot;
- the stem was stripped, so the file is also listed under its stem's first segment.

A file whose roots are not known is a candidate for every path: one that does not load during the pass (the old scan tried it again on each lookup, so a passing failure must not change the answer), one whose outer chain cannot be walked, and one with an empty outermost name, which `Package::path` drops so that the real path starts at a child. Listing a file too often only costs time. The index is published only when the pass is complete. (Luna's review found the empty name, the failed file and the partial index.)

This corrected the first form of the idea, "a file with the path must have that root": the stem case does not need it.

## Checks

The oracle: with `OPENWILLOW_RESOLVE_CHECK` set, each lookup also runs the old full scan and stops the run on any difference, in file or index.

| Check | Result |
|---|---|
| Oracle on the Fire event-order step list (real packages, zuhu PC) | 5 lookups, 5 the same, 0 different |
| `--slice-run` output with the same steps, before and after | identical JSON, for the setup alone, the first use and the full list |
| `tests/global_resolve_test.py` (synthetic, oracle on) | copies of a path, folded case, a dotted name, broken outer chains, a file that does not load, a file that fails once during the pass, an empty outermost name, a miss |

What this does not show: the oracle covers the lookups these runs make, not every path a later map will ask for. The argument above is the reason to expect those to agree; the flag can check them when they come. The stem case is not reached by the synthetic test: the earlier direct lookup answers those paths first. The partial-index rule has no test: nothing in a test can fail the pass midway. The fail-once test uses a seam, `OPENWILLOW_RESOLVE_CHECK_FAIL_BUILD`, which works only with the check flag set.

## Timings and memory

Core CLI, Release, zuhu PC, one run each. Peak private bytes of the process.

| Run | Before | After |
|---|---|---|
| Setup only | 27.5 s, 863 MB | 31.4 s, 793 MB |
| First use, extra on top of setup | 27.6 s | 5.5 s |
| Full step list | 71.1 s, 1,086 MB | 47.9 s, 793 MB |

The index holds 11,174 roots and 46,824 file entries. Its own size was not measured separately; the process's peak went down, because the fewer full scans leave less behind.

**Not everything went:** the first use still costs about 5.5 s here, for its two lookups: both end in `Startup.upk`, which each lookup loads afresh and searches with `findExport`. Setup is about 4 s slower, because the one pass loads all 914 files where the first scan stopped at 868, and then loads the answer again. Not run in UE or on hummer.
