# Link 6's take: UpdateAmmoCounts and PickupPickupable to the end of `key` (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-UPDATEAMMOCOUNTS (decision 2026-11-26-43), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/ammo/`. This note is
in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is
UNVERIFIED in the running game.**

**Why.** With #778, link 6 (`make`, `touch`, `key`) reaches `PickupPickupable`. It stops at `UpdateAmmoCounts`'
`NotRead`, after partial writes (NATIVE_LINK67_KEY_AND_LOOK.md section 4). `key` is link 6's last step, so the take's
end is the link's end. Its evidence is the gun being the pawn's.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`): `WillowPlayerController.PickupPickupable`, `ScriptAnnounceAmmoGain`,
  `ClientSpawnPickupableMesh`, `ShouldUseCoopRange` and `UnTouchedPickupable`. Also `WillowPickup.GiveTo` and
  `GetPickupableMeshActor`, `GameInfo.PickupQuery`, `WillowWeapon.GivenTo`, `AssociateAmmoPool` and `CanBeUsedBy`,
  `WillowInventory.CanBeUsedBy`, and both `HasRoomInInventoryFor`.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read: the execs and the class-table
  slots of section 3's natives, each slot in its class's primary class table (the NATIVE_COMPONENT_SKIP_FLAG.md
  section 6 method).
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. Every field below lands exactly where the
  routine reads it (no shift for actors and definitions).
- **Maya's data** (`ow-package --def-dump`): the slice gun's balance `AR_Bandit` has no `DlcItemSet`.
- **A local probe** (temporary code, reverted, never committed). It ran the installed Off strict `--slice-gate` on
  #778's head 2f6f7da, under the machine lock, with a call trace of script-to-script calls. It stubbed each stop it met,
  in order, to see the next. The stubs are not answers, and the stubs for section 3 wrote nothing (`SetOwner`'s stub
  ran the port's own `set_owner`).

## 2. `PickupPickupable` for the slice's gun (one pickup, not bulk)

In order (NATIVE_INVENTORY_EQUIP.md has the outline):
1. the globals definition's `PickupRadius`; the gun's definition does not pick up in bulk, so no nearby pickups are
   gathered;
2. **`UpdateAmmoCounts(True)`** (3.1), silent;
3. the inventory count before (unreadied plus readied);
4. affordability (#778) and `HasRoomInInventoryFor` (bound: room);
5. **`WorldInfo.Game.PickupQuery(pawn, pickup)`**, read on the **controller's** `WorldInfo` (4.1). It is the game
   info's script: the base mutator's say, else the pawn's `InvManager.HandlePickupQuery` (bound). False gives the
   failure message and the pickup's `FailedPickup`;
6. **`ClientSpawnPickupableMesh`**: `WillowPickup.GetPickupableMeshActor` spawns a `PickupableMeshActor` at the
   pickup's `Location` with **`GetSaveRotation()`** (3.2). The inventory's `AttachMeshToPickup` follows (bound), then
   `SetTarget(Pawn)` on the mesh actor;
7. for a weapon with ready-after-pickup and a player pawn that can use it: the pawn's `bGrabWeaponPickup` (not the
   slice's case: `key` passes false);
8. the co-op range test (`ShouldUseCoopRange`, script; false for a weapon), then **`GiveTo(pawn, bReady)`** on the
   pickup:
   - `WillowPickup.GiveTo` (script): a vending-machine base told; an item told `PickupDisassociated`; **each
     `WaypointComponent` of the pickup's `ComponentList`** (3.3) told `RemoveWaypoint`; then `DroppedPickup.GiveTo`;
   - that runs the weapon's `AnnouncePickup` → `Pawn.HandlePickup` → **`Actor.MakeNoise`** (3.4). Then the
     controller's `HandlePickup` → `ServerAnnounceWeaponPickup`, and the weapon's pickup stats;
   - `Inventory.GiveTo` → the manager's pickup stats → `WillowInventoryManager.AddInventory`:
     - `WillowWeapon.CanBeUsedBy` → **`Controller.IsUsingVehicle(False)`** (3.5), then `WillowInventory.CanBeUsedBy`
       → `IsHumanControlled` (bound), **`IsDLCRequirementMet`** (3.6), **`AttributeExpression.EvaluateExpressions`**
       over `OnUseConstraints` (4.2), the level requirement (bound);
     - the readied checks (bound), then `InventoryManager.AddInventory` → **`Actor.SetOwner`** (3.7) →
       `WillowWeapon.GivenTo` → `AssociateAmmoPool` → **`GetResourcePoolContext`** (3.8), then
       `WillowInventory.GivenTo` → **`Actor.SetBase`** (section 5's stop);
9. after it, by the script (not reached by the probe): the touched and seen slots cleared, `UnTouchedPickupable` on
   every player controller, then **`UpdateAmmoCounts`** again. That call is silent exactly when the inventory count
   changed, so ammo gains are announced only when no item was added.

## 3. The natives read here

### 3.1 `WillowPlayerController.UpdateAmmoCounts(bool bSilent)`

The exec calls the controller's virtual, which does this:
1. **the globals definition** (the engine's `WillowGlobals`, its definition loaded on first use by name, as
   NATIVE_WILLOW_GLOBALS.md describes). With none: nothing;
2. **eight ammo kinds, in this order** (the event's kind byte in brackets):

   | Definition field | Cache field | Kind |
   |---|---|---|
   | `AmmoDefinition_RepeaterPistol` | `LastKnownAmmoCount_RepeaterPistol` | 3 |
   | `AmmoDefinition_RevolverPistol` | `LastKnownAmmoCount_RevolverPistol` | 2 |
   | `AmmoDefinition_SMG` | `LastKnownAmmoCount_SMG` | 1 |
   | `AmmoDefinition_Shotgun` | `LastKnownAmmoCount_Shotgun` | 4 |
   | `AmmoDefinition_CombatRifle` | `LastKnownAmmoCount_CombatRifle` | 0 |
   | `AmmoDefinition_SniperRifle` | `LastKnownAmmoCount_SniperRifle` | 5 |
   | `AmmoDefinition_RocketLauncher` | `LastKnownAmmoCount_RocketLauncher` | 6 |
   | `GrenadeAmmoDefinition` | `LastKnownAmmoCount_Grenades` | 7 |

   (definitions on `GlobalsDefinition`, caches floats on `WillowPlayerController`);
3. **for each definition that is set:**
   - the controller's pool for it, through `Controller.GetResourcePoolForResourceDefinition(definition, true)`. That is
     the provider's lookup **with creation** (NATIVE_COMBAT_GROUNDWORK.md section 1.5): a missing pool is made;
   - the value is the pool's `CurrentValue`, read through the returned handle when it still names a live pool of the
     manager, else 0;
   - **when the value differs from the cache:**
     - when it is larger and `bSilent` is false, the script event `ScriptAnnounceAmmoGain(kind, gain)` is raised.
       Here gain is (value − cache) truncated to an int. The event's script sends the received-ammo HUD message
       (`ReceiveLocalizedAmmoMessage`);
     - then the cache := the value.

   An equal value changes nothing.

**Proposed Off answer:**
- **preflight**, write-free and load-free:
  - the globals definition resident (else `NotRead`). With no globals object: nothing;
  - the eight definition fields declared as objects, and the eight caches as floats, else `VmError::Script`;
  - each set definition resident;
  - `GetResourcePoolForResourceDefinition`'s own preflight for all eight, each checked against the state before the
    call (review r1 of #785). A lookup's creation can change what a later lookup meets, so a later lookup's refusal
    in the commit carries the partial-writes prefix;
- **commit**, in the table's order:
  - the lookup with creation (the existing rule; a creation is a write);
  - the compare;
  - the event through the VM;
  - the cache write.

  A refusal after the first write (a creation, a cache, or the event's script) carries the "stopped after partial
  writes: " prefix.

### 3.2 `WillowPickup.GetSaveRotation()`: see NATIVE_PICKUP_SAVE_ROTATION.md

The rule and its Off answer are NATIVE_PICKUP_SAVE_ROTATION.md section 5 (decision 2026-11-26-05), which this lane's
reading agrees with. That section's default for a rigid-body pickup is the physics `LayerAbsent`. The actor's
`Rotation` is the answer only under the hub's ruling, with its stand-in log line (review r1 of #785).

**What is new here** (the probe): the slice's pickup does reach the rigid-body case. Its `Physics` is `PHYS_RigidBody`
and its `CollisionComponent` is set. So under that note's default, link 6 stops here with the physics `LayerAbsent`.
Section 5's order below assumes the ruling.

### 3.3 `Actor.ComponentList(class BaseClass, out ActorComponent)` (iterator)

- a None class: no iteration;
- otherwise each non-None entry of the actor's `Components` whose class is the given class or below it, in array
  order. The array is re-read at each step (the loop's index goes on).

**Proposed Off answer:** an iterator in Off:
- `Components` declared as an array of objects, else `VmError::Script`;
- the class test by the entry's class chain, with no load. A lazy entry is judged by the class its handle carries;
- **the game's behaviour is a live re-read by index** (review r1 of #785). Each step reads `Components` at the next
  index as it is then, so a body that adds or removes entries moves what the loop sees. That is the rule to follow:
  - an iterator that can step (one entry per step, reading the live array at the loop's index) reproduces it exactly;
  - the VM's iterator interface today hands over a list up front, so it cannot re-read. With that interface, the
    iterator keeps a copy of `Components` and compares it with the live array after each body. Any difference is a
    `NotRead` ("ComponentList: the loop body changed Components"), with the partial-writes prefix, since the body
    ran. Equal arrays give the game's answer exactly;
- on the slice, the body is `RemoveWaypoint` and the pickup has no `WaypointComponent` (probe), so no body runs.

### 3.4 `Actor.MakeNoise(float Loudness, optional name NoiseType)`

The exec reads its two arguments and calls an empty routine. It is a direct call, not a virtual. **It does nothing in
this build.** **Off answer:** nothing.

### 3.5 `Controller.IsUsingVehicle(bool bCheckDrivenVehicle)`

- with no `Pawn`: false;
- the pawn's own "vehicle" virtual: for `WillowPlayerPawn`, `WillowPawn` and `GearboxPawn` it answers none;
- otherwise, with the flag set and the pawn's `DrivenVehicle` set: that vehicle's answer to the same virtual (a vehicle
  class's body was not read);
- else false.

**Proposed Off answer:**
- no pawn: false;
- a pawn of those three first native classes: false, unless the flag is set with a `DrivenVehicle`, which is
  `NotRead`;
- any other first native class: `NotRead`;
- `DrivenVehicle` declared as an object, else `VmError::Script`.

It replaces NATIVE_GO_DOWN_PATH.md's `NotRead` for this key.

### 3.6 `WillowInventory.IsDLCRequirementMet(PlayerController)` (for a `WillowWeapon`)

The exec calls the inventory's virtual. For a `WillowWeapon`:
- with no `DefinitionData.BalanceDefinition`, or a balance whose `DlcItemSet` is None: true;
- otherwise the item set's own check with the player controller (not read).

**Maya:** `AR_Bandit` has no `DlcItemSet`, so true.

**Proposed Off answer:**
- `DefinitionData` declared as a struct with an object `BalanceDefinition`, and `DlcItemSet` declared as an object on
  the balance's class; each else `VmError::Script` (review r1 of #785);
- true for those two cases; a set `DlcItemSet` is `NotRead`;
- other inventory classes' bodies were not read: `NotRead` naming the class.

### 3.7 `Actor.SetOwner(Actor NewOwner)`

The rule `ow-world`'s `actors::set_owner` already implements, with **two steps it leaves out**:
- nothing when the owner does not change, or the actor is being deleted or pending kill;
- a new owner whose owner chain reaches the actor: nothing;
- the old owner gets `LostChild` (stop if script changed the owner);
- the owner is set, and the new owner gets `GainedChild` (stop if changed);
- **then the owner-visibility refresh:** each primitive component in the actor's `AllComponents` is marked for
  reattach and reattached (deferred while its owner is attached, by the component rules). This applies to a component
  with `bOnlyOwnerSee` or `bOwnerNoSee` set, and to every primitive component when the actor has `bOnlyOwnerSee`. The
  flags land exactly under the primitive component's usual shift; the "marked" flag is by fit `bNeedsReattach`;
- **then `bNetDirty`** is set.

**Proposed Off answer:** bind the script native `Actor.SetOwner(object)` in Off to `set_owner`, then the refresh and
`bNetDirty` (review r1 of #785):
- **up front**, before any write: the deferred-reattach planner's preflight (`component_setters`) over the components
  as they are. Its refusal stands, with no prefix;
- `LostChild` and `GainedChild` are script events. They run before the refresh and can change the components, their
  flags or the owner the plan was built from. So the refresh is applied to the state **after** the events: the planner
  runs again then. Any difference from the up-front plan (a refusal it did not have, or a component set that changed)
  is a refusal with the partial-writes prefix;
- then `bNetDirty`.

### 3.8 `WillowWeapon.GetResourcePoolContext()`

The weapon's `Instigator`'s `Controller`, or, when that is None, the `Instigator`'s `DrivenVehicle`'s `Controller`. With
no `Instigator`: None. **Off answer:** that read, with no writes (each field declared, else `VmError::Script`).

**The slice:** at `AssociateAmmoPool` the harness's gun has no `Instigator` (probe). So the answer is None, no pool is
associated, and the weapon's stored ammo is not moved. In the game, `Inventory.GivenTo` sets the `Instigator` only
after `AssociateAmmoPool`. Whether the game's dropped gun keeps an `Instigator` from its drop was not read.

## 4. What the port lacks that is not a native read

### 4.1 The pickup world's controller has no `WorldInfo` (a harness gap)

The probe found the player controller's `WorldInfo` None, and the pawn's too. The pickup's own is set. So step 5's
`WorldInfo.Game.PickupQuery` on the controller reads through None and is false. The take then fails: the failure
message, then `FailedPickup` → `ConvertFixedToRigidBody` → the unbound `PrimitiveComponent.GetRootBodyInstance`.

In the game every spawned actor has its level's `WorldInfo`.

**Proposed** (the hub's call): the pickup world's set-up writes the world's `WorldInfo` into the controller's and the
pawn's `WorldInfo`. It is a labelled harness input, UNVERIFIED, like `pickup_game_info`.

### 4.2 `AttributeExpression.EvaluateExpressions` is not a bound native

The rule is NATIVE_ATTRIBUTES.md section 11 (ow-attr's `expressions_hold`): mode 0 is And, 1 is Or, and an empty list
is true. It is used inside ow-attr but not bound as the static script native
`AttributeExpression.EvaluateExpressions(byte,object,array,object)`.

**Proposed:** bind it in Off to that rule. The gun's `OnUseConstraints` is empty (probe), so true.

## 5. Predicted stops (from the probe, each earlier stop stubbed)

Link 6, after `UpdateAmmoCounts` (3.1) is answered, meets in order:
1. 4.1 (a harness gap);
2. 3.2 (under the hub's ruling; by NATIVE_PICKUP_SAVE_ROTATION.md's default, its physics `LayerAbsent` stops link 6
   here);
3. 3.3;
4. 3.4;
5. 3.5;
6. 3.6;
7. 4.2;
8. 3.7;
9. 3.8.

Then: **`Actor.SetBase`, inside `WillowInventory.GivenTo`**, an existing `NotRead` (NATIVE_GO_DOWN_PATH.md 3.2, "the
class's own implementation is not resolved"), after partial writes. That is the predicted next stop once all of the
above are answered. Its body is the next read.

Caveats:
- the stubs wrote nothing (`SetOwner`'s ran the port's rule), so a later read of `LastKnownAmmoCount_*`,
  `bNetDirty` or a reattach flag was not exercised;
- `UpdateAmmoCounts`' second call and section 2's step 9 were not reached.

Link 7 is unchanged (the HUD `LayerAbsent` in `look`).

## 6. Tests (synthetic fixtures)

1. `UpdateAmmoCounts`:
   - (silent) a toy globals definition with two ammo definitions, pools at 10 and 0, caches 0: both caches written, no
     event;
   - (not silent) pool 10 → 25 with cache 10: `ScriptAnnounceAmmoGain(kind, 15)` once, then the cache 25;
   - a lower value: the cache written, no event;
   - an equal value: nothing;
   - a missing pool: made by the lookup's rule, value 0;
   - the order of the eight kinds, by a recording event.
2. `UpdateAmmoCounts` with no globals: nothing. A mistyped cache: `VmError::Script`, nothing written.
3. `GetSaveRotation`: NATIVE_PICKUP_SAVE_ROTATION.md section 6's tests.
4. `ComponentList`:
   - a None class: no iteration;
   - two waypoint components among three entries: those two, in order;
   - a subclass entry counts;
   - a None entry is skipped.
5. `MakeNoise`: nothing written.
6. `IsUsingVehicle`:
   - no pawn: false;
   - a `WillowPlayerPawn`: false;
   - flag set with a `DrivenVehicle`: `NotRead`;
   - an unlisted pawn class: `NotRead`.
7. `IsDLCRequirementMet`: no balance: true. No item set: true. An item set: `NotRead`.
8. `SetOwner`:
   - the `set_owner` cases;
   - then `bNetDirty` set;
   - a component with `bOwnerNoSee`: its reattach planned and applied;
   - an owner-loop refusal: nothing written.
9. `GetResourcePoolContext`: no instigator: None. The instigator's controller. A None controller with a driven
   vehicle's controller.
10. `EvaluateExpressions` through the VM: an empty list in either mode: true; And and Or over two toy expressions.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - a vehicle's answer to 3.5;
   - the DLC item set's check (3.6);
   - `Actor.SetBase` (the next stop);
   - whether the game's dropped gun keeps an `Instigator`;
   - 3.2's body pose for a moving body.
3. **Fitted:** 3.7's reattach mark (by fit `bNeedsReattach`).
4. **The hub's calls:**
   - the controller and pawn `WorldInfo` harness input (4.1);
   - 3.2's ruling (NATIVE_PICKUP_SAVE_ROTATION.md section 5).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Pick up ammo with a full gun list, and read `LastKnownAmmoCount_*` before and after; watch for the received-ammo
   message.
2. Pick up a gun: expect no ammo message (the silent second call).
3. Read a dropped gun's `Instigator` before it is picked up.
