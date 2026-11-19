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
  - any `NotYet` it returns comes back unprefixed: the native writes only at its end, so no refusal follows a write of
    its own. A refusal leaves both fields as they were.
  - Name-part fields the package leaves untagged are read as the store loads them, from the archetype or class
    defaults (Min 1, Max 100, Priority 1 per the note). One store-backed fixture checks it (review r1, item 3).
- **`WillowWeapon.CacheWeaponCard()` at call time** (section 5.3): `NotYet { NotRead }` at entry, before anything of its
  own, with the note's text and record, unprefixed (it writes nothing).
- **The preflight** (section 5.1), `name_parts::preflight`, before the weapon is made:
  - first the `CacheWeaponCard` refusal, the same `NotYet { NotRead }` **without** the prefix. While
    `CACHE_WEAPON_CARD_READ` is false it always comes first;
  - then the name-part schema on the declared classes, each failure a `VmError::Script` naming the field. It is kept
    so it holds once the card is read and the refusal goes.
- **Link 8's driver** (`ow-cli` `slice_gate::run_equip`) runs the preflight inside its `make` step, before
  `prepare_line`, and grades it by variant (`classify_error`). It adds no globals set-up: per the review of #491, once
  the card is read the driver runs NATIVE_WILLOW_GLOBALS.md section 6.1's set-up and section 6.2's steps 1 to 3 only,
  and that is the next lane's work (#491, implemented on top of this branch).
- **What changed at the gate** (rebased onto main 7668d07, with #483's `ApplyInternalSlotEffectModifiers`; the same
  build with the preflight call disabled as the before): without the preflight, link 8 reaches `init` and stops at
  `CacheWeaponCard`'s call-time `NotRead`, marked "stopped after partial writes: " by the driver (1 of 5 steps). With
  it, the stop is the same `NotRead` at `make`, unprefixed, with nothing written (0 of 5 steps). The step share drops,
  as the note intends: the run cannot finish on either path, and the stop is now certain before any write.
- **Not done:** the card's presentation lookup, its card entries and its ordering (the note's open questions 2 and 3),
  the globals preflight, and any live check.
- Synthetic tests (`equip::name_parts::tests`, 15 `#[test]`s, invented classes and values only; the first report's 16
  counted two tests of other modules that the name filter also matched):
  - picks, ties, level edges, priority 0 and below, And over the expressions;
  - a part replacing at equal priority, never clearing a pick, filling a None pick, and the last slot being walked;
  - both fields written None included, and nothing written with no type;
  - an expression `NotYet` returned unprefixed with nothing written;
  - the card's call-time `NotRead` and the preflight's, both unprefixed; the schema holding and five schema failures
    (the last a struct whose own name only ends with `WeaponDefinitionData`); binding by key in Off, and nothing bound
    in Reference;
  - one store-backed fixture: an untagged name part takes its class defaults (Min 1, Max 100, Priority 1) and
    qualifies.
- **Review r1 of #506** (four items):
  1. **The prefix is the caller's.** Both natives return their `NotYet` unprefixed. The layer that wrote adds the mark:
     link 8's driver maps every phase error after `make` through `ow_world::pools::after_partial_writes` (no new
     helper). This matches `ApplyInternalSlotEffectModifiers` (#483), which runs in the same `InitializeInternal`.
  2. **The test count** is 15, as above.
  3. **The class defaults** have a store-backed fixture.
  4. **The schema** compares the struct's own name exactly (`WeaponDefinitionData`, `AttributeExpressionData`).
