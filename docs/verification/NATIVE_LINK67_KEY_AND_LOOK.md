# Links 6 and 7: the pickup's `key` (affordability and the take) and link 7's `look` (the HUD) (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK67-KEY-AND-LOOK, the analyst half of the clean room: **another
lane implements this note.** Raw output stays under ignored `local/analysis/link67/`. This note is in our own words:
no decompiler output, pseudo-code, addresses or listing structure. **Every rule here is UNVERIFIED in the running
game.**

**Why.** With #764 and #765 (E, in review at the time of writing), link 6 passes `touch` and stops in `key` at the
unbound `WillowPlayerController.CanAffordToPickUpPickupable(interface)`. Link 7 passes `touch` and stops in `look` at
the pickup targeting's HUD refusal: no HUD in the headless world. This note reads both, and goes on through `key` as
far as the reads reached.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The natives** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - `WillowPlayerController.CanAffordToPickUpPickupable` and `PayForPickupable`, and the routine each hands to;
  - the controller's `GetNumGoldenKeysAvailable` and the replication info's `GetCurrencyOnHand`, whose table slots
    the two routines call (each exec calls the same slot, which names the slot by fit);
  - `WillowPlayerController.UpdateAmmoCounts`;
  - the script event names' registration, and the one native that raises each of `SawPickupable` and
    `ClearSeenPickupable`: the HUD's targeting pass of NATIVE_PICKUP_TARGETING.md.
- **The script** (`research/script_disasm.py`):
  - `WillowPlayerController.PickupSomething`, `ServerPickupSomething`, `PickupPickupable` and
    `HasRoomInInventoryFor`, and `WillowInventoryManager.HasRoomInInventoryFor`;
  - `WillowPickup.InitializeFromInventory` and `Behavior_ChangeUsabilityCost`;
  - `GameInfo.GenericPlayerInitialization`, `PlayerController.ClientSetHUD` and `SpawnDefaultHUD`,
    `WillowPlayerController.ClientSetHUD`, `HUD.PostBeginPlay`, and `WillowHUD.PostBeginPlay`, `InitHud` and
    `SetWPRI`.
- **Data** (`ow-package --properties`, local, names only):
  - `Default__WillowPickup` tags none of `bCostsToPickUp`, `CostsToPickUpType` or `CostsToPickUpAmount`;
  - `Default__WillowGameInfo` tags `HUDType` = `WillowHUD`;
  - `Default__WillowHUD` does not tag `HUDDef`.
- **The port on main 78a88b72:** `ow-cli` `slice_pickup.rs` (`look`, `key`), `ow-mission` `interactive.rs` (the
  use-cost pair, the same shape), and the native tables (which natives on the path are bound).

## 2. `CanAffordToPickUpPickupable(Pickupable)`

### 2.1 What the native does

1. **No pickupable** (the interface value None): **true**.
2. **The cost query.** The pickupable is asked, through its C++ `IPickupable` table, for its cost to the controller's
   `Pawn`. It answers whether the pickup costs anything, and fills a currency type and an amount.
   - This method has **no script name**: `IPickupable`'s ten script functions do not include it. Its body was not
     located: it sits in the pickup's interface table, which the class-table query does not reach, and searches for
     reads of the cost fields' places found no match.
   - **By the earlier reading** (NATIVE_INVENTORY_EQUIP.md, medium-high), a `WillowPickup` answers from its own
     fields: costs = `bCostsToPickUp`, type = `CostsToPickUpType`, amount = `CostsToPickUpAmount`. This lane did not
     re-locate that body: **UNVERIFIED (by fit)**.
3. **No cost: true.**
4. **Golden keys** (currency type 3): **amount ≤ the controller's `GetNumGoldenKeysAvailable()`**.
5. **Any other type:** the controller's `PlayerReplicationInfo` cast to `WillowPlayerReplicationInfo`:
   - the cast fails (None, or another class): **true**;
   - otherwise **amount ≤ `GetCurrencyOnHand(type)`**.
6. **No writes**, no load, no event.

### 2.2 `PayForPickupable(Pickupable)` (the same query, for the take's end)

No pickupable or no cost: nothing. Golden keys: the controller's script event `SpendGoldenKey` once per unit of the
amount (none for an amount ≤ 0). Any other type, with the cast replication info: `GetCurrencyOnHand(type)` is read and
its answer dropped, then `AddCurrencyOnHand(type, -amount)` (the slot is that native's by fit). A failed cast: nothing.
It never checks affordability (the script asks first). This is the shape `ow-mission`'s `interactive.rs` already gives
the use-cost pair.

### 2.3 The slice's gun

Link 6's gun drops as a `WillowPickup` (NATIVE_PICKUP.md). Its class default tags no cost field. In script, only the
`Behavior_ChangeUsabilityCost` behavior writes them; `InitializeFromInventory` does not. So for the fixture's pickup,
`bCostsToPickUp` is false: **`CanAffordToPickUpPickupable` answers true** and `PayForPickupable` does nothing (a
prediction from the data; a bound behavior could change it, and none runs on the slice's drop).

### 2.4 Off answer (for the implementer)

**Preflight**, write-free and load-free:
1. the pickupable None: **true**;
2. the pickupable resident, else `NotYet { NotRead }`, "CanAffordToPickUpPickupable: a pickupable the store has not
   loaded (a load is a write)";
3. its class declares `bCostsToPickUp` (bool), `CostsToPickUpType` (byte) and `CostsToPickUpAmount` (int), else
   `NotYet { NotRead }`, "CanAffordToPickUpPickupable: the cost query of <class>". A `DroppedPickup` that is not a
   `WillowPickup` declares none of them, and its own answer was not read;
4. `bCostsToPickUp` false: **true**;
5. type 3: `NotYet { NotRead }`, "CanAffordToPickUpPickupable: the controller's golden-key count". The body of
   `GetNumGoldenKeysAvailable` is the controller's table slot, which was not read; `interactive.rs` refuses the same
   case the same way;
6. otherwise the controller's `PlayerReplicationInfo`: None, or not a `WillowPlayerReplicationInfo`, **true**.
   Otherwise the answer is **amount ≤ `GetCurrencyOnHand(type)`**, through the port's bound native (`ow_combat`'s
   `down.rs`).

Nothing is written, so no answer carries the partial-writes prefix. `PayForPickupable` takes the same preflight, then
its writes (2.2); a refusal after `AddCurrencyOnHand` cannot come, since that native is the last step.

## 3. Link 7's `look`: what the targeting needs from the HUD

### 3.1 The pass and drawing

- The pass of NATIVE_PICKUP_TARGETING.md is the only native that raises `SawPickupable` and `ClearSeenPickupable`
  (each event name has one user besides its registration). It is a virtual of the HUD with no direct caller. Whether
  the HUD's tick or its render callback calls it was not read (the class table query refuses `WillowHUD`).
- **It reads no drawing state:** neither the HUD's `Canvas` nor `RenderDelta`. It reads the HUD's `PlayerOwner`, as
  that note says. Hover particles, the highlight colour and the interaction icon are presentation (that note's section
  9). **So the targeting result is independent of drawing.** A headless pass needs the HUD's fields and a view, not a
  canvas.

### 3.2 Where the game's HUD comes from

1. At login, `GameInfo.GenericPlayerInitialization` calls the controller's `ClientSetHUD` with the game info's
   `HUDType`. `WillowGameInfo`'s default `HUDType` is `WillowHUD`.
2. `PlayerController.ClientSetHUD` destroys an old HUD and spawns one of that class, the controller its `Owner`, into
   `myHUD`. `WillowPlayerController.ClientSetHUD` wraps it with the HUD's visibility, the Scaleform UI start, the lobby
   and save-game steps, and registers every `DroppedPickup` with the HUD's post-rendered actors.
3. The spawn's `PostBeginPlay` (play has begun in the game):
   - `HUD.PostBeginPlay`: `PlayerOwner` := `Owner` cast to `PlayerController`;
   - `WillowHUD.PostBeginPlay`: `InitHud` (the loading-movie path), `LoadBuildInfo`, `WPlayerOwner` := `PlayerOwner`
     cast to `WillowPlayerController`, `SetWPRI` (which retries on a timer until the replication info exists), a timer
     for `OpenHUDMovie`, and two material instances made with `New` and parented.
4. **`HUDDef`** is not set by any of these, and the class default leaves it None. `GlobalsDefinition.HUDDefinition`
   exists and is the likely source, set natively. **Where it is set was not read** (NATIVE_PICKUP_TARGETING.md's open
   question 2 stays open).

### 3.3 Is there a headless answer?

**Not yet, by reading alone:**
- a HUD made by the game's route runs `ClientSetHUD`'s and `PostBeginPlay`'s presentation steps (the movie timers,
  the GFx manager, the material instances). Each is a new stop in the port;
- `HUDDef`'s source is unread. A HUD with `HUDDef` None ends the pass (that note's section 1, step 2): the look sees
  nothing, an ordinary failure, not the game's behaviour;
- **even with a HUD, the next stop is the camera:** the pass reads the owner's view before anything else (that note's
  section 10, step 4). That is a `LayerAbsent` the hub left to a host lane.

**Proposed Off answer:** keep step 2's HUD `LayerAbsent` until two things exist: a HUD stand-in with `PlayerOwner`,
`WPlayerOwner` and a `HUDDef` (a harness input, the hub's call), and `HUDDef`'s source read. Then the stop moves to the
camera `LayerAbsent`, the same kind, and no count changes. **The hub's call:** whether a harness HUD is worth it before
the camera exists.

## 4. `key`, after affordability

`PickupSomething(bReadyAfterPickup)` (the slice calls it with false), on the controller:
1. **The guard:** the current pickupable (`GetCurrentPickupable`) is not None, the pickupable's
   `DenyPickupAttempt(self)` is false, and the inventory pawn exists. Otherwise nothing more;
2. `CanAffordToPickUpPickupable` (section 2). Not affordable: `NotifyUnableToAffordPickupable`, and stop;
3. `bReadyAfterPickup` false, so `AllowReadyAfterPickup` is not asked;
4. `HasRoomInInventoryFor`: the inventory manager's script (bound natives: the readied and unreadied counts, their
   limits, the item's space requirement). With room: `ServerPickupSomething(false)`, which calls `PickupPickupable`
   with the current pickupable;
5. without room: the weapon or gear swap, or the "full" message and the stat (NATIVE_INVENTORY_EQUIP.md);
6. after a take or a swap: `PayForPickupable` (2.2), nothing for the slice's free gun.

**`PickupPickupable`** (the authority's take; NATIVE_INVENTORY_EQUIP.md describes it):
- it reads the globals definition;
- for a bulk-pickup definition it gathers nearby pickups (an iterator over `WillowPickup`s);
- **then `UpdateAmmoCounts(True)`, before the per-pickup loop**: its affordability, room check, `PickupQuery`,
  `ClientSpawnPickupableMesh`, `GiveTo` and the clearing of the touched and seen slots.

**`UpdateAmmoCounts(bSilent)`** only hands its flag to a controller table slot, whose body was not read (the class
table query refuses this class).

**Off answer:** `NotYet { NotRead }`, "UpdateAmmoCounts: the controller's ammo refresh". The call writes only after
the earlier steps, so with `PickupPickupable`'s writes not yet started it carries no prefix. The slice's harness
prefix comes from `make`'s writes, as for every later link 6 stop.

## 5. Tests for the implementer (synthetic, with `package()` / `tag()`)

Each refusal leaves #518's three counters, the class-code loads and the name table unchanged.
1. A None pickupable: true.
2. A toy `WillowPickup` with `bCostsToPickUp` false: true, nothing written.
3. A currency cost (type 0) with amount 5: true with 5 on hand, false with 4. The replication info is a toy
   `WillowPlayerReplicationInfo` answering through the bound `GetCurrencyOnHand`.
4. The same cost with the controller's `PlayerReplicationInfo` None, or of a plain `PlayerReplicationInfo` class:
   true.
5. A golden-key cost (type 3): the golden-key `NotRead`.
6. A pickupable class that declares no cost fields: the cost-query `NotRead`. A lazy pickupable: the not-loaded
   `NotRead`.
7. `PayForPickupable`:
   - free: nothing;
   - currency 5: the replication info's currency goes down by 5 (through the bound `AddCurrencyOnHand`);
   - golden keys 2: `SpendGoldenKey` raised twice (a toy handler counts);
   - amount 0 or less: none.
8. `UpdateAmmoCounts`: its `NotRead`, nothing written.
9. Reference: unchanged (both stay unbound and the stub policy answers).

## 6. Predicted stops (a prediction, not a run)

- **Link 6** (`make`, `touch`, `key`), with section 2's answer bound: `CanAffordToPickUpPickupable` answers true for
  the free gun. `HasRoomInInventoryFor` then runs on bound natives (an empty fixture inventory has room), and
  `ServerPickupSomething` reaches `PickupPickupable`. **The next stop is `UpdateAmmoCounts`' `NotRead`**, after
  partial writes (the run's `make`). This holds unless a step before it in `PickupPickupable` stops first: the bulk
  test's definition read, or the iterator.
- **Link 7** (`make`, `touch`, `look`, `key`): **unchanged at the HUD `LayerAbsent`** in `look`. With a harness HUD
  (3.3) it would move to the camera `LayerAbsent`. `key` is not reached.
- No link's step count is predicted to change from section 2 alone, since link 6 still stops inside `key`.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the pickupable's C++ cost query (section 2.1, step 2);
   - the controller's golden-key count and `UpdateAmmoCounts`' body (table slots of a class the query refuses);
   - the HUD pass's caller (tick or render), and where `HUDDef` is set.
3. **By fit:** the replication info's `AddCurrencyOnHand` slot in `PayForPickupable`; the cost fields' offsets (the
   implementer reads them by name).
4. **The hub's call:** a harness HUD before the camera exists (3.3).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On a dropped gun's pickup, read `bCostsToPickUp`: expect false, and `CanAffordToPickUpPickupable` true.
2. With 0 money, call `CanAffordToPickUpPickupable` on a pickup whose `bCostsToPickUp` is true, type 0, amount 1:
   expect false.
3. Read Maya's HUD's `HUDDef` and the globals definition's `HUDDefinition`: expect the same object.
