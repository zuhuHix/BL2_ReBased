## 2026-11-15: Rust item registry reads tag streams in the tool's file order

AI-assisted (Claude Code). Lane W4-REGISTRY of the Rust port. It adds `crates/ow-items/src/registry.rs` and the
`--item-registry` command (`crates/ow-cli/src/cmd/items.rs`). Its oracle is `tools/item_registry.py`: the Rust JSON
must match that tool's `registry.json` byte for byte (architecture section 8, W4-REGISTRY). No parsing code changes.

- **Values come from tag streams, not heap objects.** The tool reads each object with `ow-package --properties
  --property-offset 4 --array-schema <its schema lines>` and writes only what is tagged. An untagged property is
  `null`, an untagged index is `-1`, and an attribute init written as `{"init": ...}` holds only its tagged fields.
  The definition store's heap objects carry class and struct defaults instead, so reading them would change those
  bytes. The registry therefore walks the owner export's tag stream itself (`ow_package::props::read_tagged` with the
  `SchemaTyper` over the tool's lines). It turns the result into the value the tool sees after `json.loads` and
  `weapon_recipe.plain`. That includes the C++ `setprecision(9)` text of a float, which Python reads back as an
  `int` when it has no `.` and no exponent.
- **Which holder owns a path comes from the store.** `DefStore::census()` gives every file of the store's scan and
  every holder of every path. The registry keeps the tool's file list and order (`weapon_catalog.package_files`):
  `.upk` files only, `WillowGame/CookedPCConsole` first, then `DLC/`, each sorted as pathlib sorts (case-folded on
  Windows). The first holder in that order is the owner. That is `FirstInScanOrder` applied in the tool's order,
  which is not the store's: the store sorts `DLC/` before `WillowGame/` and includes `.u` and `.umap` files. As the
  tool's convention, this stays UNVERIFIED as a game rule.
- **A path exported twice in one file is read from its last export.** The tool's per-file `{path: index}` dict keeps
  the last one, so the registry does the same.
- **Consequences.**
  - Overlays patch heap objects and do not reach this reading, so `--item-registry` refuses `--overlay`. Reading
    from the heap would need the store to say, per slot, whether a value was tagged.
  - The `SchemaTyper` is temporary (architecture section 2.1). When it is removed, the registry moves to the
    reflection typer, and its output must be compared with the tool's again.
- **Verified.** On the invented install of `tests/item_registry_test.py`, built with `ow_package::testkit`, the tool
  (run with the Rust `ow-package` as its reader) and the Rust registry gave the same 25,891 bytes. That test's cases
  are re-expressed as Rust tests.
- **Not verified.**
  - The installed game: `DefStore::census()` has no body yet, so `build_registry` answers `NotYet`.
  - Reader parity with the C++ `--properties` text on real packages, which the tool's recorded run used.
- **Shape (architecture section 3.7, hub decision in review r1 of PR #310).**
  - `build_registry(defs, space) -> Result<Registry, ItemError>`, with a small `ItemError` in `ow-items`' `lib.rs`.
    The registry's `game` field is the store's root as Python's `str(Path(..))` prints it, so the tool run with the
    same absolute `--game` writes the same text.
  - `Registry::candidates_at(&self, balance, variant, stage, manufacturer)` keeps its values behind a `RefCell`, so
    W4-ROLL can call it with `&Registry`. It is the tool's offline stand-in (`weapon_recipe.attribute_value`'s subset),
    kept for comparison with the tool; the roll path evaluates through the `ow-attr` evaluator.
  - For every command of the items family, `--cooked` names the install folder, not the cooked folder.
- **Review r1 fixes.**
  - Lists built by walking a variant's slots (name parts, the census checks) follow the tool's slot dict order (the
    balance's own slots, then the weapon slots), not the sorted key order the registry is written in.
  - `attribute_value` stops at a depth of 200 with "no value", where the tool gets Python's `RecursionError`, which
    its callers read the same way. That cut-off is ours and UNVERIFIED (Python's depends on its stack).
  - Values are decoded grouped by owner file, as the tool's `preload` does. Only decoded values stay; at most four
    packages are kept for later lookups.
  - `--census <file>` writes the tool's `census.json` without its timestamp, run time and `check_*` cross-checks,
    including `packages_scanned`, `scan_errors` (keyed by the POSIX path relative to the install folder, the text
    `ow-package: <error>`) and `paths_with_two_classes` in file-scan order.
  - On POSIX, `.upk` matches case-sensitively, as `rglob` does there. A UNC prefix (`\\server\share`) is kept.
- **The gate.** `tests/item_registry_rust_test.py` (ctest `item-registry-rust`) runs the tool with the C++ reader on
  an invented install and compares its `registry.json` and kept `census.json` byte for byte with the same tool on the
  Rust reader, and with the Rust `--item-registry`. The Rust comparison is reported skipped while `DefStore::census()`
  answers `NotYet`. On that install the Rust library's output (written by the crate's ignored fixture test) matched the
  tool on the C++ reader: 30,265 of 30,265 registry bytes and 4,353 of 4,353 census bytes.
- **Found by the gate, not fixed here.** For a file that is not a package, the Rust reader's stderr line differs from
  the C++ one (rule R7, `ow-container` / `ow-package`). The tool keeps that line as the file's `scan_errors` text, so
  the installed game's two bad-magic packages will differ there until the reader is fixed. The test marks the case
  `expectedFailure`, so the fix flips it.
- **What the store's census must print (review r2).** The registry writes a failed file's scan error as
  `ow-package: ` followed by its census `FileFailure.error`, so that error must print exactly as the reader's
  `--exports` stderr does after that prefix, nothing more (no `ow-package: ` of its own, no other wording). The hub
  passes this to W3-DEFS. The test compares that text in its own `expectedFailure` case (skipped while the census has
  no body); the rest of the census is compared with the scan errors by path only, so the gate does not go red when
  the census body lands before the reader is fixed.
- The rules the registry carries keep the tool's labels: the weapon merge was checked in game for part identity; the
  item merge, item weights and item stage windows are UNVERIFIED.
