## 2026-11-19: Read InitializeAttributeSlots as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-INIT-ATTRIBUTE-SLOTS, analyst only. Number 73 was given by the hub. Slice link 8 (equip the gun) now stops
next at the unbound `WillowInventory.InitializeAttributeSlots(bool)` in the weapon's `InitializeInternal`. The new
own-words note `docs/verification/NATIVE_INIT_ATTRIBUTE_SLOTS.md` and two `NATIVE_INDEX.md` rows (replacing the one row
that pointed at NATIVE_ATTRIBUTES.md) record, as behaviour only:

- **The bool:** "the name parts are already set". The weapon passes it when its parts are not rolled, the item when
  neither its parts nor its name parts are. With it, the name-part pass runs in the same call.
- **The item's lists:** its definition (`WeaponTypeDefinition` / `ItemDefinition`, which is also
  `GetInventoryDefinition`'s rule), its part list (nine slots, no name parts) and its name-part list (prefix, title).
- **The slot build:**
  - the definition's then each part's `AttributeSlotUpgrades` grow named slots: activation adds the rounded
    `AttributeSlotBaseGrade` once, and every entry adds its `GradeIncrease`;
  - a slot's data comes from the first matching `AttributeSlotEffects` entry, the definition's first, then the parts'
    in list order;
  - activated records go into the item's 19-slot array from index `K`, the number of distinct names among the
    activated slots already there, without skipping a kept slot, up to `AttributeSlotMaxActivated` (taken as 19
    outside 1..19), each clamped by its maximum and then its minimum grade. Off supports only items whose activated
    slots are a packed prefix with distinct names, and refuses any other shape.
- **The name-part pass** changes already activated slots in place.
- **The value pass:** on the authority `ComputedModifierValue = BaseModifierValue + (EffectGrade - 1) x
  PerGradeUpgrade`, copied into `ReplicatedAttributeSlotModifierValues`; elsewhere the copy goes the other way. This
  agrees with NATIVE_WEAPON_RULES.md's 9-card fit when the base grade is 1.
- **The skill part:** external run-as-skill slots feed a runtime `SkillDefinition` that becomes
  `RuntimeAttributeSlotSkill`.
- **The Off answers** are given step by step, with a planning order that decides every refusal before the first
  write. The named `NotYet`s (`Missing::NotRead`) are IAS-NO-DEFINITION (no definition), IAS-SLOT-SHAPE (activated
  slots that are not a packed, unique prefix), IAS-BASE-GRADE-CONTEXT and IAS-VALUE-CONTEXT (initialization data
  that are not context-free), IAS-ROUNDING-RANGE (a rounding input outside the exact conversion range, or a grade sum
  that would wrap) and IAS-RUNTIME-SKILL (an external run-as-skill slot in a skill mode other than 2; mode 2 answers
  nothing). The evaluations' context was read as the item with no second context, at medium confidence; the Off
  answers still refuse context-dependent data, and lifting that is the hub's call.

The note cross-references NATIVE_ATTRIBUTES.md section 9, whose open `ComputedModifierValue` computation is now read
(one added line there points at the new note), and the `AttributeSlots[19]` static-array shape of decision
2026-11-19-49.

Method: the script was read with `research/script_disasm.py`, the layouts with `tools/ghidra/class_layout.py`, and the
natives and the item classes' virtual methods with Ghidra 12.1.4 headless, read-only, under the machine lock. The class
vtables came from each class's registration call. Raw output stays under ignored
`local/analysis/init-attribute-slots/`. The note was scanned with the lane brief's pattern (no hits). No bl-sdk session
was granted, so nothing was observed in the running game; every rule is UNVERIFIED. This lane implements nothing.

Open (the note's questions): the evaluation contexts (read as the item, medium confidence); how the runtime skill
is constructed, and the enum entry names; the order of new slots; the name-part pass's in-place grades and lookup
lists; the destination indices for an item whose activated slots are not a packed, unique prefix.

Edits (review rounds 1 and 2 of #460): the destination indices and the Off shape restriction, the planning order and
the after-write rule, the rounding steps and their range, the value pass's order and context, and the revised
refusal list.
