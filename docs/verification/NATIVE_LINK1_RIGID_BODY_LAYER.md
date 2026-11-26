# Link 1's rigid-body layer: what InitializeRBPhysics builds for Maya (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK1-RIGID-BODY-LAYER (decision 2026-11-26-NNN), the analyst half of
the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/rblayer/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule
here is UNVERIFIED in the running game.**

**Why.** With #760, links 1, 3, 4, 5, 13 and 14 stop at `WillowPawn.InitializeRBPhysics`' physics `LayerAbsent`, "a
component's rigid body" (NATIVE_LINK1_RBPHYSICS_AND_AFTER.md 2.A). The open question was whether #574's at-rest ruling
for pickups can extend here. This note reads what the per-component set-up builds for Maya's attached components.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The natives** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - the pawn's slot (a tail call to the actor's rigid-body set-up), and that set-up, already outlined in 2.A;
  - **the component virtual it calls on each attached entry** (the component rigid-body set-up), read in the primary
    class table of each class on Maya's path, found as in NATIVE_COMPONENT_SKIP_FLAG.md section 6:
    - `ActorComponent`, `DynamicLightEnvironmentComponent`, `SpecialMoveComponent`, `GearboxDialogComponent` and
      `StatusEffectsComponent`: an **empty body**;
    - `PrimitiveComponent` and `CylinderComponent`: one shared body (the primitive set-up);
    - `SkeletalMeshComponent`, `GearboxSkeletalMeshComponent`, `CustomSkeletalMeshComponent` and
      `GearLikenessMeshComponent`: one shared body (the skeletal set-up);
  - the **shape virtual** the primitive set-up asks for: for `PrimitiveComponent` and `CylinderComponent` it answers
    nothing (a body-less class); `SkeletalMeshComponent`'s looks a body up in its physics asset.
- **Field places** (the local copy of `tools/ghidra/class_layout.py`). The usual component shifts apply: 8 bytes for
  `PrimitiveComponent`'s fields, 16 for `SkeletalMeshComponent`'s. Under them every field read lands on a declared
  property of the right kind, which is the structural check:
  - `bDisableAllRigidBody` and `bInstancedStaticRB` (two bits of one bool group) and `BodyInstance`;
  - `bUseSingleBodyPhysics`, `bHasPhysicsAssetInstance` and `bSkelCompFixed` (`bSkelCompFixed` is the bit the set-up
    writes from its "fixed" argument, which fits its name), `PhysicsAsset`, `SkeletalMesh`, `ClothSim`,
    `ClothMeshIndexData`, `bEnableSoftBodySimulation` and `Attachments`;
  - on the mesh, a cloth bool, by place `bClothMetal` (fitted).
- **The script** (`research/script_disasm.py`): `WillowPawn.SetupSkeletalMeshPhysics`, which calls
  `InitializeRBPhysics` and then `SetupSkeletalMeshPhysicsForPhysXEffects`; that one's body.
- **Data** (`ow-package --def-dump`, local): `Pawn_Siren`'s components, the body class's attachment templates, and
  `Char_Siren.Mesh.Skel_SirenBody`.

## 2. What the game builds

### 2.1 The actor's set-up (2.A, restated)

With a physics scene (the game always has one) and the pawn not `bDeleteMe`, each **attached** `Components` entry gets
the component set-up, with **fixed** true unless the pawn's `Physics` is `PHYS_RigidBody` (then only the
`CollisionComponent` is visited). Then `SetPushesRigidBodies(bPushesRigidBodies)`.

### 2.2 The component set-up, by class

- **Non-primitive components** (`ActorComponent` and the four Maya's pawn holds, listed in section 1): **nothing.**
- **`PrimitiveComponent` and `CylinderComponent`** (the primitive set-up):
  - nothing unless there is a physics scene, `bDisableAllRigidBody` and `bInstancedStaticRB` are clear, and
    `BodyInstance` is None;
  - then it asks the class for its **shape**. These two classes have none, so **nothing is made**;
  - (a class with a shape would get a new body instance, stored in `BodyInstance`, set up fixed or not in the scene,
    and terminated again if its owner is pending kill.)
- **`SkeletalMeshComponent` and its three subclasses above** (the skeletal set-up), with a physics scene and
  `bDisableAllRigidBody` clear:
  1. **`bSkelCompFixed` := fixed** (a write);
  2. **with `bUseSingleBodyPhysics` set:** the primitive set-up, whose skeletal shape is the physics asset's body for the
     mesh's root bone. That **makes one body**;
  3. **with it clear:** when `bHasPhysicsAssetInstance` is set and there is a `SkeletalMesh`, the articulated set-up (a
     component virtual, by fit `InitArticulated`; not read). That **makes the asset's bodies**. Otherwise nothing;
  4. **with no `ClothSim`:** `ClothMeshIndexData` is emptied. When the mesh has its cloth bool (by place
     `bClothMetal`, fitted) a cloth step follows (not read);
  5. **with `bEnableSoftBodySimulation`:** a soft-body step in the scene (not read);
  6. **then each attached `Attachments` entry's component** gets the same component set-up, with the same fixed flag.

### 2.3 Maya's case

The pawn's `Physics` is not `PHYS_RigidBody`, so **fixed** is true and every attached entry is visited:
- the four non-primitive components: nothing;
- the cylinder (attachment 13): no shape, so nothing;
- **the body mesh (attachment 0, `SkeletalMeshComponent`):**
  - its template has `bUseSingleBodyPhysics` 0, `bHasPhysicsAssetInstance` false and `bEnableSoftBodySimulation`
    false;
  - it has a `PhysicsAsset`, `Phat_Siren`, but with the instance flag false the articulated step does not run;
  - `ClothSim` is None, and `Skel_SirenBody`'s cloth bool is false;
  - so: **`bSkelCompFixed` := true, and `ClothMeshIndexData` emptied** (it is already empty);
- **its socket attachments** (the custom head and the gear-likeness meshes, attached to the body mesh): each is a
  skeletal subclass with the same flags off, so each gets **`bSkelCompFixed` := true** and nothing else.

**No body is made.** `BodyInstance` and `PhysicsAssetInstance` stay None. `bPushesRigidBodies` is false, so
`SetPushesRigidBodies` makes no pushing body (2.A).

### 2.4 What the slice reads from those bodies later

Nothing, since there are none:
- the port's "bodies present" test (#734, NATIVE_LINK1_TAIL_SETTERS.md: `PhysicsAssetInstance` on a skeletal
  component, `BodyInstance` otherwise) stays false, so the filter updates of `SetRBChannel`, `SetBlockRigidBody` and
  `SetNotifyRigidBodyCollision` keep doing nothing;
- `bSkelCompFixed` is read by physics blending and ragdoll set-up, not reached by the slice's links as far as read.

The next step that **would** make bodies is `SetupSkeletalMeshPhysicsForPhysXEffects`'
`SetHasPhysicsAssetInstance(true)`. It runs only when `Engine.GetPhysXLevel()` is above 0 (section 4).

## 3. The answer for the port

### 3.1 (a) Yes: a rule with no rigid-body layer, for exactly these cases

The port can answer `InitializeRBPhysics` for Maya **with no physics layer**: the game makes no body there. Unlike #574
(an at-rest stand-in for a body the game does make), this is the game's own answer, and its writes are script-visible
bools and an array the port has. The rule replaces 2.A's attached-entry `LayerAbsent`. **UNVERIFIED.**

**The walk** (2.A's order and filter; "attached" is the attach record, as 2.A says). For each visited entry, by its
**first native class**:

| Class | Answer |
|---|---|
| `Engine.ActorComponent` (no native subclass), `Engine.DynamicLightEnvironmentComponent`, `GearboxFramework.SpecialMoveComponent`, `GearboxFramework.GearboxDialogComponent`, `WillowGame.StatusEffectsComponent` | nothing |
| `Engine.PrimitiveComponent`, `Engine.CylinderComponent` | nothing (no shape) |
| `Engine.SkeletalMeshComponent`, `GearboxFramework.GearboxSkeletalMeshComponent`, `WillowGame.CustomSkeletalMeshComponent`, `WillowGame.GearLikenessMeshComponent` | the skeletal rule below |
| any other | `NotYet { NotRead }`, "WillowPawn.InitializeRBPhysics: the rigid-body set-up of <class>" |

**The skeletal rule**, on an entry with `bDisableAllRigidBody` clear (set: nothing):
- `bUseSingleBodyPhysics` not 0: `NotYet { LayerAbsent }`, "a component's rigid body", 2.A's text and layer (one body
  from the asset);
- `bHasPhysicsAssetInstance` set with a `SkeletalMesh`: the same `LayerAbsent` (the asset's bodies);
- `ClothSim` not zero: nothing from the cloth step. Zero, with the mesh's cloth bool set: `NotYet { NotRead }`,
  "InitializeRBPhysics: the cloth set-up";
- `bEnableSoftBodySimulation` set: the same physics `LayerAbsent` (the soft body);
- otherwise **the writes**: `bSkelCompFixed` := fixed, `ClothMeshIndexData` := empty when `ClothSim` is zero;
- then each `Attachments` entry whose component is attached (the attach record under the same owner), the same rule,
  recursively, in array order.

**Preflight** (write-free and load-free, over the whole walk and its recursion, before any write):
- every class, flag and field above declared with its kind (`bDisableAllRigidBody`, `bUseSingleBodyPhysics`,
  `bHasPhysicsAssetInstance`, `bSkelCompFixed`, `bEnableSoftBodySimulation`, `ClothSim`, `ClothMeshIndexData`,
  `SkeletalMesh`, `Attachments` and its `Component` member), else `VmError::Script`;
- every entry, attachment component and `SkeletalMesh` resident, else `NotYet { NotRead }` (a load is a write);
- every refusal above checked before the first write. The game writes component by component, so a refusal on a later
  component would follow writes on earlier ones. The port refuses up front instead, and so needs no partial-writes
  prefix. (That ordering difference is the port's; nothing on Maya's path refuses.)

**The physics scene.** The game asks for one first. The port has none, and treats it as present: the rule above is what
the game does with one.

**Then `SetPushesRigidBodies`**, as 2.A already rules (Maya: nothing).

### 3.2 (b) The smallest record, for the cases that do make bodies

Only the `LayerAbsent` rows above make bodies. What the slice's script would see of them is the objects themselves:
`BodyInstance` (an `RB_BodyInstance`) or `PhysicsAssetInstance` set, which the "bodies present" tests read (#734). So the
smallest record is: **make those objects, with no simulation behind them, and keep the fixed flag.** That is a ruling
like #574's, needed only when a slice component reaches those rows. Maya does not, so none is proposed now.

## 4. The predicted next stop (a prediction, not a run)

With 3.1 bound (and #760 for the attach):
1. `InitializeRBPhysics`: the writes of 2.3, no refusal;
2. `SetupSkeletalMeshPhysicsForPhysXEffects` asks **`Engine.GetPhysXLevel()`**, a static native that is neither bound
   nor indexed. **That is the next stop**, the unbound native's refusal;
3. if it answers 0, nothing else runs there, and `ApplyBodyClassStartingValues` goes on to `InitializeBehaviorProviders`
   (NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md: the pass-2 `LayerAbsent`). Above 0, `SetHasPhysicsAssetInstance(true)` on
   the body mesh would make the asset's bodies: the case of 3.2. What the setting holds is not read (by its name, the
   PhysX effects level).

## 5. Tests (synthetic fixtures)

1. A toy pawn (not rigid-body physics) with one attached `CylinderComponent` and one attached
   `DynamicLightEnvironmentComponent`: nothing written, no refusal.
2. An attached `SkeletalMeshComponent` with every flag off and `ClothSim` zero: `bSkelCompFixed` true and
   `ClothMeshIndexData` empty. With the pawn `PHYS_RigidBody` and the mesh its `CollisionComponent`: `bSkelCompFixed`
   false.
3. The same with `bHasPhysicsAssetInstance` set and a mesh: the physics `LayerAbsent`, nothing written.
4. The same with `bUseSingleBodyPhysics` 1, or `bEnableSoftBodySimulation` set: the `LayerAbsent`.
5. `bDisableAllRigidBody` set: nothing, `bSkelCompFixed` unchanged.
6. A skeletal entry with an attached `Attachments` component of a skeletal subclass: both written. One with
   `bHasPhysicsAssetInstance` set: the `LayerAbsent`, and **neither** written (the up-front preflight).
7. An attachment whose component is not in the attach record: not visited.
8. An attached component of an unread class (a toy native component class): the `NotRead` naming it.
9. A cloth bool set on the mesh with `ClothSim` zero: the cloth `NotRead`.
10. A `GearLikenessMeshComponent` with no `SkeletalMesh`: the writes, no cloth step (the mesh test is guarded).

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the articulated set-up, the cloth step and the soft-body step;
   - the shape lookup of a single-body skeletal component;
   - `GetPhysXLevel`'s value in the game.
3. **Fitted:** the mesh's cloth bool as `bClothMetal`.
4. **The hub's call:** 3.2's record, only if a slice component reaches a body-making row.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read her body mesh's `bSkelCompFixed`, `BodyInstance` and `PhysicsAssetInstance`: expect true,
   None and None (or set, if the PhysX level made one).
2. Read `Engine.GetPhysXLevel()` on the maintainer's settings.
