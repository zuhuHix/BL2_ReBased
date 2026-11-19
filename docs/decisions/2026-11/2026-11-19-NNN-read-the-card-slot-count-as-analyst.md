## 2026-11-19: The weapon card's slot count, read as analyst

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-CARD-SLOT-COUNT, slice link 8. Analyst note only (clean room):
docs/verification/NATIVE_CARD_SLOT_COUNT.md. Written in own words from local, read-only analysis of the executable
with Ghidra, under the machine lock, plus the layout tool. Raw output stays under ignored `local/analysis/cardslot/`.
No live session was run.

- **Read:** `WillowWeapon.CacheWeaponCard` takes its slot count from the weapon's
  `DefinitionData.WeaponTypeDefinition`: its `AttributeSlotMaxActivated` through the engine routine behind the script
  native `GetAttributeSlotMaxActivated` (1 to 19 as is, else 19). It is read after the stats array is cleared and
  before the first slot.
- **Read:** slots 0 to count−1 all run the outline's slot step, with no special case for slot 0. The bits and fields it
  reads match the layout tool's `AttributeSlotData`.
- **For the implementer:** replace the "the card's slot count" `NotYet` with that count. An unloaded weapon type is a
  `NotRead`, never loaded. The schema check goes in the equip preflight.
- **Link 8's expected next stop:** one of the body note's two remaining refusals (the tie order or the displayed
  value), or past `init`.
- **UNVERIFIED.** Every rule is unconfirmed in the running game; the note lists the live checks.
