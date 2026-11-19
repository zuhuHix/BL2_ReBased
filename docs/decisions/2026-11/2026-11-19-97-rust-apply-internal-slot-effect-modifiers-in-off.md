## 2026-11-19: ApplyInternalSlotEffectModifiers in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-APPLY-SLOT-EFFECT-MODIFIERS, slice link 8. Coded from
docs/verification/NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md alone, the note's text at 4301807 (clean room: lane
ANALYST-APPLY-SLOT-EFFECT-MODIFIERS wrote it, decision 2026-11-19-87, PR #471; its round-1 fix is pending). The
executable and the analysis output were not opened. Synthetic fixtures only. Stacked on #468 (decision 2026-11-19-82).

- **What is bound (`ow-inventory` `equip/internal_slots.rs`, Off only).**
  `WillowInventory.ApplyInternalSlotEffectModifiers(bool,int,array)`, as the note's sections 1 and 3 give it:
  - with the flag false, the recorded slot effects are removed, notifications on;
  - `AppliedAttributeSlotEffects` is emptied, always;
  - the first `MaxSlotsActivated` slots are visited. A slot that is activated, internal and names an attribute gets a
    new modifier of its stored type and value, added to the context its attribute resolves from the item.
    `bRunEffectsAsSkill` is not read;
  - a true add appends a record to the out array, after what it held, and, with the flag true, to the backup list.
- **The note's Off assumptions, each UNVERIFIED.**
  - B1: the count stops at the declared dimension.
  - B2: contexts are resolved in the preflight.
  - B3: a modifier is made only for a slot whose context resolved. Its outer is None, because the port has no
    transient package.
- **Preflight, P1 to P9, before any write.**
  - P1 to P4 are shapes: the out argument's record struct, the five slot fields (`declared_slots`, the shape check of
    decision 2026-11-19-49 with a field list), `AppliedAttributeSlotEffects`, and the modifier class.
  - P5 to P7 are values: the count, each slot's five values (a `ModifierType` outside the enum is a data error), and
    each record's three objects.
  - P8: each qualifying slot's context, from the item.
  - P9: for each removal and each add, a new read-only `ow-attr` test, `AttributeSystem::modifier_storages`. It is the
    chain walk of the add and the remove up to the point where they touch a stack, which the note asks the
    implementer to add.
- **The lazy-load ruling.**
  - P8 and P9 read only loaded definitions. After the cheap checks P1 to P7, an explicit pre-plan load step loads the
    attributes the plan will use and the entries of their two resolver chains. One still unloaded after it is a named
    `NotYet`.
  - The step is needed on the installed data. Without it, every weapon's slot attribute holds its
    `WeaponAttributeContextResolver` as an unloaded store handle, and the plan refused.
- **Writes, W1 to W3.** A `NotYet` after a write is terminal and gets "stopped after partial writes: " through
  `ow_world::pools::after_partial_writes`; nothing is rolled back.
- **`ow-attr`.** Two public additions: `modifier_storages` (read-only) and `new_modifier` (the modifier constructor
  the apply natives use).
- **Tests.** Six tests in `equip/internal_slots_tests.rs`:
  - the flag true: records, backup list, values;
  - the flag false after true: removed then reapplied, not doubled;
  - true twice: doubled, as the note's correction says;
  - the count: 0, and past the dimension;
  - an unresolvable context refused with nothing written;
  - Reference.
- **Installed runs, Off.**
  - `--slice-gate`, strict: 0 ok / 13 not yet / 1 error. Link 8 passes this native and stops later in `init`, at the
    unbound native `WillowWeapon.CacheWeaponCard()`. That is the note's section 4 prediction off the name-part branch.
  - `--equip-run` on the 50 weapons: 49 stop at `CacheWeaponCard`, and the Jakobs sniper rifle stops earlier, as
    before (the note's open question 4).
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note: per-slot context resolution
  against B2 (open question 1), a count above 19 (2), the stale out records (3), and `BulletSpeed` (4).
