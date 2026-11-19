# Native rules of an inventory's value: ComputeValueOfParts, ComputeMonetaryValue and RecomputeNormalizedDamageValues (draft, 2026-11-19)

AI-assisted (Claude), analyst lane COMPUTE-VALUE-OF-PARTS (decision 2026-11-19-34). Script first: the installed
packages read with `research/script_disasm.py`. The natives were then read from the game executable with Ghidra
(`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock). Where the decompiler dropped call arguments or blurred a
conversion, the machine instructions were checked. Written in our own words: no decompiler output, pseudo-code,
addresses, offsets or listing structure is reproduced here. Field names come from the cooked packages
(`tools/ghidra/class_layout.py`). The `WillowWeapon` layout agrees with the class size the executable registers.
**Every rule is UNVERIFIED in the running game**, except where section 4 names the earlier card match. The last
section lists the live observations that would confirm the rest.

**Clean room.** This lane implements nothing. The natives stay unbound (or `NotYet`) in Off until a fresh implementer
codes them from this note alone.

**Why it matters.** Slice link 8 (equip): per #406, every installed weapon's init phase (the spawn's
`InitializeInternal`) stops at `ComputeValueOfParts`. No record had its body: NATIVE_WEAPON_RULES.md section 6 records
that the earlier
read could not resolve it. It is read here.

## Summary

| Native | Script signature (from the package) | On the slice | Confidence | Status |
|---|---|---|---|---|
| `WillowWeapon.ComputeValueOfParts`, `WillowItem.ComputeValueOfParts` | `()` | the spawn's `InitializeInternal` (after the parts, after the name parts, and at the end) | high | UNVERIFIED (the value formula it feeds matched 69 of 69 cards, section 4) |
| `WillowInventory.ComputeMonetaryValue` | `()` | inside `ComputeValueOfParts`, and from `InitializeInventory` | high | UNVERIFIED |
| `WillowWeapon.RecomputeNormalizedDamageValues` | `()` | the next unread native of the weapon's `InitializeInternal` after it | high | UNVERIFIED |

## 1. ComputeValueOfParts

One rule for weapons and items. Each has its own body, but the two differ only in which definition-data struct and
which rarity rule they use. The rule writes three fields of the inventory, in this order:

1. **Nothing happens without a definition.** The first definition of the definition data must be set:
   `WeaponTypeDefinition` for a weapon, `ItemDefinition` for an item.
2. **`RarityLevel`** is computed (section 2).
3. **`MonetaryValueModifierTotal`** (the `WillowInventory` property of that name in the packages; the value calculators
   read it as the attribute `D_Attributes.Inventory.InventoryPartMonetaryValueModifierTotal`, the name
   NATIVE_WEAPON_RULES.md and `ow-items` use; how that attribute reaches the property was not read: UNVERIFIED) starts
   at the definition's `BaseMonetaryValueModifier`.
   - Then one part at a time, in the `AllParts` order (NATIVE_EQUIP_PARTS.md section 2: the nine part slots, then the
     two name-part slots), the total is multiplied by the part's `MonetaryValueMod` attribute, evaluated with the
     inventory as context. A part whose `MonetaryValueMod` is not set, and an empty slot, leave the total as it is.
     **The name parts are included** in this walk.
   - Each product is stored back in single precision before the next part. Nothing is rounded to an integer.
   - The attribute is evaluated by its value-resolver chain, as for any attribute (NATIVE_ATTRIBUTES.md).
4. **`MonetaryValue`** = `ComputeMonetaryValue()` (section 3), which reads the total through the value attributes.

The installed script calls it three times in the spawn's `InitializeInternal`: after the parts are chosen, after the
name parts are chosen, and once more near the end, before the card is cached. Each call recomputes all three fields
from scratch, so only the last one counts. An empty name-part slot is simply skipped, so the first call's value lacks
the name parts' modifiers.

## 2. RarityLevel

- **Weapon:** the definition's `BaseRarity` (an attribute initialization value), plus each part's `Rarity`, over the
  same slot walk with the name parts included. Every value is evaluated with **its own definition or part** as
  context, then truncated toward zero to an integer **before** it is added. A truncated sum of untruncated values
  would differ.
  - An unset value adds 0. With no base attribute and no initialization definition, the evaluator's answer is the
    value's constant times its scale constant, and an unset struct holds zero there. An empty slot adds nothing.
  - **The mission rule below does not apply to weapons:** the weapon rule has no mission-item case.
- **Item:**
  - for an item whose definition is a `MissionItemDefinition` (or a subclass), the answer is the globals definition's
    `MissionItemRarityLevel`; it is 0 when there is no globals object or globals definition;
  - every other item uses the weapon's rule on the item's own definition and parts.

## 3. ComputeMonetaryValue

The same rule NATIVE_LOOT_SPAWN.md gives:
1. 0 without an inventory definition;
2. otherwise the definition's `MonetaryValue` (an attribute initialization value) is evaluated **with the inventory
   itself as context**, truncated toward zero, and capped by the definition's `FormOfCurrency` (99,999,999 credits,
   500 Eridium, 999 otherwise; 0 past the enum's end).

This read settles the context that note left open.

`ComputeValueOfParts` ends with this same body. So `MonetaryValue` is always written last, from the total computed
just before it.

## 4. How this matches the earlier card evidence

NATIVE_WEAPON_RULES.md section 6 matched 69 of 69 golden sale values with a calculator of the form
`Att_Manufacturer_<Maker> × PartTotal × Att_UniversalPriceIncreasePerLevelScaler ^ level × Att_BaseCost_Guns_<Type>`,
truncated. That match needed every part's **and both name parts'** `MonetaryValueMod` in the part total.

The read here is that structure:
- `PartTotal` is `MonetaryValueModifierTotal` (read by the calculator as the attribute
  `InventoryPartMonetaryValueModifierTotal`), which includes the name parts;
- the calculator is the definition's `MonetaryValue` initialization data, evaluated with the inventory as context;
- the truncation is `ComputeMonetaryValue`'s.

The card match therefore confirms the outcome of the last call (69 cards, 2026-10-03), but not the native's order of
effects.

## 5. The next native: RecomputeNormalizedDamageValues

In the weapon's `InitializeInternal`, the calls after the first `ComputeValueOfParts` are `InitializeAttributeSlots`
and `ApplyInternalSlotEffectModifiers`, both already in NATIVE_INDEX.md, with the script function
`ApplyAllWeaponAttributeEffects` between them. The first unread native after it is
`WillowWeapon.RecomputeNormalizedDamageValues()`. It rebuilds the two "normalized" damage values:

1. **Instant-hit:**
   - `NormalizedInstantHitDamageModifierStack` becomes a copy of `InstantHitDamageModifierStack`;
   - `NormalizedInstantHitDamageBaseValue` = the weapon type definition's `NormalizedInstantHitDamage` initialization
     value, evaluated with the weapon as context;
   - `NormalizedInstantHitDamage` = that base with the copied stack applied, by the engine's stack rule
     (NATIVE_ATTRIBUTES.md, "the stack formula").
2. **Melee:** the same with `MeleeDamageModifierStack` and the definition's `NormalizedMeleeDamage`, into
   `NormalizedMeleeDamageModifierStack`, `NormalizedMeleeDamageBaseValue` and `NormalizedMeleeDamage`.

No rounding; single precision.

After it, the script goes on to the name parts, **only on the name-part branch** (corrected by
NATIVE_CACHE_WEAPON_CARD.md section 1, read with the converted jump targets):
1. `ChooseRandomNameParts` (native, read there);
2. `ComputeValueOfParts` (its second call);
3. `InitializeAttributeSlotsForNameParts` and `ApplyInternalSlotEffectModifiers` (both in the index);
4. `ApplyNamePartWeaponAttributeEffects` (script);
5. `RecomputeNormalizedDamageValues` (its second call, still inside the branch).

Then, on every path: `ComputeValueOfParts` (its third call), `CacheWeaponCard` (native, read in outline there),
`RecomputeAttributeBaseValues` (in the index), `ClientInitializeFromWeaponTypeDefinition` (script) and
`GetFiringModeDefinition` (in the index).

Which of these the equip run stops at next was not run: the init phase stops at `ComputeValueOfParts` until it is
bound, and the equip run's weapons list is not on this machine.

## 6. For an implementer, in the port's terms

- `ComputeValueOfParts` and `ComputeMonetaryValue` share their last step. Implementing `ComputeMonetaryValue` once
  (FIX-POOL-SPAWN-B) and calling it at the end of `ComputeValueOfParts` matches the read.
- The slot walk is the existing `AllParts` iterator, with the name parts included.
- Truncation happens per value for rarity, and once at the end for the money value. The part total is never rounded.
- The evaluation contexts differ:
  - the money value and the part modifiers use the inventory;
  - the rarity values use the definition or the part itself;
  - the normalized damage uses the weapon.

## Open questions

1. Whether `ComputeValueOfParts` is called on items outside the spawn (the shop and the save-load paths), and with
   what part data.
2. `ChooseRandomNameParts` and `CacheWeaponCard`: read since, in NATIVE_CACHE_WEAPON_CARD.md.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- On a dropped gun read `RarityLevel`, `MonetaryValueModifierTotal` and `MonetaryValue`, and recompute them from its
  parts. The rarity should be the sum of the truncated values, and the total should include the prefix's and title's
  `MonetaryValueMod`.
- On a mission item, read `RarityLevel` against the globals definition's `MissionItemRarityLevel`.
- On a gun, read `NormalizedInstantHitDamage` and `NormalizedMeleeDamage` against their base values and the copied
  stacks, before and after an equip that adds a modifier to `InstantHitDamage`.
