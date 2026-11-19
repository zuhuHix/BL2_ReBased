## 2026-11-19: Read the inventory value natives as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane COMPUTE-VALUE-OF-PARTS, analyst only (slice link 8). Per #406, every installed weapon's init phase stops at
`ComputeValueOfParts`, whose body no record had. The new own-words note `docs/verification/NATIVE_INVENTORY_VALUE.md`
and three `NATIVE_INDEX.md` rows record:

- `ComputeValueOfParts` (weapons and items) writes three fields:
  - `RarityLevel`: the definition's base rarity plus each part's rarity, each truncated before the sum. A mission
    item takes the globals definition's `MissionItemRarityLevel` instead;
  - `MonetaryValueModifierTotal` (the property's package name; calculators read it as the attribute
    `InventoryPartMonetaryValueModifierTotal`): the definition's base modifier times every part's and both name parts'
    value
    modifiers, unrounded;
  - `MonetaryValue`: through the same body as `ComputeMonetaryValue`.

  It walks the parts in the `AllParts` order, with the name parts included.
- `ComputeMonetaryValue` evaluates with the inventory itself as context. That settles an open question of
  NATIVE_LOOT_SPAWN.md, and that note now says so. NATIVE_EQUIP_PARTS.md's "Not read" entry points here.
- The next unread native of the weapon's `InitializeInternal`, `RecomputeNormalizedDamageValues`: the normalized
  instant-hit and melee damage are rebuilt from copies of the plain damage values' modifier stacks.

The structure agrees with the 69-of-69 sale-value match of NATIVE_WEAPON_RULES.md section 6. That match confirms the
outcome, not the native's order of effects. Raw output stays under ignored `local/analysis/cvop/`; the Ghidra runs
were read-only and took the machine lock. Nothing is implemented: the natives stay unbound or `NotYet` in Off until a
fresh lane codes them from the note. Everything else is UNVERIFIED in the running game; the note lists the bl-sdk
checks.
