## 2026-11-19: Rust inventory value natives in Off: ComputeValueOfParts and ComputeMonetaryValue

AI-assisted (Claude Code). Lane FIX-COMPUTE-VALUE, implementer (slice link 8, equip). Number 36 was given by the hub.
The branch is `origin/main` 2981b801 (written on 388a55ee, rebased as main moved). The source is 9554's
NATIVE_INVENTORY_VALUE.md (decision 2026-11-19-34), first read from its research branch at 08f84f18, then at its review
r1 (0cf2e5b8), which is the version now on main (#419), byte for byte. This lane did not analyse it and opened no
`local/analysis/`, Ghidra or REA output. No dependency, no CLI flag, no parsing change. There was no real-game session;
every rule is **UNVERIFIED** in the running game. The value formula the money step feeds matched 69 of 69 cards earlier
(the note's section 4); that match does not confirm the native's order of effects. Reference has no C++ stand-in and is
unchanged: neither native is bound there (R3).

- **New module `ow_inventory::equip::value`** (Off only), registered by the inventory pack after the equip natives.
  - **`WillowWeapon.ComputeValueOfParts()` and `WillowItem.ComputeValueOfParts()`** (section 1). Without the
    definition data's first definition (`WeaponTypeDefinition`, `ItemDefinition`), nothing is written. Otherwise:
    1. `RarityLevel` (section 2): the definition's `BaseRarity` plus each set part's `Rarity`, each evaluated with its
       own definition or part as context and truncated toward zero before it is added.
       - An unset value adds 0 (the evaluator's constant times scale of an all-zero struct); an empty slot adds
         nothing.
       - For a `MissionItemDefinition` item it is the globals definition's `MissionItemRarityLevel` instead: the
         installed `GetGlobalsDefinition` on the world's globals object, 0 without a globals definition. A world
         without a globals object answers the evaluator's own `NotYet` (the port has not built that layer; the game
         always has the object). The mission rule does not apply to weapons.
    2. `MonetaryValueModifierTotal`, the `WillowInventory` property of that name in the packages: starts at
       `BaseMonetaryValueModifier`, then is multiplied one part at a time, in walk order, by the part's
       `MonetaryValueMod`, each product stored back in single precision, nothing rounded to an integer. Each modifier
       is evaluated by its value chain with the inventory as context. A part with no `MonetaryValueMod`, and an empty
       slot, leave the total as it is. The value calculators read the total as the attribute
       `D_Attributes.Inventory.InventoryPartMonetaryValueModifierTotal`; how that attribute reaches the property is
       not read (UNVERIFIED), and this module writes only the property.
    3. `MonetaryValue` = the rule below.

    The part walk is the `AllParts` slot order of NATIVE_EQUIP_PARTS.md section 2 **with the name parts**: nine part
    slots, then the two name-part slots, empty slots skipped. It reads the slots directly rather than through the
    `AllParts` iterator (FIX-ALLPARTS, landed while this lane worked): same order, and no script loop is needed.
  - **`WillowInventory.ComputeMonetaryValue()`** is lane FIX-POOL-SPAWN-B's (`ow-inventory` `definition`, decision
    2026-11-19-29), which landed while this lane worked. This lane first bound its own copy; after the rebase it binds
    none, and `ComputeValueOfParts` ends with that lane's body, now a shared function
    (`definition::monetary_value_of`; the native is a thin wrapper of it, unchanged in behaviour). That body still
    answers `NotYet { NotRead }` for a `MonetaryValue` that reads an attribute or an initialization definition: the
    evaluation context was NATIVE_LOOT_SPAWN.md's open question 2. NATIVE_INVENTORY_VALUE.md section 3 now answers it
    (the inventory itself), so that refusal can be lifted in `definition`; this lane leaves that lane's rule and tests
    as they are.
  - **Not taken here:** `RecomputeNormalizedDamageValues`, which the note also reads (section 5), and the note's open
    questions (callers outside the spawn; `ChooseRandomNameParts` and `CacheWeaponCard`).
- **Verified (automated, local, pc-hummer).**
  - **Synthetic tests** (`ow-inventory`, invented class set, every init value a constant at scale 1):
    - a weapon's rarity is the sum of the truncated values: 2 + 1 + 3 = 6, where the truncated sum would be 8;
    - its modifier total includes the prefix name part (2 x 1.5 x 3 = 9), and a part with no modifier and an empty slot
      change nothing;
    - its money value is `ComputeMonetaryValue`'s body on a constant (the caps stay `definition`'s own tests);
    - an unset part `Rarity` adds 0, and a part with no `MonetaryValueMod` leaves the total;
    - no definition writes nothing;
    - an item uses the rule on its own definition, and a mission item with no globals object is an error, not a
      guessed 0;
    - Reference binds neither `ComputeValueOfParts`.
  - **Installed `--inventory-move --equip-run`** (50 weapons, decision 03's recipe, the list rebuilt by FIX-ALLPARTS),
    Off, before and after, on main cb01e96f:
    - init: before, 50 of 50 at the unbound `WillowWeapon.ComputeValueOfParts()`. After, 50 of 50 pass the rarity and
      the modifier total and stop in the money step, at `definition`'s open-question-2 refusal (every installed
      weapon's `MonetaryValue` reads attributes). On 5554eadf, with this lane's own copy evaluating from the
      inventory, the same step reached `InventoryAttributeContextResolver`, whose equipped-only flag property is not
      read (NATIVE_ATTRIBUTES.md section 5): that is the stop once the refusal is lifted (a temporary trace, not kept,
      placed it);
    - base values: 50 of 50 pass, before and after;
    - equip: the same before and after: 49 at `AttributeSlots: expected an array, found Struct` and 1 at the game
      info query (with `AllParts` bound by FIX-ALLPARTS; on 388a55ee it was 50 at the unbound `AllParts`);
    - unequip: 50 of 50 pass, before and after.
  - **`--slice-gate`**, strict: byte-identical before and after, 0 / 11 / 3 on cb01e96f. Link 8 has no driver.
- **Open.**
  - Lifting `definition`'s open-question-2 refusal (the context is now answered: the inventory), then the
    `InventoryAttributeContextResolver` rule, for the money step and the rest of the spawn's `InitializeInternal`.
  - How the attribute `InventoryPartMonetaryValueModifierTotal` reaches the property `MonetaryValueModifierTotal`.
  - `RecomputeNormalizedDamageValues`, for an implementer.
  - The note's two open questions, for an analyst.
- **Gates** (on cb01e96f; on 2981b801 after the last rebase: 2,007 / 0 / 45, the same equip run): workspace build
  clean; workspace tests 2,004 passed / 0 failed / 45 ignored (`ow-inventory`'s
  value tests among them); `crate_edges_test.py` OK; decisions check OK.
- **The note's review r1 of #419** (re-read at 0cf2e5b8). It settles the points this lane first left open: the property
  keeps its package name `MonetaryValueModifierTotal` (the attribute name is the calculators' view of it); the multiply
  is one part at a time, stored back in single precision; an unset value adds 0; the mission rule does not apply to
  weapons. An interim version of this branch, never pushed, answered `NotYet` for an unset part `Rarity` and wrote
  `InventoryPartMonetaryValueModifierTotal`; both are gone.
