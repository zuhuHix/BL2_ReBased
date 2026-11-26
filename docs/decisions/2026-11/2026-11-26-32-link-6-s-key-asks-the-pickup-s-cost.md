## 2026-11-26: Link 6's key asks the pickup's cost

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-LINK67-KEY (queue 0ah), on main c345fdb. Clean room: implemented from
docs/verification/NATIVE_LINK67_KEY_AND_LOOK.md sections 2 and 4 alone (#772 with the hub's review fixes); this lane
did not read the executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided**, in the new `ow-inventory` module `pickup_cost`, registered with the pickup natives (Off only):
  - **`WillowPlayerController.CanAffordToPickUpPickupable(interface)`**, section 2.4's preflight. It writes, loads and
    interns nothing:
    1. no pickupable: true;
    2. a pickupable the store has not loaded: `NotRead`;
    3. the cost query **by fit (UNVERIFIED)**: the pickupable's own `bCostsToPickUp` (bool), `CostsToPickUpType`
       (byte) and `CostsToPickUpAmount` (int), read by name. A class that does not declare all three with those kinds
       is `NotRead`, "the cost query of <class>";
    4. no cost: true;
    5. golden keys (type 3): `NotRead`, the controller's golden-key count;
    6. the controller's `PlayerReplicationInfo`:
       - a handle the store has not loaded is `NotRead`, checked before the class test;
       - None, or not a `WillowPlayerReplicationInfo` (the class table, nothing loaded), is true;
       - otherwise the answer is `amount <= GetCurrencyOnHand(type)`, asked through the VM, so the bound native
         answers;
  - **`PayForPickupable(interface)`** takes steps 1 to 4 and step 6's not-loaded check and cast, not step 5. Golden
    keys raise the controller's `SpendGoldenKey` once per unit (an amount of 0 or less: none). Any other type, with
    the cast replication info, calls `AddCurrencyOnHand(type, -amount)` last; a failed cast does nothing. The note's
    dropped `GetCurrencyOnHand` read is not made, since it writes nothing;
  - **`UpdateAmmoCounts(bool)`** is the note's `NotRead`, "UpdateAmmoCounts: the controller's ammo refresh". Section 4
    gives that Off answer and test 8 tests it;
  - a method resolved from a class whose code is not loaded is a named `NotRead` (a class load is a write);
  - no pack bound any of the three keys before (checked by grep). The all-packs wiring test passes.
- **Reference:** unchanged; it binds none of the three.
- **The slice gate** (installed, Off, strict, release build, under the machine lock; main c345fdb and this change): 6
  ok / 8 not ok, 51.2% on both.
  - **Link 6** passes `CanAffordToPickUpPickupable`: true for the free gun, as section 2.3 predicts. It stops, still
    in `key` (2/3 steps), at **the unbound `WillowPlayerController.GetPawnInventoryManager()`**, not at
    `UpdateAmmoCounts` as section 6 predicted.
  - **Why** (read with `research/script_disasm.py`; the listings stay under ignored `local/analysis/link67_key/`):
    the controller's own `HasRoomInInventoryFor` (section 4, step 4) asks `GetPawnInventoryManager()` first, and no
    pack binds it.
  - Its rule is already read: NATIVE_CONTROLLER_HELPERS.md, "WillowPlayerController.GetPawnInventoryManager", high
    confidence, UNVERIFIED. For an on-foot pawn it is the pawn's `InvManager` as a `WillowInventoryManager`, else None.
    The note's section 4 did not list this unbound native.
  - Link 7 and every other row are unchanged.
- **Tests** (`pickup_cost_tests.rs`, invented packages), the note's tests 1 to 9:
  - the replication info's currency natives and `SpendGoldenKey` are counting toy handlers;
  - `CanAffordToPickUpPickupable` is checked write-, load- and intern-free on every call.

  Mutation-checked: `<` for `<=`, no cast, and the golden-key count refused in `PayForPickupable` each fail a test.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game, and the pickupable's own cost query (by fit).
- **Sensitive files:** none.
