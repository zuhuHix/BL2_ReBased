## 2026-11-19: The pickup's inventory accessors in Off

AI-assisted (Claude Code, Opus 5.5). Lane LINKS-6-8-NEXT, implementer. The hub assigned -103; filed as -40 because the
decision tool allows two digits only.

- **Sources:** `docs/verification/NATIVE_INVENTORY_EQUIP.md`, "Pickup interface accessors" (analyst lane G12,
  2026-10-05, not this thread). No native code or analysis output was opened.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged rules:** no dependency was added, no bounds check was loosened and no existing `NotYet` guard was
  relaxed.
- **The base:** `fix/rust-willow-globals` (#476, a0653806, after its rebase onto main), the topmost PR links 6 and
  7's stop needs. The stop is reached only once #476's globals natives and #462's `GetInventoryDefinition` are both
  there.

**How the stop was found.** On a local, unpushed integration of main a629f56f with #455 (801e956b), #462 (be10d201)
and #468's own three commits, the installed Off `--slice-gate` gave:
- links 6 and 7 at `make`: the unbound `WillowGlobals.GetWillowGlobals()`;
- link 8 at `init`: the unbound `WillowInventory.ApplyInternalSlotEffectModifiers`.

Both are bound by open lanes (#476, and -97 on #468). With those added too, the stops were:
- links 6 and 7 at `make`: the unbound `DroppedPickup.GetPickupableInventoryDefinition()`;
- link 8 at `init`: the unbound `WillowWeapon.CacheWeaponCard()`.

**What changed** (`crates/ow-inventory/src/pickup.rs`, Off only; Reference binds nothing, as before).
- `DroppedPickup.GetPickupableInventory()`: the pickup's `Inventory`, as stored.
- `DroppedPickup.GetPickupableInventoryDefinition()`:
  - the note's "that inventory's definition", read as the inventory's own `GetInventoryDefinition()`, dispatched
    through the VM on its class, so the callee answers for itself (an unbound one is `UnboundNative`, unchanged).
    UNVERIFIED that the native asks that virtual;
  - where the note is silent, a named `NotYet { NotRead }`: a pickup with no `Inventory`, and an inventory class that
    declares no `GetInventoryDefinition`.
- Both write nothing.

**Installed Off `--slice-gate`** (the integration above, with this change): links 6 and 7 get past the accessor,
still in `make` (0 of 3 and 0 of 4 driver steps passed). They stop at the unbound stock native
`SkeletalMeshComponent.SetAnimTreeTemplate(object)`. Link 8 is unchanged: 1 of 5 driver steps, at `CacheWeaponCard`.
No note reads either native, so each needs an analyst lane.

**Tests:** one new test in `pickup_tests.rs` on invented packages, covering:
- the stored inventory and the dispatched definition;
- both named `NotRead`s;
- an unbound callee's `UnboundNative`;
- nothing written.

Reference still binds none of the six.
