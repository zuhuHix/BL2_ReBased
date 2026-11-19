## 2026-11-19: Read the internal slot effect native as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-APPLY-SLOT-EFFECT-MODIFIERS, analyst only. With `InitializeAttributeSlots` bound (decision
2026-11-19-82), slice link 8 stops in `init` at `WillowInventory.ApplyInternalSlotEffectModifiers`. The installed
`--equip-run` stops there for 49 of 50 weapons.

**What was added.** An own-words note, `docs/verification/NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md`, plus one
`NATIVE_INDEX.md` row. The note records, as behaviour only:

- **The order.**
  1. With the flag false, the item's `AppliedAttributeSlotEffects` records are removed from their contexts.
  2. The list is emptied, always: with the flag true, its modifiers are forgotten without being removed.
  3. Each of the first `MaxSlotsActivated` slots that is activated, not external and names an attribute gets a new
     modifier with the slot's stored type and value. Its context is resolved from the item, and the modifier is
     added there.
  4. A successful add appends a record to the caller's out array, which is not emptied first. With the flag true it
     is also appended to the backup list.
  - The native refuses nothing and does not clamp the count. `bRunEffectsAsSkill` is not looked at.
- **A correction to NATIVE_ATTRIBUTES.md section 9:** the emptying of the backup list does not depend on the flag.
- **The two installed spawn calls.** The first passes true. The second, only on the name-part branch, passes false.
- **An Off answer list** with three labelled Off assumptions, all UNVERIFIED:
  - a count above the slot array's dimension stops at the dimension, as decision 2026-11-19-49 does;
  - contexts are resolved before any write;
  - skip allocation for unresolved contexts. After a false add the object stays where the chain left it: an earlier
    resolver may hold it.
  The preflight covers:
  - the definitions the call touches, loaded first (loads are not writes, an Off ruling);
  - the shape checks and the kinds of the stored values;
  - the context resolution;
  - a chain check of every value-resolver entry of every planned remove and add, which predicts no outcome.
  A backup record with no modifier is a no-op. A `NotYet` after a write is terminal: matched by its type, kind and
  record kept, with the "stopped after partial writes" prefix.
- **What link 8's `init` reaches next.**
  - On the name-part branch: `ChooseRandomNameParts`, unbound.
  - Otherwise: `CacheWeaponCard`, unbound.
  - The one weapon that went further stops before this native, in `ApplyAllWeaponAttributeEffects`, on a `BulletSpeed`
    property the installed `WillowWeapon` does not declare.

**How it was read.** The Ghidra run was read-only and took the machine lock. The structures were checked against the
installed layouts with `tools/ghidra/class_layout.py`. Raw output stays under ignored
`local/analysis/apply-slot-effect-modifiers/`. The note passed the clean-room scan.

**Status.** Nothing is implemented: the native stays unbound in Off until a fresh lane codes it from the note. Every
rule is UNVERIFIED in the running game. No bl-sdk evidence on this native was on record. The note lists the checks;
no session was run.

**Edits: review r1 of #471.**
- Definition loads are ruled outside the write rule and done first (P0).
- P9 checks every chain entry of every planned operation with the read-only storage lookup, without predicting
  outcomes; it also checks an add before its modifier exists.
- A None-`Modifier` backup record is a no-op.
- The post-write rule matches the typed `NotYet` and keeps its kind and record.
- B3 is renamed "skip allocation for unresolved contexts" and says where a false add's object stays.
- The live checks use a trace of notification events on notify-enabled properties, and a fixture that would tell
  entry-time from per-slot context resolution. Where no hook can tell them apart, B2 stays UNVERIFIED.
