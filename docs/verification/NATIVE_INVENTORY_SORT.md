# Native backpack sort, filter and sub-header rules (2026-10-04)

AI-assisted (Claude). Behaviour notes written from a local reading of `Borderlands2.exe` in Ghidra, the installed
script (`research/script_disasm.py`) and installed class defaults (`ow-package --properties`), under the policy in
[LEGAL.md](../LEGAL.md) ("Analysing the executable") and [NATIVE_ANALYSIS.md](../NATIVE_ANALYSIS.md). Nothing below is
listing or pseudo-code; functions are named by their script name or by what they do, and the few addresses are
identifiers only. Raw output is under ignored `local/analysis/E/`.

**Every rule here is `UNVERIFIED` in the running game** unless it says otherwise. The only in-game evidence is the
2026-09-30 observation in [INVENTORY_MOVIE_PROTOTYPE.md](INVENTORY_MOVIE_PROTOTYPE.md) (PageDown cycle ALL -> TYPES ->
BRANDS -> ITEMS -> VALUE -> ALL; WEAPONS, RELICS, CLASS MODS seen in that order; ASSAULT RIFLES before SUB-MACHINE
GUNS; brand headers alphabetical; VALUE headerless and dearest first; ITEMS non-weapons only; TYPES weapons only).
Every one of those observations is reproduced by the rules below; the rest of this note is new.

## 1. Where the order lives

* The five modes are **data**: `StatusMenuInventoryPanelGFxObject.BackpackSortConfigurations` (class default). Each
  entry is a `SortFilterConfiguration {SortType, FilterType, CategoryType, SortTitleLookupKey}`. The movie's
  PageDown/PageUp send `extOnChangeSort(+1/-1)`; the next index is `WillowInventoryGFxMovie.GetNextSortConfiguration`
  (native, wraps around the array; the observed cycle direction fits). The chosen index is stored in
  `WillowPlayerController.BackpackSortPreferenceIndex` and written to profile setting 166 when the inventory screen
  closes (`SaveBackpackSortPreference`); the panel starts from it next time.
* The list handed to the panel is the unreadied weapons followed by the unreadied items
  (`StatusMenuExGFxMovie.PrepareListOfAllInventory`). `InventoryListPanelGFxObject.SetList` loads them into the data
  provider and then `ApplySortConfiguration` (native) does, in this order: **sort all entries with the comparator for
  `SortType`; mark entries hidden by the filter for `FilterType`; assign each entry a category (header) index with the
  function for `CategoryType`; rebuild the Scaleform list**, skipping hidden entries and inserting a header row
  whenever the category index changes from the previous visible entry (first entry always starts a header when it has
  one). An entry without data (an empty slot) sorts after every real item and has no category.
* The sort itself is the engine's unstable quick sort over 16-byte entries. Ties that the comparator reports as equal
  are therefore in no guaranteed order. Where a comparator is documented below as "ties: pointer", it falls back to
  the item objects' memory addresses (lower first), which in practice is allocation order; a host should use
  backpack index (pickup order) there and label it a host choice.

## 2. The five stock configurations (read from the installed defaults)

| # | Title key (label) | SortType | FilterType | CategoryType |
|---|---|---|---|---|
| 0 | `all` (ALL) | `IST_MajorTypeThenRarityThenSubtype` | `IFT_NoFilter` | `CAType_InventoryType` |
| 1 | `types` (TYPES) | `IST_MajorTypeThenSubtypeThenRarity` | `IFT_FilterNonWeapons` | `CAType_WeaponType` |
| 2 | `manufacturers` (BRANDS) | `IST_Manufacturer` | `IFT_NoFilter` | `CAType_Manufacturers` |
| 3 | `items` (ITEMS) | `IST_MajorTypeThenRarityThenSubtype` | `IFT_FilterWeapons` | `CAType_InventoryType` |
| 4 | `value` (VALUE) | `IST_Value` | `IFT_NoFilter` | `CAType_None` |

Enum orders (decoded from `WillowGame.upk`): sort `EquippedThenMajorTypeThenRarityThenSubtype` 0,
`MajorTypeThenSubtypeThenRarity` 1, `MajorTypeThenRarityThenSubtype` 2, `Manufacturer` 3,
`ClassRequirementThenRarity` 4, `Value` 5; filter `NoFilter` 0, `FilterUncomparable` 1, `FilterWeapons` 2 (hides
weapons), `FilterNonWeapons` 3 (hides everything but weapons), `FilterNonShields` 4 ... `FilterNonGenerics` 9; category
`None` 0, `InventoryType` 1, `Manufacturers` 2, `WeaponType` 3, `ClassRequirement` 4, `PersonalOrShop` 5, `Equipped` 6.
Sort types 0 and 4 and the other filters/categories belong to other panels (vendor, trade, compare) and are not used by
the backpack.

## 3. Item facts the comparators read

All comparators call these on the entries' inventory objects (all fields are on the base inventory class):

* **Equipped flag** = `Inventory.bReadied`. Backpack items are never readied, so the "equipped" clauses below never
  fire in the Backpack; they matter for vendor/trade panels.
* **Is weapon** = the object is a `WillowWeapon`. **Is mission weapon** = the weapon's mission-inventory test (the
  `IMissionInventory` interface), the same test that picks the MISSION WEAPONS header.
* **RarityLevel** = the integer item field `RarityLevel` (not the colour tier; see
  [NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) section 4, "rarity level = sum of truncated part rarities").
* **ExpLevel** = the item's `ExpLevel` (read through the `IBalancedActor` interface, `GetExpLevel`).
* **Category key** = `GetCategoryKey()`: by default the item's `ZippyFrame` name, replaced by the instance-data
  string `CategoryKey` when the item has one (`WillowItem`), and the constant `mod` for grenade mods. For weapons
  `ZippyFrame` is the type's `ScaleformFrameName` (`pistol`, `ar`, ... in the installed defaults). The frame names
  double as keys of the `[CategoryLabels]` localisation section (`ar` ASSAULT RIFLES, `pistol` PISTOLS, `repeater`,
  `revolver`, `rocket` ROCKET LAUNCHERS, `shotgun`, `smg` SUB-MACHINE GUNS, `sniper`, `artifact` RELICS, `comm` CLASS
  MODS, `mod` GRENADE MODS, `shield` SHIELDS, `health` MED KITS, `sdu` UPGRADES, `ammo`, `grenade`, `elemental`
  ELEMENTAL ARTIFACTS).
* **Value** = the item's stored `MonetaryValue` (no quantity factor).
* **Manufacturer** / **grade** = `GetManufacturer()` and `GetManufacturerGradeIndex()`.

## 4. Comparators (negative = first argument first)

Common preface, every comparator: if either entry is missing, 0; an entry whose data is empty sorts after every entry
that has data; two empties are equal.

**Level tie-break "L"** (used at the end of several chains): let a, b be the two items' `ExpLevel`. If both are below
51 the result is 0 (equal); otherwise higher level first (b - a). So for normal level 1-50 gear it never decides.

**Rarity-then-level "R"**: higher `RarityLevel` first (b - a); if equal, L.

### 4.1 `IST_MajorTypeThenRarityThenSubtype` (ALL, ITEMS)

1. Equipped items after unequipped (never in the backpack).
2. Weapons before non-weapons.
3. Two weapons: mission weapons first; then higher `RarityLevel` first; then, only if the category keys are equal,
   L. **Weapons of different type at equal rarity are not ordered by their type name here**: the read code only uses
   the key comparison to gate L and otherwise falls to the pointer tie. This looks odd (the mode is called "...then
   subtype"); the read is solid but the effect is not game-confirmed.
4. Two non-weapons: category key compared case-insensitively **ascending** (artifact < comm < health < mod < sdu <
   shield, which is the observed RELICS before CLASS MODS); if equal, the item's frame string (`GetZippyFrame`) the same
   way, with an item whose frame is the name "None" sorted first; if equal, R.
5. Remaining ties: pointer.

### 4.2 `IST_MajorTypeThenSubtypeThenRarity` (TYPES)

1. Equipped items after unequipped.
2. Weapons before non-weapons (the filter removes non-weapons here anyway).
3. Two weapons: mission weapons first; then category key ascending (`ar` before `pistol` ... `smg` ... `sniper`, the
   observed order); if equal, R. Two non-weapons: category key ascending, then R.
4. Result 0 is returned as such (ties unordered).

### 4.3 `IST_Manufacturer` (BRANDS)

1. Equipped items after unequipped.
2. If either item has no manufacturer: 0 (equal to everything, which is a non-transitive comparison; the engine does
   it, so entries without a manufacturer end up wherever the sort leaves them).
3. Manufacturer display name compared case-insensitively **ascending**; then manufacturer grade index ascending;
   then R.

### 4.4 `IST_Value` (VALUE)

Higher `MonetaryValue` first. No other criterion; equal values compare equal (unordered). No equipped clause.

### 4.5 `IST_EquippedThenMajorTypeThenRarityThenSubtype` (vendor/trade, not the backpack)

Equipped first, then 4.1. `IST_ClassRequirementThenRarity` (not used by the five defaults): equipped after
unequipped, then by the index of the item's class-requirement entry in a global list, ties by pointer.

## 5. Filters (true = hide the entry)

* `IFT_NoFilter`: nothing hidden.
* `IFT_FilterWeapons` (ITEMS): hides every `WillowWeapon`.
* `IFT_FilterNonWeapons` (TYPES): hides everything that is not a `WillowWeapon`.
* `IFT_FilterUncomparable` (Compare view): hides items that do not share the compared slot type; the Compare view
  list the maintainer saw (weapons only under one WEAPONS header) is this filter. Exact slot rule: not read.
* `IFT_FilterNonShields`, `...NonGrenadeMods`, `...NonClassMods`, `...NonUpgrades`, `...NonHealth`, `...NonGenerics`
  (vendor panels): hide items whose category key is not respectively `Shield`, and so on. Only the shield test was read.

## 6. Sub-headers

A header row is shown before the first visible entry and whenever the category index of a visible entry differs from the
previous visible entry's. The label text is localised from the `[CategoryLabels]` section of `WillowGame.int`
(uppercase text in game); the category functions are:

* `CAType_InventoryType` (ALL, ITEMS): equipped item -> `equipped` (EQUIPPED); unequipped weapon -> `weapon` (WEAPONS) or
  `missionweapon` (MISSION WEAPONS) by the mission-weapon test; anything else -> its category key (RELICS, CLASS MODS,
  GRENADE MODS, SHIELDS, ...). Because the comparator puts mission weapons first, MISSION WEAPONS comes before WEAPONS.
* `CAType_WeaponType` (TYPES): equipped -> `equipped`; else the weapon's category key lowercased as a label key
  (`ar` ASSAULT RIFLES, `smg` SUB-MACHINE GUNS, ...).
* `CAType_Manufacturers` (BRANDS): equipped -> `equipped`; no manufacturer -> **no header at all**; else a header whose
  text is the manufacturer's display name (which is why the Bandit header reads like the manufacturer name and not like
  the card logo text).
* `CAType_None` (VALUE): no headers.

Empty cells are not part of the sorted data; the trailing empty cells the maintainer saw come from the movie, not
from this code (the list is built from the real items only: `BackpackThings`).

## 7. Host mapping

`tools/hud_overlay/inventory.js` (`sortModes`, `changeSort`, `sortedItems`) is Lane D's. A faithful host list is: build
entries from the backpack in pickup order; sort with the comparator of the current mode (4.1-4.4) using a stable sort
(documented host choice for ties); drop entries hidden by the mode's filter (section 5); emit a header before the
first entry and at each label change (section 6). PageDown = +1, PageUp = -1 with wrap over the five entries; reset the
selection to the first entry after every change; persist the index across screen openings.

## 8. What was not read, and how to confirm

* Item level semantics beyond "ExpLevel read through the interface at the fourth virtual slot".
* The exact slot test of `IFT_FilterUncomparable`, the class-requirement list, and the manufacturer display-name
  source (read only as "a localised name string").
* Which exact quicksort variant runs (affects only unordered ties).
* **Confirmation:** one capture of a seeded backpack in the original game per mode (same method as the 2026-09-30
  observation) that includes two weapons of equal rarity but different type (item 4.1 point 3), two items of equal
  value (4.4), a mission weapon, a shield and a grenade mod. The golden item cards in
  [REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md) give the rarity levels needed to predict the order.
