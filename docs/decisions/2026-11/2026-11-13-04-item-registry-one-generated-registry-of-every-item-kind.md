## 2026-11-13: Item registry: one generated registry of every item kind

AI-assisted (Claude Code). Lane J of the hub run of 2026-11-13, from the maintainer's decision the same day: before more item
logic (shields, vendors, loot stats), the project gets the item system itself down. Items are assembled from parts, so one
hand-made card per item is not a base to build on. Weapons already had a census and a roller; the other kinds had fragments.

- **Now:** `tools/item_registry.py` reads every installed package (base game and each DLC folder) with `ow-package` and writes
  `local/items/registry/registry.json` (ignored; game data) with schema tag `openwillow-item-registry/1`, plus a census
  (`census.json`). It covers every inventory balance class (`WeaponBalanceDefinition`, `MissionWeaponBalanceDefinition`,
  `InventoryBalanceDefinition`, `ClassModBalanceDefinition`, `ItemBalanceDefinition`). Each balance's kind comes from the
  definitions its chain names: weapon, shield, grenade mod, class mod, relic, usable, customization, mission item. A balance
  records its base chain, its manufacturers with grade windows and its definitions. It also records the merged part
  candidates per slot, each with its weight source and stage window left unevaluated, and its name part candidates and
  gestalt. Definitions, parts, part lists, name parts, manufacturers and gestalts are listed with the package and DLC folder
  that own them. The shape is in the tool's docstring and `docs/verification/ITEM_REGISTRY.md`. Later lanes (the item roll,
  the reference cross-check) read this file.
- **Reuse, no edits:** the package scan and first-owner rule of `weapon_catalog`, `weapon_recipe.merge_slots`,
  `attribute_value` and the weight constants, `prepare_vendors.item_parts`' order (definition lists first, then each balance's
  collection, root first). The existing tools are compared with the registry on the same data: weapons against
  `weapon_recipe.merge` at three stages, 1,104 equal and 3 equal once the 4 rows that name no part are left out. Items
  against `prepare_vendors.item_parts`: 4,719 equal, 0 different. The stock shield against `prepare_stock_shield.candidates`
  at level 8: equal, 891 combinations.
- **A class mod balance with several `ClassModDefinitions` gets one variant per definition** instead of one merged slot set: how the
  game picks among them is not read.
- **Manufacturer and definition:** the leaf-most step that sets them, as the item tools read them. `weapon_recipe.merge` takes
  the root-most; where they differ (20 balances) the registry writes both.

**Verified:** the census on the zuhu desktop's install (counts in `ITEM_REGISTRY.md`). Every chain ends at a root. Every part,
part list, definition, manufacturer, gestalt and name part named resolves to an export of the expected class, except one
definition reference whose path is an item pool in its first package (one of 4 paths with two classes, all in DLC mission
packages). Weapon counts equal `WEAPON_PARTS_CATALOG.md` section 1, in total and per DLC folder. The synthetic test
`tests/item_registry_test.py` (ctest `item-registry-synthetic`) builds invented packages and reads them with the real
`ow-package`.

**UNVERIFIED:**
- that non-weapon items merge part lists as weapons do (reused unchanged, not confirmed in the running game);
- item part weights, and stage windows read with the item level as the stage;
- the name candidates;
- the manufacturer, grade and class mod definition picks;
- when a grade's own definition replaces the balance's;
- the relic weight hook (not read).

1,035 grenade mod stage windows read a balance attribute whose only resolver is a constant. The existing tools treat that as
no bound. The registry records the constant as a hint and does not apply it; which reading the game uses is open.
