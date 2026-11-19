## 2026-11-19: Read CacheWeaponCard's body as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-CACHE-WEAPON-CARD-BODY, analyst only (clean room). New companion note
`docs/verification/NATIVE_CACHE_WEAPON_CARD_BODY.md` to NATIVE_CACHE_WEAPON_CARD.md (decision 2026-11-19-96), and its
NATIVE_INDEX row. The decision number is a placeholder until the hub assigns one.

- **Sources:** `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries and the local table reader, one
  headless run at a time under the machine lock; field names from `tools/ghidra/class_layout.py`. Raw output stays
  under ignored `local/analysis/cwc/`. There is no bl-sdk session, so every rule is **UNVERIFIED**.
- **Read:**
  - **The body:** the virtual slot reaches the body the outline described (through `WillowWeapon`'s table).
  - **The presentation lookup:** the DLC set's override, then its default list; then the globals definition's
    override, then its default list; then the status-effect presentation's damage-type mapping.
  - **The modifier records:** accumulated from the weapon type's and every part's `WeaponAttributeEffects` and
    `ExternalAttributeEffects`.
  - **The card entries:** each definition's `WeaponCardAttributes` (only with a record), its `CustomPresentations`,
    and the damage type's `WeaponCardPresentations`.
  - **The order:** by priority, largest first, unstable on ties. The value formula is read too.
- **Off answers** (final after review r1): the card is built before the native's first write. Off refuses (`NotRead`)
  only on the slot count (a slot from 1 to 18 activated), a priority tie between appended entries, or an appended entry
  with a record, whose `bShouldDisplay` needs the display conversion read in outline only.
- Not implemented here: another lane codes it from the notes.

**Review r1 of #491 (eight findings, fixed in the note).**
1. **The slot count:** it is an inventory definition's `AttributeSlotMaxActivated`, clamped; which definition is not
   tied. Off visits slot 0 always and refuses `NotRead` before any write when a slot from 1 to 18 is `bActivated`.
2. **The equip driver:** the outline's unconditional card refusal is removed. The driver runs NATIVE_WILLOW_GLOBALS.md
   section 6.1 and section 6.2 steps 1 to 3. The tie and display refusals run at call time, because the card's inputs
   depend on the made weapon and, on the name-part branch, on the prefix and title written during init.
3. **A card schema** is checked once in the preflight (`VmError::Script` per field), with a class-defaults rule.
4. **The record key** is the added presentation's `Attribute`, after the damage-type replacement; the first map's
   record wins with no merge. A reread also corrected the note: an existing entry gains its increase even when the
   record requirement fails, and `BasePriority` is read through a presentation method that answers that field.
5. **The tie and display refusals** cover appended entries only, ties are exact float equality, and a card with a
   recorded appended entry still refuses until the display conversion is read.
6. **Off only**, grading by variant and kind, refusal texts naming the native.
7. **NATIVE_INDEX:** "body identified (static read)".
8. **A None damage type** matches a `MappingData` entry with a None `DamageType` and adds no `WeaponCardPresentations`;
   "does not resolve" is a None `ResolvedContext` in the port.
