# WillowPickup.InitializePickupForRBPhysics: a minimal answer without simulation (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-PICKUP-RB-MINIMAL (decision 2026-11-19-190, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py`;
- the installed packages: `WillowPickup`'s class defaults, `tools/ghidra/class_layout.py` for field names, types and
  order, and the value names of two engine enums read from `Engine.upk`;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock), plus
  a local read of the native's call arguments from the image.

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/pickuprb/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

**Builds on** NATIVE_ATTACH_MESH_TO_PICKUP.md part 2 (`research/rust-attach-mesh-to-pickup-2` f32d69b), sections 6.3
and 6.6, and its implementation #520 (`fix/rust-attach-mesh-to-pickup-2`, `crates/ow-inventory/src/attach_mesh.rs`).
"6.6 row n" points at that note's table. Where this note and 6.6 differ, this note was read later and wins: the order
of the guards (section 2) and the physics-asset test (section 2, step 4.4) are corrected here.

**Why it matters.** Links 6 and 7 stop at `make`. With the pickup's class default `bUseRBPhysics` true, #520 refuses
with a `NotYet` of kind `LayerAbsent`: "WillowPickup.InitializePickupForRBPhysics: the pickup's rigid-body set-up"
("a rigid-body physics layer (the port has none)"). Their 7 driver steps (link 6: `make`, `touch`, `key`; link 7:
`make`, `touch`, `look`, `key`) pass none. 6.6 ruled that the native's script-visible writes do not separate from the
physics. This note re-opens that with what was unread there.

## 1. The entries 6.6 left unread are the engine's own script-callable natives

6.6 left rows 1, 4, 7, 8, 10, 12, 13, 14, 15 and 16 partly unread, as "entries" of the component, the pickup, the
inventory or the globals definition. Each was matched to the **script-callable native** whose exec step reads its
arguments and calls the same entry, and nothing else. With the arguments read from the image:

| 6.6 row | What the native does | The script native with the same entry | Argument |
|---|---|---|---|
| 1 | the sleep set-up | `WillowPickup.StartRigidBodyTimers` | none |
| 4 | the component's block-rigid-body set | `PrimitiveComponent.SetBlockRigidBody` | true |
| 7 | the inventory's definition, for its physics asset | `WillowInventory.GetInventoryDefinition` | none |
| 7 | with a physics asset: `PhysicsWeight` := 1.0, the asset set, the asset-instance flag set | `SkeletalMeshComponent.SetPhysicsAsset`, `SetHasPhysicsAssetInstance` (by the fields they write) | the asset, false; true, false |
| 8 | without one: the pickup's own rigid-body set-up | `WillowPickup.InitializeRBPhysics` | none |
| 10 | the component's rigid-body-collision notify set | `PrimitiveComponent.SetNotifyRigidBodyCollision` | true |
| 12 | `RBChannel` := the channel, then the component's collision-filter update | `PrimitiveComponent.SetRBChannel` (the same two steps) | `RBCC_WillowPickup` |
| 13 | nine channel bits, each followed by the same filter update | `PrimitiveComponent.SetRBCollidesWithChannel` (the same two steps) | section 2, step 4.8 |
| 14 | the body woken, then its position set | `PrimitiveComponent.WakeRigidBody`, `PrimitiveComponent.SetRBPosition` | no bone; the pickup's `Location`, no bone |
| 15 | the actor's physics mode set | `Actor.SetPhysics` (the same entry, with the script's default extra arguments) | `PHYS_RigidBody` (10) |
| 16 | the spin | `GlobalsDefinition.ApplySpinToPickup` (the same routine, on the globals definition) | the component |

**What the matching shows, and what it does not.** The bodies of the component and actor entries were not read. The
matching shows that each is the engine work behind a native the script can call itself, run with the arguments above.
`StartRigidBodyTimers`'s effect is 6.6 row 1. `GetInventoryDefinition` has its own note
(NATIVE_INVENTORY_DEFINITION.md). `InitializeRBPhysics` was **not read**. `ApplySpinToPickup` was read (step 6).

**The channel and mode names** come from the engine's own enums (`ERBCollisionChannel`, `EPhysics` in `Engine.upk`).
Channel 24 is `RBCC_WillowPickup`, which supports the reading.

## 2. The native's sequence, in order (question a)

The guards are: `bUseRBPhysics`, then whether there is a component, then the per-step conditions given below.

0. `bUseRBPhysics` clear: nothing at all (#520, unchanged). Otherwise, in order:
1. **Sleep set-up** (`StartRigidBodyTimers`, 6.6 row 1): `bForceRBToSleep` := false. Timer `MaxRBAwakeTimer`, once,
   after `MaxRBAwakeTime` (15 s by class default). Timer `CheckForRigidBodySleepState`, 1.0 s, looping. Both are on
   the pickup.
2. `RotationRate` := a global constant that sits in zero-initialised data. By fit it is zero (UNVERIFIED).
3. **The component** C is `CollisionComponent` when that is a `PrimitiveComponent`, else none. `CollisionComponent` is
   the dropped mesh `WillowWeapon.AttachMeshToPickup` set, or the class default `CollisionCylinder`.
4. **Only with C:**
   1. G := the globals definition (`WillowGlobals`' `GetGlobalsDefinition`), possibly none;
   2. `Scale3D` := (1, 1, 1) (fitted, 6.6);
   3. `BlockRigidBody` := true (`SetBlockRigidBody`; the field named by that native, UNVERIFIED);
   4. `CollideActors`: when it is clear, it is set and **the deferred reattach** runs (part 1, section 2); when it is
      already set, nothing. Then `BlockActors` := false. The deferred reattach:
      - `bNeedsReattach` := true;
      - when C has an owner, and that owner is `bStatic` or `bTickIsDisabled`, and the owner is not `bDeleteMe`: the
        owner's component update (not read; two further object-flag tests on that path were not read either);
      - when C has an owner and that does not hold: nothing more;
      - when C has no owner: part 1's detach and reattach (not read), which part 1 found does nothing script-visible
        when C's `Scene` is zero.
   5. **The physics-asset test** (corrects 6.6, which said the asset must hold bodies). The asset branch is taken
      when all three hold:
      - C is a `SkeletalMeshComponent`;
      - the pickup's `Inventory` is set, and its `GetInventoryDefinition()` answers a definition;
      - that definition's `PhysicsAsset` (`WillowInventoryDefinition.PhysicsAsset`, fits exactly) is not None.

      There is no body count and no other test of the asset. **The asset branch:** `PhysicsWeight` := 1.0, then
      `PhysicsAsset` := that asset, then `bHasPhysicsAssetInstance` := true, with the physics instance built.
      **Otherwise:** `WillowPickup.InitializeRBPhysics()` (not read).
   6. With G: `MaxImpactEffectPlayCount` (an int) := `DropImpactEffectMax` (a float), truncated toward zero;
      `ImpactEffectDelay` := `DropImpactEffectDelay`; `ImpactVelocityAkRtpc` := `DropImpactEffectVelocityAkRtpc`.
      Each pair fits at the computed layouts;
   7. `bNotifyRigidBodyCollision` := true (`SetNotifyRigidBodyCollision`, as step 4.3). Then, with G:
      `ScriptRigidBodyCollisionThresholdCompiled` (a byte) := `DropImpactEffectThreshold` × 0.25, truncated;
   8. `RBChannel` := `RBCC_WillowPickup`. Then nine bit updates of `RBCollideWithChannels`, in this order:
      `RBCC_WillowPickup` clear, `RBCC_DeadPawn` clear, then `RBCC_Default`, `RBCC_GameplayPhysics`,
      `RBCC_EffectPhysics`, `RBCC_TossedItems`, `RBCC_TossedItemsPlayerVehicle`, `RBCC_TossedItemsEnemyVehicle` and
      `RBCC_TossedItemsPlayerVehicleEnemyVehicle` set. Each one is a single shift-and-mask of its own bit, followed
      by the filter update;
   9. the body woken, and its position set to the pickup's `Location`. Nothing script-visible.
5. **Always** (with or without C): `Physics` := `PHYS_RigidBody` (`SetPhysics`). The port already binds
   `Actor.SetPhysics` as "write the `Physics` byte" (`ow-combat` `hit.rs`). The engine's mode change does more
   physics work (6.6 row 15), which was not read.
6. **Only with C:**
   - `bTorque` clear (the class default): **only with G**, `ApplySpinToPickup(C)`. That is three draws from the
     engine's **seeded** random generator (the linear-congruential one with the public UE3 seeded-random constants).
     It is not the generator script `FRand` uses. The first draw is scaled into `PickupSpinVMin`..`PickupSpinVMax`
     and the second into `PickupSpinHMin`..`PickupSpinHMax` (all `GlobalsDefinition` fields). The third picks a sign:
     at most 0.5 is −1. The spin is then applied to C's body as an impulse, which writes nothing script-visible.
     Without G: no draw;
   - `bTorque` set: `Torque` is applied to C's body the same way, with no draw. Then `bTorque` := false when
     `bTorqueTemporary` is set.
7. **Always:** timer `EnableRagdollCollision`, 0.3 s, once, on the pickup.

**What links 6 and 7 observe of these writes.** Two lists are kept apart here. The first is which of the native's
writes the drivers read. The second is what the game's simulation would change that the drivers read.

- **Writes the drivers read:**
  - **`Physics`** (step 5), read by `ValidTouch` in `touch`. It matters only together with the toucher being the
    `Instigator` and a positive `Velocity.Z`, and only for `PHYS_Falling`. `PHYS_RigidBody` takes the same branch
    as the class default `PHYS_None`, so this write does not change the touch's answer.
  - **The timers** (steps 1 and 7) would be read only if time advanced. The harness advances no time between
    `make` and `key`, so none of them fires.
  - **None of step 4's component writes** is read by `touch` (the trace uses actor locations and the bounding
    cylinder), by `look` or by `key`.
- **What the drivers read that a simulation would change:** the pickup's `Location` (touch trace, look, key) and,
  through `RotationRate` and the body, its `Rotation`. Link 7's targeting reads `Rotation` when the definition's
  `PickupFlagOffset` is not zero. The native writes neither; in the game the simulation moves them on later frames,
  and the harness runs none.
- **What the drivers read that this native does not touch:** `bPickupable`, `LifeSpan`, `Inventory` (the
  targeting's reads), and the decoy's `Inventory`, `bPickupable` and destroyed state, plus the gun's place in Maya's
  inventory chain (the `taken` check).

## 3. Is a minimal own-code answer enough for the slice? (question b)

**As a rule, yes** (hub ruling on #574, UNVERIFIED): the pickup at rest at its spawn location, with no simulation. A
drop to the floor is not available, because the headless pickup world has no collision (`slice_pickup.rs` spawns with
`bNoCollisionFail` for that reason).

**But on the slice, the note's answer alone does not pass this native.** Every case with a component reaches step
4.5. There the asset branch is a missing layer, and the branch without an asset is unread. Both refuse (section 4).

**What the slice fixture's lookup returns** (a second local probe of the installed slice gate, section 5, using
loaded-only reads and the write-free `inventory_definition_of`). On both links:
- `CollisionComponent` is an `Engine.SkeletalMeshComponent`;
- `Inventory` is a `WillowGame.WillowWeapon`;
- its definition is a `WillowGame.WeaponTypeDefinition` already on the heap, so not refusal 5, and the lookup raised
  none of its own refusals;
- that definition's `PhysicsAsset` holds an `Engine.PhysicsAsset`.

So the slice takes **the asset branch**. Before it, the reattach check (refusals 3 and 4) applies only when the mesh's
`CollideActors` is clear. This was not probed. By section 4 it gives the flag only on the slice: the owner is the
pickup, which is neither static nor tick-disabled. **The slice's first refusal under this note's answer is therefore
refusal 1.** Refusal 6 would come after it. Under the hub's ruling below, both are answered and the native passes.
Only a pickup with no `PrimitiveComponent` passes the whole native without that ruling.

**The Off answer, for the implementer** (it replaces #520's `RB_PHYSICS` refusal):

**Preflight, write-free, before the first write.** It uses only loaded objects and reads nothing that would load. It
runs in this order and checks only what the chosen path needs:
1. `bUseRBPhysics` (its declaration, then its value). Clear: return at once, with no further check and no write
   (#520's answer, unchanged).
2. C: `CollisionComponent`'s declaration and value, and whether that object is a `PrimitiveComponent`.
3. The declarations of the pickup's fields that steps 1, 2, 5, 6 and 7 write or read (`bForceRBToSleep`,
   `MaxRBAwakeTime`, `RotationRate`, `Physics`, `bTorque`, `bTorqueTemporary`, `Torque`). Without C, the preflight
   ends here and the path is steps 1, 2, 5 and 7.
4. With C only:
   1. G as the pickup world's set-up prepared it (`slice_pickup.rs` `globals_preflight`: the world's one
      `WillowGlobals` and its definition). Without it, that preflight's own refusal stands. This answer depends on
      that preload;
   2. the declarations of C's fields in step 4, and of the pickup's and G's fields in steps 4.6, 4.7 and 6;
   3. with `CollideActors` clear, the deferred reattach check: refusal 3 or 4 when it applies;
   4. the step 4.5 test:
      - C is not a `SkeletalMeshComponent`, or `Inventory` is None: refusal 2;
      - otherwise `GetInventoryDefinition()` as NATIVE_INVENTORY_DEFINITION.md section 3 answers it
        (`ow_inventory::definition::inventory_definition_of`, which writes nothing). Its refusals pass through;
      - a None definition: refusal 2;
      - a definition object that is not loaded: refusal 5;
      - `PhysicsAsset`, read as a handle that is not followed and not loaded: not None is refusal 1, None is
        refusal 2;
   5. last, once the asset test lets the run go on: with G, and `bTorque` clear, and no seeded stream in the port,
      refusal 6.

**The hub's ruling on top of this (for IMPL-PICKUP-RB, recorded in its brief and decision file).** On the slice in
Off, the asset branch is answered at rest: its non-physics writes run, no rigid body is made, and it is logged
UNVERIFIED. That replaces refusal 1. The spin draws on a dedicated `GameRng` stream for the engine's seeded generator,
labelled UNVERIFIED, which removes refusal 6. Refusals 2 to 5 stay.

**Writes, in section 2's order, when the preflight passes** (only a pickup with no `PrimitiveComponent` gets here
today):
- steps 1, 2, 5 and 7: `bForceRBToSleep`, the two timers on the port's timer service (`ow_world::timers`),
  `RotationRate`, `Physics`, and the `EnableRagdollCollision` timer;
- with C, once refusals 1 and 2 are answered by later lanes: step 4's field writes. The channel bits may be one write
  of `RBCollideWithChannels` that changes only the nine named bits and keeps all the others; the filter updates
  between them have no script-visible effect;
- step 6.

**The random stream (a port choice for the hub, UNVERIFIED).** The three draws are on the engine's seeded generator.
`GameRng` has no stream for it: its `ScriptStream::FRand` is script `FRand`, and the game's `FRand` is not advanced by
this native. A draw on `FRand` or on any `CrtStream` would shift another rule's draws. #518's aggregate draw count
would not catch that.

The proposal: a new named stream for the engine's seeded generator, added after the existing ids so that no existing
stream's seed moves. Until it exists, the implementer refuses step 6's spin as refusal 6 rather than draw on another
stream. The draws come after the preflight, in step 6's order, and the `bTorque`-set path makes none.

**What the port leaves out, and why it counts as the empty physics layer:** the filter updates, the body wake and
position, the mode change's physics work and the impulses. Each acts on a physics body or scene, and the port has
neither. This is the same shape as the empty scene layer the hub ruled for `Actor.AttachComponent` (part 1, section
5). The entries were matched, not read, so this is the hub's ruling, not a read result.

**Labels:**
- Fitted (6.6): `Scale3D`, `CollideActors`, `BlockActors`, `MaxImpactEffectPlayCount`, `ImpactEffectDelay`,
  `ImpactVelocityAkRtpc`, `ScriptRigidBodyCollisionThresholdCompiled` and `RBChannel`.
- Named by the script natives that share their entries: `BlockRigidBody` and `bNotifyRigidBodyCollision`.
- Fitted with #563's component shift: `PhysicsWeight`, `PhysicsAsset` and `bHasPhysicsAssetInstance`.
- Fits exactly: `WillowInventoryDefinition.PhysicsAsset`.

All are UNVERIFIED.

## 4. Typed refusals, all raised in the preflight (question c)

| # | When | Kind | Text | Record |
|---|---|---|---|---|
| 1 | the asset branch (step 4.5) | `LayerAbsent` | "InitializePickupForRBPhysics: the pickup's physics asset instance", layer "a rigid-body physics layer (the port has none)" (#520's `RB_LAYER`) | `docs/verification/NATIVE_PICKUP_RB_MINIMAL.md` |
| 2 | the branch without an asset (step 4.5) | `NotRead` | "WillowPickup.InitializeRBPhysics: the pickup's rigid-body set-up without a physics asset" | `docs/verification/NATIVE_PICKUP_RB_MINIMAL.md` |
| 3 | `CollideActors` clear, C has an owner, the owner is `bStatic` or `bTickIsDisabled`, and the owner is not `bDeleteMe` | `NotRead` | "the deferred reattach: the owner's component update" (#520's `REATTACH_OWNER`) | `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md` |
| 4 | `CollideActors` clear, C has no owner, and C's `Scene` is not zero | `NotRead` | "the deferred reattach: the component's detach and reattach" (#520's `REATTACH_SCENE`) | `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md` |
| 5 | the inventory's definition is not a loaded object | `NotRead` | "InitializePickupForRBPhysics: the physics asset of an inventory definition that is not loaded" | `docs/verification/NATIVE_PICKUP_RB_MINIMAL.md` |
| 6 | last in the preflight: C and G present, `bTorque` clear, and the port has no seeded stream | `NotPorted` | "GlobalsDefinition.ApplySpinToPickup: the engine's seeded random stream" | `docs/verification/NATIVE_PICKUP_RB_MINIMAL.md` |

**This native has its own reattach check**, and it is narrower than #520's. #520's shared preflight refuses any C
with an owner. Here, an owner that is neither `bStatic` nor `bTickIsDisabled`, or that is `bDeleteMe`, gets
`bNeedsReattach` only, which this lane read. When the three conditions hold, refusal 3 is raised even though two
object-flag tests on that path were not read. That is the conservative side. The ownerless case keeps part 1's rule:
`Scene` zero is the flag only, and `Scene` not zero is refusal 4. On the slice C's owner is the pickup, which
`AttachComponent` set. The pickup's class defaults clear `bStatic`, `bTickIsDisabled` and `bDeleteMe`, so the slice
gets the flag only.

**What would need real physics:** the pickup falling, tumbling and coming to rest, and the sleep timers' and
`EnableRagdollCollision`'s effects on a live body. Links 6 and 7 read none of them. **No physics crate is needed for
the slice**, so there is no ATTENTION.

## 5. The next stop (question d)

**The probe** was a local merge of origin/main d52bcc7 with #520 (`fix/rust-attach-mesh-to-pickup-2` 4d0539b), never
pushed. The native's refusal was replaced by a stand-in that writes nothing and returns, then `--slice-gate` ran under
the machine lock.

**Result of that stand-in:** links 6 and 7 both stay `notyet` at `make`, with 0 of their 7 steps passing. The stop
moves to #520's next native, `WillowPickup.SpawnPickupParticles` ("stopped after partial writes: ... the pickup's
particle effect: needs a particle-effects layer (the port has none)"). That is #520's `PARTICLES_LAYER`, raised
because the pickup's `PickupParticles` is set. The other twelve links read the same as on origin/main: 1, 3-5, 13 and
14 `error` at `GameEngine.GetOnlineSubsystem`; 8 `error` at `CacheWeaponCard`; 9-12 `notyet`; 2 `ok`.

**A second probe** on the same merge replaced the stand-in with one that writes nothing and reports the asset test's
inputs (section 3). Its result: the slice takes the asset branch, and the gate's link results were unchanged.

**The note's answer alone does not get that far.** It stops at `make` on refusal 1. With the hub's ruling (the asset
branch at rest, and the seeded stream), the native passes on the slice. The run then reaches
`SpawnPickupParticles`, the next read for links 6 and 7. `InitializeRBPhysics` is not on the slice's path.

## Open questions

1. **Not read:** `WillowPickup.InitializeRBPhysics`; the deferred reattach's owner update and its two object-flag
   tests; whether the slice mesh's `CollideActors` is clear before the native (it decides whether the reattach runs).
   The slice gun's definition holds a `PhysicsAsset` (probe, section 3).
2. **UNVERIFIED:** that the matched entries do nothing script-visible beyond their fields (section 1); that
   `RotationRate`'s constant is zero.
3. **A hub choice:** the seeded random stream (section 3).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `WillowPickup.InitializePickupForRBPhysics` on a weapon drop. Record, before and after, on the pickup:
   `Physics`, `RotationRate`, `bForceRBToSleep`, `bTorque`, `MaxImpactEffectPlayCount`, `ImpactEffectDelay`,
   `ImpactVelocityAkRtpc` and its timers. On the mesh: `Scale3D`, `CollideActors`, `BlockActors`, `BlockRigidBody`,
   `bNotifyRigidBodyCollision`, `ScriptRigidBodyCollisionThresholdCompiled`, `RBChannel`, `RBCollideWithChannels`,
   `PhysicsAsset`, `PhysicsWeight`, `bHasPhysicsAssetInstance` and `bNeedsReattach`. Expect section 2's values.
2. The component guard: call the native on a pickup whose `CollisionComponent` is not a `PrimitiveComponent`, or is
   None. Expect only steps 1, 2, 5 and 7 (`Physics`, `RotationRate`, `bForceRBToSleep`, the timers).
3. The asset test: read the dropped gun's `Inventory.GetInventoryDefinition().PhysicsAsset`, and whether
   `InitializeRBPhysics` runs (hook it) when that is None.
4. The reattach: on a mesh with `CollideActors` clear, the four owner cases (none; set with `bStatic`; set with
   `bTickIsDisabled`; set with neither), and `bDeleteMe` set on the owner. Record `bNeedsReattach` and the
   component's attachment.
5. The random stream: call `FRand` before and after the native, against a seeded expectation. Expect it not advanced
   by the native. Then repeat with `bTorque` and `bTorqueTemporary` set on the pickup before the call. Expect no spin
   draw, `bTorque` cleared after, and the applied spin equal to `Torque`.
