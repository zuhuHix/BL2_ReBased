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
  stack with PreAdd 0.1 and Scale -0.1. The negative scale goes to the divisor; the combine takes the divisor's
  reciprocal and multiplies, each step rounded to single precision, giving bits `0x3e3a2e8b`. A direct
  single-precision division gives `0x3e3a2e8c`, and so does double precision rounded once, and the test asserts all
  three, so it pins both the precision and the order of the operations. (The first fixture, PreAdd 0.2 and Scale
  -0.3, did not pin the order: a direct division gives the same bits there; review of #446.) Which form the game uses
  is still architecture 5.3's open question (UNVERIFIED).
- **(c) Every output checked first.** `RecomputeNormalizedDamageValues` settles everything that can refuse before its
  first write: each stack's modifiers, each base, and then all six outputs (the two copied stacks, the two bases, the
  two values) against their declarations, with the value each will hold. A class missing one, or declaring one with
  the wrong kind (an int `NormalizedMeleeDamage`, review of #446), is a data error that changes none of the others.
  Two tests cover it with toy weapon classes. The order (review r2 of #446): the six output addresses first, then
  every stack's modifiers, then the bases (the only step that runs a resolver), then the staged values, then the
  writes. So a class missing an output is refused before any resolver runs; a test shows a state-changing base
  resolver never runs there. A bare None in `WeaponTypeDefinition` is an absent definition, so it gives the
  missing-definition `NotYet` as before; a test covers that too.
- **(d) One implementation each: withdrawn (review of #446).** The first version of this lane replaced the module's
  stack reader with `ow_attr::storage::stack_value` and its type lookup with a shared helper. The review found that
  this was not behaviour-neutral: the modifiers were then read after the base was evaluated, and the shared
  conversions accepted less (an interface value, a bare None, a None interface element). Both are back as they were
  in decision 2026-11-19-45, and `stack_value` is crate-private again. Two new tests pin the order: a base resolver
  that changes a modifier does not change the recompute (the modifiers are read first), and a malformed stack is
  reported before an unread base.
- **UNVERIFIED.** As decision 2026-11-19-45: nothing here was checked in the running game.
