## 2026-11-13: Items roll from the registry, and a checker proves each roll legal

AI-assisted (Claude Code). Lane B of the hub run of 2026-11-13, the step after the item registry
(`2026-11-13-04-item-registry-one-generated-registry-of-every-item-kind.md`) and the native note on item rules
(`docs/verification/NATIVE_ITEM_RULES.md`).

- **Now:** `tools/item_roll.py` rolls shields, grenade mods, class mods and relics from the registry: manufacturer and grade from
  the grade windows, definition, one part per slot, level, name parts, rarity level and band (by name, no colours). `check()`
  proves a roll legal against the registry. `--census` rolls every balance of the four kinds over the player's install and
  checks every roll; output under ignored `local/items/roll/`. Record: `docs/verification/ITEM_ROLL.md`.
- **Reuse, no edits:** `weapon_recipe`'s `CrtRand`, `pick`, `best_name_part` and `attribute_value`; `item_registry`'s
  `candidates_at` and census; `prepare_stock_shield`'s name rule, restated over the registry's lists.
- **Rules from the note:** the relic weight hook (a part with slot upgrades, none naming a slot the relic defines, weighs 0;
  only where the definition sets `bAllowInventoryDefToModifyPartWeight`, true by class default for `ArtifactDefinition` only);
  item rarity as the truncated base plus the truncated rarity of each part.
- **Stand-ins where the note leaves the question open, each UNVERIFIED:** a class mod takes one uniform pick among its
  balance's `ClassModDefinitions`; `EPRM_Complete` is the registry's merge (the note's reading (b)); name parts count in the
  rarity sum; the manufacturer is drawn in the roll, weighted by the grade's spawn modifier, although the game draws it in the
  pool roll.
- **Census seeds are spread over the 32-bit range**, not 1 to N: the C runtime generator's first draw grows slowly with small
  seeds and would favour each list's first candidate.
- **Verified (internal consistency only):** 35,660 rolls over the install at six stages, 0 checker failures; 12 synthetic cases.
  `prepare_stock_shield.py`'s balance, level 8 and seed 1 give the same parts and name parts as that tool's card, which is
  unchanged.
- **UNVERIFIED:** every rule and stand-in above in the running game. A live check is spawned drops of one shield, one class mod
  and one relic balance, compared on slot frequencies and rarity levels.
