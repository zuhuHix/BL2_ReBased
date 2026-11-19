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
| Weapons (the four read classes) | the weapon's `DefinitionData.WeaponTypeDefinition` | high | UNVERIFIED |
| Items (the nine read classes) | the item's `DefinitionData.ItemDefinition` | high | UNVERIFIED |
| Exactly `Engine.WillowInventory` | the game: None, after an engine log line saying the function has no implementation there | high | UNVERIFIED |
| What the game refuses | nothing: the native never fails and never writes | high | UNVERIFIED |
| Off's own refusals | named `NotYet`s of kind `NotRead` for the cases this read does not cover (section 3) | the port's choice | UNVERIFIED |
| Logging | the game logs only for the exact base; Off emits no diagnostic | the port's choice | UNVERIFIED |

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

1. `this` is exactly one of the four read weapon classes (`WillowWeapon`, `WillowBuzzaxeWeapon`, `WillowTurretWeapon`,
   `WillowVehicleWeapon`): answer the `WeaponTypeDefinition` field of `this.DefinitionData`, as stored.
2. `this` is exactly one of the nine read item classes (section 2): answer the `ItemDefinition` field of
   `this.DefinitionData`, as stored.
3. `this` is exactly `Engine.WillowInventory`: answer None, with no diagnostic. The game also logs a line here (section
   2, step 3); Off does not reproduce it, since it changes no state script can see.
4. Any other class: a `NotYet` of kind `NotRead`, naming the class, because its engine rule was not read. That
   includes:
   - a plain `Engine.Weapon`;
   - a `WillowInventory` subclass outside the two trees;
   - **any descendant of `WillowWeapon` or `WillowItem` beyond the thirteen read classes** (for example a DLC
     package's). Such a class may share its parent's rule, as a script-only class does in UE3, or bring its own engine
     override. The port's class table holds no class flags to tell the two apart (decision 2026-11-15-26).
5. **Missing layout, for a class of step 1 or 2**, each a `NotYet` of kind `NotRead` before anything is answered, the
   same treatment the module's existing definition natives give a missing layout:
   - the class declares no `DefinitionData`: "GetInventoryDefinition on a class without DefinitionData";
   - its `DefinitionData` struct declares no selected field (`WeaponTypeDefinition` for step 1, `ItemDefinition` for
     step 2): "GetInventoryDefinition on a DefinitionData without <field>".
   A layout that is declared but malformed (`DefinitionData` declared as something other than a struct, or the
   selected field declared as something other than an object reference) stays a data error, not a `NotYet`, as the
   existing definition natives treat it.
6. **A declared field that holds None** is not a refusal: the answer is None (section 2: "as stored").

The engine rule underneath (section 2) is the evidence. Steps 3 to 5 are the port's Off answers for what the read does
not cover, and they are not claims about the game.

Reference: the C++ has no stand-in, so the key stays unbound there.

## 4. Relation to `ow_inventory::definition`

- That module binds three `WillowInventory` natives in Off (`GenerateUniqueID`, `ComputeMonetaryValue`,
  `GetManufacturer`). Lane IMPL-PICKUP moved its paths from `WillowGame.WillowInventory.*`, which bound nothing on the
  installed data, to `Engine.WillowInventory.*`.
- **`GetInventoryDefinition` belongs with them.** It is a native of the same class, it is reached from the same
  inventory scripts, and it reads the same `DefinitionData`.
- The module's own reading of "the inventory definition" agrees with section 2 for the two read class trees.
- **How the module's helper chooses today** (the one `ComputeMonetaryValue` uses): it takes the first of
  `WeaponTypeDefinition` and `ItemDefinition` that the `DefinitionData` struct **declares**, even when that field holds
  None. It does not look for the first field that is set. On the installed data the two agree, because a weapon's
  struct declares only `WeaponTypeDefinition` and an item's only `ItemDefinition`.
- `GetInventoryDefinition` chooses **by the object's class** (section 3). An implementer should add that as its own
  rule, or scope any shared helper so that `ComputeMonetaryValue`'s present behaviour (the first declared field) stays
  exactly as it is.

## Open questions

1. A plain `Engine.Weapon`, any `WillowInventory` subclass outside the two read trees, and any `WillowWeapon` or
   `WillowItem` descendant beyond the thirteen read classes (step 4): whether each follows the rule of its tree or has
   its own. No such object is expected in the slice. UNVERIFIED.
2. Whether any installed weapon or item can reach `GetInventoryDefinition` with `DefinitionData` still unset (the
   answer would be None; what the caller then does is the caller's rule).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- **A populated reference.** On a held weapon, call `GetInventoryDefinition()` and compare with
  `DefinitionData.WeaponTypeDefinition`. Do the same on an item of each kind (a shield, a grenade mod, a class mod, a
  relic, a usable item, a mission item), comparing with `DefinitionData.ItemDefinition`.
- **A null reference, without a fallback.** On a scratch weapon (spawned, or a clone), set
  `DefinitionData.WeaponTypeDefinition` to None while keeping the rest. Expect None, not some other definition (for
  example the balance's). Do the same for an item's `ItemDefinition`.
- **The exact base, with its diagnostic.** Construct a bare `Engine.WillowInventory` object, if the SDK can, and call
  the function with the log captured. Expect None and one "not implemented" line. If the SDK cannot construct one, this
  receiver is an open verification gap.
- **Other receivers.** A plain `Engine.Weapon`, and any `WillowWeapon` or `WillowItem` descendant beyond the thirteen
  read classes (from a DLC package), where one can be obtained. Each is an open verification gap until obtained.
- **The reference during the drop.** Hook `WillowInventory.DropFrom` while dropping a gun. At the hook's entry read
  `DefinitionData.WeaponTypeDefinition`, then post-hook `GetInventoryDefinition` while that `DropFrom` is running and
  read its return value. This shows whether
  the reference is already set when the drop proceeds (open question 2).
