## 2026-11-26: GetPawnInventoryManager answers an on-foot pawn's manager

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-GETPAWNINVENTORYMANAGER, with queue 0ai (the #773 review LOWs), on
fix/getpawninventorymanager-and-773-lows: #774 (d35bcbb8) with #773 (0eaa2f12, which holds #770) merged. Clean room:
implemented from docs/verification/NATIVE_CONTROLLER_HELPERS.md, "WillowPlayerController.GetPawnInventoryManager",
alone; this lane did not read the executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided**, in `ow-inventory`'s `pickup` module (Off only), for
  **`WillowPlayerController.GetPawnInventoryManager()`**:
  - no `Pawn`: None;
  - **an on-foot pawn**, meaning its first native class is `WillowGame.WillowPawn` or `WillowGame.WillowPlayerPawn`:
    its `InvManager` when that is a `WillowInventoryManager` (by the class table), else None. The note says that for
    these the "vehicle/base" virtual is the base `WillowPawn`'s do-nothing stub. The first native class is read with
    `ow_world::world::render_hooks::first_native`, now `pub`;
  - **any other pawn class, a vehicle among them:** `NotYet { NotRead }`, "GetPawnInventoryManager on a <class> (its
    vehicle/base virtual, and so whether its Driver answers, is not read)". The note reads vehicle seats at medium
    confidence and leaves them for later;
  - a pawn or a manager the store has not loaded: `NotYet { NotRead }`;
  - it writes, loads and interns nothing;
  - no pack bound the key before (checked by grep). The all-packs wiring test passes.
- **Reference:** unchanged; it binds nothing.
- **Queue 0ai** (the #773 review LOWs, code from decisions 2026-11-26-30 and -31):
  - **(a)** a socket child whose class declares no `Owner` is a `VmError::Script` naming it, before any write.
    - The per-component `Owner` check at the attach step already refused it, before the later loop #773 had added. So
      that loop is removed: no mutation of it could fail a test.
    - Mutating the remaining check fails the new test. With it removed, the next refusal is the parent's
      `Attachments` entry type, still write-free;
  - **(b)** NATIVE_GETPHYSXLEVEL_AND_AFTER.md tests 6 and 7:
    - a synthetic `if (GetPhysXLevel() > 0) { Recorder(true) }`, on a toy Core package with the core `>` operator
      (native 151);
    - level 0 calls nothing, and level 1 calls the recorder once with true;
  - **(c)** a composed test:
    - the body composition's socket build, then `InitializeRBPhysics` on the same world;
    - the socket child is fixed through the record, as its parent is;
    - with the socket publish removed, #770's back-linked guard refuses instead.
- **The slice gate** (installed, Off, strict, release build, under the machine lock; the #774 + #773 merge, then this
  branch): 6 ok / 8 not ok, 51.2% on both.
  - **Link 6** gets further into `key`: it passes `HasRoomInInventoryFor` and reaches `PickupPickupable`. It stops at
    **`UpdateAmmoCounts`' `NotRead`**, after partial writes, now a `NotYet` instead of an error. That is
    NATIVE_LINK67_KEY_AND_LOOK.md section 6's prediction.
  - Every other row is unchanged.
- **Tests:**
  - `pickup_tests.rs`, invented packages:
    - no pawn, no manager, a `WillowInventoryManager`, another class;
    - a vehicle's `NotRead`, and a cold pawn's;
    - Reference binds nothing.

    Each call is checked write-, load- and intern-free. Mutation-checked: no manager-class test, and any pawn class
    accepted, each fail a test;
  - the LOW tests above.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game, and the vehicle redirection.
- **Sensitive files:** none.
