# Native: an inventory's definition, WillowInventory.GetInventoryDefinition (2026-11-19)

AI-assisted (Claude Code). Lane ANALYST-INVENTORY-DEFINITION, analyst only (clean room, docs/LANE_BRIEF.md; decision
2026-11-19-71). The sources, in the order used:
- the records, where no note reads it (NATIVE_INVENTORY_EQUIP.md lists it as not read);
- the installed packages, through `ow-package --exports` and `--payload` and `tools/ghidra/class_layout.py`;
- the script, through `research/script_disasm.py`;
- `Borderlands2.exe` locally, with the project's Ghidra tooling (read-only queries and the local read-only table reader,
  under the machine lock).

Raw output stays under ignored `local/analysis/`; this note is in our own words. No bl-sdk session was granted, and no
recorded live evidence covers it. **Every rule here is UNVERIFIED** until the live checks at the end confirm it. The
lane that wrote this note does not implement it: the native stays unbound or `NotYet` in Off until a fresh implementer
codes it from this note alone.

**Why it matters.** The pickup drivers of slice links 6 and 7 (lane IMPL-PICKUP, decision 2026-11-19-68) stop in
`make`. The weapon's drop script, `WillowInventory.DropFrom` (reached from `WillowWeapon.DropFrom` through
`Weapon.DropFrom`), calls the unbound `WillowInventory.GetInventoryDefinition()` while it sets up the dropped pickup.

## Summary

| Question | Answer | Confidence | Status |
|---|---|---|---|
| Declaration | `Engine.WillowInventory.GetInventoryDefinition()`, native, no parameters, returns a `WillowInventoryDefinition` (the return property's class, from the package) | high | UNVERIFIED |
| Script overrides | none in the installed packages | high | UNVERIFIED |
| Weapons | the weapon's `DefinitionData.WeaponTypeDefinition` | high | UNVERIFIED |
| Items | the item's `DefinitionData.ItemDefinition` | high | UNVERIFIED |
| A `WillowInventory` that is neither | None, after an engine log line saying the function has no implementation there | high | UNVERIFIED |
| Refusals | none: it never fails and never writes | high | UNVERIFIED |

## 1. Where it lives

- `WillowInventory` is an `Engine` class (it extends `Inventory`). `Engine.Weapon` extends `WillowInventory`, so every
  weapon is a `WillowInventory`. `WillowGame.WillowWeapon` extends `Weapon`, and `WillowGame.WillowItem` extends
  `WillowInventory`.
- The script declaration is native and has no script body, and no class in the installed packages declares its own
  `GetInventoryDefinition`. (`InventoryBalanceDefinition.GetInventoryDefinition` is an unrelated function of another
  class.)
- Script calls it as a final call to that one declaration (for example in `WillowInventory.DropFrom`). The native then
  answers through the object's own engine class: one rule for weapons, one for items, and a default for the base.

## 2. What it returns, by the inventory's class

The answer depends only on the object's class. It reads one field and nothing else: no attribute, no balance, no
owner, no world.

1. **A weapon** (read for `WillowWeapon`, `WillowBuzzaxeWeapon`, `WillowTurretWeapon` and `WillowVehicleWeapon`, which
   share one rule): the `WeaponTypeDefinition` of the weapon's `DefinitionData` (a `WeaponDefinitionData` struct).
2. **An item** (read for `WillowItem`, `WillowEquipAbleItem`, `WillowArtifact`, `WillowClassMod`, `WillowGrenadeMod`,
   `WillowShield`, `WillowUsableItem`, `WillowMissionItem` and `WillowUsableCustomizationItem`, which share one rule): the
   `ItemDefinition` of the item's `DefinitionData` (an `ItemDefinitionData` struct).
3. **`WillowInventory` itself**, with neither rule: the engine writes a log line naming
   `AWillowInventory::GetInventoryDefinition` as not implemented, and the answer is None. Nothing else happens; it is not
   a crash.

In each case the field is returned as it is: None when it is unset, and no check that it is the right kind of
definition. Which `DefinitionData` field is returned was matched by meaning: the read is the first field of the
class's `DefinitionData` in the project's layout tool, which is `WeaponTypeDefinition` for a weapon and `ItemDefinition`
for an item (high).

## 3. For an implementer: the Off answers, step by step

Bind `Engine.WillowInventory.GetInventoryDefinition` (not a `WillowGame.` path; see section 4). With `this` the
inventory:

1. `this` is a `WillowGame.WillowWeapon` or a subclass of it: answer the `WeaponTypeDefinition` field of
   `this.DefinitionData`, as stored (an object or None).
2. `this` is a `WillowGame.WillowItem` or a subclass of it: answer the `ItemDefinition` field of `this.DefinitionData`,
   as stored.
3. `this` is exactly `Engine.WillowInventory`: answer None. Logging the line is optional; it changes no state script
   can see.
4. Any other class, such as a plain `Engine.Weapon` or a `WillowInventory` subclass in a package not read here (for
   example a DLC): a `NotYet` of kind `NotRead`, naming the class. Its engine rule was not read.
5. A class in step 1 or 2 that does not declare `DefinitionData`, or whose `DefinitionData` lacks the field, is bad
   data (the installed classes all declare it).

Reference: the C++ has no stand-in, so the key stays unbound there.

## 4. Relation to `ow_inventory::definition`

- That module binds three `WillowInventory` natives in Off (`GenerateUniqueID`, `ComputeMonetaryValue`,
  `GetManufacturer`). Lane IMPL-PICKUP moved its paths from `WillowGame.WillowInventory.*`, which bound nothing on the
  installed data, to `Engine.WillowInventory.*`.
- **`GetInventoryDefinition` belongs with them.** It is a native of the same class, it is reached from the same
  inventory scripts, and it reads the same `DefinitionData`.
- The module's own reading of "the inventory definition" (the one definition `DefinitionData` holds, `WeaponTypeDefinition`
  for a weapon's data, `ItemDefinition` for an item's) agrees with section 2 for the two read class trees.
- One difference matters for an implementer. The native chooses the field **by the object's class**, not by which
  field is set: a weapon answers its `WeaponTypeDefinition` even when it is None. A shared helper should follow the
  class rule.

## Open questions

1. A plain `Engine.Weapon`, or any `WillowInventory` subclass outside the two read trees: whether it has its own rule.
   No such object is expected in the slice. UNVERIFIED.
2. Whether any installed weapon or item can reach `GetInventoryDefinition` with `DefinitionData` still unset (the
   answer would be None; what the caller then does is the caller's rule).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- On a dropped or held weapon, call `GetInventoryDefinition()` and compare with `DefinitionData.WeaponTypeDefinition`.
- The same on an item of each kind (a shield, a grenade mod, a class mod, a relic, a usable item, a mission item):
  compare with `DefinitionData.ItemDefinition`.
- Hook `WillowInventory.DropFrom` while dropping a gun, and read the value the call returns.
