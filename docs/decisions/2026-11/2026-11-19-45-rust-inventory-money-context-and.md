## 2026-11-19: Rust inventory money context and RecomputeNormalizedDamageValues in Off

AI-assisted (Claude Code). Lane FIX-VALUE-B, implementer (slice link 8, equip). Number 45 was given by the hub. The
branch is `origin/main` df99b25e. It was written stacked on `fix/rust-compute-value` 3c25d81c (decision 2026-11-19-36),
whose module it extends, and rebased once that landed (#425, 174d92bb). The source is NATIVE_INVENTORY_VALUE.md as on main (#419, decision 2026-11-19-34), sections 3 and
5. This lane did not analyse it and opened no `local/analysis/`, Ghidra or REA output. No dependency, no CLI flag, no
parsing change. There was no real-game session; every rule is **UNVERIFIED** in the running game. Both changes are Off
only. Reference has no C++ stand-in for either and is unchanged (R3).

- **The money step's context** (`ow-inventory` `definition`, lane FIX-POOL-SPAWN-B's module, decision 2026-11-19-29).
  `ComputeMonetaryValue`'s body (`monetary_value_of`, shared with `ComputeValueOfParts`) answered `NotYet { NotRead }`
  for any `MonetaryValue` that reads an attribute or an initialization definition, because the evaluation context was
  NATIVE_LOOT_SPAWN.md's open question 2. NATIVE_INVENTORY_VALUE.md section 3 answers it: the inventory itself. The
  refusal is gone; the body now evaluates through the existing attribute-init evaluator with the inventory as source.
  Whatever the evaluator cannot do still answers its own `NotYet` (an unregistered resolver class, an unread resolver
  rule). The truncation, the currency caps and the other refusals are unchanged.
- **`WillowWeapon.RecomputeNormalizedDamageValues()`** (new, `ow-inventory` `equip::value`; section 5). Instant-hit,
  then melee:
  - the normalized stack (`NormalizedInstantHitDamageModifierStack`, `NormalizedMeleeDamageModifierStack`) becomes a
    copy of the weapon's (`InstantHitDamageModifierStack`, `MeleeDamageModifierStack`);
  - the base (`...BaseValue`) is the weapon type definition's init value (`NormalizedInstantHitDamage`,
    `NormalizedMeleeDamage`) evaluated with the weapon as context;
  - the value (`NormalizedInstantHitDamage`, `NormalizedMeleeDamage`) is that base with the copied stack applied by the
    stack formula, as a float, nothing rounded.
  Both values are computed before anything is written. Three points the note does not settle, each labelled in the
  module:
  - the combine uses the single-precision form of this VM's attribute let (`let_forms`); which form the game uses is
    architecture 5.3's open question (UNVERIFIED);
  - the writes are plain property writes with no attribute-change notification (the note names none; UNVERIFIED);
  - a weapon without a `WeaponTypeDefinition` is `NotYet { NotRead }`.
  Modifiers are read as `ow-attr` reads a stack: a None element and a type outside the three are skipped.
- **Verified (automated, local, pc-hummer).**
  - **Synthetic tests** (`ow-inventory`, invented classes and two toy resolvers, `Test.Same` for the context and a value
    resolver that answers only on the expected context):
    - `definition`: a `MonetaryValue` whose base attribute resolves on the inventory gives 30 x 2 = 60, on a weapon and
      on an item; with an empty context chain the constant stays the base (5 x 2 = 10); an attribute whose context
      resolver class is registered nowhere is the evaluator's `NotYet`, naming that class (it replaces the
      open-question-2 assertion);
    - `equip::value`: instant-hit (40 + 2) x 1.5 = 63 from an attribute only a weapon context answers, a None element
      and a type-7 modifier skipped; melee 10 + 3 = 13; both stacks copied; a weapon without a type definition is
      `NotYet` and writes nothing; an unread melee attribute refuses after the instant-hit value is computed and before
      anything is written; Reference binds none of the three natives.
  - **Installed `--inventory-move --equip-run`** (the 50 weapons of decision 2026-11-19-36, Off), on 3c25d81c (before)
    and this branch (after):
    - init: before, 50 of 50 at the money step's open-question-2 refusal; after, 50 of 50 past it, at
      `InventoryAttributeContextResolver`, whose equipped-only flag property is not read (NATIVE_ATTRIBUTES.md
      section 5). That is the ow-attr `NotYet` the previous lane predicted, inside the money evaluation. So no
      installed weapon reaches `RecomputeNormalizedDamageValues` yet; only the synthetic tests exercise it;
    - bases, equip and unequip rows: identical before and after (bases 50 pass; equip 49 at `AttributeSlots: expected
      an array, found Struct`, 1 at the game info query; unequip 50 pass; 3 to 7 records after equip, 0 after unequip).
  - **`--slice-gate`**, strict: byte-identical before and after, 0 / 11 / 3. Link 8 has no driver.
  - **Cards** (NATIVE_WEAPON_RULES.md section 6): no comparison path. No installed weapon gets a `MonetaryValue` in
    the port yet (the money step stops at the resolver above), and the 69 golden cards were matched by a separate
    evaluator, not by this code.
- **Gates** (this branch on 3c25d81c): workspace build clean; workspace tests 2,010 passed / 0 failed / 45 ignored
  (`ow-inventory` 65 / 0 / 0); `crate_edges_test.py` OK; decisions check OK against the merge base 2981b801 (2 added:
  this one and decision 36); leak gate 0 errors, 0 warnings. ctest and verify_packages were not rerun (no C++ change).
- **Follow-ups from the review of #425** (FIX-COMPUTE-VALUE, decision 36), in its module:
  - `ComputeValueOfParts`'s rarity takes the layout's `mission_rule` flag: an item checks for a
    `MissionItemDefinition`, and a weapon skips the check entirely, as the note reads (before, a weapon's definition
    was class-checked too, which changed nothing for a real weapon type);
  - new passing tests: a mission item with an item part and a title name part (rarity 9 each) takes the globals
    definition's `MissionItemRarityLevel` (3), then 0 when the toy globals give no definition, its modifier total and
    money still by the rule; a weapon whose toy definition is a mission item definition sums its rarity (2 + 3 = 5)
    and asks for no globals. The globals come from toy `WorldHooks` and a toy native in place of the installed
    `GetGlobalsDefinition`.
  - Afterwards: `ow-inventory` 67 / 0 / 0; workspace 2,012 / 0 / 45; `crate_edges_test.py` OK; the installed equip
    run's rows byte-identical to the run above.
- **After the rebase onto df99b25e:** workspace tests 2,020 passed / 0 failed / 45 ignored; `crate_edges_test.py` OK;
  decisions check OK against origin/main (1 added); the installed equip run's rows byte-identical; `--slice-gate`
  strict 0 / 11 / 3, where only the `maya:8` stop text of links 1, 3, 4, 5, 13 and 14 changed, which comes from main's
  FIX-PLAYER-CLASS (now at `FindAlreadyLoadedObject` of the Siren pawn data), not from this lane.
- **Open.**
  - `InventoryAttributeContextResolver`'s flag property (an analyst's question, NATIVE_ATTRIBUTES.md section 5): the
    stop of the money step and so of the spawn.
  - The combine form and the notification of `RecomputeNormalizedDamageValues`, and its behaviour without a type
    definition.
  - The note's open questions: callers outside the spawn; `ChooseRandomNameParts` and `CacheWeaponCard`.
