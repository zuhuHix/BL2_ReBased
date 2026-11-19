# WillowPickup.InitializePickupForRBPhysics: a minimal answer without simulation (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-PICKUP-RB-MINIMAL (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py`;
- the installed packages through `ow-package --def-dump` (`WillowPickup`'s class defaults) and
  `tools/ghidra/class_layout.py` (field names and order), plus the value names of two engine enums read from
  `Engine.upk`;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock), plus
  a local read of the native's call arguments from the image.

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/pickuprb/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

**Builds on** NATIVE_ATTACH_MESH_TO_PICKUP.md part 2 (`research/rust-attach-mesh-to-pickup-2` f32d69b), sections 6.3
and 6.6, and its implementation #520 (`fix/rust-attach-mesh-to-pickup-2`, `crates/ow-inventory/src/attach_mesh.rs`).
"6.6 row n" points at that note's table.

**Why it matters.** Links 6 and 7 stop at `make`. With the pickup's class default `bUseRBPhysics` true, #520 refuses
`NotYet { LayerAbsent }`, "WillowPickup.InitializePickupForRBPhysics: the pickup's rigid-body set-up" ("a rigid-body
physics layer (the port has none)"). Their 7 driver steps (link 6: `make`, `touch`, `key`; link 7: `make`, `touch`,
`look`, `key`) pass none. 6.6 ruled that the native's script-visible writes do not separate from the physics. This
note re-opens that with what was unread there.

## 1. What changed since 6.6: the unread entries are the engine's own script-callable natives

6.6 left rows 4, 7, 8, 10, 12, 13, 14 and 15 partly unread, as "entries" of the component or the pickup. Each entry the
native calls on the component was matched to the entry a **script-callable engine native** calls: that native's exec
step reads its arguments and calls the same entry, nothing else. So each is the C++ side of a script native the game
also exposes. With the arguments read from the image:

| 6.6 row | What the native does | The script native with the same entry | Argument |
|---|---|---|---|
| 4 | the component's block-rigid-body set | `PrimitiveComponent.SetBlockRigidBody` | true |
| 7 | with a physics asset from the inventory: `PhysicsWeight` := 1.0, then the physics asset set, then the asset-instance flag set (section 2) | `SkeletalMeshComponent.SetPhysicsAsset`, `SetHasPhysicsAssetInstance` (by their fields, see below) | the inventory's asset, false; true, false |
| 10 | the component's rigid-body-collision notify set | `PrimitiveComponent.SetNotifyRigidBodyCollision` | true |
| 12 | `RBChannel` := the channel, then the component's collision-filter update | `PrimitiveComponent.SetRBChannel` (the same two steps) | `RBCC_WillowPickup` |
| 13 | nine channel bits, each followed by the same filter update | `PrimitiveComponent.SetRBCollidesWithChannel` (the same two steps) | see section 2 |
| 14 | the body woken, then its position set | `PrimitiveComponent.WakeRigidBody`, `PrimitiveComponent.SetRBPosition` | no bone; the pickup's `Location`, no bone |
| 15 | the actor's physics mode set | `Actor.SetPhysics` (the same entry, with the script's default extra arguments) | `PHYS_RigidBody` (10) |

Row 7's two calls set fields that fit by name: `PhysicsAsset`, and the `bHasPhysicsAssetInstance` bit of the component's
bool word, under the component shift #563 measured. Their bodies were read only far enough to see those fields and
that the rest builds or tears down the physics instance. Row 8, the branch with no physics asset, is a pickup entry
that was **not read**.

**What the matching shows, and what it does not.** It does not read the entries' bodies. It shows that each one is the
engine work behind a native the script can call itself, run with the arguments above. So the native's body is:
field writes (6.6's rows 1, 2, 3, 5, 6, 9, 11, 17, 18 and the field halves of rows 7 and 12-13), a fixed sequence of
script-level physics calls, three random draws, and a body impulse.

**The channel and mode names** come from the engine's own enums (`ERBCollisionChannel`, `EPhysics` in `Engine.upk`).
Channel 24 is `RBCC_WillowPickup`, which supports the reading.

## 2. Every script-visible write and read, in the native's order (question a)

All of it runs only when `bUseRBPhysics` is set (the class default is true; 6.3).

1. **Sleep set-up** (6.6 row 1): `bForceRBToSleep` := false; timers `MaxRBAwakeTimer` (once, after `MaxRBAwakeTime`,
   15 s by class default) and `CheckForRigidBodySleepState` (1.0 s, looping), both on the pickup.
2. `RotationRate` := a global constant that sits in zero-initialised data. By fit it is zero (UNVERIFIED).
3. **The component**, `CollisionComponent` (set to the dropped mesh by `WillowWeapon.AttachMeshToPickup`), when it is
   a `PrimitiveComponent`. When it is not, steps 4 to 12 and 15 are skipped:
   4. `Scale3D` := (1, 1, 1) (fitted, 6.6);
   5. `BlockRigidBody` := true (row 4, through the script native's entry; the field name by that native, UNVERIFIED);
   6. `CollideActors` raised. When it was clear, the deferred reattach runs (part 1, section 2): `bNeedsReattach` :=
      true, and with an `Owner` a further owner update only when the owner is `bStatic` or `bTickIsDisabled` and not
      `bDeleteMe` (this lane read the condition; the owner update itself is not read). `BlockActors` := false;
   7. **with a physics asset** (row 7): the inventory names one through an entry of the inventory (not read), and it
      must hold bodies. Then `PhysicsWeight` := 1.0, `PhysicsAsset` := it, and `bHasPhysicsAssetInstance` := true with
      its physics instance. **Without one** (row 8): a pickup entry, not read;
   8. from the globals definition: `MaxImpactEffectPlayCount` := `DropImpactEffectMax` (truncated),
      `ImpactEffectDelay` := `DropImpactEffectDelay`, `ImpactVelocityAkRtpc` := `DropImpactEffectVelocityAkRtpc`.
      Each pair fits exactly at the globals definition's computed layout;
   9. `bNotifyRigidBodyCollision` := true (row 10, as step 5);
   10. `ScriptRigidBodyCollisionThresholdCompiled` := `DropImpactEffectThreshold` × 0.25, truncated to a byte;
   11. `RBChannel` := `RBCC_WillowPickup`. Then in `RBCollideWithChannels`: `RBCC_WillowPickup` and `RBCC_DeadPawn`
       cleared; `RBCC_Default`, `RBCC_GameplayPhysics`, `RBCC_EffectPhysics`, `RBCC_TossedItems`,
       `RBCC_TossedItemsPlayerVehicle`, `RBCC_TossedItemsEnemyVehicle` and `RBCC_TossedItemsPlayerVehicleEnemyVehicle`
       set. Each bit is one shift-and-mask, as the script native's;
   12. the body woken, and its position set to the pickup's `Location`. No script-visible field.
13. **`Physics` := `PHYS_RigidBody`** (row 15). The port already binds `Actor.SetPhysics` as "write the `Physics` byte"
    (`ow-combat` `hit.rs`). The engine's mode change does more physics work (6.6 row 15), not read.
14. With the component:
    - `bTorque` clear (the class default): **three draws from the engine's float random stream**. The first is in
      `PickupSpinVMin`..`PickupSpinVMax`, the second in `PickupSpinHMin`..`PickupSpinHMax`, and the third picks a sign
      (at most 0.5 is −1). They build a spin applied to the body as an impulse. The impulse acts only on a body that
      exists and is awake (a body-instance step), so it writes nothing script-visible;
    - `bTorque` set: `Torque` applied the same way, then `bTorque` := false when `bTorqueTemporary` is set.
15. Timer `EnableRagdollCollision`, 0.3 s, once, on the pickup (6.6 row 18).

**What links 6 and 7 read afterwards** (script, and the port's own steps):
- `touch`, `DroppedPickup.Pickup.Touch`, then `ValidTouch`:
  - the toucher's `bCanPickupInventory`, `DrivenVehicle` and `Controller`;
  - **`Physics` only together with `Instigator` and `Velocity.Z`:** the touch is refused when `Physics` is
    `PHYS_Falling`, the toucher is the `Instigator` and `Velocity.Z` > 0. The harness pickup has no toss velocity
    (zero), so `Physics` cannot change this answer. In the game it is `PHYS_RigidBody` after the native, not
    `PHYS_Falling`, so the condition is false there too;
  - `TouchPickupTrace` from the toucher's `Location` to `GetTouchTraceLocation()`, which is the pickup's `Location`
    raised by half its bounding cylinder's height;
  - `GameInfo.PickupQuery`.
- `look` (`ow_inventory::pickup_targeting`): the pickup's `Location` and `bPickupable`.
- `key` (`PickupSomething`): the controller's current pickup choice.

**None of those steps reads a field steps 1 to 15 write, except `Location`.** The native does not write `Location`:
in the game the simulation moves the pickup afterwards, frame by frame. On the headless harness no frame runs between
`make` and `touch`.

## 3. Is a minimal own-code answer enough for the slice? (question b)

**Yes, as an UNVERIFIED port answer: the pickup at rest at its spawn location, with no simulation.** Dropping it to the
floor is not available: the headless pickup world has no collision (`slice_pickup.rs`: the spawn passes
`bNoCollisionFail` because the world has none), so `ow-world`'s trace has nothing to hit there. At rest is also what
the touch and the look already assume (the fixture places).

**The proposed Off answer, in the native's order** (replaces #520's `RB_PHYSICS` refusal). Before any write, a
preflight checks every declaration below, the component's class, the globals definition, and the branch conditions:
- `bUseRBPhysics` false: nothing (#520, unchanged).
- Otherwise:
  1. `bForceRBToSleep` := false. Set the two timers of step 1 on the port's timer service (`ow_world::timers`).
  2. `RotationRate` := zero (UNVERIFIED, fitted).
  3. With a `PrimitiveComponent` in `CollisionComponent`:
     - `Scale3D` := (1, 1, 1); `BlockRigidBody` := true;
     - `CollideActors` raised with the deferred reattach of part 1 (`bNeedsReattach` := true, the owner branch as read
       in step 6 above). `BlockActors` := false;
     - step 7 refuses (below);
     - the three globals copies; `bNotifyRigidBodyCollision` := true; the threshold byte;
     - `RBChannel` and the nine `RBCollideWithChannels` bits.
  4. `Physics` := `PHYS_RigidBody` (10), the port's existing `SetPhysics` write.
  5. `bTorque` clear: three draws on the port's game random stream (#518's `GameRng`, counted as draws). Their values
     are discarded, since the impulse goes to a body the port does not have. `bTorque` set: `bTorque` := false when
     `bTorqueTemporary` is set.
  6. The `EnableRagdollCollision` timer, 0.3 s, once.

**What the port leaves out, and why that is the "empty physics layer":** the collision-filter updates, the body wake
and position, the mode change's physics work and the impulse. Each acts on a physics body or scene, and the port has
neither. That is the same shape as the empty scene layer the hub ruled for `Actor.AttachComponent` (part 1, section
5). **It is a hub ruling to make, not a read result:** the entries' bodies were matched, not read.

**Labels:** `Scale3D`, `CollideActors`, `BlockActors`, `MaxImpactEffectPlayCount`, `ImpactEffectDelay`,
`ImpactVelocityAkRtpc`, `ScriptRigidBodyCollisionThresholdCompiled` and `RBChannel` are fitted (6.6). `BlockRigidBody`
and `bNotifyRigidBodyCollision` are named by the script natives that share their entries. `PhysicsWeight`,
`PhysicsAsset` and `bHasPhysicsAssetInstance` are fitted with #563's component shift. All UNVERIFIED.

## 4. What still needs more than the minimal answer (question c)

Typed refusals, all raised in the preflight before any write:
1. **Row 7, with a physics asset:** `NotYet { LayerAbsent("a rigid-body physics layer (the port has none)") }`, #520's
   `RB_LAYER`, with the what "InitializePickupForRBPhysics: the pickup's physics asset instance". The writes `PhysicsAsset` and `bHasPhysicsAssetInstance`
   stand for a built physics instance, and the inventory entry that names the asset was not read. A minimal answer
   should not claim an instance it does not build.
2. **Row 8, with no physics asset:** `NotYet { NotRead }`, "InitializePickupForRBPhysics: the pickup's set-up without a
   physics asset".
3. **The deferred reattach's owner update** (owner `bStatic` or `bTickIsDisabled`, not `bDeleteMe`): part 1's
   `REATTACH_OWNER`, unchanged. The slice's pickup has both clear (class defaults), so its branch writes only the
   flag.
4. **The slice's own case decides which of 1 and 2 applies,** and it was not settled here: which object the inventory
   entry returns for a weapon, and whether it holds bodies. So the next stop after #520's is refusal 1 or 2, by the
   slice's data (section 5).

**What would need real physics:** the pickup falling, tumbling and coming to rest; the sleep timers' effects;
`EnableRagdollCollision`'s channel changes on a live body. None of them is read by links 6 and 7. **No physics crate
is needed for the slice**, so no ATTENTION.

## 5. The next stop (question d)

**The probe** (local merge of origin/main d52bcc7 with #520 `fix/rust-attach-mesh-to-pickup-2` 4d0539b, never pushed):
the native's refusal was replaced by a stand-in that writes nothing and returns, then `--slice-gate` ran under the
machine lock.

**Result:** links 6 and 7 both stay `notyet` at `make`, with 0 of their 7 steps passing. The stop moves to #520's
next native, `WillowPickup.SpawnPickupParticles` ("stopped after partial writes: ... the pickup's particle effect:
needs a particle-effects layer (the port has none)"). That is #520's `PARTICLES_LAYER`, raised because the pickup's
`PickupParticles` is set. The other twelve links read the same as on origin/main: 1, 3-5, 13 and 14 `error` at
`GameEngine.GetOnlineSubsystem`; 8 `error` at `CacheWeaponCard`; 9-12 `notyet`; 2 `ok`.

**So the next stop after this note's answer is `SpawnPickupParticles`, not this native's refusals 1 or 2.** The stand-in
wrote nothing, so it did not reach section 4's asset branch. With the section 3 answer, the run reaches refusal 1 or 2
first, by the slice's data. Only once one of those is answered does it reach `SpawnPickupParticles`. That native is
the next analyst question for links 6 and 7: is its effect separable from a particle layer, as this native's writes
are from physics?

## Open questions

1. **Not read:** row 8 (the set-up without a physics asset), the inventory entry that names the physics asset, and
   the deferred reattach's owner update.
2. **UNVERIFIED:** that the matched entries do nothing script-visible beyond their fields (section 1); that
   `RotationRate`'s constant is zero.
3. **UNVERIFIED:** that the slice's steps read nothing the simulation would change before the touch, beyond
   `Location`.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `WillowPickup.InitializePickupForRBPhysics` on a weapon drop. Record, before and after: `Physics`,
   `RotationRate`, `bForceRBToSleep`, `bTorque`, `MaxImpactEffectPlayCount`, `ImpactEffectDelay`,
   `ImpactVelocityAkRtpc`, and the pickup's timers. On the mesh: `Scale3D`, `CollideActors`, `BlockActors`,
   `BlockRigidBody`, `bNotifyRigidBodyCollision`, `ScriptRigidBodyCollisionThresholdCompiled`, `RBChannel`,
   `RBCollideWithChannels`, `PhysicsAsset`, `PhysicsWeight`, `bHasPhysicsAssetInstance`. Expect section 2's values.
2. Call `FRand` before and after the native on a drop with `bTorque` clear, against a seeded expectation: it should be
   three draws on.
3. Read the inventory's physics asset for a dropped weapon (the entry in step 7), to settle section 4 point 4.
