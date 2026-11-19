## 2026-11-19: Rust pickup targeting in Off

AI-assisted (Claude Code). Lane IMPL-PICKUP-TARGETING, implementer (slice link 7: the aimed pickup is the seen one).
Number 81 was given by the hub.
- **The branch.** Stacked on `origin/fix/rust-pickup` 21e1a2b8 (#452 rebased onto #453), which has link 7's driver.
- **The source.** NATIVE_PICKUP_TARGETING.md as the hub accepted it after review r3 of #461 (b394b5de,
  `research/rust-pickup-targeting`; analyst lane, decision 2026-11-19-74). The rules were coded from it alone.
- **Clean room.** This lane did not read the native and opened no `local/analysis/`, Ghidra or REA output for it. It
  read the installed packages only for field names (`tools/ghidra/class_layout.py` over the cooked packages).
- **No dependency, no parsing change.** No real-game session; every rule is **UNVERIFIED** in the running game.
  Reference is unchanged: the module binds no native and nothing calls it there.

**New module `ow_inventory::pickup_targeting`** (Off):
- **`tick(vm, hud, globals, host)`:** one pass of a `WillowHUD`, sections 1-8 of the note. It reads the HUD's
  `PlayerOwner`, `HUDDef`, `WPlayerOwner`, `CachedHoverPickup` (`H`) and `CachedBestPickup` (`S`), then the view, the
  pawn and the suspend test, `WillowGlobals.PickupList`, and per counted pickup the range gate, the 0.9 horizontal
  cone with its 1e-8 cutoff, the particle offset, the bounds and the segment score with its definite cutoffs. Then
  the ranking (strictly greater, starting at 0), the pawn test, the trace beyond a quarter of `PickupRange`, the
  winner's definition, the 0.5 view check, the range and the event.
- **Plan first, then write** (the note's section 10). The whole tick is decided read-only first; every `NotYet` on the
  path it takes comes before the first write. Then it writes:
  - `ClosestPickup` and each scored pickup's `bHasPickupParticleOffset`;
  - then `SavedLookAtInventory`, `H`, and each event before its write to `S`. The no-winner order is the clear, then
    `S`, then `H` (sections 6 and 7).

  A `NotYet` from an event's handler is terminal and marked "stopped after partial writes", its kind and record
  kept, the earlier writes left in place. Other errors pass unchanged.
- **`WPlayerOwner`** is read when the pass starts; a None value is refused only when the plan raises
  `SawPickupable` or `ClearSeenPickupable`.
- **The named `NotYet`s.** `LayerAbsent`:
  - no HUD (`hud_of`, `PlayerController.myHUD`);
  - the camera;
  - a scored candidate's component bounds;
  - the line check beyond a quarter of the range.

  `NotRead`:
  - the suspend query of a pawn that is not a plain `WillowPlayerPawn` (a subclass included), or of a driven vehicle;
  - the shrink bookkeeping when a counted pickup outlives the pickup default's `PickupShrinkDuration`;
  - a None recipient when an event is raised.
- **`Headless`** is the headless world's `Host`: it answers the camera, the bounds and the line check each with
  `NotYet { LayerAbsent }`. A later host lane supplies them.
- **The view direction and the pickup's rotation** use the engine's sine table (16384 steps to the turn). The note
  names the table for the view; using it for the pickup's rotation matrix is the engine's convention, not the note's
  (UNVERIFIED). Single precision throughout, with exact square roots where the native uses a refined fast reciprocal
  square root (the note's Off answer).
- **Port choices, none a rule of the game:**
  - the owner's `Dead` state is checked before the pawn's suspend query;
  - the shrink check runs whenever the pass has a pawn;
  - with no pawn, `ClosestPickup` is left emptied (the native refills it over `PostRenderedActors`; no reader on the
    targeting path).

**Link 7's `look`** (`ow-cli` `slice_pickup`) follows the note's look order:
1. the fixture's touched-decoy check;
2. the player's HUD, none in the headless world: `NotYet { LayerAbsent }` naming it;
3. a HUD with no owner or `HUDDef`: the pass ends, and the look fails as an ordinary error, since nothing was seen;
4. the camera, `NotYet { LayerAbsent }`.

A pass that does run must choose the aimed gun, and Maya's `CurrentSeenPickupable` must then hold it. #452's `look`
answered the camera directly; it now answers the HUD first, as the note's order says.

**Tests** (synthetic, `ow_package::testkit`):
- 16 in `pickup_targeting_tests.rs`: link 7's shape, the ranking, the capture radius and the hover bonus, the range
  gate and the hover particle, the trace exemption and the allowed hits, the 0.5 check, a missing definition,
  suspension and the pawn test, idle and no pawn, the recipient rule, the particle offset, the shrink refusal before
  any write, the headless layers, a handler's late `NotYet`, the segment cutoffs, and the cone cutoff with the sine
  table;
- 1 new in `slice_pickup_tests.rs`: the look order on a world with no HUD.

**Verified.** The host suites only: what this port does on invented data. Workspace build clean; `cargo test
--workspace` 2,200 passed, 0 failed, 46 ignored; `crate_edges_test.py` OK. The square roots and the sine table go
through `ow_core::num` (N3). Installed Off `--slice-gate`, strict, under the machine lock: 0 ok, 12 not yet, 2 error
of 14, as on #452. Links 6 and 7 are errors at `make` (1 of 3 and 1 of 4 driver steps reached), on the unbound
`WillowInventory.GetInventoryDefinition()` (#452's preflight), so `look` is not reached on installed data.

**Open.**
- A host camera, component bounds and collision, for `look` to pass beyond the HUD and the camera.
- A HUD in the headless world, and which `HUDDefinition` it holds (the note's open question 2).
- The suspend query, the shrink enable query and its limit (open questions 1 and 3).
- Whether `WillowHUD`'s field names hold, a declared-order fit at medium confidence. The packages' `WillowHUD`
  property list could not be laid out by `class_layout.py` (two field-list heads).

**Review r1 of #467** (a fix commit on 7ff524a3):
- **The plan dispatches nothing.**
  - `GetInventoryDefinition` is answered by field reads: the exact-class rule of NATIVE_INVENTORY_DEFINITION.md,
    the same allowlist and fields as #462's native. #462 is not on this stack, so the list is repeated here until it
    lands.
  - `GetGlobalsDefinition` is the globals' `TheGlobalsDefinition` field, declared on `GearboxGlobals`, when it holds
    a `GlobalsDefinition`. Otherwise it is a named `NotYet { NotRead }`: the native's first load numbers the
    challenge list, which was not read (NATIVE_WILLOW_GLOBALS.md, #469, section 6 step 4).
    - Correction: the r1 commit used the stock `GD_Globals.General.Globals`. It said `WillowGlobals` declares no
      field for its definition, but `class_layout.py` lists only a class's own fields, and the field is inherited.
  - Each getter must resolve to its native declaration. A script override, or any other function, is a named
    `NotYet { NotRead }` in the plan.
  - `Host` takes a shared `&Vm`.
- **The globals are resolved lazily** (a `Globals` provider), when the pass reaches the pickup list. So no HUDDef,
  and no camera, answer before them.
- **The planned event's handler** is resolved, its parameters checked and its binding prepared in the plan. The call
  itself stays in the write sequence.
- **The shrink refusal** comes after the winner's, the trace's and the event's checks.
- **An out-of-range winner** writes the seen-item slot twice, in section 7's order: the winner's inventory, then
  `H`, then empty.
- **The partial-writes prefix** is tested with `starts_with` in both helpers.
- **Tests:** 22 for the pass. Link 7's shape now uses a capture radius wide enough that only the cone stops the
  decoy, and checks the decoy never reaches the bounds query. New cases: an exact tie, a win decided by the 0.25
  bonus, a script override, a missing handler, the shrink order, the out-of-range seen slot, and the globals not
  resolved before the camera.

**Review r3 of #467** (a fix commit on 343bf8f0):
- **The globals are prepared before the pass.** `WorldGlobals::prepare` reads the world's globals object, checks the
  native declaration, and reads the active definition (`TheGlobalsDefinition`, which world set-up fills). Its outcome
  is held for the pass. The `Globals` provider now gets only `&Vm`, and nothing on that path dispatches script or
  calls `DefStore::find`. A refusal while preparing is still answered only when the pass reaches the pickup list.
- **The definition in use** is the world's active one, not the stock path. Off does not assume the stock
  configuration.
- **The handler call** is built with its exact arguments and validated with `Vm::check_call` in the plan, then
  stored for the commit. The testkit cannot declare a static-array parameter, so a test checks the stored call's
  arguments instead of that exact case.
- **Typed errors.** The definition-store lookups convert with `VmError::from`, so a `NotYet` keeps its variant, kind
  and record: the pickup default here, the driver's class default, and the weapon definition, balance, parts and
  accuracy pool lookups of `equip_run`'s typed preparation.
- **A missing layout** on an allowlisted inventory class is a named `NotYet { NotRead }`, and a malformed one a data
  error, as NATIVE_INVENTORY_DEFINITION.md says. #462's accessor replaces this copy once it lands.
- **Tests:** 26 for the pass. New: the checked handler call, the missing layout, and a definition-store `NotYet`
  keeping its variant.

