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
   walks the pawn's `Components` in order. For each component that is **attached** (in the game, its `bAttached`),
   and that is the `CollisionComponent` when the pawn's `Physics` is `PHYS_RigidBody`, it calls the component's rigid-body set-up,
   "fixed" unless the pawn is `PHYS_RigidBody`;
2. **then `SetPushesRigidBodies(bPushesRigidBodies)`** (a pawn virtual):
   - `bPushesRigidBodies` := the value (the same value, here);
   - when set, and the pawn has no pushing body yet: a kinematic body is made around an **attached**
     `CollisionComponent` (its radius and height), when the world has a physics scene;
   - when clear and a pushing body exists: that body is destroyed.

**What "attached" means in the port.** The port never sets `bAttached` on a component. The spawn's component update
(#605/#610) and `Actor.AttachComponent` (#520) leave it as it was (NATIVE_COMPONENT_BOUNDS.md section 5). The stand-in
is the **attach record**, NATIVE_COMPONENT_BOUNDS.md section 7 A (`ow_world::actors::attached_to`, used the same way by
`bounding_cylinder.rs`):
- **a live receiver:** a component counts as attached when it is in the attach record **under the current owner**,
  the pawn, by the record's scoped lookup;
- **a template receiver** (`Origin::Default`): `bAttached` read from the entry, as #595's template path does.

Maya's components are in that record. In the game they are attached at this point, so bodies would be built.

**Off answer.**
- **Preflight** (write-free and load-free): `bDeleteMe`, `Physics`, `Components`, `CollisionComponent` and
  `bPushesRigidBodies` declared, else `VmError::Script`.
  - A None `Components` entry is **skipped**, as the game skips it.
  - A component that is a store handle not loaded yet is `NotYet { NotRead }`.
  - An entry whose class does not declare `Owner` as an object (the field the record's lookup reads) is
    `VmError::Script`, as `component_attach.rs` answers it.
- **`bDeleteMe` set:** nothing.
- **The component walk,** in `Components` order, after the `Physics` filter (with `PHYS_RigidBody`, only the
  `CollisionComponent`):
  - **an entry attached** (by the record, or by `bAttached` on a template receiver): the game would build its rigid
    body. That is `NotYet { LayerAbsent }`, "WillowPawn.InitializeRBPhysics: a component's rigid body", with layer "a
    physics layer (rigid bodies)";
  - **a live entry not in the record**, with `bAttached` false: attachment is unknown. It follows refusal L's
    convention (NATIVE_COMPONENT_BOUNDS.md section 7 B step 5): `NotYet { LayerAbsent }`, with refusal L's text and
    layer;
  - no entry left: nothing from this step.
- **`SetPushesRigidBodies(bPushesRigidBodies)`:**
  - with `bPushesRigidBodies` clear, and no pushing body (the port makes none): **nothing, and no write.** The
    same-value write is skipped, so no write is made and no partial-writes prefix can arise;
  - set, with the `CollisionComponent` attached by the same test: the same physics `LayerAbsent`. Not attached by the
    record: refusal L's convention.
- #574's at-rest ruling covers only the pickup; extending it here is the hub's call.

**Maya:** her components are in the attach record under her pawn, so the walk meets an attached entry: **the
physics `LayerAbsent`**, with nothing written. 2.A is her stop.

**Tests** (on attach-record state):
1. a toy pawn whose `Components` are all None, `bPushesRigidBodies` clear: nothing written, and no prefix;
2. one component in the attach record under the pawn: the physics `LayerAbsent`, nothing written;
3. one colliding component not in the record, `bAttached` false: refusal L's `LayerAbsent`;
4. `PHYS_RigidBody` with a recorded component that is not the `CollisionComponent`, and nothing else: skipped by the
   filter, so nothing;
5. `bPushesRigidBodies` set with the `CollisionComponent` in the record: the physics `LayerAbsent`;
6. `bDeleteMe` set: nothing;
7. a lazy component handle: the not-loaded `NotRead`;
8. a template receiver (`Origin::Default`) with an entry whose `bAttached` is set: the physics `LayerAbsent`;
9. an entry recorded under another owner: not attached, so refusal L's convention.

*Read further 2026-11-26 in NATIVE_LINK1_RIGID_BODY_LAYER.md (decision 2026-11-26-NNN): the component set-up makes
no body for Maya's attached components (no shape on the cylinder; the body mesh's physics-asset instance flag clear).
Its only writes are `bSkelCompFixed` and an emptied `ClothMeshIndexData`, so the attached-entry `LayerAbsent` above is
replaced by that rule for the classes it names.*

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
names the list. **It is the next stop once 2.A answers.**

*Read in full in NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md section 2 (decision 2026-11-25-75): the list is named, and
Maya's answer is the kernel's pass-2 `LayerAbsent` for `PlayerEvents_Siren`, with nothing written.*

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

**What it does:** returns a `LinearColor` with R, G, B and A each **an f32 multiply** of the byte by the f32
constant 1/255 (0.003921569), from the `Color`'s R, G, B and A. Nothing is written. It is a multiply, not a divide: the
two differ by 1 ulp for some bytes (3, 6, 7, 12, 13, 14).

**Off answer:** that conversion, as `byte as f32 * (1.0f32 / 255.0)` with the constant rounded to f32 first.
**Tests:**
- (255, 0, 128, 255) gives (1.0, 0.0, 0.50196081, 1.0); (0, 0, 0, 0) gives zeros;
- **the multiply, not the divide:** byte 3 gives the f32 with bits `0x3C40C0C2`. `3.0 / 255.0` rounded to f32 would
  be `0x3C40C0C1`.

### 2.E `MeshComponent.GetInstancedMaterialInstanceConstant(int ElementIndex)` (outline)

**What it does:**
- the component's material at the index (a component virtual, by fit `GetMaterial`, not read here);
- when that is a `MaterialInstanceConstant` whose `Outer` is this component, it is the answer;
- otherwise a new one is made and set for that element (a routine not read).

**Off answer:**
- the read branch: answerable once the port's `GetMaterial` rule exists (none is indexed). Until then
  `NotYet { NotRead }`, "MeshComponent.GetMaterial";
- the creation branch: `NotYet { NotRead }`, "GetInstancedMaterialInstanceConstant: making an instanced material".

*Read in full in NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md section 3 (decision 2026-11-25-75), with `GetMaterial`,
`SetParent` and `SetVectorParameterValue`.*

**Maya:** `UpdatePreferredColors` calls this only with a `PlayerReplicationInfo`. Her archetype has none. Whether her
spawned pawn has one at this point depends on the combat driver's possession order (not read). With none, the whole
of `UpdatePreferredColors` is skipped.

### 2.F `WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook()`

**What it does:** **`ImmuneToFallingDamage` := true.** Nothing else. By fit, player pawns take no falling damage.

**Off answer:** that one write (the bool declared, else `VmError::Script`). **Tests:** a toy `WillowPlayerPawn` with
the flag clear: set after the call, and no other field changed.

## 3. What link 1 would meet (a prediction, not a run)

Past the collision setters:
1. **2.A is the stop:** the physics `LayerAbsent`. Maya's components are in the attach record;
2. past it (once the hub rules or a physics layer exists): 2.B (`NotRead` once bound, or the unbound native today);
3. then `SetDeathDefinition` and the provider registration (bound);
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
