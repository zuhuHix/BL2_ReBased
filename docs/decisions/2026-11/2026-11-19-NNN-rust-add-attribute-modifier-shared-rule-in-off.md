## 2026-11-19: `AttributeDefinitionBase.AddAttributeModifier` is one shared Off rule with a load-free preflight

AI-assisted (Claude Code). Lane IMPL-ADD-ATTRIBUTE-MODIFIER, coded from NATIVE_ADD_ATTRIBUTE_MODIFIER.md section 3 at
82e8d02e (#607) alone. The analyst was another lane, so the clean room holds. Stacked on IMPL-RESOLVE-CONTEXT
(`fix/rust-resolve-context`). **Every rule is UNVERIFIED in the running game.** Off only. Reference is unchanged: the
equip pack binds nothing there, as before.

- **The shared rule.** `ow_attr::AttributeSystem::add_modifier_native` (`ow-attr` `add_native.rs`) runs the note's
  steps 3 to 9 in order:
  - A1, an unloaded definition: `NotYet { NotRead }`;
  - A0, an unloaded context: `NotYet { NotRead }`;
  - the chain's shape. A malformed one is a `VmError::Script`. An empty chain, or a None first entry, answers false;
    only the entries before the first None are checked;
  - A2, any checked entry an unloaded value resolver: `NotYet { NotRead }`. A2 is checked over every entry before A3;
  - A3, any checked entry's class not registered: the registry's own `NotYet { NotRead }`;
  - the storage probes;
  - the write (`effects::add_modifier`, answering `succeeded()`). Once the write starts, any `NotYet` is terminal,
    through `after_partial_writes`.
- **The load-free probes.** `eval::load_free` is a new scoped guard in `ow-attr`. Inside it, `eval::ensure_loaded` on
  an unloaded definition handle refuses `NotYet { NotRead }` ("a default value's handle <name> is an unloaded handle
  (a load is a write)") and loads nothing. Outside it nothing changes. The probes are today's `check_modifier_chain`
  run inside that guard, so a missing noun entry's default stays load-free at any depth. The text names the handle,
  not the struct field the note's wording has, because the guard sits below the field reads.
- **The wrapper.** `ow_inventory::equip::natives::add_modifier_native` does steps 1 and 2:
  - the receiver check, shared with `ResolveContext`'s wrapper;
  - `Context` and `Modifier` must be object references, else a `VmError::Script`;
  - either one None answers false before the definition is read;
  - a non-bool flag is false.
- **Two bindings, either order.** The equip pack binds the wrapper through its keep-existing check
  (`bind_add_modifier`), and its old add native is removed. The action-skill pack binds the same wrapper through
  `ow_combat::hit::bind_fn`, next to `ResolveContext`. `RemoveAttributeModifier` stays unbound in skill worlds (the
  note's open question 4).
- **#523 and the replicated test.** The smaller of the two choices: test 2b, a replicated strategy, is `#[ignore]`
  with the reason "needs #523". It is not stacked on #523.
- **Tests on invented data:**
  - `ow-attr` (`noun_tests`): tests 1, 2a, 3 (the same modifier twice), 4, 5, A3, 8 and 9, plus 2b ignored;
  - `ow-inventory` (the wrapper, a synthetic package store): test 3's None cases and non-bool flag, A1, A0, A2, the
    mixed chain, test 7 (an unloaded default `InitializationDefinition`), the script errors, and Reference;
  - `ow-skills`: test 10, both registration orders.
  - Every refusal before the write checks #518's counters unchanged, and the handle still unloaded.
  - The property add's change notification is `storage.rs`'s existing rule, not changed here and not re-tested.
- **Gates:**
  - `cargo test --workspace -j 2`: 2,762 passed / 0 failed / 50 ignored (12 new, 1 of them ignored). Crate edges OK.
    Clippy has no hits in the touched files;
  - **the installed equip run** (50 weapons, Off): identical to main and to IMPL-RESOLVE-CONTEXT. No phase reaches the
    add's new refusals, because init stops earlier on every weapon;
  - **the installed Off strict `--slice-gate`**, under the machine lock: 3 ok / 10 NotYet / 1 error, no row moved.
    Link 8 is an error at `init` (steps `make`, `init`), at the unbound `WillowWeapon.RecomputeAttributeBaseValues()`.
    Link 9 is NotYet at `cast` (steps `spawn`, `cast`), at the noun resolver's strategy-2 refusal (#523), before the
    add.

### Review r1 of #616 (FIX: tests and docs; the code matches the note)

AI-assisted (Claude Code, Claude Opus 5.5). Off only, **UNVERIFIED** in the running game. This round merges #615's
r1 (`fix/rust-resolve-context` @ 847cb028) first.
1. **The chain is checked by its declared type,** as #615's r1 does. An empty `ValueResolverChain` declared as an
   array of ints is `VmError::Script` (`ToyIntChainDefinition` in the wrapper tests).
2. **A0 to A2 name the full object path:** the store's record for an unloaded handle (`DefStore::handle_path`), and
   the outers for any other object.
3. **The write's terminal rule is tested.** A toy value resolver passes its probe and refuses `LayerAbsent` in its
   storage lookup. It sits second in the chain, after a noun resolver that writes. The answer keeps its kind, carries
   "stopped after partial writes: " exactly once, and the first entry's add stays.
4. **`load_free` says what it covers.** The context resolvers' own `ensure_loaded` now runs the same check
   (`eval::load_free_refusal`). The doc names the paths that still bypass the scope: the black-market upgrade's
   `load_for_write` (a write path) and the balanced and inventory context resolvers' `find_class`. The add's probe
   reaches neither. A unit test covers the scope after an `Err` and after nesting. **There is no test of the
   context-resolver path itself:** the fixture's Pawn context resolver reads no field, so its walk never loads.
5. **Test 1 checks the notification.** The property add notifies once, and not with `bSuppressNotify`. The test world's
   `InstantHitDamage` now carries the notify flag.
6. **Tests 8 and 9 count generator draws** (`CrtRand::draws`, #518's counter), not the stream requests.
7. **Test 2b asserts the mirror:** one entry in slot 0, `NumReplicatedNouns` 1, the slot's name and value equal to the
   entry's, and `bNetDirty` set. It stays `#[ignore]` here, because this branch's base predates #523. On a local
   throwaway merge with main (which has #523) it passed: `ow-attr` 289 passed, and 2b with `--ignored` 1 passed.
8. **Hub ruling on the argument order:** the argument-shape checks stay before the None cases. A wrongly typed
   argument is a script error whatever its value, so `Context` or `Modifier` holding an int is `VmError::Script` even
   when the other is None.

### Review r2 of #616 (FIX)

AI-assisted (Claude Code, Claude Opus 5.5). Off only, **UNVERIFIED** in the running game.
1. **The argument-order ruling has a regression test.** The malformed-call cases now include (Context None, Modifier
   int) and (Context int, Modifier None). Each is `VmError::Script`, with #518's counters unchanged, so an early false
   for a None argument would fail the test.
2. **A failed lazy handle keeps its full path.** `ow-defs`' `Entry::Failed` now stores the path the handle was made
   for, and `DefStore::handle_path` returns it. A0 to A2 therefore name the full path for a handle whose load already
   failed (one of `is_unloaded`'s handles), not only the leaf. The refusal test adds such a handle, an export whose
   payload is cut short, as the context: "the context Engine.ToyBrokenPawn_0 ...".
