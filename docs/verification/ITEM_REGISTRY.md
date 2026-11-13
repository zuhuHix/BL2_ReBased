# Item registry: every item kind from the installed packages (2026-11-13)

AI-assisted (Claude Code, lane J of the hub run of 2026-11-13). This record holds counts, class names and rules in our own
words only. The registry itself (object paths, part lists, weights, windows) is game data and stays under ignored
`local/items/registry/` (`registry.json`, `census.json`). Nothing in this record was checked in the running game.

## 1. What the tool does

`tools/item_registry.py` scans every installed package with `ow-package` (the base game's `WillowGame/CookedPCConsole` and
each `DLC/<folder>`, the same file list and first-owner rule as `tools/weapon_catalog.py`). It decodes every inventory balance,
definition, part and part list, and writes one registry with schema tag `openwillow-item-registry/1`:

- **balances**, keyed by object path: class, kind, owning package and DLC folder, how many packages hold the path, the
  `BaseDefinition` chain (leaf first) and how it ends, manufacturers with their grade windows (stage window, level, a grade's
  own definition, spawn weight inits), the definitions it names, the gestalt, one or more **variants** (one per definition a
  class mod balance lists, otherwise one) with the merged part candidates per slot and the merge history, and the name part
  candidates;
- each **candidate** keeps the part, the list it came from, its weight source (flat 100 for an empty manufacturer list, or a
  weight per manufacturer plus the default) and its game-stage window, all unevaluated: a number when no runtime state is
  needed, the attribute init data otherwise, null for no bound;
- **definitions** (item definitions and weapon types), **parts**, **part_lists**, **name_parts**, **manufacturers** and
  **gestalts**, each with class, package and DLC folder, and for definitions and parts the balances that use them.

The field list is in the tool's docstring. Slots: items Alpha to Theta and Material; weapons their nine slots. The merge is
`weapon_recipe.merge_slots`; for items the definition's `<Slot>Parts` lists come first and each balance's collection is then
merged root first, as `prepare_vendors.item_parts` does. Weapons merge only the balances' collections, as
`weapon_recipe.merge` does. A collection is read with the slots of its own class, so a weapon part list collection that an
inventory balance names (7 do, all naming a weapon type) is read with the weapon slots. The tool imports these functions and
changes none of them. The registry is written with sorted keys and without a timestamp: two runs over the same install gave
byte-identical `registry.json` files (the timestamp is in `census.json`). Slot order is the registry's `slots` list. `candidates_at()` evaluates a
variant at one stage for one manufacturer the way `weapon_recipe` and `prepare_stock_shield` do.

## 2. Census of the installed game (zuhu desktop, 2026-11-13)

2,010 packages scanned in 150 s (the census records the run's time). Two remaster texture packages in the Mancana folder are not valid packages (bad
magic) and are skipped, as in the weapon census. Every count is of unique object paths.

| kind | balances | definitions | definitions no balance names | parts | parts no balance lists |
|---|---|---|---|---|---|
| weapon (`WeaponBalanceDefinition` 360, `MissionWeaponBalanceDefinition` 9) | 369 | 89 weapon types | 0 | 908 | 14 |
| shield | 120 | 49 | 0 | 159 | 0 |
| grenade mod | 67 | 19 | 0 | 242 | 82 |
| class mod (`ClassModBalanceDefinition`; `ClassModDefinition` 114 + `CrossDLCClassModDefinition` 24) | 56 | 138 | 0 | 38 | 0 |
| relic (`ArtifactDefinition`) | 71 | 48 | 0 | 166 | 43 |
| usable (`UsableItemDefinition`; 5 of the balances are `ItemBalanceDefinition`) | 98 | 80 | 0 | 31 | 0 |
| customization (`UsableCustomizationItemDefinition`) | 993 | 993 | 0 | none | - |
| mission item (`MissionItemDefinition`) | 160 | 270 | 110 | none | - |
| inventory balance naming a weapon type (`WeaponTypeDefinition` 43, `VehicleWeaponTypeDefinition` 7, `TurretWeaponTypeDefinition` 1) | 51 | (in the 89) | - | 7 (counted with the weapon parts) | - |
| no definition reached | 4 | - | - | - | - |
| `MarketingUnlockInventoryDefinition` | 0 | 2 | 2 | - | - |

Every balance chain ends at a root: no cycle and no missing base among the 1,989 balances. Name parts: 757
`ItemNamePartDefinition` and 876 `WeaponNamePartDefinition`; 1,573 of them are named as candidates by some balance. 22
manufacturers, 17 gestalt meshes, 448 item part lists and collections, 405 weapon part lists and collections.

**Distinct parts per slot among the merged candidates** (a slot no balance of a kind fills is left out):

| kind | Alpha | Beta | Gamma | Delta | Epsilon | Zeta | Eta | Theta | Material |
|---|---|---|---|---|---|---|---|---|---|
| shield | 16 | 21 | 13 | 45 | - | - | - | - | 68 |
| grenade mod | 18 | 20 | 12 | 29 | 20 | 6 | 7 | 11 | 37 |
| class mod | 20 | 6 | 6 | - | - | - | - | - | 6 |
| relic | 11 | 14 | 9 | 10 | 4 | 3 | 29 | 56 | - |
| usable | 13 | 10 | - | - | - | - | - | - | 8 |

Weapons: Body 162, Grip 39, Barrel 208, Sight 45, Stock 31, Elemental 32, Accessory1 63, Accessory2 1, Material 306.
Inventory balances naming a weapon type: Sight 6, Barrel 1 (7 parts that no weapon balance lists).

**Slots with no candidate.** Balances whose variants have no part slot at all: 4 shield balances (their definitions name no
part list and no balance in the chain adds one), 67 usable balances (ammo, money and the like), every customization and mission
item balance, 45 of the 51 balances naming a weapon type and the 4 with no definition. Slots enabled by a list but left empty after
the merge: grenade mod Alpha 1; weapons Elemental 8, Sight 4, Accessory1 2, Body 1, Barrel 1, Material 1, Accessory2 1.

**Per DLC folder** (balances / definitions / parts; the folders not listed hold none of that kind):

| folder | weapon | shield | grenade mod | class mod | relic | usable | customization | mission item |
|---|---|---|---|---|---|---|---|---|
| base game | 253 / 61 / 662 | 105 / 34 / 124 | 53 / 12 / 112 | 29 / 44 / 38 | 53 / 30 / 62 | 62 / 44 / 23 | 549 / 549 / 0 | 81 / 148 / 0 |
| Anemone | 27 / 5 / 79 | 3 / 3 / 20 | 4 / 2 / 113 | 14 / 22 / 0 | 3 / 3 / 62 | - | 179 / 179 / 0 | 41 / 44 / 0 |
| Aster | 38 / 0 / 46 | 5 / 5 / 4 | 6 / 1 / 6 | 6 / 54 / 0 | 2 / 2 / 11 | 2 / 2 / 0 | 12 / 12 / 0 | 9 / 21 / 0 |
| Orchid | 18 / 5 / 37 | 2 / 2 / 4 | 1 / 1 / 1 | - | 2 / 2 / 4 | 1 / 1 / 0 | 73 / 73 / 0 | 10 / 26 / 0 |
| Sage | 12 / 8 / 30 | 1 / 1 / 2 | - | - | 1 / 1 / 7 | 16 / 16 / 0 | 67 / 67 / 0 | 0 / 4 / 0 |
| Iris | 7 / 6 / 10 | 4 / 4 / 5 | 3 / 3 / 10 | 1 / 0 / 0 | 1 / 1 / 8 | 6 / 6 / 0 | 5 / 5 / 0 | 9 / 10 / 0 |
| Gladiolus | 8 / 0 / 24 | - | - | 5 / 15 / 0 | 9 / 9 / 12 | - | 10 / 10 / 0 | - |
| Lobelia | 4 / 3 / 13 | - | - | 1 / 3 / 0 | - | - | 2 / 2 / 0 | 0 / 2 / 0 |
| Lilac | 1 / 1 / 7 | - | - | - | - | - | - | - |
| Allium | 1 / 0 / 0 | - | - | - | - | 1 / 1 / 0 | 24 / 24 / 0 | 1 / 2 / 0 |
| Flax | - | - | - | - | - | 5 / 5 / 8 | 12 / 12 / 0 | 4 / 5 / 0 |
| Nasturtium | - | - | - | - | - | 5 / 5 / 0 | 24 / 24 / 0 | 5 / 8 / 0 |
| CommDay2013, Peony | - | - | - | - | - | - | 12 / 12 / 0 each | - |
| LootChest2, PN2 | - | - | - | - | - | - | 6 / 6 / 0 each | - |

## 3. What was checked, and against what

**Against the weapon census** (`WEAPON_PARTS_CATALOG.md` section 1): weapon balances 369, parts 908, weapon types 89, name
parts 876, part list collections 377, part list definitions 28, gestalts 17, manufacturers 22: all equal. Per DLC folder the
weapon balances, parts and weapon types equal the record's numbers too. The record's parts per slot (Body 168 and so on) count
every part named by any list of a chain, disabled slots included, plus a fallback slot for the 9 it could not place. The
registry counts merged candidates only: 887 of the 908 parts are candidates of a weapon balance, 7 more only of an inventory
balance naming a weapon type (the record counts those 7 among its 17 parts no balance lists, since it reads weapon balances
only), and 14 are candidates of no balance: the record's other 10, plus 4 launcher materials that an earlier list in their
chains names and a later Selective list replaces.

**Against the existing tools, on the same decoded data** (part identity and weights, so a drift between this tool and the
reused functions shows):
- every weapon balance at stages 1, 30 and 80 against `weapon_recipe.merge`: 1,104 equal; the other 3 are equal once the 4
  rows of one DLC balance's part list that name no part are left out. `weapon_recipe` keeps such a row as an empty candidate;
  the registry skips it and counts it in the census;
- every item balance whose chain names an `InventoryDefinition` at stages 1, 30 and 80 against `prepare_vendors.item_parts`
  (the parts some manufacturer can draw, in list order), 0 different. Only 297 of those balances have part slots: their 891
  comparisons are the ones that test the merge, and all 891 are equal. The other 3,675 comparisons (1,225 balances:
  customizations, mission items, ammo, money and other part-less usables) are empty against empty and test nothing about it.
  Not compared: the 51 inventory balances naming a weapon type (weapon slots, which `prepare_vendors` does not read) and 47
  balances with no `InventoryDefinition` in their chain: 44 class mod balances (43 name `ClassModDefinitions`, one names
  neither and is kept as one variant with no definition) and 3 with no definition;
- the stock shield (`tools/prepare_stock_shield.py`'s default balance, level 8, its first manufacturer): the registry's
  candidates at level 8 equal `prepare_stock_shield.candidates` part for part and weight for weight. Parts per slot 9, 9, 11,
  1, 1 (Alpha, Beta, Gamma, Delta, Material): 891 combinations, the count the slice plan gave. `prepare_stock_shield.py` is
  unchanged, so its card is unchanged.

**References:** every part, part list, definition, manufacturer, gestalt and name part the registry names resolves to an export
of the expected class in an installed package, except one definition reference (open point in section 4). One DLC balance's `InventoryDefinition` is a
path that the first package holding it exports as an `ItemPoolDefinition` and later mission packages export as a
`MissionItemDefinition`. 4 paths have two classes in different packages (all in DLC mission packages); the first owner is kept,
as in the weapon census, and the census lists each. Nothing failed to decode. One name list holds an empty entry; it is counted
and left out.

**Skipped, each counted in `census.json` with its reason:** 36 class default objects (`Default__...`) of 32 registry classes (3 each
for the item and the weapon part list collections), the 2 invalid packages, export paths held by more than one package (owner:
the first in scan order; per class counts in the census, for example 524 inventory balances and 265 weapon balances), and the 4
part list rows naming no part.

**Classes the registry does not read** (`unlisted_classes` in `census.json`: every export class ending in `BalanceDefinition`,
`PartListCollectionDefinition` or `PartListDefinition` outside the registry's tables, and every `<X>PartData` property of a
decoded collection outside the slots read): `AIPawnBalanceDefinition` 985, `InteractiveObjectBalanceDefinition` 287,
`GameBalanceDefinition` 26, `VehicleBalanceDefinition` 2. None is an inventory balance. No `<X>PartData` property is left
unread. Before this count existed, 6 Sight and 1 Barrel rows of the 7 weapon collections named by inventory balances were read
with the item slots and dropped; the count showed them, and the tool now reads those collections with the weapon slots.

**Synthetic test:** `tests/item_registry_test.py` (ctest `item-registry-synthetic`) builds a fake install of invented packages
and reads it with the real `ow-package`. It has one balance of every kind, a two-step base chain, a definition list, an
additive and a selective collection, a disabled slot, a manufacturer weight with a stage window, two grade windows, a class mod
balance with two definitions, a part no package exports (the census reports it), a missing base and a cycle, one path in two
packages and one path with two classes, a class default object, a DLC balance whose base lives in the base game's package, a
collection with no replacement mode (additive) and one in complete mode, a part list row naming no part and an empty name entry
(counted and left out), an unlisted balance class and an unlisted `<X>PartData` slot (both in `unlisted_classes`), an
inventory balance naming a weapon type with a weapon collection, two builds giving byte-identical output, and the three
comparisons above (with at least one non-empty item comparison).

## 4. Open and UNVERIFIED

- **The part-list merge for non-weapon items is reused from weapons and has not been confirmed in the running game.** The same
  holds for the item weights, the stage windows read with the item level as the stage, and the name candidates. Weapons are
  confirmed live for part identity and two balances' weights only (`WEAPON_PARTS_CATALOG.md` section 4).
- 1,036 candidate stage windows and 7 weights (two weapon balances, one in the base game and one in a DLC) need runtime state
  under `weapon_recipe.attribute_value`. The windows are candidate entries summed over balances: 1,035 of them are grenade mod
  entries in 56 balances naming 40 distinct parts, read from balance attributes such as a per-element minimum game stage, and
  one is a weapon entry. For those 1,035 the attribute's only value resolver is a constant resolver; the registry writes that constant as `attribute_constant`, a hint it does not
  apply. `prepare_vendors.item_parts` and `prepare_stock_shield` treat such a window as no bound, so they keep those parts at
  every stage. Which reading the game uses is not settled.
- The manufacturer and definition are taken from the leaf-most step that sets them (as the item tools do); `weapon_recipe.merge`
  takes the root-most. They differ for 12 class mod and 8 usable balances (manufacturer lists) and 1 usable balance
  (definition), and for no weapon balance. Every item tool today reads leaf first, so the registry's choice matches its
  consumers; it writes the root-first value too.
- **The first-owner rule picks the wrong class for one reference:** one DLC balance's definition path is held first as an
  `ItemPoolDefinition` and only by later mission packages as a `MissionItemDefinition`, so that balance's kind is
  `no_definition`. A later roll must not treat its kind as settled. The same rule keys the census's count of paths held by
  several packages by the first owner's class, which is why one `ItemPoolDefinition` appears there.
- How the game picks one of a class mod balance's `ClassModDefinitions` (modelled as variants), how it picks a manufacturer and
  grade, and when a grade's own definition replaces the balance's: not read. 5 grades name their own definition.
- The relic per-type weight hook (`WEAPON_PARTS_CATALOG.md` section 2) is not read; relic weights are open.
- A live check would be a `find_all` census of the item classes in the running game (the weapon census method) and spawned
  drops of one shield and one relic balance for slot frequencies. No real-game session was available.
