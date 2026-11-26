# Links 6 and 7: the pickup gun's init before `touch` (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK67-PICKUP-GUN-INIT (decision 2026-11-26-NNN), the analyst half
of the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/gunin/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule
here is UNVERIFIED in the running game.**

**Why.** Once option B's template copy lands (NATIVE_TOUCH_COMPONENT_TEMPLATE.md, NATIVE_NEW_COMPONENT_INSTANCING.md),
links 6 and 7 are predicted to pass `make` and meet `skeletal_bounds`' `TEMPLATE` `NotRead` in `touch` (source e).
The slice fixture's gun never runs its init, so the third-person mesh it hands to the pickup has no mesh. This note
reads what the game does for a dropped or spawned pickup's weapon, and what the harness must run to match it.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`):
  - `WillowWeapon.InitializeFromDefinitionData` and `InitializeInventory`, and `InitializeInternal`;
  - `ClientInitializeFromWeaponTypeDefinition` and `InitMeshes`;
  - `WillowInventory.DropFrom`, `WillowPickup.InitializeFromInventory` and `InventoryAssociated`;
  - `WillowWeapon.AttachMeshToPickup`.
- **The port's code on main 8fa9839d:**
  - `ow-cli` `slice_pickup.rs` (`make`, `drop_at`) and `inventory/equip_run.rs` (`prepare`, the routes, the
    preloads, the phases);
  - `slice_gate.rs`, the link 8 driver and `run_pickup`;
  - `ow-inventory` `gestalt.rs`, `attach_mesh.rs` and `attach_mesh/pickup_rb.rs`;
  - `ow-world` `skeletal_bounds.rs`, `skeletal.rs` and `actors/component_attach.rs`.
- **Data** (`ow-package --def-dump`, local):
  - `GD_Weap_AssaultRifle.A_Weapons.WT_Bandit_AssaultRifle` (the slice's `EQUIP_WEAPON` type):
    `ThirdPersonMeshScale` 0.92, `GestaltMesh` `Weap_AssaultRifles.GestaltDef_AssaultRifle`, and both bone-to-hide
    names None;
  - `WillowGame.Default__WillowWeapon.ThirdPersonWeaponMesh`: `SkeletalMesh`, `AnimTreeTemplate`, `Animations` and
    `PhysicsAsset` None, `Scale` 1;
  - property flags from `Engine.upk`: `Animations`, `SkeletalMesh`, `PhysicsAsset`, `Scale`, `Scale3D`, `Materials`
    and `GestaltData` are **not** transient; `Owner`, `Scene`, `bAttached`, `Bounds`, `SpaceBases`, `AnimTickArray`,
    `bHasValidBodies` and `FramesPhysicsAsleep` are transient (so #743's copy takes them from the class default).

## 2. What the game does

### 2.1 The gun is initialised when it is made, long before any drop

A weapon is made from its definition data (`CreateWeaponFromDef`, a save, a trade or a loot pool all go through
`InitializeFromDefinitionData`, and `InitializeInventory` for one made otherwise). Both call `InitializeInternal`. That
calls `ClientInitializeFromWeaponTypeDefinition`, which:
1. clears both meshes' `SkeletalMesh` (`SetSkeletalMesh(None)` on `FirstPersonMesh` and `ThirdPersonMesh`);
2. later calls `InitMeshes`.

`InitMeshes`, in order:
1. **`BuildCompositeMesh(FirstPersonMesh, DefinitionData)`.** For a type with a gestalt definition (the slice's
   rifle) this is the gestalt path. It fills the first-person component's `GestaltData` and **answers the gestalt
   definition's shared `GestaltSkeletalMesh`** (NATIVE_BUILD_COMPOSITE_MESH.md; `ow-inventory` `gestalt.rs`). That
   is a package export, the same object for every rifle of the type; the parts live in `GestaltData`;
2. `ThirdPersonMesh.InitGestaltMeshDataFromOther(FirstPersonMesh)`: the third-person component takes the same
   `GestaltData`;
3. **`SetSkeletalMesh(that mesh)` on `FirstPersonMesh`, then on `ThirdPersonMesh`.** This is what gives the
   third-person mesh its `SkeletalMesh`;
4. `ThirdPersonMesh.SetScale(type's ThirdPersonMeshScale)` (0.92 for the slice's rifle) and `HideBonesInMesh` on it;
5. the weapon material (`BuildWeaponMaterial`), set at index 0 on both meshes; then the part particles, the effects
   and the visible ammo bones, and the extra slots.

### 2.2 The drop comes after, and the pickup copies the initialised mesh

`WillowInventory.DropFrom` makes the pickup (`CreatePickup`), sets its velocity and torque, then calls the pickup's
`InitializeFromInventory(self, Instigator, true)`. That sets the pickup's `Inventory` and calls `InventoryAssociated`,
which:
1. drops any old collision component;
2. calls **`Inventory.AttachMeshToPickup(pickup)`**: `new(Pickup) class'SkeletalMeshComponent'(ThirdPersonMesh)`.
   The new component **copies the initialised third-person mesh**: its `SkeletalMesh`, `GestaltData`, `Scale` 0.92
   and `Materials` (#743 section 3.2). Then `SetAnimTreeTemplate(None)`, `SetForceRefPose(true)`, `SetHidden(false)`,
   `SetOwnerNoSee(false)`, `HideBonesInMesh`, `AttachComponent`, and the component becomes the pickup's
   `CollisionComponent`;
3. `SetLightEnvironment`, then `InitializePickupForRBPhysics`, then `PickupAssociated`.

Then `SetPickupability`. **`touch` comes later still**, when a pawn walks into the pickup. A loot pool's pickup goes
the same way: the pool makes the weapon (with its init), then its pickup (by fit, through the same
`InitializeFromInventory`; the pool's spawn call was not read).

## 3. What links 6 and 7's harness must run (the port)

Today `make` prepares each gun on the pickup routes (`prepare_line_typed`, `prepare_unowned_typed`). Those make the
weapon, Maya, her controller and manager, and nothing more: no manufacturer inputs, no preloads, and no
`InitializeInternal`. Link 8 prepares **the same weapons-file line** (`EQUIP_WEAPON`) with all of them, and runs the
init as its `init` phase. Link 8 passes on main, so every native that init reaches on this line is bound.

**The recipe, per gun, before `drop_at`,** in link 8's order:

| # | Step | Kind | On main |
|---|---|---|---|
| 1 | the weapon with the link 8 route's manufacturer inputs (`WeaponRoute::Link8` sets them; the pickup routes do not) | harness | covered (link 8) |
| 2 | `preload_muzzle_flashes`, `preload_gestalt_mesh`, `preload_gestalt_skeletal_mesh`, `preload_material_part_material`, `preload_material_classes` | harness loads (UNVERIFIED stand-ins for boot and streaming loads) | covered (link 8) |
| 3 | `install_mesh_layer` (the skeletal-mesh data source) | harness | covered (link 8); **the pickup world does not install it today** |
| 4 | `preload_threshold_attributes` | harness load | covered (link 8) |
| 5 | **`WillowWeapon.InitializeInternal(false)`** (`run_phase_typed("init")`): `BuildCompositeMesh`, `InitGestaltMeshDataFromOther`, `SetSkeletalMesh`, `SetScale`, `HideBonesInMesh`, `BuildWeaponMaterial`, `SetMaterial`, the effects | script and natives | covered: link 8 passes it on this line |

Then `make` goes on as today: `drop_at`, which clears `Owner` and `Instigator` and spawns and initialises the pickup.

**After the copy, the steps of `make` that now see a mesh** (they saw none before):

| Step | With a mesh | On main |
|---|---|---|
| `new(Pickup) SkeletalMeshComponent(ThirdPersonMesh)` | the copy | `NEW_INSTANCING` today; `NEW_TEMPLATE_COPY` until lane D's copy; the instancing pass does nothing (every component field None, NATIVE_NEW_COMPONENT_INSTANCING.md section 4) |
| `SetAnimTreeTemplate(None)` | needs `Animations` None and `AnimTickArray` empty. `Animations` is copied (not transient), but the template's is None: `InitMeshes` sets no tree | covered (the hub's ruling on NATIVE_SET_ANIM_TREE_TEMPLATE.md) |
| `SetForceRefPose`, `SetHidden`, `SetOwnerNoSee` | the deferred reattach; the copy's `Owner` and `Scene` are transient, so None and zero | covered (`attach_mesh.rs`) |
| `HideBonesInMesh` → `HideBoneByName` | the rifle names no bone to hide, so it is never called | covered |
| `AttachComponent` | the generic attach: `Owner` the pickup; no mesh-dependent step | covered (`component_attach.rs`) |
| `SetLightEnvironment` | one write | covered |
| `InitializePickupForRBPhysics` | #574's at-rest rule. It reads the definition's physics asset as a handle, not the mesh. It writes the component's `Scale3D` (1, 1, 1), its `PhysicsAsset` (the asset branch), and the pickup's `Physics` `PHYS_RigidBody`. **It does not touch `Scale`** | covered (`pickup_rb.rs`) |
| `SetPickupability` and the rest | as before | covered |

**Link 7's decoy** is an unowned gun (`prepare_unowned_typed`). The recipe gives it the same steps. Its init runs with
no owner, a case link 8 never runs: **not exercised on main** (UNVERIFIED). Whether the game's loot weapon has an
owner at its init was not read.

**The weapon's `ThirdPersonMesh` must be its own component** for the init's writes to land on what the pickup copies.
That follows the subobject-instancing ruling (#435), as link 8's weapon does.

## 4. The Off answer per step (for the implementer)

- **Harness only; no new native rule.** Each step above is a harness call that link 8 already makes, or the
  installed script.
- **Order:** steps 1 to 5 run inside `make`'s mark (`slice_pickup::build`'s `wrote`), after the weapon is made and
  before `drop_at`.
- **Refusals:** a preload's or the init's refusal after the weapon's creation is terminal for `make`, and is marked
  "stopped after partial writes: " (the existing `after_writes`), its kind and record kept. A refusal before the
  weapon exists is unprefixed. Each harness load logs its "harness input:" lines, as link 8's do, and
  `run_pickup` keeps them in the row's `harness`.
- **The mesh layer** (step 3) is installed once per world, as `install_mesh_layer` does: a world that already has it
  gets no second install.
- **Nothing new is loaded beyond link 8's preloads.** The init's own natives are write-free and load-free in their
  preflights, as their notes say.

## 5. Where `touch` then stops (a prediction, not a run)

The pickup's `GetBoundingCylinder` reads the copied component (`skeletal_bounds::skeletal_box`):
1. **not sleeping** (`FramesPhysicsAsleep` is transient, so 0), **visible** (`SetHidden(false)`), no fixed bounds and
   no parent anim component: sources a and b are skipped;
2. **sources c and d:** `PhysicsAsset` may be set by the at-rest rule, but `bHasValidBodies` is transient, so false,
   and no body exists: skipped;
3. **source e is not met**, because the copy holds the shared gestalt mesh;
4. **source f, the stored box.** The mesh is a package export (the gestalt definition's `GestaltSkeletalMesh`, resident
   by step 2's preload), so `skeletal::mesh_bounds` can find its export:
   - **without the mesh layer** (the pickup world as it is today): `NotYet { LayerAbsent }`, "Actor.GetBoundingCylinder:
     a skeletal mesh component's bounds", layer "a skeletal mesh's stored bounds". **That is where source f's
     stored-bounds `LayerAbsent` comes in** (NATIVE_COMPONENT_BOUNDS.md section 9.6);
   - **with the mesh layer** (step 3): the stored bounds answer, and then only the simplest world transform is
     answered. The combined scale is `Scale` 0.92 × `Scale3D` (1, 1, 1) × the pickup's draw scale 1, which is not
     exactly 1. **So the stop is the bounding cylinder's `WORLD_TRANSFORM` `NotRead`**, "Actor.GetBoundingCylinder:
     the component's world transform".

So the predicted order of stops on links 6 and 7:
1. today: `new`'s `NEW_INSTANCING`, in `make`;
2. with #749's rule: `NEW_TEMPLATE_COPY` (`NotPorted`), until lane D's copy lands;
3. with the copy and today's fixture: `TEMPLATE` `NotRead` in `touch` (source e, no mesh);
4. with the copy and this recipe without step 3: source f's stored-bounds `LayerAbsent`;
5. with the whole recipe: **`WORLD_TRANSFORM` `NotRead`** (the scale 0.92);
6. once a scaled transform is answered: `TouchPickupTrace` and the pickup query (bound), then `Pickupable_IsEnabled`,
   still unbound (its rule: NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 5).

## 6. A correction to two notes

NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 3.4 item 3 and NATIVE_NEW_COMPONENT_INSTANCING.md section 6 said that with
the gun's init the mesh would be "built at run time, not a package export", so source f's accessor would find no
export. **That is wrong for the slice's rifle.** On the gestalt path `BuildCompositeMesh` answers the shared
`GestaltSkeletalMesh`, an export (section 2.1). Source f's `LayerAbsent` comes only from a missing mesh layer, and with
the layer the stop moves on to `WORLD_TRANSFORM` (section 5). A non-gestalt weapon with two or more part meshes takes
the merge (`MERGE_LAYER`), which does make a new mesh. The slice's rifle does not.

## 7. Tests (synthetic fixtures)

1. **The order:** with a toy weapon whose toy `InitializeInternal` logs a line and sets its `ThirdPersonMesh`'s
   `SkeletalMesh`, `make` logs the init before the pickup's `InitializeFromInventory`.
2. **The init's refusal:** a toy init that refuses `NotYet { NotRead }` makes `make` answer it with the
   partial-writes prefix, and no pickup is spawned.
3. **The decoy:** link 7's make runs the same steps for the decoy, after the first gun's drop.
4. **The mesh layer:** a world that already has the layer gets no second install (the log line once).
5. **`skeletal_box` with an export mesh:**
   - no layer: the stored-bounds `LayerAbsent`;
   - a layer answering toy bounds, with `Scale` 0.92: `WORLD_TRANSFORM`;
   - with `Scale` 1: the box answered (origin plus root offset, extent × `BoundsScale`).

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - whether the game's skeletal bounds for a gestalt-mode component use the shared mesh's stored bounds, or a box
     built from the parts;
   - the loot pool's call that makes the pickup;
   - the decoy's init with no owner.
3. **The hub's call:** whether a scaled world transform should be answered in `skeletal_bounds` (the next stop once
   the recipe lands).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Drop a Bandit rifle. On the pickup's mesh component, read `SkeletalMesh` (expect the gestalt definition's
   `GestaltSkeletalMesh`), `Scale` (expect 0.92) and `Scale3D` (expect 1, 1, 1).
2. Read the same component's `Bounds` after the drop, and the pickup's `GetBoundingCylinder` answer.
