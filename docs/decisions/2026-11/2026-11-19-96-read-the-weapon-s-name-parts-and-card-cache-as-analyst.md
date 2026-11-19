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
  - Off answers `NotRead` whatever the data, and every path reaches the native. So the refusal is cheap and certain,
    and the equip driver answers it in its preflight, before the weapon is made, with no prefix (review r1 of #484).
    The native's own call-time answer stays for a run that skips the preflight; that one carries the partial-write
    prefix;
  - the game's first load of the globals definition writes (`TheGlobalsDefinition`, the challenge numbering). Off
    never runs it: its getter answers `NotRead` while the field is unset (NATIVE_WILLOW_GLOBALS.md section 6.3). The
    port's package and definition loads are not writes (NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md section 3). Once the
    native is read, the equip driver runs the globals preflight of NATIVE_WILLOW_GLOBALS.md section 6.2, steps 1 to 3.
- **The BulletSpeed weapon (H6's report): a real gap, not a fixture problem.** The installed `WeaponBulletSpeed`
  attribute's value resolver names `BulletSpeed`, a property no installed class declares. The resolver's answer for a
  failed lookup is unread, so the port's existing typed `NotRead` stays, with no change. Link 8's weapon carries no
  BulletSpeed part. NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md section 4 calls the stopping weapon a sniper rifle, but
  the BulletSpeed accessory is an assault-rifle part; that mismatch is flagged and stays UNVERIFIED.
- **Not verified:** everything above in the running game; the vtable slot identity of `CacheWeaponCard`.
