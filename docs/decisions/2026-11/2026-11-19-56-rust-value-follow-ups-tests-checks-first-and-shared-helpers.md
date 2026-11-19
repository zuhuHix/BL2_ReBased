## 2026-11-19: Value follow-ups (review of #428): two pinning tests, outputs checked first, shared helpers

AI-assisted (Claude Code, Claude Opus 5.5). Lane FIX-VALUE-C, the LOW follow-ups of #428 (FIX-VALUE-B, decision
2026-11-19-45), on `ow-inventory`'s value natives (NATIVE_INVENTORY_VALUE.md). No new rule. Synthetic fixtures only.

- **(a) The money value reads the new total.** A new test makes the definition's `MonetaryValue` read the inventory's
  `MonetaryValueModifierTotal` through an attribute. `ComputeValueOfParts` writes the total (2 x 1.5 x 3 = 9) and then
  evaluates the money value with the inventory as context, so it gives 900, where the old total of 1.0 would give 100.
  The attribute's value resolver is a toy that reads the property directly. How the game's attribute
  (`D_Attributes.Inventory.InventoryPartMonetaryValueModifierTotal`) reaches the property is still not read
  (UNVERIFIED), so the test pins the order of the two steps, not a resolver rule.
- **(b) A negative scale in single precision.** A new test feeds `RecomputeNormalizedDamageValues` the base 0.1 and a
  stack with PreAdd 0.2 and Scale -0.3. The negative scale goes to the divisor, and each step rounds to single
  precision, giving 0.23076925 (bits `0x3e6c4ec6`). Double precision rounded once would give 0.23076923, and the test
  checks that the two differ, so it pins the form. Which form the game uses is still architecture 5.3's open question
  (UNVERIFIED).
- **(c) Every output looked up first.** `RecomputeNormalizedDamageValues` checks all six output properties (the two
  copied stacks, the two bases, the two values) before its first write. A class missing one is a data error that
  changes none of the others. A new test covers it with a toy weapon class that lacks `NormalizedMeleeDamage`.
- **(d) One implementation each.**
  - The normalized value is now `ow-attr`'s own stack formula: `ow_attr::storage::stack_value`, made public for this,
    with `NumKind::Float` and the VM's let form. It replaces the module's local stack reader. The rules are unchanged:
    a None element and a modifier type outside the three are skipped, and a non-object element is an error.
  - The weapon's type definition comes from a new shared `equip::weapon_type_definition`, which `weapon_type_of` now
    uses too. Each caller keeps its own `NotYet` text, so `RecomputeNormalizedDamageValues` still names its own
    question.
- **UNVERIFIED.** As decision 2026-11-19-45: nothing here was checked in the running game.
