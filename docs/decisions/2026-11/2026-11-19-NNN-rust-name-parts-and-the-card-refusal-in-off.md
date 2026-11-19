## 2026-11-19: Rust name parts and the card refusal in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-CACHE-WEAPON-CARD, implementer, for slice link 8 (equip). Every two-digit
number of 2026-11-19 is taken, so `NNN` is a placeholder until the maintainer rules on three-digit numbers. The
decisions check refuses it until then.

- **Clean room:** coded from `docs/verification/NATIVE_CACHE_WEAPON_CARD.md` (decision 2026-11-19-96, #484 at its
  review-r1 head) and the hub's rulings alone. The implementer opened no native code and no Ghidra output.
- **No live session.** Every rule stays **UNVERIFIED** in the running game. The note's last section lists the bl-sdk
  checks that would settle each one.

- **New module** `ow-inventory` `equip::name_parts`, registered from `manager::Pack`. It binds in Off only, by each
  declaration's own native key. Reference binds neither native (architecture rule R3).
- **`WillowWeapon.ChooseRandomNameParts()` in full** (the note's sections 2 and 5.2):
  - a None `WeaponTypeDefinition` writes nothing;
  - an entry qualifies when `MinExpLevelRequirement` <= `ExpLevel` <= `MaxExpLevelRequirement`, its `Priority` is
    above 0 (a NaN is not), and its `Expressions` hold (And mode, the weapon as the source, no override);
  - the best of a list replaces the current best at an equal or higher priority, so a later entry wins a tie;
  - the type's lists first, then the nine part slots in order, each a None slot skipped; a part's non-None best
    replaces a None pick or one of equal or lower priority, and never clears a pick;
  - both picks are computed first, then both fields are written, None included;
  - any `NotYet` it returns carries the "stopped after partial writes: " prefix, its kind and record kept, since it
    runs inside `InitializeInternal` after writes. A refusal leaves both fields as they were.
  - Name-part fields the package leaves untagged are read as the store loads them, from the archetype or class
    defaults (Min 1, Max 100, Priority 1 per the note). No fixture of this lane tests the defaults: the store's rule
    is not new here.
- **`WillowWeapon.CacheWeaponCard()` at call time** (section 5.3): `NotYet { NotRead }` at entry, before anything of its
  own, with the note's text and record, prefixed.
- **The preflight** (section 5.1), `name_parts::preflight`, before the weapon is made:
  - first the `CacheWeaponCard` refusal, the same `NotYet { NotRead }` **without** the prefix. While
    `CACHE_WEAPON_CARD_READ` is false it always comes first;
  - then the name-part schema on the declared classes, each failure a `VmError::Script` naming the field. It is kept
    so it holds once the card is read and the refusal goes.
- **Link 8's driver** (`ow-cli` `slice_gate::run_equip`) runs the preflight inside its `make` step, before
  `prepare_line`, and grades it by variant (`classify_error`). It adds no globals set-up: per the review of #491, once
  the card is read the driver runs NATIVE_WILLOW_GLOBALS.md section 6.1's set-up and section 6.2's steps 1 to 3 only,
  and that is the next lane's work (#491, implemented on top of this branch).
- **What changed at the gate** (on main d72b96d, the same build with the preflight call disabled as the before): link
  8 used to reach `init` and stop at the unbound `WillowInventory.ApplyInternalSlotEffectModifiers(bool,int,array)`
  (graded error). It now stops at `make` on the card refusal (graded
  `notyet`), with nothing written. Its step share drops from 1 of 5 to 0 of 5, as the note intends: the run cannot
  finish on either path, and the stop is now typed and certain.
- **Not done:** the card's presentation lookup, its card entries and its ordering (the note's open questions 2 and 3),
  the globals preflight, and any live check.
- Synthetic tests (`equip::name_parts::tests`, 16, invented classes and values only):
  - picks, ties, level edges, priority 0 and below, And over the expressions;
  - a part replacing at equal priority, never clearing a pick, filling a None pick, and the last slot being walked;
  - both fields written None included, and nothing written with no type;
  - an expression `NotYet` prefixed once with nothing written;
  - the card's prefixed call-time `NotRead` and the preflight's unprefixed one, the schema holding and four schema
    failures, binding by key in Off, and nothing bound in Reference.
