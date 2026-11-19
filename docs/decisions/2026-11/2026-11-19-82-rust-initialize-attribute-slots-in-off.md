## 2026-11-19: InitializeAttributeSlots in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-INIT-ATTRIBUTE-SLOTS, slice link 8. Coded from
docs/verification/NATIVE_INIT_ATTRIBUTE_SLOTS.md alone (clean room: lane ANALYST-INIT-ATTRIBUTE-SLOTS wrote the note,
decision 2026-11-19-73, PR #460, the note's text at 9fa4c4d3). The executable and the analysis output were not
opened. Synthetic fixtures only.
The field kinds of the three slot structs and of `TempStatModifier` were checked against the installed `Engine.upk`
export table (names and property classes only).

- **What is bound (`ow-inventory` `equip/slots.rs`, Off only).** `WillowInventory.InitializeAttributeSlots(bool)` and
  `WillowInventory.InitializeAttributeSlotsForNameParts()`, as the note's sections 2 to 5 give them:
  - the lists: a weapon's `WeaponTypeDefinition` and nine parts, an item's `ItemDefinition` and nine parts, the name
    parts prefix then title; another class's lists are not read (a named `NotYet`);
  - the slot build: the definition's upgrades, then each part's; a record found in the working set, else copied from
    first matching `AttributeSlotEffects` entry of the definition, then the parts. Each activation adds the base
    grade, rounded as the note's section 3 says: y = 2x + 0.5 in single precision, then the nearest int with ties to
    even, then a shift right by one. Every found entry adds its `GradeIncrease` in 32-bit integers, and an unknown
    slot is ignored;
  - the write: the clamped `AttributeSlotMaxActivated`, never past the declared dimension. Activated slots are kept
    by name, and new activated records are copied to index `K` (the distinct kept names) onwards in first-found order.
    Each copy is clamped, the maximum first and then the minimum;
  - the name-part pass, when the bool is true or as the second native: the item's activated slots grow in place,
    unclamped; other slots are looked up in the definition's and the parts' effects only;
  - the value pass: on the authority, `ComputedModifierValue = BaseModifierValue + (EffectGrade - 1) × PerGradeUpgrade`
    as a float. It is copied to `ReplicatedAttributeSlotModifierValues`, `bNetDirty` is set, and `TempStatModifier`'s
    value and base take the grade with its stack emptied. Both values are evaluated with the item as the context. An
    external run-as-skill slot in `AttributeSlotEffectMode` 2 makes no skill and is not refused. Off the authority,
    each slot takes its replicated value.
  The slot array is decision 2026-11-19-49's static array, read and written element by element.
- **The named `NotYet`s** (each `NotRead`, each text starting with the note's identifier): `IAS-NO-DEFINITION`,
  `IAS-SLOT-SHAPE` (the item's activated slots are not a packed, unique prefix, refused whether or not a copy
  follows), `IAS-BASE-GRADE-CONTEXT`, `IAS-ROUNDING-RANGE` (a rounding outside the native's exact conversion, -2^31
  included, or a grade sum that would wrap), `IAS-VALUE-CONTEXT`, and `IAS-RUNTIME-SKILL` (a mode other than 2). A class that is neither a weapon nor an item is this lane's own `NotYet`.
- **Before any write.** Every step runs on copies in memory, in the note's planning order (section 8), so every
  `NotYet` and data error is decided before the first write. The plan records its writes in the native's order and
  then replays them, so `InitializeAttributeSlots(true)` keeps its two value passes as two write passes. No planned
  `NotYet` follows a write. One that still surfaced after a write would stop the call marked "stopped after partial
  writes", its kind kept and the writes left in place. Reference binds nothing (R3).
- **Tests.** `equip/slots_tests.rs`: eleven tests on an invented class set (a slot array of 4). They cover the build
  order, the rounding (2.5, -0.5, -1.5, -2.5, 0.49999997, -2^31 exactly, and the range), a wrapping grade sum, the clamps, the limit (1, 0 and 25 against a
  dimension of 4), kept slots, the name-part pass both ways, an item, each named `NotYet` with nothing written
  (three slot shapes), mode 2, the skill part off the authority, and Reference.
- **Installed runs, Off.**
  - `--slice-gate`, strict and past-errors: still 0 ok / 13 not yet / 1 error. Link 8 (`equip_gun`, driver steps make,
    init, bases, equip, unequip) still stops in `init`, the second of five steps. The stop moved from the unbound
    `InitializeAttributeSlots(bool)` to the next unbound native, `WillowInventory.ApplyInternalSlotEffectModifiers`.
    The other 13 rows are byte-identical.
  - `--equip-run` on the 50 weapons of `local/p2/fix-allparts/weapons.txt`. Init: all 50 stopped at
    `InitializeAttributeSlots` before; now 49 stop at `ApplyInternalSlotEffectModifiers`, and one, a Jakobs sniper
    rifle, stops at an `ObjectPropertyAttributeValueResolver` `NotYet` (no `BulletSpeed` property on the weapon
    context), later in its `InitializeInternal`. Bases 50/50, equip 49/50 and unequip 50/50 are unchanged.
- **Follow-up, not done here.** `crate::equip::external_slots` (the Rust equip path, not the script's) still refuses
  an external slot with a `NotYet` that names `InitializeAttributeSlots` as unread. Whether it should now read the
  computed slots is a question for the equip path's owner.
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note: the evaluation context
  (open question 1), the runtime skill (2), the first-found order (3), and the unclamped in-place growth (4).

**Review r2 of #468.**
- **Every write is checked before the first.** The planned writes are resolved to property addresses and checked
  with the heap's own checks (the declaration, the address, the value's kind) before any is made. A `bNetDirty`
  declared as an int now refuses the call with the item unchanged.
- **Inputs are read as each pass needs them.** The definition and the slot limit come first, then the slot shape.
  The parts load at the base pass, the name parts at the name-part pass, and the `Role`, the replicated values and
  the value pass's write destinations at the value pass. A prefix's load can no longer block
  `InitializeAttributeSlots(false)` or mask IAS-SLOT-SHAPE.
- **In-place grades.** The name-part pass queues one write per applied upgrade, in upgrade order, and no longer
  rewrites a kept slot that has no upgrade.
- **Hub ruling: `EffectGrade - 1` that would wrap** is the note's IAS-ROUNDING-RANGE port restriction ("a grade sum
  that would wrap"). The plan refuses it with that `NotYet`, before any write; the value pass never fails on it.
- **Typed errors.** The class rule asks `find_class_if_present`, so a lookup error passes as itself, and the loader
  converts definition errors through `VmError::from`, so a store `NotYet` stays a `VmError::NotYet`.
- **Tests:** six added. They cover the replication boundary (an inactive slot below the limit, slots past it), the
  int `bNetDirty`, the refusal order, the in-place order, the wrapping step, and the existing cases.
