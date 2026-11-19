## 2026-11-19: ApplyInternalSlotEffectModifiers in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-APPLY-SLOT-EFFECT-MODIFIERS, slice link 8. Coded from
docs/verification/NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md alone, the note's final text on main (a3c8e22, content
d38aa0f; clean room: lane ANALYST-APPLY-SLOT-EFFECT-MODIFIERS wrote it, decision 2026-11-19-87, PR #471). The
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
  - B3: no modifier is made for a slot whose context did not resolve. One whose add then answers false still gets
    its object, which stays where the chain left it: not recorded, and not rolled back. Its outer is None, because
    the port has no transient package.
- **Preflight, P0 to P9, before any write.**
  - P1 to P4 are shapes: the out argument's record struct, the five slot fields (`declared_slots`, the shape check of
    decision 2026-11-19-49 with a field list), `AppliedAttributeSlotEffects`, and the modifier class.
  - P5 to P7 are values: the count, each slot's five values (a `ModifierType` outside the enum is a data error), and
    each record's three objects. With the flag false, a record is a removal only when its context, attribute and
    modifier are all set; one with a None modifier is a no-op, neither checked nor run.
  - P0, under the note's ruling that definition loads are not writes: every definition the call will touch is loaded.
    That is each qualifying slot's attribute and each removal's attribute, with the entries of their two resolver
    chains. A load's error comes back unchanged. P0 runs after the reads that name what it loads, and before P8.
  - P8: each qualifying slot's context, from the item.
  - P9: the chain check. `AttributeSystem::check_modifier_chain` is new and writes nothing. It checks every entry of
    every planned removal's and add's value-resolver chain, whether or not the operation will reach it:
    - the entry's resolver class must be known, and its write-free storage probe must answer. The probe is
      `ValueResolver::probe_storage`, new in review r1 of #483. It defaults to the storage lookup, which writes nothing
      for every class but the noun resolver;
    - a None entry ends the check;
    - a "not writable" lookup does not end it.
    It predicts no outcome. W1 and W3 run every planned operation and take its bool as it comes.
- **P0 on the installed data.** P0 is needed there: every weapon's slot attribute holds its
  `WeaponAttributeContextResolver` as an unloaded store handle.
- **Writes, W1 to W3.** A `NotYet` after a write, for instance from a change notification's script, is terminal. It is
  matched by its typed variant, keeps its kind and record, and gets "stopped after partial writes: " on its text through
  `ow_world::pools::after_partial_writes`. Nothing is rolled back.
- **`ow-attr`.** Three public additions:
  - `check_modifier_chain`, which writes nothing;
  - the `ValueResolver::probe_storage` trait method;
  - `new_modifier`, the modifier constructor the apply natives use.
- **Tests.** Nine tests in `equip/internal_slots_tests.rs`:
  - the flag true: records, backup list, values;
  - the flag false after true: removed then reapplied, not doubled;
  - true twice: doubled, as the note's correction says;
  - the count: 0, and past the dimension;
  - an unresolvable context refused with nothing written;
  - every chain entry checked: a writable entry, then an unread one, is refused before any write;
  - a None entry ending the check, with a false add and nothing recorded;
  - a backup record without a modifier, as a no-op;
  - Reference.
- **Installed runs, Off.**
  - `--slice-gate`, strict: 0 ok / 13 not yet / 1 error (rerun on the final note's code). Link 8 passes this native and stops later in `init`, at the
    unbound native `WillowWeapon.CacheWeaponCard()`. That is the note's section 4 prediction off the name-part branch.
  - `--equip-run` on the 50 weapons: 49 stop at `CacheWeaponCard`, and the Jakobs sniper rifle stops earlier, as
    before (the note's open question 4).
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note: per-slot context resolution
  against B2 (open question 1), a count above 19 (2), the stale out records (3), and `BulletSpeed` (4).

**Review r1 of #483.**
- **P9 writes nothing.** In Off the noun resolver's storage lookup creates a missing `NounState` entry, so the chain
  check now goes through a write-free probe, `ValueResolver::probe_storage`.
  - Noun's probe runs the same strategy checks.
  - For a missing entry it evaluates only `DefaultValue` and appends nothing. A new entry would have no replicated
    slot, so it passes.
  - For an existing entry it gives the replicated-slot refusal, `NotYet` (`NotPorted`), that a write to the entry would
    give. A planned add to such an entry is therefore refused in the preflight, and so is a planned removal: the
    conservative answer, because a removal of a modifier that is not on the stack writes nothing in the game. That
    case is UNVERIFIED.
- **Off assumption, UNVERIFIED: a `ModifierType` outside the enum.** It is a data error only on a qualifying slot. The
  note does not say what the native makes of one, and `ow-attr`'s stack treats such a modifier as a no-op.
- **The partial-writes mark** (r1, corrected in r2): every W1 failure is marked, the first removal's included.
- **Tests:** eighteen, twelve of them new:
  - a post-write `NotYet`, prefixed with kind and record kept and the earlier writes left;
  - every entry of two removals naming one modifier checked;
  - a not-writable entry not ending the check;
  - a false add after an accepting entry, the object left (B3);
  - the type check on qualifying slots only;
  - P1 and P3 shapes;
  - the noun case (no entry created, unprefixed `NotYet`);
  - P0 on a synthetic store (an unloaded attribute and its chain entry loaded; a failed load returned unchanged);
  - two more in `ow-attr`, for the noun probe (no entry created; a replicated slot refused).

**Review r2 of #483.**
- **The partial-writes mark on W1.** Every W1 failure is marked again, the first removal's included. A removal writes
  the stack and the value before its change notification runs, so its `NotYet` can follow a write. The refusals that
  come before any write were already decided by P9. r1 had left the first removal's refusal unmarked, which was wrong.
- **Off assumption, UNVERIFIED: `DefaultValue` writes nothing.** The noun probe assumes that evaluating
  `DefaultValue` writes nothing. Data whose `DefaultValue` names a `BaseValueAttribute` or an `InitializationDefinition`
  could reach another noun attribute on the same context and create its entry. No data known to the port does this.
- **The probe's default.** `ValueResolver::probe_storage`'s doc now says when an implementor must override it.
- **Tests:**
  - the first removal on a notifying property whose notification is a `NotYet`: prefixed, kind and record kept, the
    stack change kept;
  - the noun case names `Test.UnreadValueResolver`, and its control (no slot 1) creates the entry at W3;
  - the failed load matches the store's typed decode error.
