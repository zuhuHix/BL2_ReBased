## 2026-11-25: InventoryManager.InventoryActors in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-INVENTORY-ACTORS, on main 40d7d957. It implements
docs/verification/NATIVE_INVENTORY_ACTORS.md section 3 (on main with its review r1 fix, #725). Clean room: this lane did not read the executable. No real-game session was run. **Every rule is
UNVERIFIED in the running game.** Reference leaves it unbound.

**Where.** `ow-inventory` `manager::inventory_actors`, bound as an iterator beside `ItemActors` from
`manager::register_iterators`, under its declaration's own native key, Off only.

**The rule** (the list, read only: nothing loaded, written or interned, no class code loaded):
1. `BaseClass` in any of the VM's null forms is `Engine.Inventory`, found with `space.classes.by_path` (never
   `Vm::find_class`). Not laid out: `NotYet { NotRead }`, "InventoryActors with a None BaseClass: Engine.Inventory is
   not laid out at set-up (a class load is a write)". `bOnlyReadied` omitted is false;
2. the receiver is an `Engine.InventoryManager` or below it (found load-free the same way) declaring `InventoryChain`
   as an object reference; otherwise `VmError::Script`;
3. the walk from `InventoryChain` through each entry's `Inventory`, examining at most 100 entries, given or skipped,
   over the whole walk. The 101st is not tested;
4. per entry, before any field read (its class test included): a store handle not loaded yet (`DefStore::is_unloaded`)
   or a slotless stand-in is `NotYet { NotRead }`, "InventoryActors over an inventory entry not loaded at set-up (a
   load is a write)", never `ensure_loaded`. Given when its class is `BaseClass` or below; `bReadied` read only with
   `bOnlyReadied` set and the class matching. Missing or wrong-kind `Inventory` / `bReadied`: `VmError::Script`;
5. no revisit refusal: a looping chain repeats entries up to the cap (unlike the `ItemActors` port).

**The snapshot** (the port's iterator model, as `ItemActors`, UNVERIFIED): the list is taken when the loop starts,
and `IteratorNext` skips objects destroyed since. A body that unlinks a later entry still visits it in the port; the
game would not. Link 6's body (`DenyPickupQuery`) changes nothing.

**Tests** (`manager/inventory_actors_tests.rs`, invented): section 4's 1 to 11 with 9a, in 11 tests. Tests 3 and 8
assert `class_code_loads` unchanged with #518's counters and the name table. 9, 9a, the out variable of 4, and 10 run
assembled `foreach` loops through the VM over an invented `Engine.upk`; 10 is the shape of `HandlePickupQuery`'s
weapon loop (the `ItemActors` step after it is the existing port's, tested in `script_tests.rs`).

**The slice gate** (installed Off strict, release build, under the machine lock, on main 40d7d957): 5 ok / 2 NotYet /
7 errors, `links ok 5/14`, `step share X = 5.783/14 = 41.3%`, main's totals. Links 6 and 7 do not reach
`InventoryActors`: they still stop earlier, at `touch`, "stopped after partial writes: Actor.GetBoundingCylinder: the
component's template (option B) is not read".

**Gates.** `cargo test --workspace -j 8`: 3,667 passed / 0 failed / 57 ignored. `crate_edges_test.py`: OK.
