## 2026-11-19: The noun providers' arrays and replicated copy in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-NOUN-REPLICATION, implementer. The decision number is pending from the
hub (three digits); NNN is a placeholder.

- **Sources:** section 10 of `docs/verification/NATIVE_NOUN_ATTRIBUTES.md` alone, as revised in review r1 of #516
  (94663dbe; analyst lane ANALYST-NOUN-REPLICATION, not this thread). No native code or analysis output was opened.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added, no bounds check was loosened and no existing `NotYet` guard was relaxed,
  except the two `NotPorted`s section 10.5 replaces.
- **The base:** `fix/rust-link9-attr-get-value` (#515, b0cd5f4f; first written on f90d1a33).

**What changed** (`crates/ow-attr/src/value_resolvers/noun.rs`, Off only; Reference keeps the C++ reading):
- **The classes read, by exact class path:**
  - `WillowPawn`, `WillowAIPawn`, `WillowPlayerPawn` and `WillowPlayerReplicationInfo`: `NounState` on authority
    (`Role` 3), `ReplicatedNouns` always (20 slots, `NumReplicatedNouns`), the change call sets `bNetDirty`;
  - `GearboxMind`: `NounState` on authority, no replicated array;
  - `WillowWeapon`: `NounState` always, no replicated array.
- **The storage-selection table of section 10.5:**
  - strategy 0 keeps today's path where `NounState` is handed over, and has no store where it is not;
  - strategies 1 and 2 on a pawn or a replication info on authority create a missing entry with a replicated slot,
    in the note's order: `d`, then the count read again, the slot's three fields and `bNetDirty`, then the
    `NounState` append. At 20 there is no slot and slot -1;
  - off authority, they read `ReplicatedNouns` alone (the first matching occupied slot, else 0.0; get-base 0.0) and
    write nothing;
  - a mind or a weapon has no store for them.
- **An unread class** (any other class declaring `NounState`, `WillowMind` among them):
  - keeps today's strategy-0 path, with no `Role` test;
  - strategy 1 or 2, or a write to one of its slotted entries, is `NotYet { NotRead }`, "NounAttributeValueResolver: the
    noun arrays a <class path> hands over, and its replicated-copy-changed call, are not read (ReplicationStrategy
    <n>)".
- **Preflight** (steps 1 to 6), write-free and before `d` is evaluated: the key and the provider, the strategy, the
  unread class, the declarations and the `Role` byte, the count, and an existing entry's slot. Each fault is an
  `AttrError::BadData` with the note's text (reaching the VM as `VmError::Attr`, the hub's boundary ruling).
- **The mirror:** a write to a slotted entry on a pawn or a replication info is planned and checked first, then made
  in order: the entry, then the slot's `Value`, then `bNetDirty`. A slotted entry on a mind or a weapon is the step-6
  diagnostic.
- **The write signal:**
  - a creation during a read calls `EvalCx::note_writes()` at its first write;
  - so does every committed add, remove or set-base on a noun (`storage.rs` signals before it commits the planned
    write);
  - the resolver returns its own errors unprefixed.
- **The two `NotPorted`s replaced:** strategies 1 and 2 on a provider, and the mirror. Each becomes the read rule for
  the classes read, or the unread class's `NotRead`.
- **Net mode:** the attribute worlds are standalone, so the client row is not implemented (unchanged).

**Tests** (`noun_replication_tests.rs`, invented objects, 8 new):
- each read class's arrays: creation with a slot on authority for each pawn class and the replication info (strategies
  1 and 2), the mind on and off authority, and the weapon whatever its `Role`;
- the replicated-array-alone read off authority;
- the mirror on set-base, add and remove, with a duplicate add writing nothing;
- the full-array case;
- an unread class (`WillowMind`, an unread pawn subclass), refused for strategy 2 and on today's path for strategy 0;
- preflight faults leaving the heap unchanged;
- the write signal.

Three existing tests now expect the unread class's `NotRead` in place of `NotPorted`.

**Installed Off `--slice-gate`** (release build, under the machine lock): 1 ok / 3 NotYet / 10 errors.
- **Link 9** (`phaselock_enemy`): `spawn` passes; `cast` gets past the noun resolver and stops at the unbound native
  `FlagExpressionEvaluator.Evaluate(object)` (1 of 2 steps).
- **The rest:** links 1, 3, 4, 5, 13 and 14 stop at this base's `GameEngine.GetOnlineSubsystem()`.

**Review r1 of #523, folded in.** The hub's ruling applies: a preflight makes non-loading reads only, and a load
during execution counts as a write. #518's shape (`ProbeOp`, `probe_storage(.., op, ..)`,
`check_modifier_chain(.., op, ..)`, `eval::preflight_init`) is taken over verbatim, so whichever lands second
rebases mechanically; `Heap::mutations` is not needed here.
1. **The probe evaluates nothing.** A missing entry's default is validated with `preflight_init`, not evaluated, so a
   default that reads a second, missing strategy-2 noun creates nothing during the chain check (tested).
2. **Checks before writes.** A creation builds the slot's replacement and the new `NounState` element before its
   first write; a mirror builds the slot's replacement in the plan. Each field replacement is checked against its
   declaration (`AttrError::BadData` when it does not fit). The writes keep the note's order. Tested with a slot whose
   `Value` is not a float: neither the creation nor the mirror writes anything.
3. **Non-loading preflight; counted loads.** In Off the probe does not load a cold resolver or context (it passes it),
   and `check_modifier_chain` loads no resolver handle. Execution loads the resolver and context through
   `eval::ensure_loaded_counted`, which signals the load as a write. Not tested: the synthetic worlds have no cold
   handles.
4. **A write scope for the Off add, remove and set-base** (`effects.rs`, as the read fold): a `NotYet` after a
   committed write is prefixed once, kind kept; a nested default read shares the scope (tested).
5. **Get-base off authority** answers 0.0 without reading a slot (its own lookup intent; tested with a malformed
   matching slot).
6. **The count re-read after `d`** is tested: at 19, the nested noun takes slot 19 and the outer entry gets -1.

Gates: `cargo test -p ow-attr -p ow-inventory --locked -j 4` 442 passed / 0 failed / 1 ignored; `cargo test
--workspace --locked -j 4` 2,480 passed / 0 failed / 46 ignored; `crate_edges_test.py` OK.

**Review r2 of #523, folded in.**
- **The hub's rulings, recorded here.**
  - Definition loads (store objects) count as writes. They are allowed in the Off probe and in execution, and every
    one is tracked.
  - One write tracker covers a whole Off operation (add, remove, set-base, the read fold and the modifier-chain check).
    Any definition load, any committed heap change or any RNG draw counts.
  - A `NotYet` after a tracked write gets the prefix; with none, it is unprefixed.
  - Class and code lookups never load.
  - The refined design uses three monotonic counters, recorded at the start and compared at the end:
    `Heap::write_count` (every committed write, the same value included, plus creations and frees; a plain borrow
    does not count), the store's `load_count` (a handle going from unloaded to loaded or Failed; a cached Failed retry
    does not count) and the RNG's draws.
- **The tracker.** `eval::tracked` records the heap's `write_count` and the store's `load_count` at the outermost scope
  and notes a write when either moved. It replaces r1's non-loading probe, the counted-load helper and #518's
  first-round heap snapshot watch, whose shape was not taken over. The fold, the write scope and
  `check_modifier_chain` all run in it.
- **The RNG counter.** It is not added here: `Rand15` is a trait with several implementers, and #518 is adding the
  counter. The two attribute draw sites note the write at the draw instead. Whichever PR lands second dedupes.
- **Counter names.** `Heap::write_count` and `DefStore::load_count` are the smallest versions, with the names the hub
  gave.
1. **Heap-level checks before every write.** A creation and a mirror check every destination and complete
   replacement with the heap's own address and value checks (`precheck`, using `Heap::check_value`) before the first
   write, as `BadData`. Regressions: a malformed existing entry fails a creation, and a slot holding another struct
   type fails a mirror, each with the heap unchanged.
2. **Tracked loads.** The probe loads again; each load is tracked.
3. **Loads anywhere in the operation are tracked.** That covers the resolver's load in `storage_at` and the
   attribute and default helpers' loads, through the store's counter. Tests:
   - store-backed: a cold context's load is the only write, and add, remove, set-base and the chain check all refuse
     prefixed; with the context already loaded, the same refusal is unprefixed;
   - a noun removal followed by a refusal is prefixed.
4. **Removal intent.** Execution refuses an unread class's slotted entry only where a rewrite is planned: in the plan,
   or in an add before its duplicate test. A removal of an absent modifier answers `NotPresent` in the probe and in
   execution alike (tested both ways).

Gates: `cargo test -p ow-attr -p ow-inventory -p ow-object -p ow-defs --locked -j 4` 624 passed / 0 failed / 5
ignored; `cargo test --workspace --locked -j 4` 2,486 passed / 0 failed / 46 ignored; `crate_edges_test.py` OK.

**Rebased onto #518 (lane REBASE-523-ON-518, 2026-11-20).** The earlier tracker copy is dropped in favour of #518's:
- `Heap::write_count`, `DefStore::load_count` and #523's shallow `preflight_init` are gone. `eval::tracked` now
  compares #518's `Heap::committed_writes` and `DefStore::load_transitions`. The probe uses #518's deep
  `preflight_init`. `heap.rs`, `store.rs` and `internal_slots.rs` are #518's, unchanged.
- **Where the two disagreed, #518's rule is kept.** `check_modifier_chain` is not wrapped in `eval::tracked`: the probe
  may load a resolver handle, it does not prefix, and its caller tracks loads with #518's counters (as
  `internal_slots` does). The store-backed test now checks that rule for the chain check: it refuses unprefixed, and
  `load_transitions` moves. The add, remove and set-base still refuse prefixed.
- **RNG draws.** ow-attr reaches the RNG only through `Rand15`, which has no draws counter, so the two attribute draw
  sites still note the write at the draw. `GameRng::draws` covers callers that hold the VM's RNG.
