# Links 6 and 7 past the bounds, and the pickup's save rotation: WillowPickup.GetSaveRotation (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINKS67-NEXT (decision 2026-11-25-64), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/links67/`. This note
is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is
UNVERIFIED in the running game.**

**Why.** A look-ahead for links 6 and 7, with #707, #710, #713, #722 and #725 (`InventoryActors`). Today both stop in
`touch` at "Actor.GetBoundingCylinder: the component's template (option B) is not read"
(NATIVE_COMPONENT_BOUNDS.md section 9.5; `ow-world` `skeletal_bounds::TEMPLATE`). Section 1 says what option B would
need. Section 2 walks the steps past that stop on paper. Sections 3 to 5 read the first native on that walk with no
note.

## 1. What option B (the template record) would need in the port

**It is neither a missing data layer nor a load.** It has two parts, and both must hold before the pickup's component
can carry a mesh:
1. **The bytecode decoder does not give `New` a template operand; then the interpreter drops it.**
   `WillowWeapon.AttachMeshToPickup` makes the component with
   `new(Pickup) class'SkeletalMeshComponent'(ThirdPersonMesh)`:
   - **the decoder first.** `ow-script`'s opcode table (`ops.rs`) decodes `New` as four expressions: outer, name,
     flags and class. `research/script_disasm.py` decodes it the same way. So the template expression is read as a
     separate statement: the disassembly of `AttachMeshToPickup` shows `New(...)` at offset 0 and a bare
     `InstanceVariable(WillowWeapon.ThirdPersonMesh)` statement at offset 19. Option B therefore needs a **decoder
     change first**: `New` with five operands, checked structurally against the census (sizes add up, jumps land on
     statements, totals match). **The five-operand count is UNVERIFIED until that check passes.** It is a sensitive
     bytecode-layout change (rule 4), for the hub to schedule;
   - **then the interpreter.** The port's `New` (`ow-vm` `interp/eval.rs`, `new_object`) uses only the outer and the
     class operands, and builds the object from the class defaults (`instantiate_from_defaults`). With a fifth
     operand, the rule would be: the new object takes the template's property values instead of the class defaults.
     The body composition's `make_component` (`ow-world` `body_composition.rs`) copies a template's slots this way.
     What the game does beyond the copy, such as instancing the template's own subobjects, was not read here;
   - **load-free.** The template rule must load nothing: a template that is a store handle not loaded yet is
     `NotYet { NotRead }`. `make_component` itself loads its template (`ensure_loaded`), so that part is not to be
     copied;
   - **which object is the template.** Whether the weapon's `ThirdPersonMesh` is its own instanced copy or the shared
     `Default__WillowWeapon.ThirdPersonWeaponMesh` subobject follows the subobject-instancing ruling (#435, decision
     2026-11-19-44: Off copies a spawned actor's template components, or refuses, never sharing silently). It depends
     on how the fixture makes the weapon.

   **Both are VM changes, so the hub's call.**

   *Read 2026-11-26 in NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 3 (decision 2026-11-26-02): the game's handler
   reads five operands, and a census of the packages fits that count.*

   *Changed 2026-11-26 (lane NEW-FIVE-OPERANDS, decision 2026-11-26-05), so the four-operand reading above no longer
   holds:*
   - *both decoders read five operands. The argument-count check (`research/script_disasm.py --arg-check`) finds 0
     calls with an extra argument, and the same 12,968 of 12,978 functions decode. The template is now the fifth
     operand of the `New` at offset 0, not a bare statement at offset 19;*
   - *the port's Off `New` evaluates all five operands and refuses a template, by the first of the checks in
     NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 3.3 that fails. For `AttachMeshToPickup` that is "new: instancing a
     template's components", the stop of links 6 and 7 now;*
   - *the template copy itself (option B) is still not ported, and part 2 below still holds.*
2. **The template has no mesh on links 6 and 7.** The pickup links prepare their gun with `equip_run::prepare` alone.
   They make the weapon, a pawn, a controller and a manager, and never run the weapon's `init`
   (`InitializeInternal`). So `InitMeshes` never sets `ThirdPersonMesh`'s skeletal mesh.

   The class default subobject `WillowGame.Default__WillowWeapon.ThirdPersonWeaponMesh` (read with `ow-package
   --def-dump`) has `SkeletalMesh` None, and `bOwnerNoSee` true. So even with part 1, the pickup's component would
   copy a template with no mesh, and `GetBoundingCylinder` would again meet source e. In the game the dropped gun has
   run its init, and `InitMeshes` has given `ThirdPersonMesh` the composite mesh (link 8's path). **So option B
   changes links 6 and 7 only once their fixture runs the gun's init, and link 8's `InitMeshes` path passes.**

## 2. Links 6 and 7 past the bounds (on paper, not a run)

Read with `research/script_disasm.py`. The data is the fixture's (`slice_pickup.rs`, `equip_run::prepare`): Maya's
manager is new, so her `InventoryChain` and `ItemChain` are empty, and the gun is a Bandit assault rifle. In order:
1. `touch`:
   - `GetTouchTraceLocation` raises the trace end by half the cylinder's height (script);
   - then `TouchPickupTrace` (NATIVE_TOUCH_PICKUP_TRACE.md, indexed);
2. `touch`: `WorldInfo.Game.PickupQuery` (`GameInfo`'s script; #710's game info) → `HandlePickupQuery`.
   - `HasRoomInInventoryFor` (script). Its natives are all indexed: `GetInventorySpaceRequirement` (#713),
     `CountUnreadiedInventory`, `GetUnreadiedInventoryMaxSize`, `CountReadiedWeapons`, `GetWeaponReadyMax`,
     `IsUsingVehicle`;
   - then `InventoryActors` (#725) and `ItemActors` (bound). Both chains are empty, so nothing denies;
3. `touch`: `TouchedPickupable`:
   - `Pickupable_IsEnabled`, `GetPickupableInventory` and `GetInventoryDefinition` (all indexed);
   - `ShouldPlayerAutomaticallyPickup` (script) is false for a gun. The touch ends;
4. link 7 only, `look`: the pickup targeting (NATIVE_PICKUP_TARGETING.md). The headless world spawns no HUD, so this
   is its `NotYet { LayerAbsent }`, and link 7 stops there before `key`;
5. link 6, `key`: `PickupSomething`:
   - `GetCurrentPickupable`, `DenyPickupAttempt` and `AllowReadyAfterPickup` (script);
   - `GetInventoryPawn` and `CanAffordToPickUpPickupable` (indexed);
   - `HasRoomInInventoryFor` again;
   - then `ServerPickupSomething` → `PickupPickupable`;
6. `PickupPickupable`:
   - the globals (indexed) and `GetPickupableInventoryDefinition` (indexed);
   - `CanPickupInBulk`, script: the definition's `bPickupInBulk`;
   - `UpdateAmmoCounts`, `GetPawnInventoryManager` and the two counts (indexed);
   - `HasRoomInInventoryFor` and `PickupQuery` again;
7. **`ClientSpawnPickupableMesh`** (script), called right after the query passes → `WillowPickup.GetPickupableMeshActor`
   (script). That spawns an `Engine.PickupableMeshActor` at the pickup's location, with the rotation
   **`WillowPickup.GetSaveRotation()`**: **a native with no note, the first unread item. Sections 3 to 5 read it.**
   The argument is evaluated before `Actor.Spawn` runs;
8. then the gun's `AttachMeshToPickup` on the mesh actor (NATIVE_ATTACH_MESH_TO_PICKUP.md's chain), and
   `PickupableMeshActor.SetTarget(Pawn)` (script: one field);
9. `GiveTo` → `WillowPickup.GiveTo` → `foreach Actor.ComponentList(class'WaypointComponent', ...)`. That is a native
   iterator with no index row, the next unread item. Then `DroppedPickup.GiveTo`, `AnnouncePickup`, `Inventory.GiveTo`
   and `AddInventory` (NATIVE_INVENTORY_EQUIP.md), then `PickedUpBy` (the pickup's destroy);
10. the end of `PickupPickupable`: `WorldInfo.AllControllers` (NATIVE_ENGINE_CORE.md) for `UnTouchedPickupable`.

## 3. Evidence for GetSaveRotation

- **bl-sdk live reads:** none. No session was granted.
- **The script:** `WillowPickup.GetSaveRotation()` is a native returning a rotator. Its callers are
  `GetPickupableMeshActor` (above) and `GetMemento`, which writes the pickup's rotation into its save record (not on
  the slice).
- **The native:** Ghidra 12.1.4 headless, read-only, under the machine lock. The exec calls one virtual of the
  receiver. That slot was read in `WillowPickup`'s primary class table, found as in NATIVE_COMPONENT_SKIP_FLAG.md
  section 6, and the routine it reaches was read.
- **Field names:** the layout tool places `Actor.Physics`, `Actor.CollisionComponent` and `Actor.Rotation` exactly
  where the routine reads them. The physics value it tests is 10, `PHYS_RigidBody` by `EPhysics`' order.

## 4. What GetSaveRotation answers

1. **A rigid-body pickup.** When the pickup's `Physics` is `PHYS_RigidBody` and its `CollisionComponent` is set, the
   component is asked for its rigid body (a component virtual; by fit, its root body instance). When it has one:
   - the body's pose is read: position, orientation, linear and angular velocity, and whether it is awake;
   - **the answer is the body's orientation, as a rotator.** It is the usual quaternion-to-rotator step: the yaw,
     pitch and roll from the rotation matrix, in 65536-per-turn units, rounded.
2. **Otherwise** (any other `Physics`, no `CollisionComponent`, or no body): **the actor's own `Rotation`.**

**Purpose** (by fit): a physics-driven pickup's resting orientation is its body's, which the actor's `Rotation` may
lag. The save record and the pickup's display mesh both want the body's.

## 5. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `WillowPickup.GetSaveRotation()` in Off. Reads are loaded-only and by name;
it writes nothing:
1. `Physics` (a byte), compared with `PHYS_RigidBody`. The value is found in the declared `EPhysics` enum by name,
   as `pickup_rb.rs` finds it, by a lookup only. Not `PHYS_RigidBody`: **the actor's `Rotation`.**
2. `CollisionComponent` None: **the actor's `Rotation`.**
3. `PHYS_RigidBody` with a `CollisionComponent`: the body query. The port has no rigid bodies. #574's ruling makes the
   pickup's rigid-body set-up **at rest** (`pickup_rb.rs`: `Physics` = `PHYS_RigidBody`, no body made). Two answers;
   **the hub's call:**
   - **default:** `NotYet { LayerAbsent }`, "WillowPickup.GetSaveRotation: a rigid-body pickup's body pose", with
     layer "a physics layer (rigid bodies)". In the game the body exists, and its orientation is the answer;
   - **with the hub's ruling, extending #574's "at rest":** the port's body query answers "no body", so the game's own
     fallback applies, **the actor's `Rotation`**, with a log line naming it as an UNVERIFIED stand-in. A body at rest
     on the ground may still be turned from the actor's spawn rotation, so this is the stand-in's cost.
4. Malformed `Physics`, `CollisionComponent` or `Rotation`: `VmError::Script` naming it.

**What link 6 would meet** (a prediction, not a run). The pickup went through `InitializePickupForRBPhysics`, so its
`Physics` is `PHYS_RigidBody` and its `CollisionComponent` is the mesh component. Under the default, link 6 stops
here with the physics `LayerAbsent`. Under the ruling, `GetSaveRotation` answers the pickup's `Rotation`. The spawn of
the `PickupableMeshActor` follows, then `AttachMeshToPickup` on it. The next unread item after that is
`Actor.ComponentList` in `WillowPickup.GiveTo` (section 2, item 9).

## 6. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each refusal leaves #518's three counters and the name table unchanged:
1. a toy `WillowPickup` with `Physics` `PHYS_Falling` and a `Rotation` (1, 2, 3): the answer is (1, 2, 3);
2. `PHYS_RigidBody` with `CollisionComponent` None: the `Rotation`;
3. `PHYS_RigidBody` with a `CollisionComponent`: the default `LayerAbsent`, nothing written;
4. the same under the hub's ruling (if given): the `Rotation`, and the stand-in's log line;
5. `EPhysics` missing `PHYS_RigidBody` by name: `VmError::Script`, and no class-code load;
6. `Physics` or `Rotation` of the wrong kind: `VmError::Script`;
7. through `GetPickupableMeshActor` on a toy pickup: the spawned `PickupableMeshActor`'s `Rotation` is the answer;
8. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above. The branch and the fallback are read. The component's body query and the body's
   pose readers are fits by their use.
2. **The hub's call:** section 1 (a VM rule for `new`'s template, and the pickup fixture's gun init), and section 5
   step 3 (the default or the at-rest stand-in).
3. **Not read:** `Actor.ComponentList` (section 2, item 9), the next unread item.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Drop a gun, let it settle, and read the pickup's `Rotation` and `GetSaveRotation()`. Expect them to differ while
   the body has turned it, and the display mesh actor spawned at pickup to take `GetSaveRotation()`.
