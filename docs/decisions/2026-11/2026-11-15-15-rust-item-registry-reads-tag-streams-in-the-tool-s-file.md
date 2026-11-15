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
- The rules the registry carries keep the tool's labels: the weapon merge was checked in game for part identity; the
  item merge, item weights and item stage windows are UNVERIFIED.
