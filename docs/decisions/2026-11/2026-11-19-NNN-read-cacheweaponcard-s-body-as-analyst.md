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
- **Off answers:** the card is built before any write. Off refuses (`NotRead`) only on a priority tie between entries,
  or when an entry with a record needs its displayed value for `bShouldDisplay`; that display conversion is read in
  outline only.
- Not implemented here: another lane codes it from the notes.
