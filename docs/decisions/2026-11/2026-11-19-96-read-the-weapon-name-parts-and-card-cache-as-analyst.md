## 2026-11-19: Read the weapon's name parts and card cache as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-CACHE-WEAPON-CARD, analyst, for slice link 8 (equip init). This lane
reads and writes docs only; another lane implements from the note alone.
- `InitializeInternal` was read with `research/script_disasm.py`, its jumps converted to in-memory statement starts.
- The two natives were read with Ghidra under the machine lock. Raw output stays under `local/analysis/`.
- No live session was run, so every rule stays **UNVERIFIED** in the running game.

- **New record:** `docs/verification/NATIVE_CACHE_WEAPON_CARD.md`, with two `NATIVE_INDEX.md` rows and a pointer from
  NATIVE_INVENTORY_VALUE.md's open question.
- **The script's order:**
  - the name-part branch runs when `bSelectRandomPartsOnInitialization` or the `bForceSelectNameParts` parameter is
    true. It holds `ChooseRandomNameParts`, the second `ComputeValueOfParts`, the name-part slots and effects, and the
    second `RecomputeNormalizedDamageValues`;
  - the third `ComputeValueOfParts` and then `CacheWeaponCard` run on every path.
- **`ChooseRandomNameParts` (high):**
  - the best entry of the type's lists, then of each of the nine part slots in order, by level range, a priority
    above 0 and And-mode expressions on the weapon;
  - "at most" comparisons, so later entries and slots win ties;
  - both name-part fields are written at the end;
  - a None type writes nothing. No random draw.
  - Off can implement it.
- **`CacheWeaponCard` (outline medium, body identity low):**
  - read in outline: the globals definition (a possible first load), the cleared card list, the activated slots'
    entries with a flag cleared when a slot has no presentation, and replication of up to 5 later entries on the
    authority;
  - not read: the presentation lookup, the type and part card entries, and the ordering;
  - Off answers `NotRead` at entry, before any write of its own, with the partial-write prefix.
- **The BulletSpeed weapon (H6's report): a real gap, not a fixture problem.** The installed `WeaponBulletSpeed`
  attribute's value resolver names `BulletSpeed`, a property no installed class declares. The resolver's answer for a
  failed lookup is unread, so the port's typed `NotYet` is right.
- **Not verified:** everything above in the running game; the vtable slot identity of `CacheWeaponCard`.
