## 2026-11-16: Rust vendors, interactive objects and stations: the map reading, the cost natives, the rest NotYet

AI-assisted (Claude Code). Lane W7-WORLDLOGIC of the Rust port (architecture section 8, "Wave 7 and the host track";
section 9, "Vendors"). Number 60 was reserved for this lane by the hub. There was no real-game session: every rule
below is read from the records, the installed script or package data and is **UNVERIFIED** in the running game unless
a line says how it was confirmed. No parsing behaviour of the reader changes.

- **What changed.** `crates/ow-mission/src/vendors.rs`, `interactive.rs` and the new `stations.rs` (one `pub mod`
  line in `crates/ow-mission/src/lib.rs`, the line the brief allows), their test modules
  (`vendors/tests.rs`, `interactive/tests.rs`, `stations/tests.rs`), and the ignored installed-game run
  `crates/ow-mission/tests/w7_worldlogic_installed.rs`. No dependency, no `Cargo.toml` change, no other crate edited.
- **Rule R2 decides the split.** The use key's path, the afford check, the use and the payment order, the item
  status of a shop row, the featured price, buy-back, station registration, the checkpoint, `CanResurrectHere`, the
  respawn placement order and the exit-point round robin are installed script. The VM runs them; this lane wrote no
  Rust copy of them. What they do is recorded in own words in the module documentation (`stations.rs` holds the
  station table, `vendors.rs` and `interactive.rs` the shop and use paths), read with `research/script_disasm.py`
  (listings under ignored `local/p2/w7-worldlogic/`). Rust owns the natives the records read, the offline arithmetic
  and the reading of the map.
- **Vendors.**
  - The machine reading ports `tools/prepare_vendors.py`'s: opportunity point, population factory, shop kind,
    balance, the `Inventory` and `FeaturedItem` attachments, the level-first package order, the streamed levels.
  - Placed actors are read as the tool reads them: the first of the prefixes 4, 8, 26 whose tag walk ends exactly at
    the payload's end. The registry's tag lookup reads at 4 only (right for definitions, not for placed actors), so
    `MapView` has its own small reader with a local copy of the registry's value conversion (`plain`, private to
    `ow-items`; a shared one is an open item).
  - Reference mode reproduces the tool's listing (`loot_pools.expand` and `flatten`); Off mode refuses it.
  - Off mode: `GenerateInventory` (native, not read) is `NotYet`; `roll_attachment` fires one attachment's pool
    through W4-POOLS' Off roll with no variance formula (the machine's is not read). Not the shelf.
  - Prices: `trunc(value * markup)` in single precision with `markup = level * 0.14 + 4.1`, the shape the live dump
    confirmed on 79 of 79 rows (VENDORS.md section 3); the two attributes are live values. The featured item's cap
    (`GetCappedCurrencyAmount`) is not read, so none is applied. The sale price is `NotYet`. Buy-back is the item's
    monetary value (script).
  - The shop reset: `MinutesBetweenShopResets * 60` from the class default (script); the countdown is `NotYet`.
- **Interactive objects.**
  - `choose_usable`: the per-frame choice along the view ray (NATIVE_USE_INTERACTION.md), on caller-given hits.
  - `next_evaluation`: the 1/30 s throttle.
  - `interactive::Pack`, Off mode only: `CanAffordToUseUsableObject` and `PayForUsedObject` as
    NATIVE_CONTROLLER_HELPERS.md reads them. The cost query covers `WillowInteractiveObject`, `WillowAIPawn` and the
    `WillowPawn` base; other classes are `NotYet`. Currency goes through `ow-combat`'s `GetCurrencyOnHand` /
    `AddCurrencyOnHand` (one implementation); golden keys fire `SpendGoldenKey` per unit, and their count is
    `NotYet`. `WillowInteractiveObject.IsComponentUsable` is bound to `NotYet` (not read).
  - Reference mode binds nothing: the C++ reference binds none of these, so the stub policy answers there as before.
    No C++ stand-in was met, so no stand-in is reproduced.
  - So an interactive object's use stops in Off at `IsComponentUsable`, the first native its script reaches. The
    definition's `OnUsedBy` event native is not read either.
- **Stations.** `stations()` reads the placed fast-travel, level-travel and New-U stations with their definition,
  `bInitiallyActive`, destination, exit points and touch values. Their natives (`GetTravelStationDefinition`,
  `GetIsRegistered`, `TouchDenied`, `IsFastTravelEnabled`, `FindTravelStation`, the exit-point updates) are not read
  and left unbound.
- **The installed Sanctuary, headless** (ignored test, output under `local/p2/w7-worldlogic/`, stage 8):
  - 10 levels.
  - **8 vending machines**: 2 weapons, 3 Ammo Dump, 2 Dr. Zed, 1 Black Market, the set VENDORS.md section 1 records.
  - The Reference listing equals `tools/prepare_vendors.py`'s `local/slice/vendors.json` on **1,846 of 1,846
    compared fields** (machine, kind, each row's pool and probability, each leaf's balance, weight and class). That is
    agreement with the tool, not with the game.
  - Off: 52 of 58 attachment pools roll through `roll_pool`. The other 6 are W4-POOLS' `NotYet`: the class-mod pools
    (run-time fill, decision 2026-11-13-04) and one keyed pool. 3 of the 8 machines roll every attachment.
  - **198 interactive objects**, every one with its definition resolved and a behavior provider below it: 89 placed
    `WillowInteractiveObject`, 4 placed stations, 97 lootables and chests spawned by `PopulationFactoryInteractiveObject`
    (the count `prepare_interactives.py` records), 7 vending machines and the Black Market. The placed ones include
    3 generic doors, 4 generic buttons and the Quick Change station. 3 hold a use-cost behavior: the golden chest,
    Moxxi's tip jar and a generic switch. Use resolved in Off: 0, for the reason above.
  - **4 stations**: FastTravelStation_0 (initially active), LevelTravelStation_1, ResurrectTravelStation_0 (touch
    5000/240) and ResurrectTravelStation_1 (7500/1200). Each has 4 exit points, as SLICE_WORLD_PLACEMENT.md 1e records.
- **The bl-sdk reads that would confirm the rules** (none run):
  - a machine's `ShopInventory` slots right after a `ResetInventory`, and their count per kind;
  - `GetSellingPriceForInventory` asked about the shop's own stock and about a sold item;
  - the money before and after a real sale;
  - `SecondsUntilShopsReset` on the replication info over a minute;
  - `CanAffordToUseUsableObject` and the money before and after the golden chest, and before and after the Quick
    Change station;
  - `IsComponentUsable` hooked on a lootable press;
  - the `OnUsedBy` event name on an interactive definition's provider;
  - `GetTravelStationDefinition` on each Sanctuary station;
  - `CurrentUsableObject` along the view ray with two usable objects in a line.
