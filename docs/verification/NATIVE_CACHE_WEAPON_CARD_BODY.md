# CacheWeaponCard's body: the presentation lookup, the card entries and their order (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-CACHE-WEAPON-CARD-BODY (decision number pending from the hub). A
companion to NATIVE_CACHE_WEAPON_CARD.md (decision 2026-11-19-96), whose section 3 read this native in outline.
- **Sources:** the game executable, read with Ghidra (`tools/ghidra/run.ps1`, read-only queries and the local table
  reader, one headless run at a time under the machine lock); field names from `tools/ghidra/class_layout.py`.
- **Own words only:** no decompiler output, pseudo-code, addresses, offsets or listing structure. Raw output stays
  under ignored `local/analysis/cwc/`.
- **No live session** was run. **Every rule is UNVERIFIED in the running game.**

**Clean room.** This lane implements nothing. A fresh implementer codes the Off answers from this note and
NATIVE_CACHE_WEAPON_CARD.md.

## 1. Which body the call reaches (the outline's open question 1)

`WillowWeapon.CacheWeaponCard`'s exec entry calls one virtual function of the weapon. `WillowWeapon`'s table was found
through its class registration and its constructor, and that slot holds **the body the outline described**. So
section 3 of NATIVE_CACHE_WEAPON_CARD.md reads the right function. No subclass of `WillowWeapon` was looked at (open
question 5 there stays open).

## 2. The presentation lookup (the outline's open question 2)

For an attribute A, with the weapon and the globals definition. A None attribute finds nothing.
1. **The DLC set first.** When `DefinitionData.BalanceDefinition` is set and its `DlcItemSet` is set:
   - the set's `WeaponAttributePresentationOverride` list, when set, is searched first;
   - then its `DefaultAttributePresentation` list, when set.
   - The first presentation found is the answer.
2. **Then the globals definition:**
   - its `WeaponAttributePresentationOverride` list, when set;
   - then its `DefaultAttributePresentation` list, when set.
3. **A list search:** the list's `Attributes` array (presentations) in order. The first entry that is not None and
   whose `Attribute` is A is found. None when no entry matches.
4. **The damage-type replacement.** When the presentation found is a `WeaponStatusEffectAttributePresentationDefinition`
   (or a subclass):
   - the weapon's damage type is taken: the elemental part's `CustomDamageTypeDefinition` when the part and that
     field are set, else the weapon type's `DefaultDamageTypeDefinition`;
   - the presentation's `MappingData` (`AttributePresentationDamageTypeMapping` entries) is searched in order for the
     first entry whose `DamageType` is that one, and that entry's `Presentation` is the answer;
   - **with no matching entry, the answer is None.**
   - **The comparison is plain object identity.** So when the weapon's damage type is None (no elemental override and
     a None `DefaultDamageTypeDefinition`), the first entry whose `DamageType` is None matches, and its `Presentation`
     is the answer. With no such entry, the answer is None.
   - The class test was read as a test against one class of the game's package, looked up once by the engine. Reading
     it as `WeaponStatusEffectAttributePresentationDefinition` rests on two facts: it is the only presentation
     subclass in the installed packages, and the step reads that class's `MappingData` (UNVERIFIED by name).
5. Otherwise the presentation found is the answer.

## 3. The card entries (the outline's open question 3)

The native builds the entries with a helper object, in four phases.

### 3.1 The accumulated modifiers

Before anything is visited, two maps from an attribute to an accumulated record are filled, from the weapon type and
then from each definition slot (the slot walk of section 3.2, name parts included). The `WeaponAttributeEffects` of
each fill the first map, and their `ExternalAttributeEffects` fill the second.
- For each effect entry whose attribute is not None:
  1. **Its value:** its `BaseModifierValue`, evaluated with the weapon as context (the attribute system's
     initialization-data evaluation, NATIVE_ATTRIBUTES.md).
  2. **Its record:** the record for that attribute is made on first sight, all zeros.
  3. **By `ModifierType`:**
     - `MT_Scale` (0) with a negative value goes into a separate "negative scale" sum;
     - otherwise `MT_Scale`, `MT_PreAdd` (1) and `MT_PostAdd` (2) each add a non-zero value into their own sum;
     - a zero value adds nothing.
- So a record holds four sums: scale, pre-add, post-add and negative scale.

### 3.2 The visits, in order

1. **The weapon type**, then each **definition slot** in this order, a None slot skipped:
   - `BodyPartDefinition`, `GripPartDefinition`, `BarrelPartDefinition`, `SightPartDefinition`, `StockPartDefinition`,
     `ElementalPartDefinition`, `Accessory1PartDefinition`, `Accessory2PartDefinition`, `MaterialPartDefinition`;
   - then `PrefixPartDefinition` and `TitlePartDefinition` (the call includes the name parts).
2. **Each visited definition** (the type's and the parts' own visit, read for `WeaponTypeDefinition` and
   `WeaponPartDefinition`):
   - for each `WeaponCardAttributes` entry (`Attribute`, `PriorityIncrease`), in order: the presentation P of the
     entry's `Attribute` is looked up (section 2, the damage-type replacement included) and added with the entry's
     `PriorityIncrease`, **with a record required** (below);
   - then each of its `CustomPresentations`, added with no increase and no record required.
3. **The damage type's `WeaponCardPresentations`:** each added with no increase and no record required. The damage
   type is section 2, step 4's. When it is None, nothing is added here.

**Adding a presentation** P, with an increase I. A None P adds nothing.
- The entries are keyed by presentation, so a presentation is listed once.
- **An entry for P already exists:** I is added to its priority. Nothing else is checked, so an entry made earlier
  without a record requirement still gains here even when a record is required and P's `Attribute` has none.
- **No entry yet:**
  - **the record is looked up by P's `Attribute`.** P is the presentation being added, so for a `WeaponCardAttributes`
    entry it is the lookup's answer after the damage-type replacement, not the entry's own `Attribute`. The first map
    of section 3.1 is searched first. A record found there is the answer, with no merge: the second map is searched
    only when the first holds none for that attribute. A None `Attribute` has no record;
  - when a record is required and there is none, no entry is made;
  - otherwise the entry is made with that record (or none), no constraint presentation, and a priority of P's
    `BasePriority`, then I is added. The priority is read through a presentation method; for both presentation
    classes the method was read, and it answers `BasePriority`.

### 3.3 The order and the values

The entries are sorted by priority, **largest first**. The sort is not stable, so the order of equal priorities is not
defined (it depends on the sort's internals; UNVERIFIED either way). Then, in that order, each entry with a
presentation and a priority of at least 0.0001 is appended to `WeaponCardModifierStats`, after section 3 of the outline's
slot entries:
1. **The base value B:**
   - 1 when the presentation has no `Attribute`;
   - else that attribute's base value with the weapon as context.
   - When that context does not resolve, B is 1 if `bUseOneAsBaseValueForPresentationIfContextFailsToResolve`, else 2
     if `bUseTwoAsBaseValueForPresentationIfContextFailsToResolve`, else the value the evaluation answered.
   - **In the port**, "does not resolve" is the base-value read's `ResolvedContext` coming back None. The value the
     evaluation then answers is the no-context answer of NATIVE_ATTRIBUTES.md (`GetValue` / `GetBaseValue`: 0.0; the
     same branch is the `DefaultValue` path of `StaticGetAttributeValueOrDefault`).
2. **The value V:**
   - 0 with no record;
   - else ((scale + 1) / (1 − negative scale)) × (pre-add + B) + post-add − B.
   - Then, when the presentation is `bDisplayAsPercentage` and |B| ≥ 0.00000001, V is divided by B.
3. **`bShouldDisplay`:**
   - true when there is no record;
   - else true when the presentation's displayed form of V is at least 0.00000001 in size.
   - That display conversion (percent scaling, sign style, inverse, rounding by `RoundingMode` and `FloatPrecision`)
     was read in outline only (section 5).
4. **The entry:** `AttributePresentation` is the presentation, `OptionalConstraintPresentation` is None, and
   `ModifierValue` is V.

Section 3, step 7 of the outline (replication) then runs on these entries.

## 4. For an implementer: the Off answers, step by step

**Off only; Reference behaviour is unchanged.**

This replaces NATIVE_CACHE_WEAPON_CARD.md section 5.3's refusal at entry, which stood because the outline's steps 4 to
6 were unread. With this note those steps are read, and the native refuses only in the cases of step 3 below.

**The outline's section 5.1 card refusal is removed.** That unconditional `NotRead` in the equip driver's preflight
existed only because the card was unread. With this note implemented it goes, and the equip driver's preflight runs:
- NATIVE_WILLOW_GLOBALS.md section 6.1 (the world set-up, `TheGlobalsDefinition` written);
- that note's section 6.2, **steps 1 to 3 only** (the globals instance, its class, `TheGlobalsDefinition` holding a
  `GlobalsDefinition`). Step 4 is the pickup route's own schemas, which the card does not read;
- the outline's name-part schema and class defaults (its section 5.1, after the removed refusal);
- the card schema of step 1 below.

The hub's rulings apply:
- preflight what is cheap and certain before any write;
- a `NotYet` after a write is terminal, its text prefixed "stopped after partial writes: ", its kind and record kept,
  and nothing is rolled back;
- a lazy definition load counts as a write;
- grade by the `VmError` variant and the `NotYet` kind, never by text.

Every refusal text names the native: "WillowWeapon.CacheWeaponCard: …".

1. **The card schema, in the equip driver's preflight** (checked once, on the declared classes, before the run's
   first write; no prefix). Each missing or mistyped field is a `VmError::Script` naming it:
   - `InventoryBalanceDefinition.DlcItemSet`: an object reference (`DownloadableItemSetDefinition`);
   - on `DownloadableItemSetDefinition` and on `GlobalsDefinition`: `WeaponAttributePresentationOverride` and
     `DefaultAttributePresentation`, object references to `AttributePresentationListDefinition`, whose `Attributes`
     is a dynamic array of `AttributePresentationDefinition` references;
   - `WeaponStatusEffectAttributePresentationDefinition.MappingData`: a dynamic array of the
     `AttributePresentationDamageTypeMapping` struct, which declares `Presentation` (an
     `AttributePresentationDefinition` reference) and `DamageType` (a `DamageTypeDefinition` reference);
   - on `WeaponTypeDefinition` and on `WeaponPartDefinition`:
     - `WeaponCardAttributes`, a dynamic array of the `AttributePriorityData` struct, which declares `Attribute` (an
       `AttributeDefinition` reference) and `PriorityIncrease` (a float);
     - `CustomPresentations`, a dynamic array of `AttributePresentationDefinition` references;
     - `WeaponAttributeEffects` and `ExternalAttributeEffects`, dynamic arrays of the `AttributeEffectData` struct,
       which declares `AttributeToModify` (the effect's attribute, an `AttributeDefinition` reference),
       `ModifierType` (a byte) and `BaseModifierValue` (an `AttributeInitializationData` struct);
   - `WeaponTypeDefinition.DefaultDamageTypeDefinition` and `WeaponPartDefinition.CustomDamageTypeDefinition`:
     `WillowDamageTypeDefinition` references; `WillowDamageTypeDefinition.WeaponCardPresentations`: a dynamic array of
     `AttributePresentationDefinition` references;
   - on `AttributePresentationDefinition`: `BasePriority` (a float), `Attribute` (an `AttributeDefinition` reference),
     and the bools `bDisplayAsPercentage`, `bUseOneAsBaseValueForPresentationIfContextFailsToResolve` and
     `bUseTwoAsBaseValueForPresentationIfContextFailsToResolve`;
   - `WillowWeapon.WeaponCardModifierStats`: a dynamic array of the `ModifierValuePresentationData` struct, which
     declares `AttributePresentation` and `OptionalConstraintPresentation` (`AttributePresentationDefinition`
     references), `ModifierValue` (a float) and `bShouldDisplay` (a bool);
   - also the fields the outline's steps 3 and 7 read (section 3 there): the slots' `bActivated`,
     `bIncludeInModifierText`, `AttributeToModify`, `ConstraintAttribute` and `ComputedModifierValue`, and
     `ReplicatedWeaponCardModifierValues` with its `ModifierValue` and `ModifierStatIndex`.

   **Class defaults.** Cooked data omits a field that holds its class default. So a presentation with no stored
   `BasePriority`, or a flag with no stored value, takes the default of its class (and a struct entry takes its
   struct's default). The implementer reads these through the class defaults, never as 0.

2. **At call time, before any write of the native's own,** compute the whole card into a list, writing nothing:
   - the definition read (the outline's step 1). In Off the globals getter never loads; the preflight of step 1's
     list has made sure it answers the preloaded object;
   - the slot count rule of step 3;
   - the slot entries (the outline's step 3);
   - sections 3.1 to 3.3. Each effect value and base value goes through the port's attribute evaluation. Its `NotYet`
     returns as it is.

   **Why at call time and not in the preflight.** The card's inputs are not fixed before the weapon is made:
   - section 3.1's effect values and section 3.3's base values are evaluated with the weapon as context;
   - the slots' `bActivated` (step 3) is set by the init's slot steps;
   - on the name-part branch, `ChooseRandomNameParts` writes `PrefixPartDefinition` and `TitlePartDefinition` during
     `InitializeInternal`, and those two parts are visited by section 3.2.

   The native runs inside the spawn's `InitializeInternal`, after writes, so every call-time `NotYet` carries the
   prefix, even though the native itself has written nothing when it refuses.

3. **Refusals the computation can meet,** each `VmError::NotYet` of kind `NotRead`, record this note, found before
   the native's first write:
   - **the slot count.** Which inventory definition supplies it is not read (open question 1 below; it is that
     definition's `AttributeSlotMaxActivated`, clamped as the outline's step 3 says). The clamp always gives at least
     1, so **slot 0 is always visited**. If any slot from 1 to 18 is `bActivated`, the count could change the result:
     "WillowWeapon.CacheWeaponCard: the card's slot count". Otherwise the count cannot change the result, and the run
     carries on;
   - **two entries with exactly equal priority** (plain float equality), which must be ordered: "WillowWeapon.CacheWeaponCard:
     the card order of equal-priority entries";
   - **an entry with a record**, whose `bShouldDisplay` needs the display conversion: "WillowWeapon.CacheWeaponCard:
     the presentation's displayed value". Entries without a record display, and need no refusal.
   The tie and the display refusals look only at **entries that will be appended**: a presentation set and a priority
   of at least 0.0001 (section 3.3). Entries below that threshold are dropped and cannot tie or need display.
   - So a card with any appended entry whose presentation's `Attribute` has a record (for example, a matching
     `WeaponCardAttributes` entry) still refuses until open question 2 below is read.
   - **A `WeaponStatusEffectAttributePresentationDefinition` found by name** is not a refusal: the lookup reads its
     `MappingData` (section 2, step 4).
4. **The writes, in order:**
   1. clear `WeaponCardModifierStats`;
   2. clear the slot flags the outline's step 3 clears;
   3. append the slot entries, then the section 3.3 entries;
   4. replication on the authority (the outline's step 7).

**What link 8 meets.** The equip driver's weapon reaches `CacheWeaponCard` after `ComputeValueOfParts`. With this
note, Off can build the card except in the three cases of step 3: a slot from 1 to 18 activated, two appended entries
tied on priority, or an appended entry with a modifier record. Which of these the slice's weapon meets depends on its
slots and on its parts' `WeaponCardAttributes` and `CustomPresentations` and its damage type's
`WeaponCardPresentations`. An implementer can read that from the VM; this note does not assume it.

## 5. Open questions

1. **The slot count's definition.** The count is an inventory definition's `AttributeSlotMaxActivated`, through the
   same clamp the script getter `GetAttributeSlotMaxActivated` uses. Which definition the card asks (the weapon type
   is the likely one) was not tied by the reading.
2. **The presentation's display conversion**, used for `bShouldDisplay`: its sign-style rule, the inverse rule and the
   rounding step. Read in outline:
   - percentages are scaled by 100;
   - `RoundingMode` selects one of four roundings;
   - `FloatPrecision` is limited to the range 0 to 10.
3. **The tie order** of equal priorities (the sort is not stable).
4. **The class test** of section 2, step 4, which is read by inference.
5. The outline's open questions 4 and 5 stand.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **The lookup.**
   - On a DLC gun (a balance with a `DlcItemSet`), compare each `WeaponCardModifierStats` entry's
     `AttributePresentation` with the DLC set's lists and the globals' lists. Expect the DLC set's override first.
   - On an elemental gun with a status-effect presentation, expect the `MappingData` entry for its damage type.
2. **The order.** On guns with several card entries, read `WeaponCardModifierStats` after the drop. Expect the slot
   entries first, then the rest by descending priority (`BasePriority` plus the `PriorityIncrease` of every definition
   that lists the attribute).
3. **The values.** For one entry with a pre-add and a scale on the same attribute, recompute V from section 3.3 and
   compare it with `ModifierValue`.
4. **Display.** On an entry whose value is very small, read `bShouldDisplay`, to place the display conversion's zero.
5. **The slot count.** On a weapon whose type and parts give different `AttributeSlotMaxActivated` values, count the
   slot entries at the head of `WeaponCardModifierStats`, to tie the count to one definition.
