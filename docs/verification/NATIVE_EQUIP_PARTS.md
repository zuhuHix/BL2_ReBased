# Native equip parts: the part iterator `AllParts` (draft, 2026-11-19)

AI-assisted (Claude Code), analyst lane FIX-EQUIP-NATIVES. Read from the game executable with Ghidra 12.1.4
(`tools/ghidra/run.ps1`, a read-only query of `WillowWeapon.AllParts` with two levels of callees, against the
project made by `tools/ghidra_import.ps1`) and written in our own words. No decompiler output, pseudo-code or listing
structure is reproduced here, and the raw output stays outside the repository. Field names and positions come from
the cooked packages (`tools/ghidra/class_layout.py`, whose `Core.Object` oracle of 60 bytes holds). The callers named
below are the installed script, read with `research/script_disasm.py`. **Every rule is UNVERIFIED in the running
game**; nothing here was observed live.

This note hands the rule to the implementer of `ParityMode::Off`. The analyst lane left `AllParts` unbound, so an Off
run stops there with the VM's `UnboundNative` text (decision 2026-11-19-03). A fresh implementer should build the Off
rule from this note alone (docs/LANE_BRIEF.md, "Clean room").

## 1. Where it sits

`AllParts(DefinitionData, out Part, bIncludeNameParts)` is a script iterator: a `foreach` over it runs its body once
per part. One native body serves both script declarations, `WillowItem.AllParts` and `WillowWeapon.AllParts`; the two
registered natives point at the same code. The installed script calls it from these functions:

- the weapon's `ApplyAllExternalAttributeEffects` (the equip, from `WeaponEquipping.BeginState`), with the flag true;
- the weapon's `ApplyAllWeaponAttributeEffects` (the spawn's `InitializeInternal`), with the flag passed through;
- the weapon's zoom effect functions, `InitMeshes` and `InitPartParticles`;
- the item's `ApplyAllExternalAttributeEffects`, `ApplyAllItemAttributeEffects` and
  `ClientInitializeFromItemDefinition`, and the grenade mod's `Init` and `CanAIUse`.

It reads no other state and changes nothing but its out argument.

## 2. The rule

- **Slots, in order.** The definition data struct has eleven part slots after its four leading fields (the type, the
  balance, the manufacturer and the grade index). The iterator visits them in their declared order:
  - weapon (`WillowWeaponTypes.WeaponDefinitionData`): `BodyPartDefinition`, `GripPartDefinition`,
    `BarrelPartDefinition`, `SightPartDefinition`, `StockPartDefinition`, `ElementalPartDefinition`,
    `Accessory1PartDefinition`, `Accessory2PartDefinition`, `MaterialPartDefinition`, then the two name parts
    `PrefixPartDefinition` and `TitlePartDefinition`;
  - item (`WillowItemTypes.ItemDefinitionData`): `AlphaItemPartDefinition` through `ThetaItemPartDefinition` (eight),
    `MaterialItemPartDefinition`, then `PrefixItemNamePartDefinition` and `TitleItemNamePartDefinition`.
  The native picks each slot by its position in the struct, and the positions above are the package layout's (all
  eleven are 4-byte object references, starting at byte 16 of the 68-byte struct in both structs). The type,
  balance, manufacturer, grade index, `GameStage` and `UniqueId` are never visited.
- **Empty slots are skipped.** A slot that holds None is not visited; the body runs only for set parts.
- **The flag.** The two name-part slots (the tenth and eleventh) are visited only when `bIncludeNameParts` is true
  (any nonzero value counts as true). With it false the walk ends after the material slot, whatever the name slots
  hold. The first nine slots are visited either way, the material slot included.
- **Each step.** Before each step the out argument is set to None. If there is a part left, the out argument gets it
  and the body runs. When no part is left the loop ends, so the out argument is None after a loop that ran to its
  end. What it holds after a `break` in the body was not settled.
- **Look-ahead.** The iterator finds the next set slot before it runs the body for the current one. A body that
  writes the definition data while the loop runs could see this; no installed caller does, and whether the native
  reads the caller's struct or a copy of it was not settled.
- **Edge cases.** All eleven slots None: the body never runs and the out argument is None. A duplicate part (the same
  object in two slots) is visited once per slot. Parts are not type-checked: the out argument receives whatever
  object the slot holds.

## 3. What it means for the equip

The weapon's equip script applies the weapon type's `ExternalAttributeEffects`, then (in this iterator, flag true)
each set part's `ExternalAttributeEffects` in the slot order above, name parts included, then the external slot
modifiers. The spawn script applies each part's `WeaponAttributeEffects` the same way, with the name parts only when
it was asked for them.

`ow_inventory::equip` already applies the parts in the first nine slots' order for its Rust equip (its documentation
notes the order and the flag as unread). Per this note, the equip walk would also visit the prefix and title, because
the equip passes the flag true. Whether a name part ever carries `ExternalAttributeEffects` in the installed data was
not counted here.

## 4. For the implementer

- Bind `WillowItem.AllParts` and `WillowWeapon.AllParts` in Off as one iterator over the definition data value the
  script passes. Visit the slots by field name, in the order of section 2, and stop before the name parts when the
  flag is false.
- Check against the existing tests: the order of the first nine slots and the material's place are those of
  `ow_inventory::equip::WEAPON_PART_FIELDS`.
- Reference: the C++ VM binds no `AllParts`, so Reference keeps the stub policy's answer (R3).
- Real-game check (none was run for this note): on a gun whose prefix or title part carries an external effect, if
  any exists, equip it and read the owner's attribute stack with bl-sdk. The effect should be present after the equip.

## 5. Not read

- `ComputeValueOfParts` (the spawn's `InitializeInternal` reaches it first; NATIVE_WEAPON_RULES.md section 6 records
  that its body was not resolved).
- `InitializeAttributeSlots`' computed slot value (`ComputedModifierValue`; NATIVE_ATTRIBUTES.md section 9, "Open").
