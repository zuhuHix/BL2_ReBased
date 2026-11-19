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
     `EffectGrade` grows by the definition's `AttributeSlotBaseGrade`, evaluated and rounded half up (to the nearest
     integer, a half going up);
   - in every case its `EffectGrade` then grows by the entry's `GradeIncrease`.

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
   2. Among the item's first slot-limit slots, those already `bActivated` are kept, by name.
   3. Each activated working record whose name is not kept is copied whole into the item. The copies go to the
      positions after the kept slots, one by one, in the working set's order, until the slot limit is reached.
      Activated records past the limit are dropped. Records that are not activated are never written.
   4. After each copy, `MaximumGrade` caps its `EffectGrade` first (when `bEnforceMaximumGrade`), then `MinimumGrade`
      raises it (when `bEnforceMinimumGrade`), so the minimum wins a conflict.
   5. The kept slots are left untouched.
5. **The name-part pass** (section 4), when the bool is true.
6. **The value pass** (section 5).

The working set's order is the order in which its records were first found, from the definition's upgrades and then
the parts'. That is the order of new records in the item (as read; UNVERIFIED).

## 4. InitializeAttributeSlotsForNameParts()

The same steps with two differences:
- the working set starts with the item's own activated slots among the first slot-limit ones, by name. A name part's
  upgrade to one of them changes that slot **in place**: its `EffectGrade` grows, and the min and max clamps of step
  4.4 are **not** applied again to it (as read; UNVERIFIED);
- only the name-part list's upgrades are applied, prefix then title. A slot a name part names but nobody has activated
  is still looked up in the definition's and the part list's `AttributeSlotEffects`, not in the name parts' own lists
  (medium confidence).

The activated records it makes are written as in section 3 step 4, then the value pass runs. When it is reached from
`InitializeAttributeSlots(true)`, the value pass runs twice; the second result is the same.

## 5. The value pass

For each of the item's first slot-limit slots:
- **On the authority** (the item's `Role` is `ROLE_Authority`), each activated slot:
  1. the item's `TempStatModifier` stack attribute is set to the slot's `EffectGrade`, as a float: its base and its
     value both take it, and its modifier stack is emptied;
  2. `BaseModifierValue` and `PerGradeUpgrade` are evaluated (initialization data, NATIVE_PROGRESSION.md section 1);
  3. **`ComputedModifierValue` = `BaseModifierValue` + (`EffectGrade` - 1) x `PerGradeUpgrade`**, in single precision,
     stored as a float (no rounding to an integer). The same value goes to that slot's element of
     `ReplicatedAttributeSlotModifierValues`, and the item's `bNetDirty` is set.

     This agrees with NATIVE_WEAPON_RULES.md's fit (`BaseModifierValue + PerGradeUpgrade x` the summed grade
     increases), which matched 9 of 9 cards, whenever the activation's base grade is 1. The rule is now read, not
     fitted, but still UNVERIFIED in play.

  A slot that is not activated is left as it is.
- **Elsewhere**, each slot's `ComputedModifierValue` takes its element of `ReplicatedAttributeSlotModifierValues`,
  activated or not.

**The skill part** (on the authority, medium confidence). For each activated slot that has both `bExternalSlot` and
`bRunEffectsAsSkill`:
- **The runtime skill.** The first such slot asks the item, once, for a runtime skill. When the definition's
  `AttributeSlotEffectMode` is not 2, that is a newly constructed `SkillDefinition`. The class is identified by its
  layout, which matches field for field. Its fields:
  - `BaseRange`: one billion;
  - when the mode is 1, also `DurationType` 1 and `InitialDuration` = the definition's
    `AttributeSlotEffectSkillDuration`, evaluated;
  - with mode 2 there is no skill, and nothing below happens.
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
| the definition | the class's rule (section 2); **None: a named `NotYet`** (the native reads the slot limit from it unguarded; the game has no answer) |
| the part and name-part lists | the class's slots in the order of section 2 |
| finding a slot record | the value (section 3 step 3); a slot not found: the entry is ignored |
| the activation's `AttributeSlotBaseGrade` | its evaluated value, rounded half up, **when the initialization data reads no attribute and no initialization definition** (then the context does not matter); otherwise **a named `NotYet`**: the evaluation's context and override are not read (open question 1) |
| writing the records, the limit and the clamps | the value (section 3 step 4) |
| the name-part pass | the value (section 4), with the in-place grade rule |
| the value pass, not the authority | the replicated values (section 5) |
| the value pass, the authority | the formula **when both initialization data are context-free** as above; otherwise **a named `NotYet`** (open question 1: the context, presumably the item, whose `TempStatModifier` the pass sets first) |
| the skill part | **a named `NotYet`** whenever an activated slot is both external and run-as-skill: constructing the runtime `SkillDefinition` (its outer, its name, what happens to an earlier one) is not read (open question 2) |

Decide every `NotYet` above before the first write where the inputs allow it: the definition, the slot limit, every
slot's data and the context-free test can all be read first. The value pass's evaluations come after the records are
written, so a `NotYet` there is "after a write" in the hub's sense.

The slot array is the declared static array of 19 (decision 2026-11-19-49), read and written element by element.

## Open questions

1. **UNVERIFIED:** which context source and override the native passes when it evaluates `AttributeSlotBaseGrade`,
   `BaseModifierValue`, `PerGradeUpgrade` and `AttributeSlotEffectSkillDuration`. The decompilation does not show the
   arguments. That the value pass sets the item's `TempStatModifier` to the grade first suggests the item.
2. **UNVERIFIED:** how the runtime `SkillDefinition` is constructed (its outer and name), whether an earlier
   `RuntimeAttributeSlotSkill` is replaced or reused, and the names of the enum entries used (`AttributeSlotEffectMode`
   1 and 2, `DurationType` 1, `EffectTarget` 2).
3. **UNVERIFIED:** that the working set's order is first-found order, and so the positions new slots take.
4. **UNVERIFIED:** that the name-part pass leaves in-place grade increases unclamped, and that a name part's own
   `AttributeSlotEffects` are never searched.

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
