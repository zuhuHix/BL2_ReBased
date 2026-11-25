# The inventory chain iterator: InventoryManager.InventoryActors (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-INVENTORY-ACTORS (decision 2026-11-25-60), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/invactors/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule
here is UNVERIFIED in the running game.**

**Why.** With #710 (the pickup world's game info) and #713 (`GetInventorySpaceRequirement`), link 6's next stop after
the skeletal bounds is the unbound `InventoryManager.InventoryActors(class, object, bool)`. Lane E's throwaway stack
showed it. `native_index_find.py InventoryActors` has no row. Its sibling `WillowInventoryManager.ItemActors` has one
(NATIVE_INVENTORY_CONTEXT.md section 3) and is bound in Off (`ow-inventory` `manager.rs`, decision 2026-11-19-67).

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`; the export list of `Engine.upk`):
  - `Engine.InventoryManager.InventoryActors(class BaseClass, out Inventory Inv, optional bool bOnlyReadied)` is a
    native iterator;
  - its script callers are the engine's `PrevWeapon`, `NextWeapon`, `GetBestWeapon`, `FindInventoryType`,
    `DiscardInventory` and `HandlePickupQuery`. In `WillowGame` the callers are `WillowInventoryManager`'s
    `HandlePickupQuery`, `GetWeaponList`, `GetEquippedWeapons`, `FillWeaponSwitchSlots`, `EquipWeaponFromSlot`,
    `CopyInventory`, `SafelySetQuickSlot`, `IsInventoryItemInInventory` and `AssignNextAvailableDefaultQuickSlot`.
- **Link 6's site**, `WillowInventoryManager.HandlePickupQuery(Pickup)`:
  1. `HasRoomInInventoryFor(Pickup)` false: answer false;
  2. the pickup's inventory's own `HandlePickupQuery(owner pawn, Pickup)` false: answer false;
  3. `foreach InventoryActors(class'WillowInventory', Inv)`, with the flag omitted: when `Inv.DenyPickupQuery(Pickup)`,
     answer false. The loop's out variable is the same local that held the pickup's inventory;
  4. then `foreach ItemActors(class'WillowItem', Item)`, the same test;
  5. answer true.
- **The bodies the loop calls:**
  - `Inventory.DenyPickupQuery` and `Weapon.DenyPickupQuery` deny when the pickup's inventory has exactly the
    loop item's class;
  - **`WillowWeapon.DenyPickupQuery` always answers false;**
  - `WillowItem` has its own, which step 4 reaches.
- **The native:** Ghidra 12.1.4 headless, read-only, under the machine lock. The exec was read, with `ItemActors`'s
  beside it for comparison.
- **Field names:** the local copy of `tools/ghidra/class_layout.py` places `InventoryManager.InventoryChain`,
  `Inventory.Inventory` and `Inventory.bReadied` exactly where the native reads them. The fallback class for a None
  `BaseClass` comes from the class registration the native calls, which names `Inventory` in `Engine`.

## 2. What InventoryActors yields

On the manager the call runs on:
1. **The arguments.** `BaseClass`, then the out variable, then `bOnlyReadied` (omitted gives false).
   **A None `BaseClass` means `Engine.Inventory`.**
2. **The walk.** From the manager's `InventoryChain`, through each entry's `Inventory` link (the "next" field), in
   chain order. This is the weapon chain. The items' `ItemChain` is a separate chain, which `ItemActors` walks.
3. **The filter.** An entry is given when its class is `BaseClass` or below it, and, with `bOnlyReadied` true, when its
   `bReadied` is set.
4. **The cap.** The walk counts **every entry it examines**, given or skipped, over the whole iteration (not per
   step). Once it has examined 100, the iteration ends at the next entry, even if matching entries remain. The
   101st entry is not tested.

   **This differs from `ItemActors`**, which counts only the entries it skips. Neither walk checks for a chain that
   comes back to an entry it already passed. In this native the cap is what ends such a loop, so an entry can be
   given more than once.
5. **When it is read.** Lazily, one entry per pass. Before the loop body runs for an entry, the next link is already
   taken from that entry. So a body that unlinks the current entry does not lose the walk. A body that changes later
   links is seen.
6. **The end.** The out variable is set to None and the loop ends. That happens when the chain ends, at the cap, or
   at once for an empty chain.

**Side finding for `ItemActors`** (UNVERIFIED): its None-`BaseClass` fallback, "a class the engine looks up in
`Engine`, not identified" (NATIVE_INVENTORY_CONTEXT.md section 3; `manager.rs` refuses it as `NotRead`), is the same
registration. It is **`Engine.Inventory`**.

## 3. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `Engine.InventoryManager.InventoryActors` as an iterator in Off, beside
`ItemActors` in `ow-inventory` `manager.rs` (`register_iterators`), under its declaration's own native key. Reference
leaves it unbound.

**The port's iterator model.** The VM's iterator natives (`Vm::register_iterator`, `IteratorResult`) give the whole
list when the loop starts: `ItemActors`, `AllParts` and the engine iterators in `natives_core.rs` all do. Use that
model here:
1. **The arguments.** `BaseClass` as `ItemActors` reads it, with the VM's null forms meaning None, and None meaning
   `Engine.Inventory`. Not a class: `VmError::Script`. `bOnlyReadied` as a bool, false when omitted. The out
   variable is parameter 1.
   - **The fallback class is found load-free:** `vm.space.classes.by_path("Engine.Inventory")`, as `manager.rs`'s
     `is_a` finds classes. Never `Vm::find_class`: it can load class code, and `class_code_loads` counts that as a
     write;
   - a class not laid out yet (`by_path` answers None): `NotYet { NotRead }`, "InventoryActors with a None BaseClass:
     Engine.Inventory is not laid out at set-up (a class load is a write)".
2. **The receiver.** It is an `Engine.InventoryManager` or below it, declaring `InventoryChain` as an object
   reference. Otherwise `VmError::Script`.
3. **The list.** Walk `InventoryChain` through each entry's `Inventory`:
   - examine at most 100 entries, counting each one, matching or not;
   - give an entry when it is `BaseClass` or below it, and, with the flag, when `bReadied` is set. `bReadied` is read
     only when `bOnlyReadied` is set **and** the class matches, as the `ItemActors` port reads it;
   - a missing `Inventory` or `bReadied`, or one of the wrong kind, on an entry that is read: `VmError::Script`
     naming it;
   - **no revisit refusal.** A chain that loops is given as the cap allows, entries repeating. This is the one place
     the rule differs from `ItemActors`'s port;
   - an entry that is a store handle not loaded yet: `NotYet { NotRead }`, "InventoryActors over an inventory entry
     not loaded at set-up (a load is a write)". The check is `DefStore::is_unloaded` (or the slotless check, as
     `ow-attr` `eval.rs`'s `slotless_refusal` makes it). It runs **before any field read of the entry**, its class
     test included, and never calls `ensure_loaded`.
4. **The snapshot** (as `ItemActors`, UNVERIFIED): the list is taken whole when the loop starts. The game reads each
   next link just before running the body. The two agree unless the body changes the chain past the current entry.
   Link 6's body (`DenyPickupQuery`) changes nothing.

**`ItemActors`'s None `BaseClass`** may be answered the same way, as `Engine.Inventory`, in the same lane or a
follow-up. It is the hub's call.

**What link 6 would meet** (a prediction, not a run). The weapon chain of the toucher's inventory manager is
walked with `WillowInventory`:
- an empty chain gives nothing;
- each held weapon is a `WillowWeapon`, whose `DenyPickupQuery` answers false;
- either way, the loop does not deny, and `HandlePickupQuery` goes on to `ItemActors(class'WillowItem', ...)`. That is
  bound in Off with a non-None class.

So, with no item denying, the query answers true, and the touch goes on.

## 4. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each refusal leaves #518's three counters and the name table unchanged:
1. a manager with chain A (a toy `WillowWeapon`), B (a toy `Inventory` that is not a `WillowInventory`), C (another
   `WillowWeapon`), walked with `WillowInventory`: A then C;
2. the same with `bOnlyReadied` true and only C readied: C;
3. a None `BaseClass`: every `Engine.Inventory` entry, A, B and C, with `class_code_loads` unchanged. With
   `Engine.Inventory` not laid out, the fallback's `NotRead`, and `class_code_loads` unchanged;
4. an empty chain (`InventoryChain` None): nothing, and the out variable None after the loop;
5. **the cap:** a chain of 150 matching entries: the first 100 only. A chain of 99 non-matching entries then 5
   matching ones: only the first matching one (the 100th examined);
6. **a looping chain:** A → B → A ... with `Inventory` as the class: entries repeat until 100 have been examined, and
   there is no refusal;
7. `InventoryChain` or an entry's `Inventory` declared as something other than an object reference: `VmError::Script`;
8. a lazy entry handle on the chain: the not-loaded `NotRead`, nothing loaded, and `class_code_loads` unchanged;
9. a body that unlinks the current entry: the loop still visits the rest (the snapshot, the same as the game for this
   case);
9a. **a body that unlinks a later entry** (the port's model, labelled so): with chain A, B, C, the body on A sets A's
    `Inventory` to C. B is still visited, because the list was taken when the loop started. It is skipped only if B
    has been destroyed, since `IteratorNext` skips destroyed objects. In the game, B would not be visited;
10. `WillowInventoryManager.HandlePickupQuery` on a toy manager whose chain holds a `WillowWeapon`: the loop does not
    deny, and the call goes on to `ItemActors`;
11. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above. The walk, the filter, the cap and the fallback class are read. The snapshot is
   the port's model, not the game's.
2. **Not read:** how entries join `InventoryChain` (head or tail), so the order in which items enter it is not known.
   The iterator gives chain order, whatever that is.
3. **Not run:** what link 6's toucher holds in `InventoryChain` in the slice world.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. With Maya holding two weapons, hook `WillowInventoryManager.HandlePickupQuery` while she touches a gun. Expect the
   `InventoryActors` loop to visit both, in `InventoryChain` order, and to deny nothing.
2. `foreach InventoryActors(None, Inv)` on her manager gives every chain entry.
