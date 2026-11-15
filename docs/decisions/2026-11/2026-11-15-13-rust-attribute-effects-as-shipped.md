## 2026-11-15: Rust attribute effects as shipped

AI-assisted (Claude Code). Lane W3-ATTREFFECTS of the Rust port fills `crates/ow-attr/src/{effects,storage}.rs`: the
property-level add, remove and set-base of stack and plain attributes, the change notification, the by-name `Object`
natives, the script VM's typed let (`let_base`), `AddAttributeModifier` / `RemoveAttributeModifier` /
`SetAttributeBaseValue` through a definition's value chain, and `AttributeEffect.ApplyAttributeEffects` /
`RemoveAttributeEffects`. Rules: NATIVE_ATTRIBUTES.md sections 1, 3, 4 and 7, read from native code and script.
**Every rule here is UNVERIFIED in the running game**; no real-game session ran. Hub decisions (review r1 of PR #313)
are marked "hub".

**Shipped.**

- The chain rule of architecture 3.5: the value resolvers are asked in order while the previous one succeeded, a None
  entry is a failure, the last resolver asked gives the outcome, nothing is rolled back.
- Stack attributes refuse a modifier object already on the stack, remove every copy, and recompute in the caller's
  `CombineForm`. Plain attributes are edited in place (Scale multiplies raw), and their remove is a successful no-op;
  a respec therefore keeps a plain edit. Every operation computes before it writes.
- `let_base` refuses a value of another kind with `BadData` (the C++ stores it silently), keeps an int exact, and does
  not notify.
- New public items in `storage.rs` (additive, listed in architecture 3.5): `ATTRIBUTE_MODIFIER_CLASS`, `storage_of`,
  `Notified`, and `owner_notification`, the two overrides the record read. `ResourcePool` clamps `CurrentValue` (min
  test first, at or above the max becomes the max). `Skill` sets `bForceRefreshModifiersNextTick` on `Grade`. Every
  other owner answers `Notified::NotRead`, which the world counts with no effect (real-game question 11). Classes are
  matched by class name, and the overrides apply to subclasses (UNVERIFIED). W5-WORLD wires this into the worlds'
  `attribute_changed`.
- `apply_effects` evaluates each value once, at apply time (also for an entry without an attribute, as the script
  does), tries the override context first, and keeps a record for each successful add. It runs in one pass in entry
  order, because a later entry's value may read what an earlier one changed.

**Hub decisions (review r1).**

- **An `Err` from `apply_effects` cleans up (hub).** An `Err` is this port refusing to go on (a `NotYet`, a data
  error), not a game outcome. Before returning it, the call removes every modifier it made that a stack may hold or a
  record names, through the attribute's chain: the records, and also a modifier that a chain stored without a record.
  Then it frees what it made. An `Err` therefore leaves no stack modifier from the call. Plain edits stay, as they stay
  in the game. A modifier whose cleanup removal itself fails is not freed, because a stack may still hold it; the
  original error is returned. The no-rollback rule of 3.5 is about a chain's outcome and still holds on `Ok`.
- **One heap walk per call (hub).** The modifiers no stack holds and no record names are freed with one
  `ObjectSpace::free_many` at the end of the call, on the error path too.
- **Every success notifies (hub).** A successful add, remove or set-base on a notify-flag property notifies, as section
  4 reads ("after a successful add, remove or set-base"). That includes a plain no-op remove and an add of a modifier
  type outside 0..=2, which change nothing. Status: UNVERIFIED; the record does not say whether a no-change success
  notifies in the game.
- **`remove_effects` tries every record (hub).** A record whose removal fails does not stop the others; the first
  error is returned at the end.
- **An attribute flag on a bool is not read (hub).** The census behind the flags (NATIVE_ATTRIBUTES.md, "The model in
  one page") counts flagged float, int and byte properties: 226 float, 24 int and 2 byte stack attributes, 227, 28 and
  2 base companions, plus a few plain fields. It does not say whether bool or other kinds were checked for the flags.
  So `storage_of` answers `NotYet { NotRead }` for a flagged property that holds no number, not `BadData`. W3-ATTREVAL's
  `ObjectPropertyAttributeValueResolver` answers `BadData` for the same case, so a definition-chain add on such a
  property still gives `BadData`; aligning the two is that file's owner's call.

**Other choices, UNVERIFIED.**

- The by-name natives look the name up over the class chain (the record's Open item about inherited properties).
- A stack set-base truncates for an int base.
- Modifiers made by `apply_effects` are named `AttributeModifier` with no number. `AttrWorld` has no hook to make a
  named object (the VM's per-class numbering is out of reach), and nothing in the rules reads the name.

**Open items.**

- An O(1) free of an object nothing references would be a W1-OBJ contract change. `ObjectSpace::free_many` clears
  references to the freed objects across the whole heap, so each call that frees costs one heap walk.
- Modifier lifetime: `remove_effects` frees nothing, because a script copy of the records may still hold them. They
  accumulate until something like a garbage collector exists.
- `Storage` has no variant for `BadassAttributeValueResolver`'s set-base, and `Option<Storage>` cannot tell "not an
  attribute" from "not writable" (`effects.rs` reports both as `Refusal::NotWritable`; the native's false is the same).
  Both are `api.rs` contract questions, which the hub tracks.
- The worlds' `attribute_changed` should call `storage::owner_notification` and count `Notified::NotRead` (W5-WORLD).

**Tests.** 28 tests in `crates/ow-attr/src/{effects,storage}.rs`, on a synthetic world (invented classes and values)
with W3-ATTREVAL's value resolvers. They re-express the `weapon_gen_test.py` part effects in slot order (damage 18,
clip 9, 11.333333015 at stage 8) and its pool-minimum case (3.75), and the `skill_effects_test.py` effect entries. They
also cover the `ResourcePool` clamp, a plain-attribute respec, the error cleanup (entry 2 of 4 failing), and
`apply_effects` and `set_base` end to end through the evaluator.
