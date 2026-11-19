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
- **Review r2 of #452** (a commit on top). Where this section and the text above differ, this section holds.
  - **The `Pickup` state.** The driver enters it explicitly through the pickup's own `GotoState`. Spawn skips the
    initial state in a world that has only a placed `WorldInfo`. The driver checks the state before `Touch`, so the
    state's handler is the one that runs. A new package test has a toy pickup with a global and a state `Touch`.
  - **Writes and errors.** The driver keeps `VmError`s until the gate grades them. Its first write is in `make` (the
    gun and its owner, Maya's place, the detach, the spawns, the decoy). From there on only a `NotYet` gets the
    "stopped after partial writes" prefix, with its kind and record kept, and any other error passes through
    unchanged. New tests cover a late `NotYet`, an ordinary error, a mark applied once, and nothing marked before a
    write.
  - **Preflight.** Before any write, `make` checks:
    - the weapon class's `DroppedPickupClass`, read from its class default object;
    - its `Owner` and `Instigator` fields;
    - the pickup class's `InitializeFromInventory` signature (object, object, bool) and its `Pickup` state;
    - the binding of `WillowInventory.GetInventoryDefinition`, which the fixed fixture reaches. An unbound one is
      the VM's own unbound-native error.
  - **Link 7's fixture.** One player: link 7's decoy gun has no owner of its own (`equip_run::prepare_unowned`).
    Maya is at the world origin facing +X, her gun's pickup 150 uu ahead and the decoy 45 uu to her side. `look`
    first checks that her `CurrentTouchedPickupable` is the decoy, then stays the camera `NotYet`.
  - **`GetInventoryPawn`.** It supports only the concrete `WillowPlayerPawn` class; a subclass is the named `NotYet`
    (new test). A failed class lookup is that error, not the refusal.
  - **Evidence.** The evidence comes from successful reads only: the gun in Maya's inventory chain, its pickup
    destroyed, and the decoy still a live pickup holding its own gun.
  - **Gates after r2:**
    - `ow-inventory` 78 / 0 / 0; `ow-cli` 90 / 0 / 1 and 2 / 0 / 0; `check_slice_run_off_test.py` OK;
    - installed `--slice-gate`, strict, under the machine lock: 0 / 12 / 2. Links 6 and 7 are `error` at `make` with
      the unbound `WillowInventory.GetInventoryDefinition()`, now from the preflight, before any write. Only row 6's
      scope text changed from r1.
- **Review r3 of #452** (the last round; commits on top).
  - **Typed fixture preparation.** `equip_run`'s preparation chain keeps `VmError`s (`prepare_line_typed`,
    `prepare_unowned_typed`); the String forms are wrappers for `--equip-run`. The first successful mutation (the
    gun's creation) sets a flag, and from there `make` marks a `NotYet` as "stopped after partial writes", kind and
    record kept. That covers one from the accuracy pool's setup, after the gun and its owner exist. Before r3,
    `prepare_line` flattened it to text and it lost both the mark and its kind.
  - **Typed grading.** The gate grades the driver's errors by variant (`slice_gate::classify_error`), not by their
    text. A new test drives a real `NotYet` through `make`'s fixture building (`build`, with a preparation that
    really writes) and that grading, once after a mutation and once before one.
  - **Preflight.** It now propagates a failed lookup of `WillowInventory.GetInventoryDefinition` and requires it to
    be declared native and bound.
  - **Decoy evidence.** The decoy must also read as still enabled for pickup (`bPickupable`).
  - **Gates after r3:** `ow-cli` 92 / 0 / 1 and 2 / 0 / 0; `check_slice_run_off_test.py` OK. Installed `--slice-gate`,
    strict, under the machine lock: 0 / 12 / 2, links 6 and 7 unchanged (`error` at `make`, the unbound
    `WillowInventory.GetInventoryDefinition()`).

