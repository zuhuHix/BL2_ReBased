# Native rules of an item's attribute slots: WillowInventory.InitializeAttributeSlots (draft, 2026-11-19)

AI-assisted (Claude), analyst lane ANALYST-INIT-ATTRIBUTE-SLOTS (decision 2026-11-19-73). Method:
- the script that calls the natives was read from the installed packages with `research/script_disasm.py`;
- the field and struct layouts were read with `tools/ghidra/class_layout.py`;
- the natives, their helpers and the item classes' virtual methods were read from the game executable with Ghidra
  (`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock).

Written in our own words, as behaviour only: no decompiler output, pseudo-code, quoted script statements, addresses,
offsets or call structure is reproduced here. Field names come from the cooked packages. **Every rule is UNVERIFIED in
the running game.** The last section lists the live observations that would confirm them.

**Clean room.** This lane implements nothing. Everything here stays unbound or `NotYet` in Off until a fresh
implementer codes it from this note alone.

**Why it matters.** Slice link 8 (equip the gun) now stops next at the unbound `WillowInventory.InitializeAttributeSlots(bool)`
in the weapon's `InitializeInternal` (H6's lane, decision 2026-11-19-67).

This note extends [NATIVE_ATTRIBUTES.md](NATIVE_ATTRIBUTES.md) section 9, which gave the slot record, the grade
accumulation and the max-then-min clamp at medium confidence. All of that is confirmed here, and the parts that note left
open are now read: where a slot's data comes from, the slot limit, what an already activated slot does, and the
`ComputedModifierValue` computation. The 19-slot static array is the shape decision 2026-11-19-49 settled
(`AttributeSlots[19]`, read element by element as the class declares it).

## Summary

| Piece | Kind | Confidence | Status |
|---|---|---|---|
| `WillowInventory.InitializeAttributeSlots(bool bIncludeNameParts)` | native | high | UNVERIFIED |
| `WillowInventory.InitializeAttributeSlotsForNameParts()` | native | high | UNVERIFIED |
| the item's definition, part list and name-part list (virtual methods of `WillowWeapon` and `WillowItem`) | native | high | UNVERIFIED |
| the slot value pass, shared by both classes | native | high, medium for its skill part | UNVERIFIED |

## 1. The callers and the bool

`WillowWeapon.InitializeInternal` and `WillowItem.InitializeInternal` (script) call `InitializeAttributeSlots` once,
after the parts are chosen and `ComputeValueOfParts` has run:
- the weapon passes `true` exactly when `bSelectRandomPartsOnInitialization` is false;
- the item passes `true` exactly when neither `bSelectRandomPartsOnInitialization` nor its `bForceSelectNameParts`
  argument is set.

So the bool means **the name parts are already set**: an item built from given parts (a save, a fixed definition)
includes its name parts in this one call. When the parts are rolled, the script picks the name parts later and then
calls `InitializeAttributeSlotsForNameParts` on its own (NATIVE_INVENTORY_VALUE.md section 5 lists that order).

## 2. The item's three lists (virtual methods, per class)

- **The definition:** on a `WillowWeapon`, `DefinitionData.WeaponTypeDefinition`; on a `WillowItem`,
  `DefinitionData.ItemDefinition`. The script native `WillowInventory.GetInventoryDefinition()` answers the same
  virtual method, so this is also its rule.
- **The part list:** the set part slots of `DefinitionData`, in declaration order, empty slots left out, and no name
  parts:
  - weapon: Body, Grip, Barrel, Sight, Stock, Elemental, Accessory1, Accessory2, Material;
  - item: Alpha, Beta, Gamma, Delta, Epsilon, Zeta, Eta, Theta, Material.
- **The name-part list:** the set name-part slots, prefix then title (`PrefixPartDefinition` and `TitlePartDefinition`
  on a weapon, `PrefixItemNamePartDefinition` and `TitleItemNamePartDefinition` on an item).

## 3. InitializeAttributeSlots(bIncludeNameParts)

The item's own `AttributeSlots` are not cleared first. In order:

1. **A working set of slot records,** keyed by slot name and empty at the start.
2. **The definition's upgrades.** For each entry of the definition's `AttributeSlotUpgrades`, in order, and then the
   same for each part of the part list in list order, the entry's slot record is found (rule below). An entry whose slot
   is found is applied:
   - when the record is not yet activated and the entry has `bActivateSlot`, the record becomes activated and its
     `EffectGrade` grows by the definition's `AttributeSlotBaseGrade`, evaluated (the item as the context, no second
     context; section 5) and rounded half up as below;
   - in every case its `EffectGrade` then grows by the entry's `GradeIncrease`.

   **Types and the rounding.** `EffectGrade` (the slot) and `GradeIncrease` (the upgrade entry) are 32-bit ints, and
   both additions are integer additions. `AttributeSlotBaseGrade` is an `AttributeInitializationData` whose evaluation
   gives a float `x`. The rounding is three steps:
   1. `y` = 2x + 0.5, rounded to single precision (to nearest, ties to even);
   2. `n` = `y` converted to a 32-bit int, to nearest with ties to even (the processor's default rounding mode, not
      set by the native; UNVERIFIED that the game never changes it);
   3. the grade grows by `n` shifted right by one, arithmetically (`floor(n / 2)`).

   Whenever 2x + 0.5 is exact in single precision this is `floor(x + 0.5)`: a half goes up, towards plus infinity, for
   negative values too (2.5 gives 3, -0.5 gives 0, -2.5 gives -2). Close below a half the single-precision step can
   round up (0.49999997 gives 1).

   **The conversion's range.** The native conversion is exact for every finite `y` from -2^31 to just below 2^31,
   both -2^31 itself (representable in single precision, it converts to the int32 minimum) and the largest single
   below 2^31 included. A `y` at or above 2^31, below -2^31, infinite or not a number (an `x` that is not finite)
   converts to the processor's "integer indefinite" value, which is the same bit pattern as -2^31. The native does not
   tell the two apart. The Off answer converts exactly over the native's exact range, -2^31 included, and refuses
   every other `y` (out of range or not finite) with `NotYet` IAS-ROUNDING-RANGE. Not reproducing the indefinite
   value is a port restriction, not a rule of the game. Both grade additions are 32-bit two's-complement additions,
   which wrap in the native; the port refuses a sum that would wrap with the same identifier (a port restriction too).
   The plan decides both (section 8).

   An entry whose slot is not found is ignored.
3. **Finding a slot record** for a slot name:
   - the working set's record of that name, if there is one;
   - otherwise a new record built from the first `AttributeSlotEffects` entry with that `SlotName` in the definition's
     list;
   - failing that, in the part list's parts' `AttributeSlotEffects` lists, the first part in list order that has one;
   - failing all of these, there is no record.

   The new record copies the effect entry, then joins the working set:
   - `SlotName`, `bExternalSlot`, `bRunEffectsAsSkill`, `bIncludeAlliesAsTarget`, `bEnforceMinimumGrade`,
     `bEnforceMaximumGrade`, `MinimumGrade`, `MaximumGrade`, `TargetInstanceDataName`, `AttributeToModify`,
     `ConstraintAttribute`, `ModifierType`, `BaseModifierValue` and `PerGradeUpgrade` are copied as they are;
   - the record's `bIncludeInModifierText` takes the entry's `bIncludeInFunStats`;
   - `bActivated` starts false, and `EffectGrade` and `ComputedModifierValue` start at 0.
4. **Writing the activated records into the item** (the same step closes section 4):
   1. The **slot limit** is the definition's `AttributeSlotMaxActivated`, taken as 19 when it is below 1 or above 19.
   2. Among the item's first slot-limit slots, those already `bActivated` are kept, by name, as a set: `K` is the
      number of **distinct** kept names, not the number of kept slots.
   3. Each activated working record whose name is not kept is copied whole into the item, in the working set's order.
      The first copy goes to index `K`, the next to `K + 1`, and so on, until the index reaches the slot limit.
      Activated records past the limit are dropped. Records that are not activated are never written. A written
      name joins the kept set (the working set holds each name once anyway).
   4. After each copy, `MaximumGrade` caps its `EffectGrade` first (when `bEnforceMaximumGrade`), then `MinimumGrade`
      raises it (when `bEnforceMinimumGrade`), so the minimum wins a conflict.
   5. The destination does not skip kept slots. When the item's activated slots fill indices 0 to `K - 1` with
      distinct names (a **packed, unique prefix**, the shape every earlier call of this native leaves), the kept slots
      are left untouched. Otherwise a copy can overwrite a kept slot:
      - index 2 activated and 0 to 1 empty: `K` is 1, so the first copy goes to index 1 and the second to index 2,
        over the kept slot;
      - the same active name at indices 0 and 1: `K` is 1, so the first copy goes to index 1, over the duplicate.

      That is the native as read (UNVERIFIED). The Off answer supports only the packed, unique prefix and refuses
      any other shape with `NotYet` IAS-SLOT-SHAPE (section 8).
5. **The name-part pass** (section 4), when the bool is true.
6. **The value pass** (section 5).

The working set's order is the order in which its records were first found, from the definition's upgrades and then
the parts'. That is the order of new records in the item (as read; UNVERIFIED).

## 4. InitializeAttributeSlotsForNameParts()

The same steps with two differences:
- the working set starts with the item's own activated slots among the first slot-limit ones, by name. A name part's
  upgrade to one of them changes that slot **in place**, at once, while the upgrades are applied: its `EffectGrade`
  grows, and the min and max clamps of step 4.4 are **not** applied again to it (as read; UNVERIFIED). When two of the
  item's activated slots share a name, the working set holds one of them; which one was not read (the Off answer
  refuses that shape, IAS-SLOT-SHAPE);
- only the name-part list's upgrades are applied, prefix then title. A slot a name part names but nobody has activated
  is still looked up in the definition's and the part list's `AttributeSlotEffects`, not in the name parts' own lists
  (medium confidence).

The activated records it makes are written as in section 3 step 4, then the value pass runs. When it is reached from
`InitializeAttributeSlots(true)`, the value pass runs twice: once at the end of this pass, and again at the end of
`InitializeAttributeSlots`. Both passes write, and the port keeps both as separate write passes. The second writes the
same values as the first **only for what Off plans**: context-free data, and a skill part that is absent or in mode 2.
Outside that, the two passes can differ in the native: a context-dependent evaluation is evaluated again (after the
first pass set `TempStatModifier`), and a supported skill mode makes a fresh runtime skill on each pass. Off refuses
those cases (IAS-VALUE-CONTEXT, IAS-RUNTIME-SKILL), so no equivalence is claimed for them.

## 5. The value pass

For each of the item's first slot-limit slots, in index order, one slot finished before the next:
- **On the authority** (the item's `Role` is `ROLE_Authority`), each activated slot, in this order:
  1. the item's `TempStatModifier` stack attribute is set to the slot's `EffectGrade` converted to a float: its base
     and its value both take it, and its modifier stack is emptied;
  2. `BaseModifierValue`, then `PerGradeUpgrade`, is evaluated (initialization data, NATIVE_PROGRESSION.md section 1),
     each with **the item as the context object and no second context** (read from the call's arguments, medium
     confidence: the argument order follows that note's evaluator), and each result kept in single precision;
  3. **`ComputedModifierValue` = `BaseModifierValue` + float(`EffectGrade` - 1) x `PerGradeUpgrade`**: the 1 is
     subtracted in integers, the difference converted to a float, then one single-precision multiply and one
     single-precision add (no fused multiply-add). It is stored as a float, with no rounding to an integer;
  4. the same value goes to that slot's element of `ReplicatedAttributeSlotModifierValues`;
  5. the item's `bNetDirty` is set;
  6. the slot's skill part (below), when it is both external and run-as-skill.

     This agrees with NATIVE_WEAPON_RULES.md's fit (`BaseModifierValue + PerGradeUpgrade x` the summed grade
     increases), which matched 9 of 9 cards, whenever the activation's base grade is 1. The rule is now read, not
     fitted, but still UNVERIFIED in play.

  A slot that is not activated is left as it is.
- **Elsewhere**, each of the first slot-limit slots' `ComputedModifierValue` takes its element of
  `ReplicatedAttributeSlotModifierValues`, activated or not; the slots past the limit and everything else are left as
  they are.

After the last slot, `RuntimeAttributeSlotSkill` is set (skill part). The value pass reads nothing it wrote for an
earlier slot except `TempStatModifier`, which only a context-dependent evaluation could read.

**The skill part** (on the authority, medium confidence). For each activated slot that has both `bExternalSlot` and
`bRunEffectsAsSkill`:
- **The runtime skill.** The first such slot asks the item for a runtime skill; once one is made, later slots of the
  pass reuse it. When the definition's `AttributeSlotEffectMode` is not 2 (and the item is the authority and has a
  definition), that is a newly constructed `SkillDefinition`, made on every value pass: an earlier
  `RuntimeAttributeSlotSkill` is never read, so it is replaced, not reused. The class is identified by its layout,
  which matches field for field. Its fields:
  - `BaseRange`: one billion;
  - when the mode is 1, also `DurationType` 1 and `InitialDuration` = the definition's
    `AttributeSlotEffectSkillDuration`, evaluated;
  - with mode 2 there is no skill: nothing is appended, `RuntimeAttributeSlotSkill` is left as it was, and the pass
    asks again at the next such slot, with the same answer.
- **The skill effect.** Each such slot appends one entry to the skill's `SkillEffectDefinitions`:
  - `AttributeToModify` from the slot, and `bIncludeSelfAsTarget` set;
  - `EffectTarget` 2 when the slot has `bIncludeAlliesAsTarget`, and `TargetInstanceDataName` and `ModifierType` from
    the slot;
  - `BaseModifierValue`: constant 1, the slot's `ConstraintAttribute` as its base attribute, and the slot's
    `ComputedModifierValue` as its scale;
  - `GradeToStartApplyingEffect` 1 and `PerGradeUpgradeInterval` 1;
  - everything else zero or empty.
- **The item's `RuntimeAttributeSlotSkill`** is set to the skill when at least one entry was appended.

The enum entries are given by number: their names were not resolved here.

## 6. What it refuses, and edge cases

- **No refusal** of its own: an upgrade entry naming an unknown slot, and an activation past the limit, are dropped
  silently.
- **A missing definition:** the type's upgrades are skipped, but the slot limit is then read from no definition, which
  the native does not guard. The game has no case for it.
- **Already activated slots** (an item initialised before) keep their place and data in `InitializeAttributeSlots`; a
  working record of the same name is not written.
- **Writes:** `AttributeSlots` (the new records, and in the name-part pass the in-place grades), `TempStatModifier` and
  its base and stack, `ReplicatedAttributeSlotModifierValues`, `bNetDirty`, `RuntimeAttributeSlotSkill`, and the
  runtime skill's fields.

## 7. For an implementer, in the port's terms: the Off answers, step by step

| Step | Off answer |
|---|---|
| the bool | the value; `true` adds the name-part pass |
| the definition | the class's rule (section 2); **None: `NotYet` IAS-NO-DEFINITION** (the native reads the slot limit from it unguarded; the game has no answer) |
| the part and name-part lists | the class's slots in the order of section 2 |
| finding a slot record | the value (section 3 step 3); a slot not found: the entry is ignored |
| the activation's `AttributeSlotBaseGrade` | its evaluated value, rounded as section 3 says, **when the initialization data reads no attribute and no initialization definition** (then the context does not matter); otherwise **`NotYet` IAS-BASE-GRADE-CONTEXT** (open question 1); a `y` outside the native's exact conversion range or not finite, or a grade sum that would wrap: **`NotYet` IAS-ROUNDING-RANGE** (port restrictions) |
| the item's slots before the call | a packed, unique prefix of activated slots (section 3 step 4.5): the value; any other shape: **`NotYet` IAS-SLOT-SHAPE** |
| writing the records, the limit and the clamps | the value (section 3 step 4) |
| the name-part pass | the value (section 4), with the in-place grade rule |
| the value pass, not the authority | the replicated values (section 5) |
| the value pass, the authority | the formula **when both initialization data are context-free** as above; otherwise **`NotYet` IAS-VALUE-CONTEXT** (open question 1: the item is read as the context, at medium confidence, and its `TempStatModifier` is set first) |
| the skill part, mode 2 | the value: nothing (no skill, no entry, `RuntimeAttributeSlotSkill` unchanged); deliberately not refused |
| the skill part, any other mode | **`NotYet` IAS-RUNTIME-SKILL** whenever an activated slot is both external and run-as-skill: constructing the runtime `SkillDefinition` (its outer and its name) is not read (open question 2) |

The identifiers are for the port's `NotYet` texts (each starts with its identifier) and its tests. Every one is
`Missing::NotRead`, with this note as the record. Section 8 gives the order in which they are decided.

## 8. The planning order: every refusal before the first write

Both entry points are planned read-only first, on a copy of the item's slots, and written only once the plan is
complete. **No refusal is meant to come after a mutation**: every `NotYet` above is decided in the plan. Only other
errors (a property the class does not declare, a failed read) could arise later, and the plan reads every field it
needs, so it raises them first too.

**If a `NotYet` still surfaces after a write** (a port defect, or a rule a later lane adds), the Off rule is the
hub's: the call stops there, the `NotYet` keeps its kind and record, its message starts with "stopped after partial
writes", and the writes already made stay (no rollback in Off). Any other error passes through unchanged.

**`InitializeAttributeSlots(b)`:**
1. The definition (IAS-NO-DEFINITION), then the slot limit from it.
2. The item's first slot-limit slots: their shape (IAS-SLOT-SHAPE). The kept names and `K` follow.
3. The base pass on scratch records: the working set from the definition's and then the part list's upgrades (section
   3 steps 2-3). At each activation, the base grade's context-free test (IAS-BASE-GRADE-CONTEXT), its evaluation, the
   range of its rounding and the grade sums (IAS-ROUNDING-RANGE).
4. The projected write of step 4: which records go to indices `K` onwards, and their clamped grades.
5. When `b` is true, the name-part pass on the projected slots (section 4): the in-place grade increases on the projected
   copies, the new records it activates with the same tests as step 3, and their projected write after the projected
   prefix. This is the step that catches a prefix or title that activates a new slot: its base grade, and below its
   values and skill, are checked here, before the base pass has written anything.
6. The value pass on the projected final slots, once for each time it will run (once, or twice when `b` is true). On the
   authority, for every activated projected slot among the first slot-limit ones: both initialization data
   (IAS-VALUE-CONTEXT), the computed value, and, for an external run-as-skill slot, the mode (IAS-RUNTIME-SKILL unless it
   is 2). Elsewhere, the replicated values.
7. Then the writes, in the native's order:
   1. the base pass's copies and clamps;
   2. with `b`, the name-part pass's in-place increases, its copies and clamps, and its own value pass;
   3. then `InitializeAttributeSlots`'s value pass.

   Each value pass writes per slot in section 5's order.

**`InitializeAttributeSlotsForNameParts()`:** steps 1 and 2, then step 5 on the item's own slots, then step 6 for
its one value pass. Its writes, with no base pass:
1. the name-part pass's in-place increases (in upgrade order), then its copies and clamps;
2. its one value pass, per slot in section 5's order.

In the native the in-place increases are written while the upgrades are applied, so a later entry's activation is
evaluated after an earlier entry's write; in the plan both are decided first.

**Why the plan is exact.** Every value the plan computes reads only context-free data (constants and scales, no
attribute and no initialization definition) and the slots themselves, so the writes cannot change a planned value. The
one write the value pass reads back, `TempStatModifier`, is visible only to a context-dependent evaluation, which the
plan refuses.

The slot array is the declared static array of 19 (decision 2026-11-19-49), read and written element by element. (It
belongs to section 7: the port's slot array.)

## Open questions

1. **UNVERIFIED:** the context of the evaluations. The base grade and the value pass's two evaluations pass the item
   and an empty second argument (read from the call's arguments in the listing, medium confidence: which position is
   the context and which the optional second context follows NATIVE_PROGRESSION.md's evaluator). The skill duration's
   arguments were not read. Lifting IAS-BASE-GRADE-CONTEXT and IAS-VALUE-CONTEXT on this reading is the hub's call.
2. **UNVERIFIED:** how the runtime `SkillDefinition` is constructed (its outer and name), and the names of the enum
   entries used (`AttributeSlotEffectMode` 1 and 2, `DurationType` 1, `EffectTarget` 2). An earlier
   `RuntimeAttributeSlotSkill` is replaced, as read (section 5).
3. **UNVERIFIED:** that the working set's order is first-found order, and so the positions new slots take.
4. **UNVERIFIED:** that the name-part pass leaves in-place grade increases unclamped, and that a name part's own
   `AttributeSlotEffects` are never searched.
5. **UNVERIFIED:** the destination indices of section 3 step 4 when the item's activated slots are not a packed, unique
   prefix, and which duplicate the name-part pass changes.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- On a freshly dropped gun with an attribute slot (a Maliwan or a Hyperion part with `AttributeSlotUpgrades`), read
  `AttributeSlots` (names, `bActivated`, `EffectGrade`, `ComputedModifierValue`), the definition's
  `AttributeSlotBaseGrade` and `AttributeSlotMaxActivated`, and the parts' `GradeIncrease` values. The grade should be
  the rounded base grade plus every increase, and the value `BaseModifierValue + (EffectGrade - 1) x PerGradeUpgrade`.
- On the same gun read `ReplicatedAttributeSlotModifierValues`: equal to the values on the authority.
- Hook `InitializeAttributeSlots` and `InitializeAttributeSlotsForNameParts` at a drop and at a save load. Record the
  bool and the slots before and after each call (section 1, and the in-place rule of section 4).
- On an item with a run-as-skill external slot (a class mod), read `RuntimeAttributeSlotSkill` and its
  `SkillEffectDefinitions` after initialisation (section 5's skill part).

Cases with distinct values, each from a hook on the two natives with the slots read before and after the call. A
live game only offers the data the packages hold, so each case first needs an item whose definition and parts have the
shape; finding one is part of the check, and where none exists the case is listed as having no observable test.
- **Competing effect entries, several new names:** a definition and a part that both list `AttributeSlotEffects` for one
  slot name with different `BaseModifierValue`, plus upgrades that activate two or more new names. Expect the
  definition's entry to win (section 3 step 3) and the new slots at indices `K`, `K + 1`, ... in first-found order.
- **Conflicting clamps, then a name part:** a slot with both clamps enforced and `MinimumGrade` above `MaximumGrade`
  (for example 4 and 2), then a prefix that adds a `GradeIncrease`. Expect the grade 4 after the base pass and 4 plus
  the increase after the name-part pass (unclamped in place).
- **An out-of-range limit:** a definition whose `AttributeSlotMaxActivated` is 0, negative or above 19. Expect 19 slots
  considered, not the nearer end. No cooked definition may have such a value; then this has no observable test, and
  only a hook that changes the value in memory before the call would show it (a session change, the maintainer's call).
- **A negative half tie:** an `AttributeSlotBaseGrade` that evaluates to -0.5, -1.5 or 2.5. Expect grade increments 0,
  -1 and 3. Cooked base grades are likely small positive constants, so this likely needs the same in-memory change.
- **Client slots and the copy boundary:** on a client in a co-op session (the maintainer's call), before the call set
  every one of the 19 `ComputedModifierValue`s to -1 and each `ReplicatedAttributeSlotModifierValues` element to its
  own sentinel, 100 plus its index, on an item whose slot limit `L` is below 19 and with an inactive slot below `L`.
  Expect `ComputedModifierValue` = 100 + i for every i below `L`, the inactive one included, and -1 left at every i
  from `L` on.
- **Contexts, item against owner:** an item whose `PerGradeUpgrade` (or base grade) reads an attribute that resolves
  to different values on the item and on its owner, for example 2 on the item and 5 on the owner, with
  `BaseModifierValue` 10 and an `EffectGrade` of 3. Expect `ComputedModifierValue` 10 + 2 x 2 = 14 if the item is the
  context, and 10 + 2 x 5 = 20 if the owner is. For the base grade, an attribute of 2 on the item and 5 on the owner
  with no increases gives `EffectGrade` 2 or 5. A cooked item with that shape may not exist; then the attribute's value
  is changed in memory before the call (a session change, the maintainer's call). Repeat the initialisation of one
  item (a save reload) and check the values are stable.
- **The runtime skill:** seed `RuntimeAttributeSlotSkill` with a known object before the call. On a definition in a
  mode other than 2, initialise a class mod twice (equip, unequip, reload): expect a new object each time, not the
  seed, and record its outer and its name. On a mode-2 definition, if any, expect the seed left in place after the
  call (the pass makes no skill and writes none).
