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
  NotRead, never loaded. The schema check goes in the equip preflight.
- **Review r1 of #525:** the unloaded-weapon-type refusal sits where the weapon type is first read (after the None
  early return, before any load and before the balance, damage-type and slot reads). The next stop is left to the
  gate run. The stored value and its class default are to be read directly, and the live checks compare captured
  slots in slot order across the count boundary.
- **Link 8:** this removes the slot-count stop; the card's other refusals stay.
- **UNVERIFIED.** Every rule is unconfirmed in the running game; the note lists the live checks.
