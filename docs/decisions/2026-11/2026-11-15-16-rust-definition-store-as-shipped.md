## 2026-11-15: Rust definition store as shipped

AI-assisted (Claude Code). Lane W3-DEFS of the Rust port fills `crates/ow-defs/src/*` (the contract of PR #307,
architecture section 3.4) and `crates/ow-cli/src/cmd/defs.rs` (`--def-dump`, Rust-only), with one additive method on
the package store (`PackageStore::global_candidates`, granted) and `Printer` in `crates/ow-cli/src/cmd/objects.rs` made
`pub(crate)` (granted). No parsing behaviour changes: every decode is W3-DECODE's (decision 2026-11-15-08). This file
records how the store does what section 3.4 says, and what was checked.

- **Identity.** One heap object per object path. A path's key is its text with ASCII case folded and `:` read as `.`.
  The path of a reference is its full path: an import's path as the package stores it; an export's path in its file
  when its outermost export is a `Package` object (as `Startup.upk` holds `GD_*` packages), else `<file stem>.<path>`.
  The holder is picked by the policy and cached per key:
  - `FirstInScanOrder`: the first of the files `PackageStore::global_candidates` lists (the store's global fallback
    candidates, in scan order) that holds the path, matched as `Package::find_export` matches (folded, a leading
    `<stem>.` dropped). Each candidate is read once into a list of its export paths and dropped; only the holder's
    file is loaded into the package store. A candidate that fails to load, or one of whose exports has no path, is
    no holder, as the fallback treats a file whose load or search fails. (A first version loaded every candidate into
    the package store; on the whole game root an audit run grew to 11 GB before it was stopped.)
  - `Loaded(set)`: the first package of the set, loaded by stem, that holds the path.
  Every map is keyed by the filed id (decision 2026-11-15-10).
- **Lazy loading in place.** A reference decoded inside a definition goes through the store's `ReferenceResolver`,
  which returns the path's object, allocating it on first sight with `ValueCx::resource` for the holder export (a
  slotless object of that export's class, `Origin::Resource`). Loading instantiates the holder export
  (`cdo::instantiate_export`, the class default first), frees the new object and moves its class, name and slots into
  the handle, whose origin becomes `Origin::Export`. Because the resolver never loads, no load nests and no object
  being built is reached: two definitions naming each other are two handles, and `InstantiationCycle` cannot come from
  the store. A handle's outer is the outer's object, except a code outer (a `Package` object, a class), which stays
  `None`.
- **What stays a stand-in.** No holder (logged `defs: no holder for <path>` once per path; keyed by path in Off, by the
  import under Reference), the decoder's `?`, engine-native classes, code exports, class default objects and their
  subobjects. `find` gives `Ok(None)` for no holder and `NotADefinition` for code; a `Default__<Class>` path gives the
  class default object.
- **Prefix.** Derived from the class chain: `Engine.ActorComponent` 8; `Engine.Actor` whose outer export is a `Level`
  26; neither `Core.Component` nor `Engine.Actor` 4; any other only when the stream from the derived prefix reaches
  its `None` tag within the export (headers readable, values skipped by size), else `NotYet`. UNVERIFIED as a rule of
  the game.
- **Failed loads** are cached per handle with their error: no second decode, no second log line.
- **Overlays.** `Overlay::from_live_capture` reads `weapon_dump.py`'s JSON and applies `live_overlay.py`'s `clean`
  rules (empty struct entries dropped, `BaseValueScaleConstant` 1.0 dropped without `BaseValueAttribute`,
  `ModifierType` 0-2 to its names, `Class'path'` to the path). One difference: `live_overlay.py` skips a top-level
  empty value for a property the cooked export does not tag; here it is a patch, which sets the property to the empty
  value it already has by default. The JSON reader is our own (RFC 8259 plus `NaN`, `Infinity`, `-Infinity`, nesting
  bounded at 128). Patches are placed by holder when pushed: applied at once to a loaded object, else kept by the
  holder export and applied when it loads; a path no file holds stays pending. Typing follows the declaration (int,
  float through a double rounded to single, byte by number or enum name, name, string, object paths to the path's
  object without loading it, class paths, structs from their defaults, arrays); the heap checks the result.
- **Census.** `census()` reads every file of the scan with `Package::load` and drops it, in the shape of
  `ow-package --exports` (exact path text, the class as `--exports` prints it); a file whose listing fails anywhere
  contributes nothing and is a failure. A failure's `Display` (`FileFailure`) is exactly what the reader's
  `ow-package <file> --exports` prints after `ow-package: ` (the error and its causes joined with `: `; no path, no
  wrapper), which `tools/item_registry.py` keeps as a scan error; the error's own `Display` is the first part, its
  cause in `source()`. `exports_of_class` filters the census by the first holder's `Package.Class`.
- **Cost.** The first `FirstInScanOrder` lookup builds the global index, which reads every package once (6.6 s for the
  installed `CookedPCConsole`, 934 files, one run measured locally).

- **Verified (automated).** 25 `ow-defs` unit tests (and one ignored real-game check) and 3 `cmd/defs.rs` tests on
  invented packages: loading in place, cyclic references, identity by path through an import and an export reference
  under both policies, no-holder stand-ins and their log line, total and strict loading, the three prefixes and
  `PrefixMismatch`, the structural check, the cached failure, `raw_tail`, `owners`, the census and `exports_of_class`,
  every overlay edge case (static arrays whole and `Prop[2].Field`, a later overlay winning, `origin` per element,
  `Default__` by kind), the JSON reader and both overlay formats, and a synthetic capture in `weapon_dump.py`'s shape
  whose patches change exactly the objects whose values differ; a census failure printing the reader's `--exports` text.
  A test of `PackageStore::global_candidates` in `store.rs`.
- **Verified (local, installed game, pc-hummer; C++ `ow-package.exe` built from the unchanged `src/`).**
  - The Phase 0R step 6 check: every `WillowGame.WeaponPartDefinition` export of `Startup.upk` (658), `--def-dump
    <path> --cooked <CookedPCConsole>` against the C++ `--object-dump <index> 4 --cooked <CookedPCConsole> --all`, in
    both parity modes (1,316 Rust runs). Every run of both binaries exits 0 and prints the same `path`. Under
    `OW_PARITY=reference` 642 of 658 are byte-identical (line ends normalised); Off mode 632 of 658. The 16 others (26
    in Off) differ in one property, `FirstPersonParticle` (16 objects: the C++ prints a component path, Rust skips the
    value and logs it), and in Off mode in 10 more log lines. All of them are the decoder's, not the store's: Rust
    `--object-dump <index> 4 --all` (W3-DECODE) on the same 658 exports has exactly the same differences from the C++,
    part by part, in both modes (658 of 658 equal signatures). The store's own identity, holder choice and prefix
    changed no printed value. That decode difference is reported to W3-DECODE's owner below, not fixed here.
  - After the holder lookup was changed to keep no candidate loaded (above), 30 of those parts (every 22nd) were run
    again: 30 of 30 byte-identical to the C++ in both modes.
  - The census of the whole game root (`Borderlands 2`, base game and DLC folders, the ignored test
    `store::tests::real_census_and_owners`, 44 s, peak 8.7 GB with the `owners()` sampling below): 2,010 package
    files, 1,201,223 export paths (exact text), 4,050 class default entries kept apart, 2 files that do not read.
    305,284 paths have holders in two or more files; `owners()` on every 400th of them (764) lists the same holder files
    in the same order as the census for 763. The one other path has 2 holders in the census and 5 in `owners()`: the
    census groups by exact path text (the tools' convention), `owners()` matches as `Package::find_export` does (folded,
    a leading `<stem>.` dropped), so it also finds the spellings the census keeps apart.
  - The owner choice against `tools/item_registry.py` (its census: `.upk` files, the base game's in `sorted()` order,
    then each DLC folder's): of the 7,747 paths of registry classes, the first holder in the store's scan order is the
    tools' owner for 7,461. The 286 others are all file-order conventions, not identity code: 277 where the store's
    path sort puts a `DLC/` file before the base game's `WillowGame/` (the tools read the base game first) and 9 where
    two base-game files sort differently because the store compares names by bytes (upper case first) and Python's
    `WindowsPath` sort folds case. So `FirstInScanOrder` over the game root does **not** pick the tools' owner for every
    registry path, which section 6's gate as first written asked. `census()` carries every holder with its file, and
    the registry (W4-REGISTRY) re-sorts the holders into the tools' order and so reproduces their owners.
- **Identity order (hub decision).** `FirstInScanOrder` keeps the store's scan order: the C++ reader's
  `std::filesystem` order, which the VM's parity reference uses. No `IdentityPolicy` variant for another file order is
  added: which order the game loads packages in is UNVERIFIED. The `ow-defs` gate of architecture section 6 ("`owners()`
  + `FirstInScanOrder` picks the same owner as the tools for every registry path, the one DLC mismatch listed") is
  amended to: **the same owner as the tools for every registry path under the tools' order, which the registry
  applies** to the `census()` holders. The 277 + 9 differences above are the record of what the two orders do.
- **Also here.** `tests/def_dump_test.py` (granted) runs `--def-dump` on an invented package with and without
  `--overlay` (the test format and a live capture); ctest runs it on the Rust release binary only
  (`def-dump-synthetic`, the C++ binary has no `--def-dump`), and it is the line of `tests/rust_parity.d/defs.txt`.
  The registry formats a census failure with `FileFailure`'s `Display` (`crates/ow-items/src/registry.rs`, granted),
  and W3-ATTREVAL's test of an unloaded definition with an empty store now expects the contract's heap error
  (`crates/ow-attr/src/value_resolvers/tests.rs`, granted): that store did not make the object, so `ensure_loaded`
  leaves it, and reading its slots fails with `SlotMissing`.
- **Open.**
  - Memory: `owners()` loads every holder's file into the package store (so `Owner::id` is set) and keeps it; the
    764-path audit sample over the whole game root peaked at 8.7 GB. Holder lookups for `find` keep only path lists.
  - Cost: the first `FirstInScanOrder` lookup builds the global index, reading every package once (6.6 s for
    `CookedPCConsole`).
  - The decode difference the comparison found (`FirstPersonParticle`, a component reference the C++ prints and the
    Rust decoder skips with a log line, 16 of 658 parts in both modes; and in Off mode 10 runs with log-only
    differences) belongs to W3-DECODE (`ow_object::decode`); not changed here.
- **UNVERIFIED.** Every holder rule as a rule of the game (D16); the prefix rule; that the live values' origin is
  what an overlay models (question 3). The real "39 changed objects" run with the local capture was not made: the
  capture `local/realgame/cards/live_weapon_data.json` is on neither machine.
