## 2026-11-19: ApplyInternalSlotEffectModifiers: a write-free preflight, and the mark only after a write

AI-assisted (Claude Code, Opus 5.5). Lane FIX-483-FOLLOWUP, implementer: the hub's follow-up (F1 to F6) to landed #483
(decision 2026-11-19-97). The decision number is pending from the maintainer (three digits); NNN is a placeholder.

- **Source:** NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md (on main) and the hub's findings and rulings. This lane did not
  analyse the native (its analyst was ANALYST-APPLY-SLOT-EFFECT-MODIFIERS, #471); it opened no `local/analysis/`,
  Ghidra or REA output. No real-game session: every rule stays **UNVERIFIED** in the running game.
- **The hub's rulings:**
  - the preflight writes nothing: no noun entry, no generator state, no heap object (definition loads stay
    allowed, the note's ruling);
  - the "stopped after partial writes: " prefix goes on only after an actual write.

  The second ruling **supersedes decision 2026-11-19-97's "every W1 failure is marked"** (its r1 and r2 lines are
  labelled superseded there). It also drops 97's r2 Off assumption that evaluating a noun's `DefaultValue` writes
  nothing, since the probe no longer evaluates it.

**What changed** (Off only; Reference binds nothing, as before):
- **F1, the noun probe** (`ow-attr` `value_resolvers/noun.rs`, `eval.rs`): for a missing entry it validates
  `DefaultValue` with a new `preflight_init` and does not evaluate it.
  - The checks: the struct's fields; a `BaseValueAttribute` through the existing `preflight_read`; an
    `InitializationDefinition` loaded, with its four parts present and its two modes inside their enums.
  - It is shallow. Formula terms, conditions and variance bounds are reached only by evaluating.
  - Execution evaluates the default once, when the add creates the entry.
- **F2, W3** (`ow-inventory` `equip/internal_slots.rs`): each successful record is appended at once to the out
  argument as it stands. From script, that is the caller's storage, read through the alias. With the flag true, the
  record also goes to the live `AppliedAttributeSlotEffects`. The final `set_out` of the buffered array is gone.
- **F3, the preflight order:**
  - the reads that name the handles (slots, the applied list, its records) come first;
  - then P0, then the independent checks (P1, P4), then P8 and P9;
  - an add loads its attribute's context and value chains, and a removal loads its value chain only.
- **F4, the W1 mark:** a W1 refusal is marked only if the heap or the generator changed since W1 began.
  - A plain attribute's removal notifies before it writes, so its notification's refusal can come with nothing
    written.
  - A notification's script may also write before it refuses.
  - From W2 on, every `NotYet` is marked, as before.
- **F5, P1:** the third parameter must be an `out` parameter, and its records must be `AppliedAttributeEffect`, with
  the shape check as before. A non-out parameter is now refused before any write, not at `set_out` after them.
- **F6, the replicated-slot probe:** `ValueResolver::probe_storage` and `check_modifier_chain` now take the operation
  (`ProbeOp::Add`, or `ProbeOp::Remove(modifier)`).
  - The noun probe refuses a replicated entry only when the operation would rewrite it: an add always, a removal
    only when the entry's stack holds the modifier.
  - The write-time guard in `rewrite` is kept.

**Two small additions outside the native:**
- **`ow_object::heap::Heap::mutations()`**, a change count, for F4 (**replaced in review r1 of #518** by the change
  watch below). It exists because a W1 refusal can follow a
  write made anywhere, notification script included, and the heap is the one place every such write passes.
  - Every creation, every free and every mutable borrow counts: `get_mut`, and with it every write and
    `slot_mut`.
  - A borrow that changed nothing still counts, so the count can say "changed" for nothing. It never misses a
    change, so a refusal is never left unmarked after a write.
  - Its `Debug` form is constant, so tests that compare a heap's `Debug` text are unaffected. No dependency was
    added. It has its own unit test.
- **`ow_vm::api::Arg::aliasing_property`**: the out-argument alias a script call makes for an instance variable, built
  without bytecode. F2's test uses it to call the native through `NativeCall::new` as script would.

**Tests**, all synthetic. Each new one was first run against the unfixed code and failed for its finding's reason;
the two F3 tests were re-run with the old order restored:
- F1: a nested noun default and an enabled variance, each before a later typed `NotYet`, with noun state and RNG
  unchanged, plus a control showing one draw on creation;
- F2: a two-slot failure through the alias, where slot 1's notification appends a sentinel and then refuses, so slot
  0's record and the sentinel both survive; and without the refusal, the sentinel is not overwritten;
- F3, on the synthetic store: a broken attribute with no modifier class gives the store's own `Decode` error; a
  removal-only attribute with an unloadable, unused context resolver goes through and leaves that resolver unloaded;
- F4: a plain notifying attribute whose callback refuses at once gives an unprefixed `NotYet`, kind and record kept;
- F5: with OUT cleared, a data error with the backup list, the value and the heap unchanged;
- F6: a replicated entry without the modifier stays untouched, and W2 empties the list. In `ow-attr`, the replicated
  probe test covers add, a held removal and an absent removal;
- `ow-object`: the change count.

**Verified:**
- `cargo test --workspace`: 2,447 passed, 0 failed, 46 ignored.
- Installed Off `--slice-gate` strict: 0 ok / 5 NotYet / 9 errors. Link 8 (`equip_gun`) passes 1 of 2 driver steps
  and stops in `init` at the unbound `WillowWeapon.CacheWeaponCard()`, past this native, as #483 recorded.

**Review r1 of #518 (Sol high on 7c8218d0), fixed in one commit on top.**
- **The hub's ruling, recorded here:** there is no isolated or read-only store view. P0 may load, **a load counts as a
  write** (the spawn's `loaded` pattern), and a refusal after any load is marked through
  `ow_world::pools::after_partial_writes`. There is **one write-tracking path** for the whole call: a refusal is marked
  when P0 loaded anything, when the heap changed since the call began, or when the generator was drawn. It replaces
  both F4's W1-only mark and the unconditional mark after W2. For this native it supersedes the note's "definition
  loads are not writes".
- **1 (high), the lazy load:** `load_attribute` sets `loaded` before each load. A load that succeeds, followed by a
  later refusal, is now marked. P9's nested loads change the heap, so the heap check catches them.
- **2 (medium), the change count:** `Heap::mutations()` counted mutable borrows. It is replaced by a change watch:
  `Heap::watch_changes`, `Heap::changed_since` and `Heap::end_watch`.
  - While a watch is open, the first mutable borrow of each object keeps a copy of it, and `changed_since` compares
    each touched object with its copy. Creations and frees are counted too.
  - So a borrow that changes nothing, or writes back the same value, is no change. A NaN slot compares unequal, so the
    answer can only err toward "changed".
  - Outside a watch, nothing extra happens. `Object` gained `PartialEq` for the comparison.
- **3 (medium), the shallow default check:** `preflight_init` now walks every field the evaluation can reach, without
  evaluating it: the enabled formula's terms; the enabled conditional's entries (their expressions' attribute operands
  through `preflight_read`), its true values and its default; the enabled variance bounds; the enabled range limits.
  It recurses into each one as initialization data, and a definition is checked only once.
- **4 (medium), the record struct:** P1 now compares the record struct with `APPLIED_STRUCT` by identity.
- **5 (medium), the modifier byte:** each qualifying slot's `ModifierType` byte is kept raw while reading and checked
  after P0.
- **6 (low), the tests:**
  - the F2 notification also appends a sentinel to the backup list, and it survives slot 1's append;
  - the F3 removal-only fixture holds a real stack entry (a toy resolver over a runtime stack attribute), which the
    removal takes off, while the unused context resolver stays unloaded.

**New tests in r1:**
- a lazy load that succeeds, then a refusal: marked;
- an unloadable attribute with a bad type byte: the store's own load error;
- a no-op callback, then a `NotYet`: unprefixed; a callback that writes, then a `NotYet`: marked;
- an unsupported resolver in a variance bound: refused before W2 (no modifier made, no draw);
- a same-named record struct: refused in preflight;
- the heap's change watch, in `ow-object`.

The resident-only preflight refusals stay unprefixed, with noun state and RNG unchanged (the two F1 noun tests).
Mutations run locally:
- with the watch counting any borrow, the no-op test fails;
- with the variance bounds left unchecked, the refusal comes after W2, marked.

**Verified (r1):** `cargo test --workspace`: 2,452 passed, 0 failed, 46 ignored.
