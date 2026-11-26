# Links 6 and 7 in `touch`: the component template, and the touch path after the bounds (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK67-TOUCH-TEMPLATE (decision 2026-11-26-NNN), the analyst half of
the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/touchtpl/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule
here is UNVERIFIED in the running game.**

**Why.** On main, link 6 (1/3) and link 7 (1/4) stop in `touch` at "Actor.GetBoundingCylinder: the component's
template (option B) is not read" (`NotYet { NotRead }`).

## 1. Where the refusal is raised, and which note it follows

- **The code:** `ow-world` `skeletal_bounds.rs`, `TEMPLATE`, raised by `skeletal_box` at **source e**: a live,
  recorded, colliding `SkeletalMeshComponent` entry with no `SkeletalMesh`.
- **The note:** NATIVE_COMPONENT_BOUNDS.md section 9.5 and option B of section 9.6 (the recommended option, "honest
  about the input"), as the hub revised it in review r1 of #707. A component with no mesh is not answered at all,
  because in the port that is always the class-default input that the game never has.
- **Why the component has no mesh:** `WillowWeapon.AttachMeshToPickup` makes the pickup's component with
  `new(Pickup) class'SkeletalMeshComponent'(ThirdPersonMesh)`. The port's `new` ignores the template, so the component
  holds `Engine.Default__SkeletalMeshComponent`'s values. NATIVE_PICKUP_SAVE_ROTATION.md section 1 (#729) lists what
  option B needs: a decoder change, an interpreter change, and a fixture that runs the gun's init. This note reads
  what the game does for that `new`.

## 2. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`): `DroppedPickup`'s `Pickup` state `Touch` and `ValidTouch`,
  `GetTouchTraceLocation`, `GameInfo.PickupQuery`, `WillowInventoryManager.HandlePickupQuery` and
  `HasRoomInInventoryFor`, `InventoryManager.HandlePickupQuery`, `WillowPlayerController.TouchedPickupable`, and the
  definitions' `ShouldPlayerAutomaticallyPickup`.
- **The natives:** Ghidra 12.1.4 headless, read-only, under the machine lock:
  - the script `new` operator's handler, found by the warning text it reports for bad flags;
  - the object construction it calls, its allocation step, and the property initialisation;
  - `Pickupable_IsEnabled`'s exec (shared by `DroppedPickup` and `WillowPickup`). It calls one method of the
    pickup interface table. That table was read in `WillowPickup`'s constructor chain, found as in
    NATIVE_COMPONENT_SKIP_FLAG.md section 6, and the routine in its slot was read.
- **A structural census of `new`** (local, not a change to any tracked decoder):
  - a copy of `research/script_disasm.py` decoding `New` with **five** operands decodes the same 12,968 of 12,978
    script functions as the four-operand table. Memory sizes meet and jumps land on statements in both;
  - with four operands, every one of the 225 `New` expressions in the packages is followed by a bare expression.
    189 times it is a `Nothing` statement (no template given). The other 36 times it is a bare value, such as
    `AttachMeshToPickup`'s `InstanceVariable(ThirdPersonMesh)`, or a value consumed as an extra call argument.
    A bare value or `Nothing` statement does nothing at statement level, so these are the template operand.
  - **So `New` has five operands in BL2's bytecode** (high): the handler reads five, and the census fits.
- **Field places** (the local copy of `tools/ghidra/class_layout.py`):
  - `DroppedPickup`'s pickup interface table sits exactly where the constructor stores it, and so do
    `WillowPickup`'s three later interface tables;
  - the routine's bool read lands one 16-byte step after the layout tool's place for `WillowPickup.bPickupable`. That
    is exactly the padding a 16-byte-aligned quaternion in `RBState` (`Actor.RigidBodyState`) adds before it. The tool
    aligns `Quat` to 4. The field is the first bit of `WillowPickup`'s first bool group. **The 16-byte alignment is
    fitted (UNVERIFIED)**; the fit is exact, with no other candidate field.

## 3. What the game's `new` does with a template

### 3.1 The operator

The script `new` reads **five operands, in order**: outer, name (a string), flags (an int), class, template. Then:
1. **no class:** a script error ("no class passed to new"), and no object;
2. **flags outside the script-settable set:** a script warning. The flags passed on are masked to that set;
3. **an `Actor` class** (any class whose chain reaches `Actor`): a script error ("use Spawn"), and the answer is
   None;
4. **no outer:** the transient package;
5. **an empty name:** the engine picks a unique name (class name and number). Otherwise the name is made from the
   string;
6. the object is constructed with that class, outer, name, flags and template. When a template is given, the call
   also makes the new object its own subobject root. That is what turns on the transient reset of 3.2 step 2. How
   instanced subobject references in the template are then treated was not read.

### 3.2 The construction with a template (which template, which fields, any load)

- **Which template:** the fifth operand's value, as is. For links 6 and 7 that is the weapon's `ThirdPersonMesh`
  component.
  - **In the game,** that is the weapon's own component, instanced at the weapon's spawn. Its `SkeletalMesh` is set
    by `InitMeshes`, the composite third-person mesh on link 8's path.
  - **In the port's fixture,** whether it is the weapon's own copy or the shared class-default subobject follows the
    subobject-instancing ruling (#435). Either way its `SkeletalMesh` is None, because the fixture never runs the
    gun's init (#729 section 1, part 2).
- **The new object's `ObjectArchetype`** becomes the template. Without one, it is the class default object.
- **Which fields:**
  1. **every property value is copied from the template**, over the template class's property size. Fields that
     need construction (strings, dynamic arrays, structs holding them) are then deep-copied: a new array with the
     same elements, and object references shared, not duplicated;
  2. **then every transient property is reset to the class default object's value.** With a template given, the
     transient fields do not come from the template. For a skeletal mesh component that covers the runtime state the
     declarations mark transient (for example `Bounds`, `SpaceBases` and `Animations`, if their declarations carry the
     transient flag; check each in the store);
  3. then the class's own constructor runs;
  4. a class flagged as owning components gets its components instanced from the archetype. Whether
     `SkeletalMeshComponent` carries that flag was not read.
- **Loads: none.** The template is an object already in memory, and the class default object exists for a loaded
  class. Nothing is loaded from a package.

### 3.3 What the port's `new` would need (option B, restated with this reading)

These are two VM changes, both **the hub's call**:
1. **the decoder** (`ow-script` `ops.rs`): `New` with five operands. The handler's read and the census above are the
   evidence. `research/script_disasm.py` would change with it. This is a bytecode-layout change (rule 4): rerun the
   census in the same commit (`--check` totals unchanged, and no bare `Nothing` or value statement after a `New`);
2. **the interpreter** (`ow-vm` `interp/eval.rs`, `new_object`), Off:
   - evaluate the five operands in order;
   - keep today's answers for no class (None), and refuse an `Actor` class as the game does (None, with the script
     error logged);
   - with a template:
     - copy its property values by declared property, deep-copying arrays and strings;
     - reset each property declared transient to the class default's value;
     - set the archetype to the template;
   - **preflight, write-free and load-free:**
     - a template that is a store handle not loaded yet is `NotYet { NotRead }`, "new: a template the store has not
       loaded (a load is a write)";
     - a template whose class is neither the new class nor a subclass of it is `NotYet { NotRead }`, "new: a template
       of another class" (that check was not read);
     - a class that owns components, with a template whose components would be instanced, is `NotYet { NotRead }`,
       "new: instancing a template's components";
   - the made object, and the unique name it interns, are the writes. Nothing is loaded.
     `body_composition.rs`'s `make_component` copies slots the same way, but it loads its template, which this rule
     must not do.

### 3.4 Links 6 and 7 with option B (a prediction, not a run)

1. **With option B alone,** the copied template has no mesh (the fixture skips the gun's init), so `skeletal_box`
   meets source e again: the same `TEMPLATE` `NotRead`. The refusal's test would need to know that the component's
   template has no mesh either. Then source e's base rule answers the game's own input, and the hub can retire the
   refusal for that case.
2. **With the gun's init run as well** (link 8's `InitMeshes` path), the template holds the composite third-person
   mesh. The component takes **source f**, the mesh's stored bounds. That mesh is built at run time, not a package
   export, so `skeletal::mesh_bounds` answers none (it finds no export). **The next stop is source f's
   `NotYet { LayerAbsent }`, "Actor.GetBoundingCylinder: a skeletal mesh component's bounds", layer "a skeletal
   mesh's stored bounds".** The composite build's bounds were not read (NATIVE_INIT_GESTALT_MESH_DATA.md's chain).

## 4. The rest of `touch` after the bounds

The `Pickup` state's `Touch` casts the toucher to a pawn (not a vehicle), asks `ValidTouch`, and on true calls the
controller's `TouchedPickupable`. In order, marking what main covers:

| # | Step | Kind | On main |
|---|---|---|---|
| 1 | `ValidTouch`: the pawn checks (`bCanPickupInventory`, and so on) and the falling check | script | runs |
| 2 | `GetTouchTraceLocation` → `GetBoundingCylinder` | script, native | **today's stop** (section 1) |
| 3 | `TouchPickupTrace(Pawn.Location, that point)`. False: a 0.5 s `RecheckValidTouch` timer, and the touch ends | native | bound (`ow-inventory` `pickup.rs`, `ow-world` `touch_trace.rs`) |
| 4 | `WorldInfo.Game.PickupQuery(Pawn, class, pickup)`: no `BaseMutator`, so `Pawn.InvManager.HandlePickupQuery` | script | runs (#710's game info) |
| 5 | `WillowInventoryManager.HandlePickupQuery` → `HasRoomInInventoryFor`: `GetPickupableInventory`, `GetInventorySpaceRequirement`, and with a limited inventory `CountUnreadiedInventory` and `GetUnreadiedInventoryMaxSize`. With room, the weapon branch (`CanBeUsedBy`, `CountReadiedWeapons`, `GetWeaponReadyMax`) is not reached | script, natives | all bound (#713 and the manager natives) |
| 6 | `InventoryManager.HandlePickupQuery`: `InventoryActors` with each entry's `DenyPickupQuery`, then `ItemActors` the same | script, natives | bound (#725, decision 2026-11-19-67). Maya's chains are empty |
| 7 | `WillowPlayerController.TouchedPickupable`: **`Pickupable_IsEnabled`** | native | **unbound**. Read here, section 5 |
| 8 | `GetPickupableInventory`, `GetInventoryDefinition`; `CurrentTouchedPickupable` set | natives, script | bound |
| 9 | `ShouldPlayerAutomaticallyPickup(controller)`: false for a gun's definition (only `MissionItemDefinition` overrides it), so the touch ends | script | runs |

After `touch`:
- **link 7's `look`** is the pickup targeting's HUD `LayerAbsent` (NATIVE_PICKUP_TARGETING.md);
- **link 6's `key`** follows NATIVE_PICKUP_SAVE_ROTATION.md section 2, steps 5 to 10. Its unbound natives, in order,
  are:
  - `CanAffordToPickUpPickupable` (indexed in NATIVE_INVENTORY_EQUIP.md);
  - `UpdateAmmoCounts` (indexed, body not read);
  - `GetSaveRotation` (#729);
  - the `ComponentList` iterator (not indexed).

## 5. `DroppedPickup.Pickupable_IsEnabled()` / `WillowPickup.Pickupable_IsEnabled()`

**What it does:** the exec asks the pickup interface for its enabled state. **`WillowPickup`'s answer is its
`bPickupable`**, a plain read. Nothing is written. `DroppedPickup`'s own slot was not read.

NATIVE_INVENTORY_EQUIP.md already describes `bPickupable`: true by default, set by `SetPickupability`, and false while a
mission item's dependencies are not met.

**Off answer:** bind both keys, Off only. Reference keeps the stub policy:
- a receiver of class `WillowPickup` or a subclass: its `bPickupable` (the bool declared, else `VmError::Script`).
  Nothing is written or loaded;
- any other receiver: `NotYet { NotRead }`, "Pickupable_IsEnabled on <class>: its own answer is not read".

**Tests:**
- a toy `WillowPickup` with `bPickupable` set gives true; clear gives false. No write is counted;
- a toy `DroppedPickup` that is not a `WillowPickup` gives the `NotRead`;
- a subclass of `WillowPickup` with no field of its own reads the inherited `bPickupable`.

**Maya's pickup:** the slice's `WillowPickup` keeps the default, true. So `TouchedPickupable` goes on to steps 8 and 9,
and the touch ends.

## 6. Tests for the option-B interpreter rule (synthetic fixtures)

1. `new(o) class'C'(T)` with `T` a toy `C` whose int field is 7: the new object's field is 7, and its archetype is `T`;
2. the same with a transient int field set to 9 on `T` and 2 on the class default: the new object's field is 2;
3. a dynamic array of objects on `T`: the new object has its own array with the same elements. Changing one array
   leaves the other alone;
4. a template that is an unloaded store handle: the not-loaded `NotRead`, no object made, no name interned;
5. no template (`Nothing`): the class defaults, as today;
6. an `Actor` subclass: None, the script error logged, no object made;
7. **the decoder:** a synthetic function `Let(x, New(o, Nothing, Nothing, C, T)); Return(x)` decodes as two
   statements, and its memory size meets the header.

## 7. Predicted next stop

- **Today:** unchanged, the `TEMPLATE` `NotRead`.
- **After option B and the gun's init:** source f's stored-bounds `LayerAbsent` for the run-time composite mesh
  (3.4).
- **Once bounds answer:** `TouchPickupTrace` (bound) and the pickup query (bound), then **`Pickupable_IsEnabled`
  (unbound; section 5's rule)**. Then the touch ends, and link 7 meets the HUD `LayerAbsent`.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - whether `SkeletalMeshComponent` owns components (the instancing step);
   - the template class check, if the construction makes one;
   - `DroppedPickup`'s own `Pickupable_IsEnabled` answer;
   - the composite mesh's bounds.
3. **Fitted:** `RBState`'s 16-byte alignment (section 2). If it holds, fields after `RBState` in `WillowPickup` sit
   16 bytes later than the layout tool says.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Drop a gun. On the pickup's mesh component, read `ObjectArchetype`: expect the weapon's `ThirdPersonMesh`. Read
   `SkeletalMesh`: expect the weapon's composite mesh.
2. On the same component, read `Bounds` right after the drop: expect a box around the gun, not a copy of the
   weapon component's.
3. Toggle a pickup's `bPickupable` with `SetPickupability(false)` and walk over it: expect no pickup prompt.
