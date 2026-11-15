## 2026-11-15: Rust package store contract as shipped

AI-assisted (Claude Code). Lane W2-STORE of the Rust port fills `crates/ow-package/src/store.rs` and
`crates/ow-cli/src/cmd/resolve.rs`: a port of the C++ `PackageStore` (`scan`, `loadPackage`, `loadPath`, `resolve`,
`resolveLoaded`, `buildGlobalIndex`, `findGlobal`, `scanGlobal`, `resolvePath` in `src/package.cpp`) and of the
`--resolve` branch of `src/cli.cpp` (`resolvedJson`). No parsing behaviour changes. The scan, the name fold, the
resolution steps, the global fallback (the first file in scan order whose `findExport` matches) and its caches are
the C++ ones, in the C++ order, and so are the error texts. This file records where the shipped interface differs from
revision 3 of `docs/rust-port/ARCHITECTURE.md` section 3.2.

- **`new` does not scan; `open` does.** The contract has `scan` separate, so `new` only makes the root absolute and
  lexically normal. `open` is `new` then `scan`, which is what the C++ constructor does.
- **`scan()` keeps the `PackageId`s it handed out.** It empties the loaded-package cache, the global caches and the
  global index, as the C++ `scan` does. The packages themselves stay in the store, so an id given out earlier still
  names its package, in the same way a C++ caller's `shared_ptr` keeps its package. A later load of the same file gets
  a new id.
- **`resolve` returns `None` only for reference 0.** An export is checked and returned as itself. An import whose
  target is found nowhere is `Err(StoreError::ImportNotFound)`, as the C++ throws "import target not found".
- **`resolve_path` returns `None` where the package has no such export.** The C++ throws "object not found" there.
- **Added methods:**
  - `open`, `root`, `indexed_package_count`;
  - `paths_named`, which returns every file whose stem folds to a name. These are all the candidates. `load` refuses
    a name with two or more files as ambiguous, as the C++ does, and the store never picks one (identity policy is
    W3-DEFS);
  - `load_path`;
  - `adopt`, which puts an already loaded package into the store under its source path. The C++
    `resolve(const Package&)` loads that path again; the bytes are the same unless the file changed;
  - `shared`, which returns the `Arc<Package>` for threaded tools (D18);
  - `set_resolve_check` and `take_check_log`.
- **`ResolveCheck` is the C++ `OPENWILLOW_RESOLVE_CHECK` oracle as a value.** Each global lookup also runs the full
  scan and fails with `CheckDisagrees` on any difference. Its `fail_in_build` field is the test seam
  `OPENWILLOW_RESOLVE_CHECK_FAIL_BUILD`. The library reads no environment variable: `cmd/resolve.rs` reads both
  variables and passes the value in. The `OWRESOLVECHECK` lines the C++ writes to stderr are collected by the store,
  and the binary prints them before any error message, which is the C++ order.
- **`StoreError`** has one variant per C++ text. Its `Package` variant is transparent: it prints as the package error,
  and that error's cause is its `source()`. `NotYet` is there per rule R3 and is not produced yet.
- **Output line ends are `\n`.** The C++ binary writes `\r\n` on Windows. The parity harness is to normalise line
  ends (hub decision, change pending). Until then, four synthetic `--resolve` error lines that differ only in line
  ends stay out of `tests/rust_parity.d/store.txt`.

- **Verified (automated).** `tests/global_resolve_test.py` passes on the Rust binary and on the C++ binary.
  Unit tests in `store.rs` re-express its cases and add:
  - named and nested imports;
  - a cyclic import chain;
  - a local import with no package-qualified path;
  - ambiguous and missing names;
  - the match, root-match and miss caches;
  - the check lines;
  - the path order.

  A local sample of 10,000 import references was drawn from the 202,188 imports of the installed `CookedPCConsole`
  tree (914 files) with seed 20261115. Each was run through `--resolve` on both binaries, comparing the exit code,
  stdout and stderr with line ends normalised. At the time of writing, 4,159 of the 10,000 had been compared: all
  4,159 matched (4,044 resolved, 115 "import target not found" on both). The rest of the run is recorded in the
  lane's handoff, not here.
- **UNVERIFIED.**
  - Whether the game picks the same file when several packages hold a path (object identity, W3-DEFS). Parity here
    is with the C++ store.
  - Package stems with non-ASCII characters: the C++ narrows them with the ANSI code page, the Rust store uses the
    lossy UTF-8 form.
  - The scan order for non-ASCII names. It compares UTF-16 units element by element, written from the standard's
    `path` comparison, and no listing has such a name.
