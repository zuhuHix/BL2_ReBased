# maya:8 after the collision setters: InitializeRBPhysics and the player pawn's last natives (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK1-RBPHYSICS-AND-AFTER (decision 2026-11-25-73), the analyst
half of the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/rbphys/`.
This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every
rule here is UNVERIFIED in the running game.**

**Why.** The next unread natives on `maya:8`'s `ApplyBodyClassStartingValues`, after the collision setters
(NATIVE_LINK1_TAIL_SETTERS.md, #734).

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`). In order, after the collision setters:
  1. `InitializeRBPhysics` (the pawn's virtual, from `SetupSkeletalMeshPhysics`): **2.A**;
  2. `InitializeBehaviorProviders`: **2.B**;
  3. the player pawn's own part:
     - `SetDefaultStance` (indexed);
     - the `Arms`;
     - `UpdateAnimSets`' player part, with `GetFirstPersonWeaponHoldDef` (**2.C**);
     - the arms materials;
     - `UpdatePreferredColors`, with `GetLinearColor` (**2.D**) and `GetInstancedMaterialInstanceConstant` (**2.E**);
     - `OnNativeApplyBodyClassStartingValuesHook` (**2.F**).
- **The natives:** Ghidra 12.1.4 headless, read-only, under the machine lock:
  - each of the six execs only calls one virtual of the receiver;
  - each slot was read in `WillowPlayerPawn`'s primary class table, found as in NATIVE_COMPONENT_SKIP_FLAG.md
    section 6, and the routine it reaches was read;
  - `GetFirstPersonWeaponHoldDef`'s exec calls its lookup directly.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. For the actor classes the script offsets equal
  the native ones, and every field named below sits exactly where the routine reads it.
- **Maya's data** (`ow-package --def-dump`, local):
  - `Pawn_Siren`: `bPushesRigidBodies` false, `ImmuneToFallingDamage` false, `Physics` 0, `PlayerReplicationInfo`
    None;
  - `BodyClass_Siren`: `FirstPersonWeaponHoldDefs` empty, `DefaultFirstPersonWeaponHoldDef` None;
  - its parent `BodyClass_PlayerShared`: seven first-person holds (`..._Pistol` to `..._Unarmed`), default
    `WeaponHold_Player_Default`.

## 2. The natives

### 2.A `WillowPawn.InitializeRBPhysics()` (the pawn, `WillowPlayerPawn`'s slot)

**What it does:**
1. **The actor's rigid-body set-up.** Unless the pawn is `bDeleteMe`, and only when the world has a physics scene, it
   walks the pawn's `Components` in order. For each component that is **attached** (`bAttached`), and that is the
   `CollisionComponent` when the pawn's `Physics` is `PHYS_RigidBody`, it calls the component's rigid-body set-up,
   "fixed" unless the pawn is `PHYS_RigidBody`;
2. **then `SetPushesRigidBodies(bPushesRigidBodies)`** (a pawn virtual):
   - `bPushesRigidBodies` := the value (the same value, here);
   - when set, and the pawn has no pushing body yet: a kinematic body is made around an **attached**
     `CollisionComponent` (its radius and height), when the world has a physics scene;
   - when clear and a pushing body exists: that body is destroyed.

**Off answer.**
- **Preflight:** `bDeleteMe`, `Physics`, `Components`, `CollisionComponent` and `bPushesRigidBodies` declared; each
  component's `bAttached` read. A component that is a store handle not loaded yet is `NotYet { NotRead }`.
- **The port has no physics scene.** The game always has one in play. So the answer goes by what would be built:
  - **no component attached, and `bPushesRigidBodies` clear (or the `CollisionComponent` not attached): nothing.**
    The game's own gates build nothing for that state either. That matches the port's scene-layer convention
    (NATIVE_ATTACH_MESH_TO_PICKUP.md section 2: the port leaves `bAttached` as it was). The `bPushesRigidBodies`
    write re-writes the same value;
  - **any component attached** (with the `Physics` filter above), or `bPushesRigidBodies` set with an attached
    `CollisionComponent`: `NotYet { LayerAbsent }`, "WillowPawn.InitializeRBPhysics: a component's rigid body",
    with layer "a physics layer (rigid bodies)". #574's at-rest ruling covers only the pickup; extending it here is
    the hub's call.

**Maya:** `bPushesRigidBodies` false, and the port leaves her components unattached. So the answer is **nothing**.

**Tests:**
1. a toy pawn, nothing attached, `bPushesRigidBodies` clear: nothing written;
2. one attached component: the `LayerAbsent`, nothing written;
3. `PHYS_RigidBody` with an attached component that is not the `CollisionComponent`: skipped by the filter, so
   nothing (when nothing else is attached);
4. `bPushesRigidBodies` set with an attached `CollisionComponent`: the `LayerAbsent`;
5. `bDeleteMe` set: nothing;
6. a lazy component handle: the not-loaded `NotRead`.

### 2.B `WillowPawn.InitializeBehaviorProviders()` (outline; not this note's answer)

**What it does, in outline** (`WillowPawn`'s own body, which `WillowPlayerPawn` keeps):
- only while the pawn's `ConsumerHandle` is the "none" handle: it registers the pawn as a behavior consumer and stores
  the handle;
- then, with a `BodyClass`, it initialises a list of providers for that handle. They are reached through the body
  class, the entries of two of its arrays, an array on the pawn whose entries hold an object of one `WillowGame`
  class, and two more body-class fields;
- then, when one pawn flag is set, a further kernel step whose answer is stored in that flag.

The arrays and fields were not named in this lane. NATIVE_BEHAVIOR_POPULATION.md (its `WillowMind.Possess` section) reads
`WillowAIPawn`'s, and lists "the pawn's `WillowPawn.InitializeBehaviorProviders` provider list in full" as open.

**Off answer:** `NotYet { NotRead }`, "WillowPawn.InitializeBehaviorProviders (the provider list)", until a lane
names the list. **That is the next stop after 2.A.**

### 2.C `BodyClassDefinition.GetFirstPersonWeaponHoldDef(name, object, bool, bool)`

**What it does:** exactly `GetWeaponHoldDef`'s rule (NATIVE_GET_WEAPON_HOLD_DEF.md section 2, with #728's re-read),
on the first-person fields:
- `FirstPersonWeaponHoldDefs` in place of `WeaponHoldDefs`;
- `DefaultFirstPersonWeaponHoldDef` in place of `DefaultWeaponHoldDef`, also in the "fallback passed on only without
  an own default" test;
- the same `WeaponActionParentClass` for the parent.

**Off answer:** the same implementation with the two fields swapped. **Tests:** #728's, on the first-person fields.

**Maya:** reached only once the arms exist, through `GetFirstPersonWeaponHoldAnimSets`, which first recurses into the
parent class:
- on `BodyClass_PlayerShared`, `Unarmed`: by name, `WeaponHold_Player_Unarmed` (its tag was not read). An exact match
  answers at once; a name-only candidate also answers, ahead of the default;
- on `BodyClass_Siren`: no entries, no default, so None.

### 2.D `WillowPlayerPawn.GetLinearColor(Color C)`

**What it does:** returns a `LinearColor` with R, G, B and A each the byte times 1/255 (0.003921569), from the
`Color`'s R, G, B and A. Nothing is written.

**Off answer:** that conversion. **Tests:** (255, 0, 128, 255) gives (1.0, 0.0, about 0.502, 1.0); (0, 0, 0, 0) gives
zeros.

### 2.E `MeshComponent.GetInstancedMaterialInstanceConstant(int ElementIndex)` (outline)

**What it does:**
- the component's material at the index (a component virtual, by fit `GetMaterial`, not read here);
- when that is a `MaterialInstanceConstant` whose `Outer` is this component, it is the answer;
- otherwise a new one is made and set for that element (a routine not read).

**Off answer:**
- the read branch: answerable once the port's `GetMaterial` rule exists (none is indexed). Until then
  `NotYet { NotRead }`, "MeshComponent.GetMaterial";
- the creation branch: `NotYet { NotRead }`, "GetInstancedMaterialInstanceConstant: making an instanced material".

**Maya:** `UpdatePreferredColors` calls this only with a `PlayerReplicationInfo`. Her archetype has none. Whether her
spawned pawn has one at this point depends on the combat driver's possession order (not read). With none, the whole
of `UpdatePreferredColors` is skipped.

### 2.F `WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook()`

**What it does:** **`ImmuneToFallingDamage` := true.** Nothing else. By fit, player pawns take no falling damage.

**Off answer:** that one write (the bool declared, else `VmError::Script`). **Tests:** a toy `WillowPlayerPawn` with
the flag clear: set after the call, and no other field changed.

## 3. What link 1 would meet (a prediction, not a run)

Past the collision setters:
1. 2.A answers nothing;
2. **2.B is the next stop** (`NotRead` once bound, or the unbound native today);
3. after it: `SetDeathDefinition` and the provider registration (bound);
4. then the player part: `SetDefaultStance`, the arms (2.C, the materials), `UpdatePreferredColors` (2.D, 2.E,
   skipped without a `PlayerReplicationInfo`), and 2.F.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the components' own rigid-body set-up and the pushing body's creation (the physics layer);
   - 2.B's provider list;
   - `GetMaterial` and the material-instance creation of 2.E;
   - `SetVectorParameterValue` on a material instance.
3. **The hub's call:** whether #574's at-rest ruling extends to 2.A's attached case.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read `ImmuneToFallingDamage`. Expect true.
2. Read her pawn's `ConsumerHandle`. Expect a registered handle (2.B), and list its providers.
