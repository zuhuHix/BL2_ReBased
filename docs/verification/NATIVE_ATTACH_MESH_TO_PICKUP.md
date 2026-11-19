# Native rules of WillowWeapon.AttachMeshToPickup and the rest of the pickup's make step (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-ATTACH-MESH-TO-PICKUP (decision 2026-11-19-117, number to be
given by the hub).

**Sources:**
- the installed script, read with `research/script_disasm.py`, and a scan of every decodable script function of the
  nine code packages;
- the class layouts, from the cooked packages through `tools/ghidra/class_layout.py`;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling: read-only queries under the machine lock, plus
  the constructor-to-class-table method of `docs/NATIVE_ANALYSIS.md` ("When `virtual:` refuses a class") for the two
  natives that hand to a class-table entry.

There is no bl-sdk session and no recorded live evidence on these natives. Raw output stays under ignored
`local/analysis/attachmesh/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. Field names come from the cooked packages. **Every rule here is UNVERIFIED** in the running game.

**Clean room.** This lane implements nothing. A fresh implementer codes the Off rules from this note alone.

**Why it matters.** With #492's ruling (the `SetAnimTreeTemplate` registry walk answers "nothing" in Off), c376
expects slice links 6 and 7 to stop next in their `make` step at the unbound `SkeletalMeshComponent.SetForceRefPose`,
inside `WillowWeapon.AttachMeshToPickup`.

**Scope.** Two parts, as the hub allowed:
- part 1 (sections 1 to 5, #505) covers the natives from `SetForceRefPose` through `Actor.AttachComponent`;
- part 2 (section 6) covers the rest of the `make` step: `SetLightEnvironment`, `InitializePickupForRBPhysics` and
  `SpawnPickupParticles`, with where links 6 and 7 stop once the hub's scene-layer ruling on section 5 is in.

## Summary

| Native (VM key) | Does | Writes | Confidence | Off answer |
|---|---|---|---|---|
| `SkeletalMeshComponent.SetForceRefPose(bool)` | stores the flag, then the deferred reattach (2) | `bForceRefpose` (UNVERIFIED (fitted)), `bNeedsReattach` | medium | the rule of 3.1 |
| `PrimitiveComponent.SetHidden(bool)` | when the flag changes: stores it, then the deferred reattach | `HiddenGame` (UNVERIFIED (fitted)), `bNeedsReattach` | medium | the rule of 3.2 |
| `PrimitiveComponent.SetOwnerNoSee(bool)` | the same for the owner-no-see flag | `bOwnerNoSee` (UNVERIFIED (fitted)), `bNeedsReattach` | medium | the rule of 3.2 |
| `SkeletalMeshComponent.HideBoneByName(name,byte)` | looks the bone up in the component's skeletal mesh; on a match, hides it | none when there is no mesh or no match | medium | nothing with no mesh or a None name; otherwise `NotRead` (3.3) |
| `Actor.AttachComponent(object)` | attaches the component to the world's scene with this actor as its owner, then lists it in `Components` | the component's owner and flags, `Components`, the scene | medium (outline) | with the slice world's empty scene layer: the flag clears, `Owner`, the `Components` append (hub ruling, 3.4); without it `NotYet`, kind `LayerAbsent` |
| `PrimitiveComponent.SetLightEnvironment(object)` | when the environment changes: stores it, keeping the old one and reattaching only while attached | `LightEnvironment`, while attached `PreviousLightEnvironment` and `bNeedsReattach` | medium | the rule of 6.2 |
| `WillowPickup.InitializePickupForRBPhysics()` | with `bUseRBPhysics`, sets the pickup up as a rigid body (outline) | the pickup's physics fields, timers and `Physics`; its mesh's collision and rigid-body fields (table in 6.6) | medium (outline) | nothing without `bUseRBPhysics`; otherwise `NotYet`, kind `LayerAbsent` (6.3) |
| `WillowPickup.SpawnPickupParticles()` | makes, attaches and starts the pickup's particle component (outline) | `PickupParticleComponent`, the new component | medium (outline) | `NotYet`, kind `NotRead` or `LayerAbsent` (6.4) |

Each answer is **Off only; Reference keeps the stub policy** (the C++ binds none of them, and `Actor.AttachComponent`
is already a Reference stub in `crates/ow-cli/src/cmd/slice.rs`).

## 1. The script path (where the slice reaches them)

`WillowPickup.InventoryAssociated` (from `InitializeFromInventory`, which the `make` step calls) calls the inventory's
`AttachMeshToPickup(Self)`. For a `WillowWeapon`, `AttachMeshToPickup(Pickup)`:
1. makes a new `SkeletalMeshComponent` with the pickup as its outer and the weapon's `ThirdPersonMesh` as its template;
2. `SetAnimTreeTemplate(None)` (NATIVE_SET_ANIM_TREE_TEMPLATE.md, #492: "nothing" in Off by the hub's ruling);
3. `SetForceRefPose(true)`;
4. `SetHidden(false)`;
5. `SetOwnerNoSee(false)`;
6. `HideBonesInMesh(component)`, script (below);
7. the pickup's `AttachComponent(component)`;
8. the pickup's `CollisionComponent` becomes the component (a script write).

`WillowWeapon.HideBonesInMesh(InMesh)` reads the weapon's `DefinitionData.WeaponTypeDefinition`, and:
- when its `BoneToHideOnMesh` is not None, calls `InMesh.HideBoneByName(BoneToHideOnMesh, PBO_None)`;
- then the same for `AdditionalBoneToHideOnMesh`.

Reading those two names reads a field of the weapon's type definition. If that definition is still one of the
definition store's unloaded handles, the VM loads it at that read (`loaded_for_read` in
`crates/ow-vm/src/interp/load.rs`). That is the step's one possible **lazy load**. It comes after the `make` step's
earlier writes, so it changes no answer below.

**The component the slice sees.** The port's `new` ignores the template argument (NATIVE_SET_ANIM_TREE_TEMPLATE.md
section 4), so the component holds `SkeletalMeshComponent`'s class defaults. The implementer reads the **component
instance's** fields (`Owner`, `Scene`, `bAttached`, `HiddenGame`, `bOwnerNoSee`, `SkeletalMesh`), never the class
default object. They equal the class defaults today only because `new` ignores the template.

The class defaults, read with `ow-package --properties` on the default objects of `SkeletalMeshComponent`,
`MeshComponent` and `PrimitiveComponent`: none of the three lists `HiddenGame`, `bOwnerNoSee` or `SkeletalMesh`. So
each holds its zero value: false, false and None. At these calls, then, the slice's component:
- has no `Owner`;
- is not attached, and its `Scene` is empty;
- has no skeletal mesh;
- has `HiddenGame` and `bOwnerNoSee` false.

In the game it is made from the weapon's `ThirdPersonMesh` (open question 1).

## 2. The deferred reattach (shared by three natives)

`SetForceRefPose`, `SetHidden` and `SetOwnerNoSee` end with the same engine step on the component, called here the
**deferred reattach**:
1. it sets the component's `bNeedsReattach`;
2. **with an `Owner`:** when the owner passes a few checks of its own flags (not read in detail), it calls one of the
   owner's class-table entries, by its purpose the one that marks the actor's components for an update (not read).
   Nothing else happens to the component now;
3. **with no `Owner`:** the component is reattached on the spot:
   - when it has no `Scene`, nothing happens;
   - otherwise it is detached (when attached), its `Scene` and `Owner` are put aside and cleared, and then restored
     and attached again. An engine-wide notifier is also told about the component. These steps were followed only in
     outline.

The field identities come from the layout tool, which places `ActorComponent` exactly as the natives read it: `Scene`,
then `Owner`, then the bool word holding `bAttached` and `bNeedsReattach`.

**Off answer for the deferred reattach**, used by 3.1, 3.2 and part 2's 6.2:
- **Preflight, before any write of the calling native:**
  - `Owner` set: `NotYet { NotRead }`, "the deferred reattach: the owner's component update", record
    `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md`;
  - no `Owner`, and `Scene` not zero: `NotYet { NotRead }`, "the deferred reattach: the component's detach and
    reattach", record `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md`.
- **Otherwise** (no `Owner` and an empty `Scene`, the slice's case): the only write is `bNeedsReattach` = true.

`Scene` is declared in script as an `Object.Pointer` struct. "Not zero" means that **any field** of the port's value
of that struct is non-zero. The port sets no scene, so it stays at its zero value.

**After the scene layer's attach (3.4), `Owner` is the actor**, the pickup on the slice. So a later deferred reattach
on the same component takes the owner branch and answers the first `NotRead` above. That is the **expected next stop**
for any of these natives called on an attached component whose flag changes:
- `SetForceRefPose`, always;
- `SetHidden` and `SetOwnerNoSee` when the flag changes;
- part 2's `SetLightEnvironment` when the environment changes while `bAttached` is set.

The scene layer leaves `bAttached` as it was, so that last case does not arise on the slice.

## 3. The natives

### 3.1 `SkeletalMeshComponent.SetForceRefPose(bool bNewForceRefPose)`

1. It stores the argument, as 0 or 1, in an int field of the component. That the field is `bForceRefpose` is
   **UNVERIFIED (fitted)**:
   - the layout tool cannot lay out `SkeletalMeshComponent` (its map property `MorphTargetIndexMap` stops it);
   - the component declares two int fields with this meaning, `bForceRefpose` and `bOldForceRefPose`;
   - the name, and the native's own name, point to the first.
   The native writes one int field, and **`bOldForceRefPose` is never written** by it.
2. It always runs the deferred reattach (2). There is no "unchanged" short cut.

It returns nothing, raises no event and loads nothing.

**Off answer:**
1. Preflight: the deferred reattach's (2).
2. Write `bForceRefpose` = 1 for true, 0 for false (an int field). Do not write `bOldForceRefPose`.
3. Write `bNeedsReattach` = true.

On the slice: `bForceRefpose` = 1, `bNeedsReattach` = true, nothing else.

### 3.2 `PrimitiveComponent.SetHidden(bool NewHidden)` and `SetOwnerNoSee(bool bNewOwnerNoSee)`

Each hands to a class-table entry of the component. For `SetHidden`, `SkeletalMeshComponent`'s table and
`PrimitiveComponent`'s table hold the same entry, so the skeletal mesh does not override it. For `SetOwnerNoSee`,
`SkeletalMeshComponent`'s entry was read. Both entries do the same thing for their own flag:
1. when the flag already equals the argument, nothing;
2. otherwise they store the argument in the flag, then run the deferred reattach (2).

The flags are **UNVERIFIED (fitted)**: `HiddenGame` for `SetHidden` and `bOwnerNoSee` for `SetOwnerNoSee`. Each sits
at its position in `PrimitiveComponent`'s bool word as the layout tool gives it. But the word the native writes is not
the one the tool places them in, so the tool's placement is shifted here. The identity rests on the bit positions and
the names.

**Off answer**, for each:
1. Read the flag. Equal to the argument: return, no write.
2. Otherwise: preflight the deferred reattach (2), then write the flag, then `bNeedsReattach` = true.

On the slice the argument is false for both. The answer reads the component instance's flag, never the class
default object. Today that flag is the class default, false for both (section 1), so **on the slice neither native
writes**. In the game the template's values count (open question 1).

### 3.3 `SkeletalMeshComponent.HideBoneByName(name BoneName, byte PhysBodyOption)`

1. **The lookup.** The name is looked up in the component's skeletal mesh. When the name is None, or the component has
   no skeletal mesh, the answer is "no bone", and the native does nothing more. That the field read is the
   component's `SkeletalMesh` is **UNVERIFIED (fitted)**: the layout tool cannot lay out this class, but the object
   read is asked for its bone count.
2. **With a bone:**
   - when the component's `BoneVisibilityStates` does not have one entry per bone of the mesh, it is rebuilt (not
     read);
   - a per-bone transform held by the component is masked (not read in detail);
   - the bone's `BoneVisibilityStates` entry becomes 2, by fit `BVS_ExplicitlyHidden` of `EBoneVisibilityStatus`;
   - the visibility of the whole hierarchy is recomputed (not read), and one more field of the component is cleared
     (not identified);
   - with a non-zero `PhysBodyOption` and a physics asset instance, the bone's body is either terminated or disabled,
     by option (not read). The slice passes `PBO_None` (0), so this branch is not taken.

**Off answer:**
1. Name None, or the component instance's `SkeletalMesh` None: return, no write. This is the native's own answer,
   not a stand-in.
2. Otherwise `NotYet { NotRead }`, "SkeletalMeshComponent.HideBoneByName on a component with a skeletal mesh: the
   bone lookup and the visibility rebuild", record this note, before any write.

On the slice the component has no skeletal mesh (section 1), so each call answers "nothing", whatever bone names the
weapon's type definition holds.

### 3.4 `Actor.AttachComponent(ActorComponent NewComponent)` (outline)

On the actor it is called on:
1. **Guards.** Nothing happens when the component is None, when the actor's `bDeleteMe` is set, or when the actor
   carries an object flag that by fit is the pending-kill flag. In the port, that flag is the heap's destroyed state
   (`ow_object::heap::Heap::destroy`, which marks an object pending kill). This mapping is **UNVERIFIED (fitted)**,
   by the hub's ruling on review r1 of #505.
2. **The transform.** The actor's local-to-world transform is built from `Location`, `Rotation`, `DrawScale`,
   `DrawScale3D` and `PrePivot`. The layout tool places all five, and `Components` and `bDeleteMe`, exactly where the
   native reads them. With the actor's `bUseOrthonormalizedLighting` set, one more value from the actor (a class-table
   entry, not read) goes with it.
3. **The attach**, on the component:
   - when it is already attached, it is detached first;
   - `bNeedsReattach` and `bNeedsUpdateTransform` are cleared. The clear of both was **read**: one mask clears two
     bits of `ActorComponent`'s bool word. Their names come from the layout tool, which places that class exactly as
     the natives read it, so the identity is by layout and stays UNVERIFIED in the game;
   - its `Scene` becomes the world's scene, and its `Owner` the actor;
   - it is handed the transform and the extra value (two class-table entries, not read);
   - when one more entry of its own says it may be attached, its attach entry runs. That entry is the component
     type's registration with the scene: rendering, and for a primitive its collision and physics. It is not read,
     and whether it sets `bAttached` was not followed;
   - the engine-wide notifier of section 2 is told about it.
4. **The list.** The component is appended to the actor's `Components` array. There is no check for a duplicate.
5. The notifier is told once more.

It returns nothing and raises no script event in what was read.

**The guards, in Off:** a component of None, the actor's `bDeleteMe` set, or the actor destroyed in the heap
(above): a plain return, no write.

**Off answer with the scene layer (hub ruling 2026-11-19, review of #505).** A port answer, UNVERIFIED in the game.
1. **Where the layer lives.** The slice world installs an explicit, empty scene layer, in Off only. It registers
   nothing: no rendering, no collision, no physics.
2. **The layer test comes after the guards.** With no scene layer, the answer is the default below, with its
   existing texts.
3. **The one preflight.** When the component's `bAttached` is already set: `NotYet { NotRead }`, "Actor.AttachComponent
   on an attached component: the detach before the attach", record
   `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md`, before any write. It is a cautious port answer, UNVERIFIED:
   the native detaches first, and the detach was not read. Nothing else is preflighted beyond the guards.
4. **The writes, in the order read in the native, all unconditional:**
   1. the component's `bNeedsReattach` = false and `bNeedsUpdateTransform` = false;
   2. the component's `Owner` = the actor;
   3. the component appended to the actor's `Components`, with **no duplicate check**, as in the native.
5. **Not written:** `Scene`, which stays at its zero value, and `bAttached`, which stays as it was (whether the
   component's own attach entry sets it was not followed).

**Off answer without the scene layer (default):** after the guards, `NotYet::layer_absent(what, layer, record)`,
before any write, with:
- `what`: "Actor.AttachComponent: the component's attach to the world's scene";
- `layer`: "a scene that components attach to: render, collision and physics registration (the port has none)";
- `record`: `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md`.

Either `NotYet` is answered at entry: the native writes nothing itself. On the slice the `make` step has already
written, so its write tracking adds the "stopped after partial writes: " prefix and keeps the kind and record.

## 4. The `make` step, in order, with these answers (for the implementer)

These follow the hub's rules: preflight what is cheap and certain before any write; a `NotYet` after a write is
terminal, keeps its kind and record, and gets the prefix from the caller that wrote; a lazy load is a write; other
errors pass through unchanged.

On links 6 and 7, after #492's "nothing":
1. `SetForceRefPose(true)`: `bForceRefpose` = 1, `bNeedsReattach` = true (3.1);
2. `SetHidden(false)` and `SetOwnerNoSee(false)`: no write, since the component's flags are already false (3.2,
   section 1);
3. `HideBonesInMesh`: possibly a lazy load of the type definition (section 1); each `HideBoneByName` answers nothing,
   since there is no mesh (3.3);
4. `AttachComponent`, with the slice world's scene layer (3.4, hub ruling): `bNeedsReattach` and
   `bNeedsUpdateTransform` false, then `Owner` = the pickup, then the append to the pickup's `Components`. Without
   the layer, `NotYet { LayerAbsent }`, prefixed by the `make` step;
5. the script then sets the pickup's `CollisionComponent` to the component, and the step goes on (section 6).

Link 7 makes a second gun and drops it the same way, so the same answers apply to the decoy. The first stop ends the
step.

## 5. For the hub: `Actor.AttachComponent` without a scene (ruled)

**The ruling** (2026-11-19, review r1 of #505): yes, as an explicit layer. The answer is recorded in 3.4 ("Off answer
with the scene layer"). The question as it was put to the hub is kept below as the record.

- **The case for a port answer.** The script-visible part of the attach is small and was read:
  - the component's `Owner` becomes the actor;
  - its `bNeedsReattach` and `bNeedsUpdateTransform` are cleared;
  - it is appended to the actor's `Components`.

  The rest is registration with a scene that the port does not have. The slice's next statements read
  `CollisionComponent`, which the script sets itself.
- **Why the note does not take it.**
  - The component's own attach entry was not read. In the game it registers the pickup's collision, and the pickup's
    touch depends on that. The port's touch is driven by the harness, so nothing would read it on the slice. But a
    port answer would make it look as if collision existed.
  - Whether that entry sets `bAttached` was not followed.
  - `Actor.AttachComponent` is reached on other routes too: it is already listed as a Reference stub. So a ruling
    here binds it for every caller.
- **The two options.**
  - Keep the named `LayerAbsent`. Links 6 and 7 then stop here, prefixed.
  - Rule a port Off answer for a world without a scene, labelled UNVERIFIED:
    - after the guards, the three script-visible writes above, in that order (the flag clears, `Owner`, then the
      `Components` append);
    - `bAttached` left as it is (not read);
    - no registration.

    The next stop would then be the `make` step's next unbound native (section 6).

## 6. Part 2: the rest of the `make` step

### 6.1 The script order after `AttachMeshToPickup`

`WillowPickup.InventoryAssociated` goes on:
1. it takes the pickup's `CollisionComponent` as a `MeshComponent`: the new skeletal mesh, which
   `AttachMeshToPickup` set. It calls `SetLightEnvironment(DynamicLightEnvironment)` on it. In `WillowPickup`'s
   class defaults `DynamicLightEnvironment` is the default subobject `MyLightEnvironment`; in Off the value passed is
   the pickup's own spawned copy of it, since the spawn gives each actor its own copy of each template component
   (`ow_world::actors::spawn`, `crates/ow-world/src/actors.rs:568-571`, decision 2026-11-19-44). The mesh's
   `LightEnvironment` is its class default, None, so the two differ;
2. `InitializePickupForRBPhysics()`, native;
3. the inventory's `PickupAssociated(Self)`, script with an empty body (`WillowWeapon` does not override
   `WillowInventory`'s).

`InitializeFromInventory` then calls `SetPickupability(true)`, script:
- `bPickupable` becomes true;
- `RegisterMissionDirector`: for a weapon it reaches only the bound `GetInventoryDefinition`, and its cast to a
  mission item definition fails;
- `SetMissionMaterialParameter`: nothing for a non-mission item;
- `TryToSetInteractParticles(true)`. For a pickup that is not a mission director, when `PickupParticles` is None
  (the class default), it calls `SetInteractParticles` with the globals definition's `PartPickup`. That sets
  `PickupParticles` and `bNetDirty`, then calls the native `SpawnPickupParticles()`.

Then the harness's `GotoState('Pickup')`. `Object.GotoState` is bound, and `WillowPickup`'s `Pickup.BeginState` is
an empty body that does not call `DroppedPickup`'s. `DroppedPickup.AddToNavigation` (bound, NATIVE_PICKUP.md) belongs
to `DroppedPickup`'s `BeginState`, so it is not on this route as decoded.

### 6.2 `PrimitiveComponent.SetLightEnvironment(LightEnvironmentComponent NewLightEnvironment)`

It hands to a class-table entry. `SkeletalMeshComponent`'s table and `PrimitiveComponent`'s hold the same entry, so it
is not overridden. The entry:
1. when the argument is the current `LightEnvironment`, does nothing;
2. otherwise, while the component is attached (`bAttached`), copies the current `LightEnvironment` into
   `PreviousLightEnvironment`;
3. stores the argument in `LightEnvironment`;
4. while attached, runs the deferred reattach (2).

**The fields.** `bAttached` sits where the layout tool places it in `ActorComponent`. `LightEnvironment` and
`PreviousLightEnvironment` sit in `PrimitiveComponent` shifted from the tool's placement by the same amount as the
bool word of 3.2. That consistent shift in a second, independent field group supports the identities of 3.2 as well,
though both stay **UNVERIFIED (fitted)**.

**Off answer:**
1. The argument equals `LightEnvironment`: return, no write.
2. Not attached (`bAttached` false): write `LightEnvironment`, nothing else. This is the native's own rule.
3. Attached: preflight the deferred reattach (2) before any write. Then `PreviousLightEnvironment`, then
   `LightEnvironment`, then `bNeedsReattach` = true.

On the slice, under the hub's scene layer, `AttachComponent` leaves `bAttached` as it was (false). So the answer is
case 2: one write, `LightEnvironment` = the pickup's spawned copy of `MyLightEnvironment` (6.1), replacing the
class-default None. Had a scene layer set `bAttached`, case 3 would refuse, because `AttachComponent` set the
component's `Owner` to the pickup.

### 6.3 `WillowPickup.InitializePickupForRBPhysics()` (outline)

The whole body runs only when the pickup's `bUseRBPhysics` (declared by `DroppedPickup`) is set. Without it the native
does nothing. In `WillowPickup`'s class defaults, `bUseRBPhysics` is **true**, so the slice's pickup takes the body.
With it, in outline (each effect has a row in 6.6's table):
1. the pickup's rigid-body sleep set-up (a class-table entry of the pickup), then its `RotationRate`;
2. the pickup's primitive component (by fit, its `CollisionComponent`) is taken. With one, the native:
   - reads the globals definition;
   - sets three floats of the component to 1.0;
   - raises `CollideActors`, with a deferred reattach when it was not raised yet, and clears `BlockActors`;
   - sets up the rigid body from the inventory's definition, or calls another of the pickup's entries when the
     inventory has none;
   - copies three pickup fields from the globals definition;
   - sets the component's collision threshold and collision channels, with entries of the component between them;
3. the pickup's physics mode is changed (an entry of the pickup; its argument was not recovered);
4. with a component:
   - when `bTorque` is clear, a spin drawn from the globals definition is applied;
   - otherwise the pickup's `Torque` is applied, and `bTorque` is cleared when `bTorqueTemporary` is set;
5. a timer of 0.3 s, `EnableRagdollCollision`, is set on the pickup.

The layout tool places `bUseRBPhysics`, `Inventory`, `RotationRate`, `Torque`, `bTorque` and `bTorqueTemporary`
exactly where the native reads them.

**Nothing earlier on the route writes `bUseRBPhysics`.** Its script writers are `WillowInventory.DropFrom` (false,
only with its `bForceNoRBPhysics` argument), `WillowInventory.GetPickup` (false; called only by
`WillowGrenadeMod.GetPickup`) and `WillowPickup.AdjustPickupPhysicsAndCollisionForBeingDropped` (true; called only by
`WillowPawn.Died`). The harness replaces `DropFrom` with the spawn and `InitializeFromInventory`, and neither the
spawn's begin-play events, `InitializeFromInventory` nor `InventoryAssociated` writes it. So the class default holds
when the native runs.

**Off answer:**
1. `bUseRBPhysics` false: return, no write. This is the native's own rule.
2. Otherwise, before any write, `NotYet::layer_absent(what, layer, record)` with:
   - `what`: "WillowPickup.InitializePickupForRBPhysics: the pickup's rigid-body set-up";
   - `layer`: "a rigid-body physics layer (the port has none)";
   - `record`: `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md`.

The body is mostly rigid-body physics: the body set-up, the collision channels, the spin and the torque. Its
script-visible writes are interleaved with that work, and 6.6 shows they do not separate from it. So no part of it is
offered as a port answer. The `make` step adds the "stopped after partial writes: " prefix.

### 6.4 `WillowPickup.SpawnPickupParticles()` (outline; not reached on the slice in default Off while 6.3's stop stands)

It hands to a class-table entry of `WillowPickup`, resolved with the constructor-to-table method. The entry:
1. **No particle system:** when `PickupParticles` is None, it calls another of the pickup's entries (by its purpose,
   the particles' removal; not read) and returns.
2. **The component:** when `PickupParticleComponent` is None, it makes a new `ParticleSystemComponent` with the pickup
   as its outer. It stores it in `PickupParticleComponent`, sets the component's translation to an offset the
   pickup computes (not read), and attaches it with `Actor.AttachComponent` (3.4). When the component is still None,
   it returns.
3. **The template:**
   - normally `PickupParticles`;
   - when the globals definition is reachable and the pickup's cost currency is of one particular kind (by fit; the
     kind was not mapped to a name), it uses the particle system of the item set
     `GD_AnemonePackageDef.ItemSetDef_Anemone` instead, when that set has one. The set is found by name and loaded
     on first use;
   - it hands the template to the component (a native, not read).
4. **Flags and colour:**
   - three flags of the component are changed;
   - a vector of the component is set to an engine constant;
   - a colour is chosen: a default; a rarity colour from the globals definition when there is one; an override from
     the inventory's definition when that has one. It is set on the component as a named colour parameter;
5. **Show and start:** the component's `SetHidden(false)` entry (3.2), then a start call (by its purpose, the
   activation; not read).

The layout tool places `PickupParticleComponent` and `PickupParticles` exactly where the native reads them.

**Off answer** (both cases come after `SetInteractParticles` has written `PickupParticles` and `bNetDirty`, so the
`make` step prefixes either with "stopped after partial writes: "):
1. `PickupParticles` None: `NotYet { NotRead }`, "WillowPickup.SpawnPickupParticles with no particle system: the
   particles' removal", record this note, before any write of the native's own.
2. Otherwise, before any write of the native's own, `NotYet::layer_absent(what, layer, record)` with:
   - `what`: "WillowPickup.SpawnPickupParticles: the pickup's particle effect";
   - `layer`: "a particle-effects layer (the port has none)";
   - `record`: this note.

   The native makes a component, attaches it, and hands it a template, a colour and a start. None of that is
   script-visible state the slice reads, apart from `PickupParticleComponent`. A port answer like 3.4's would be for
   the hub. Section 6.5 shows the slice does not reach this native in default Off while 6.3's rigid-body stop
   stands, and where the stop moves if the hub rules a port answer for it.

### 6.5 Where links 6 and 7 stop

With #492's "nothing" and the hub's scene layer for `AttachComponent` (section 5):
1. `AttachMeshToPickup` completes (3.1 to 3.4 under the scene layer);
2. `SetLightEnvironment` makes one write (6.2, case 2);
3. **`WillowPickup.InitializePickupForRBPhysics` answers `NotYet { LayerAbsent }`** (6.3), because the pickup's
   class default `bUseRBPhysics` is true. The `make` step prefixes it with "stopped after partial writes: ".

So links 6 and 7 stop at `InitializePickupForRBPhysics`. While that stop stands, `SetPickupability` and
`SpawnPickupParticles` are not reached. Link 7 stops on its first gun, before the decoy is made.

**If the hub rules a port answer for the rigid-body set-up** (6.6 says why this note offers none), the stop moves on:
1. `PickupAssociated`: an empty script body;
2. `SetPickupability(true)`, script (6.1): `bPickupable` = true; `RegisterMissionDirector` reaches only the bound
   `GetInventoryDefinition`; `SetMissionMaterialParameter` does nothing;
3. `TryToSetInteractParticles(true)` calls `SetInteractParticles` with the globals definition's `PartPickup`, which
   writes `PickupParticles` and `bNetDirty`;
4. **`SpawnPickupParticles`** (6.4): `NotYet { LayerAbsent }`, prefixed, when `PartPickup` is set;
   `NotYet { NotRead }`, prefixed, when it is None.

6.4 needs a ruling of its own before the stop moves past it.

### 6.6 For the hub: an empty physics layer for `InitializePickupForRBPhysics` (the writes do not separate)

The hub asked whether the native's script-visible writes, apart from the physics work, can be listed as a candidate
answer for an explicit, empty physics layer, in the same shape as the scene layer. **They cannot be separated from
the physics.** So the answer stays `NotYet { LayerAbsent }` (6.3), and links 6 and 7 leave the queue for now.

**Every effect, in the native's order** (all under `bUseRBPhysics`; "later" means the rest of links 6 and 7: the
`make` step's script, the `touch` step's `Pickup` state handlers and the `look` step's pickup targeting):

| # | Effect | Field | Script sees it | Read later on links 6-7 | Read or fitted |
|---|---|---|---|---|---|
| 1 | the pickup's rigid-body sleep set-up (a class-table entry) | `bForceRBToSleep` cleared; timers `MaxRBAwakeTimer` (rate `MaxRBAwakeTime`, once) and `CheckForRigidBodySleepState` (1.0 s, looping) | yes | no | body read; field names by a constant shift of `WillowPickup`'s own fields, the name-to-rate pairing by fit |
| 2 | the pickup's rotation rate | `RotationRate` = an engine constant (by fit, zero) | yes | no | field read; value fitted |
| 3 | the component's three floats set to 1.0 | `Scale3D` | yes | no | fitted (the doubled shift below) |
| 4 | an entry of the component | not identified | not known | not known | not read |
| 5 | a flag raised, with the deferred reattach (2) when newly raised | `CollideActors` | yes | no | write read; name fitted |
| 6 | a flag cleared | `BlockActors` | yes | no | write read; name fitted |
| 7 | with the inventory's definition: the rigid-body set-up (one float of a physics object set to 1.0, two calls) | not identified | not known | not known | partly read |
| 8 | without one: an entry of the pickup, which hands on to another of its entries | not identified | not known | not known | not read |
| 9 | three pickup fields from the globals definition | `MaxImpactEffectPlayCount` (a float, truncated), `ImpactEffectDelay`, `ImpactVelocityAkRtpc` | yes | no | fitted (row 1's shift) |
| 10 | an entry of the component | not identified | not known | not known | not read |
| 11 | the collision threshold: a globals value times 0.25, truncated to a byte | `ScriptRigidBodyCollisionThresholdCompiled` | yes | no | fitted |
| 12 | the collision channel, then an entry of the component | `RBChannel` | yes | no | write read, name fitted; the entry not read |
| 13 | the per-channel calls: each sets or clears one bit by channel number, through the helper the script's `SetRBCollidesWithChannel` also uses, then an entry of the component | `RBCollideWithChannels` | yes | no | helper read; which channels and values were not recovered; the entry not read |
| 14 | two more entries of the component | not identified | not known | not known | not read |
| 15 | the physics mode entry. By its shape the actor's physics setter: when the mode changes it writes `Physics`, notifies through another field, and in the rigid-body mode calls entries of the collision component | `Physics` | yes | **yes**: `DroppedPickup.Pickup.ValidTouch` compares `Physics` with `PHYS_Falling` | body read; its argument was not recovered (by fit, the rigid-body mode); its calls not read |
| 16 | with `bTorque` clear: a spin, three draws from the engine's float random stream over ranges from the globals definition, then an impulse on the body | the random stream | yes (a later `FRand` sees the stream move) | no | draws read; the impulse not read |
| 17 | with `bTorque` set: the pickup's `Torque` applied, then `bTorque` cleared when `bTorqueTemporary` is set | `bTorque` | yes | no | read (exact placement); the torque call not read |
| 18 | a timer: `EnableRagdollCollision`, 0.3 s, once, on the pickup itself. Its target is the script function `WillowPickup.EnableRagdollCollision`, which raises two of the mesh's rigid-body channels | the pickup's timers | yes (`IsTimerActive` and the like) | no; the harness does not advance the world between `make` and `touch`, so it would not fire | read (the name from the string the native references) |

**Row 5 under the scene layer.** The flag is `CollideActors`. `SkeletalMeshComponent`'s class defaults do not list it,
so it is false, and the raise is new. The deferred reattach then runs on the mesh, whose `Owner` is the pickup after
`AttachComponent` (3.4). So part 1's rule (2) answers `NotYet { NotRead }`, "the deferred reattach: the owner's
component update", record this note, inside the native. It comes after rows 1 to 3 have written, so it is terminal,
and the `make` step adds the prefix.

**Row 18 and the port.** The timer is a script-visible write. The port's timer service (`ow_world::timers`) could hold
it, but no port answer makes it yet, nor row 1's two timers.

**Where the field names come from.**
- `WillowPickup`'s own fields sit one fixed amount past the layout tool's placement. Rows 1 and 9 agree on it (the
  bool word and `MaxRBAwakeTime` in row 1; three consecutive fields of fitting kinds in row 9). `DroppedPickup`'s
  fields (`bUseRBPhysics`, `RotationRate`, `bTorque`, `bTorqueTemporary`, `Torque`) sit exactly where the tool places
  them.
- `PrimitiveComponent`'s fields sit shifted by the amount seen in 3.2 and 6.2 up to
  `ScriptRigidBodyCollisionThresholdCompiled`, `RBChannel` and the bool word; and by twice that amount from the
  transform fields on, which fits a 16-byte-aligned matrix field ahead of them. `SpawnPickupParticles`' writes to
  `Translation` and `Rotation` (6.4) fit the same doubled shift.

All these names are **UNVERIFIED (fitted)**.

**Why the writes do not separate:**
- Row 5 stops inside the native, at the owner-branch `NotRead`, unless that branch also gets a port answer.
- Row 15 writes `Physics`, which the `touch` step reads, and its argument was not recovered. A port answer that left
  `Physics` alone would change what `ValidTouch` decides, and that would look like a rule.
- Rows 4, 7, 8, 10, 12, 13 and 14 hold component and pickup entries that were not read, and row 13's channels were not
  recovered.
- Row 16 moves the engine's random stream, which no port answer can match without the draws' ranges and the stream.

**What it would take:** the entries marked "not read", row 15's argument and calls, row 13's channels, and a ruling
on the deferred reattach's owner branch. Until then the answer stays `NotYet { LayerAbsent }`.

## Open questions

1. **UNVERIFIED:** what a component made with a template holds in the game. That decides whether the slice's
   component has an `Owner`, a `Scene`, a skeletal mesh and the template's `HiddenGame` and `bOwnerNoSee`. In the game
   the template is the weapon's `ThirdPersonMesh`; the port's `new` ignores it. With a mesh, `HideBoneByName` would
   act; with an `Owner`, the deferred reattach would take its owner branch. So, if the port's `new` ever copies the
   template, `HideBoneByName` would likely turn into its `NotRead` (3.3) on a weapon whose type names a bone. That is
   expected, not a regression.
2. **UNVERIFIED (fitted):** `bForceRefpose` against `bOldForceRefPose`; the `HiddenGame` and `bOwnerNoSee` word (a
   shifted placement in the layout tool); the `SkeletalMesh` read; the pending-kill flag.
3. The deferred reattach's owner checks and the owner's update entry; the no-owner detach and reattach.
4. `HideBoneByName`'s rebuild, transform mask and physics body options.
5. `Actor.AttachComponent`: the component's own attach entry (render, collision, physics), whether it sets
   `bAttached`, the extra value for orthonormalized lighting, and the engine-wide notifier.
6. `InitializePickupForRBPhysics` (6.6): the entries marked "not read", the physics mode entry's argument, and the
   channels set. The timer is `EnableRagdollCollision`, 0.3 s, once, on the pickup, with the script function
   `WillowPickup.EnableRagdollCollision` as its target. The harness world does not advance between `make` and
   `touch`, so the timer would not fire before the touch. It is a script-visible write that no port answer can make
   yet.
7. `SpawnPickupParticles`: the removal entry, the particle offset, the template hand-over and activation, the three
   flags, and which currency kind selects the item set's particle system.

## Live checks that would settle these rules (bl-sdk, the RTSE way; none run, no session granted)

1. **The drop.** Hook `WillowWeapon.AttachMeshToPickup` and, on the new component, record before step 3 and after
   step 7:
   - `Owner`, `bAttached`, `bNeedsReattach`, `bForceRefpose`, `HiddenGame`, `bOwnerNoSee`, `SkeletalMesh`;
   - the length of `BoneVisibilityStates`.

   On the pickup, record the length of `Components` before and after `AttachComponent`.
2. **A hidden bone.** Drop a weapon whose type definition names a `BoneToHideOnMesh`, and read that bone's
   `BoneVisibilityStates` entry afterwards (expect `BVS_ExplicitlyHidden`).
3. **The physics set-up.** Hook `WillowPickup.InitializePickupForRBPhysics` on a drop, and record before and
   after: `bUseRBPhysics`, `Physics`, `RotationRate`, `bTorque`, `bForceRBToSleep`, the mesh's `CollideActors`,
   `BlockActors`, `RBChannel` and `RBCollideWithChannels`, and the pickup's timers.
4. **The particles.** After a drop, read `PickupParticles`, `PickupParticleComponent` and its `Template`, and whether
   the component is in the pickup's `Components`.
5. **No duplicate check.** Call `AttachComponent` twice with the same component on a test actor in a scratch session,
   and read `Components` (expect the component twice, if the rule holds).
