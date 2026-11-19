## 2026-11-19: Rust pickup natives and slice pickup drivers in Off

AI-assisted (Claude Code). Lane IMPL-PICKUP, implementer (slice links 6 and 7). Number 68 was given by the hub.
- **The branch.** It is stacked on `origin/research/rust-pickup` 7e42354b (#447 with its review r2, the "Off
  contract").
- **The source.** NATIVE_PICKUP.md at that head. The key and touch parts were taken only from it, after its
  touch-prerequisite fix and its Off contract.
- **Clean room.** This lane did not read the natives and opened no `local/analysis/`, Ghidra or REA output. It read
  the installed packages only for native keys, parameter lists, class chains and field names.
- **No dependency, no parsing change.** No real-game session; every rule is **UNVERIFIED** in the running game.
  Reference is unchanged: nothing new is bound there (R3).

- **New module `ow_inventory::pickup`** (Off only, registered by the inventory pack):
  - **`WillowPlayerController.GetInventoryPawn()`:** the controller's pawn, None without one. Per the Off contract,
    only a plain player pawn (a `WillowPlayerPawn`) is supported, since it answers as no vehicle. Any other pawn is
    `NotYet { NotRead }` (open question 4).
  - **`DroppedPickup.AddToNavigation()`:** does nothing.
  - **`WillowGlobals.RemovePickup(Pickup)`:** `NotYet { NotRead }`, decided before anything changes. The note does not
    name the globals' pickup list, the holder, or how the two global events are delivered.
  - **`DroppedPickup.TouchPickupTrace(StartLoc, EndLoc)`:** `NotYet { NotRead }`, per the Off contract. What blocks
    the trace is not read, so a touch's validity cannot be settled in Off.
  - **The script runs as installed:** the touch state, `GetCurrentPickupable`, `PickupSomething`, `PickupPickupable`,
    `GiveTo`, `AddInventory`, the destroy. `PickupQuery`'s unsupported dispatch targets (Off contract) are not
    guarded here. Every path to them passes the touch trace first, which refuses, so they are unreachable in Off
    until that trace is read.
- **A fix in `ow_inventory::definition`** (lane FIX-POOL-SPAWN-B's module, decision 2026-11-19-29).
  - **The bug:** its three natives (`GenerateUniqueID`, `ComputeMonetaryValue`, `GetManufacturer`) were bound by
    `WillowGame.WillowInventory.*` paths. The packages declare `WillowInventory` in `Engine`, so on the installed
    data they bound nothing; only the toy packages had them.
  - **The fix:** the paths are now `Engine.WillowInventory.*`. A new package test checks they bind on an Engine
    class.
  - **The effect:** on the installed gate, link 8's row is unchanged. Link 6's `make` now gets past
    `GetManufacturer`.
- **Slice gate drivers** (`ow-cli` `slice_pickup`, `slice_gate`; `tools/check_slice_run_off.py` knows both drivers).
  - **Link 6, `pickup`:** steps `make`, `touch`, `key`. Evidence: the gun is the pawn's own afterwards.
  - **Link 7, `pickup-aimed`:** steps `make` (with a decoy), `touch` (the decoy), `look`, `key`. Evidence: the aimed
    gun is the pawn's and the decoy is not.
  - **What `make` does:** it takes `--equip-run`'s gun and owner. The gun leaves its owner, and its
    `DroppedPickupClass` is spawned and initialised from it with the installed `InitializeFromInventory`. That is
    what section 1 says the drop does.
  - **Harness stand-ins, none a rule:**
    - the owner links;
    - the spawn in place of `DropFrom`, whose own script stops at the unbound `WillowInventory.GetInventoryDefinition()`
      before any pickup exists;
    - `bNoCollisionFail` on that spawn (the headless world has no collision);
    - the spawn place.
  - **`look`:** the per-tick targeting needs the player's view point and direction. The headless world has no camera,
    so it is `NotYet { LayerAbsent }`, never a fitted choice. This is the same kind of answer as the hub's anim-tree
    ruling for `GetSMNode`, and it is open to the hub.
  - **Terminal refusals:** a refusal after a step has already changed what the take writes (the gun's `Owner`, the
    pickup's `Inventory`) says "stopped after partial writes", the hub's rule.
- **Verified (automated, local, pc-hummer).**
  - **Synthetic tests** (`ow_package::testkit` packages):
    - `ow-inventory` `pickup`, 6 tests: `GetInventoryPawn` (none, the player pawn, a vehicle and a plain pawn
      refused); `AddToNavigation`; `RemovePickup` refused with nothing changed; `TouchPickupTrace` refused; the
      definition natives bound on the Engine class; Reference binds none.
    - `ow-cli`: both links' drivers, steps and evidence.
    - `tests/check_slice_run_off_test.py` OK.
  - **Installed `--slice-gate`**, strict (base 7e42354b against this branch):
    - before:
      - `| 6 | pick_up_gun | notyet | none | | | SLICE_RUN_PLAN.md | no headless driver: no pickup path exists in code; ... |`
      - `| 7 | pick_up_aimed_not_nearest | notyet | none | | | SLICE_RUN_PLAN.md | no headless driver: the use-target choice ... |`
      - 0 / 14 / 0 overall;
    - after:
      - `| 6 | pick_up_gun | error | pickup | make | WillowInventory.GetInventoryDefinition() | | |`
      - `| 7 | pick_up_aimed_not_nearest | error | pickup-aimed | make | WillowInventory.GetInventoryDefinition() | | |`
      - 0 / 12 / 2 overall; every other row byte-identical.

    Both links stop in `make`, in the installed `InitializeFromInventory`, at the unbound
    `WillowInventory.GetInventoryDefinition()`. No note reads that native: NATIVE_INVENTORY_EQUIP.md calls it a
    "trivial field read" without saying which field. So the touch (the trace `NotYet`), the look and the key are not
    reached on the installed data yet.
- **Gates:** workspace build clean; workspace tests 2,096 passed / 0 failed / 45 ignored; `crate_edges_test.py` OK;
  decisions check OK; leak gate clean. ctest and verify_packages were not rerun (no C++ change).
- **Open.**
  - For an analyst: `WillowInventory.GetInventoryDefinition` (the next stop of both links); `TouchPickupTrace`'s
    blocking rule; `RemovePickup`'s list and events; the targeting's open question 1.
  - For the hub: the view point for `look` in a headless world.
