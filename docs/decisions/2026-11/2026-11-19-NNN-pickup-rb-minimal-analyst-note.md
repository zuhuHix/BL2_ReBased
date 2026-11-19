## 2026-11-19: A minimal answer for the pickup's rigid-body set-up, as analyst

AI-assisted (Claude Code, Opus 5.5).

Lane ANALYST-PICKUP-RB-MINIMAL, analyst only. Another lane implements it (IMPL-PICKUP-RB). The number is a
placeholder (NNN) for the hub to assign.

Links 6 and 7 stop at `make` on #520's refusal of `WillowPickup.InitializePickupForRBPhysics` (layer absent,
rigid-body physics). #520's note (NATIVE_ATTACH_MESH_TO_PICKUP.md 6.6) ruled that the native's writes do not separate
from the physics work. This lane re-opens that question. The new own-words note
`docs/verification/NATIVE_PICKUP_RB_MINIMAL.md` and one `NATIVE_INDEX.md` row record, as behaviour only:

- **The entries 6.6 left unread are script-callable natives.** Each was matched to the script native whose exec step
  calls the same entry, and the arguments were read from the image:
  - the sleep set-up is `StartRigidBodyTimers`;
  - the component's physics calls are `SetBlockRigidBody`, `SetNotifyRigidBodyCollision`, `SetRBChannel`,
    `SetRBCollidesWithChannel`, `WakeRigidBody` and `SetRBPosition`, and the mode change is `Actor.SetPhysics`;
  - the asset lookup is `WillowInventory.GetInventoryDefinition`, then the definition's `PhysicsAsset`;
  - the branch without an asset is `WillowPickup.InitializeRBPhysics` (not read);
  - the spin is `GlobalsDefinition.ApplySpinToPickup`.
- **One consistent sequence:**
  - with `bUseRBPhysics` set: the sleep set-up and `RotationRate` first;
  - only with a `PrimitiveComponent`: the component's field writes, the reattach, the asset test, the
    globals-definition copies and the channel bits;
  - always: `Physics` := `PHYS_RigidBody`;
  - only with a component: the spin, which is three draws on the engine's seeded generator and needs the globals
    definition, or the stored torque;
  - always: the ragdoll-collision timer.
- **What the drivers observe:** of the native's writes, only `Physics`. `ValidTouch` reads it, and `PHYS_RigidBody`
  takes the same branch as `PHYS_None`. The timers do not fire, since the harness advances no time. What a simulation
  would change (`Location`, `Rotation`) is not written by this native.
- **Hub ruling on #574:** the Off answer is the pickup at rest with no simulation, labelled UNVERIFIED. The
  physics-body work is the empty physics layer, the same shape as the empty scene layer ruled for `AttachComponent`.
- **Typed refusals, all in a write-free preflight that uses loaded-only reads and the pickup world's globals
  preload:**
  - the asset branch (layer absent);
  - the branch without an asset (not read);
  - the reattach's owner update when the owner is static or tick-disabled and not being deleted (not read);
  - an ownerless component with a scene (not read);
  - an inventory definition that is not loaded (not read);
  - the spin until the port has a seeded stream (not ported). The new stream is a port choice for the hub; `FRand`
    and the existing streams must not be used.
- **So the slice does not pass this native yet:** its dropped mesh always reaches the asset test, and both branches
  refuse. A local probe (a merge of origin/main with #520, never pushed) answered the whole native with no write.
  Links 6 and 7 then stopped at `make` on #520's `WillowPickup.SpawnPickupParticles` (particle layer absent). The
  other links did not change.

Method: the script with `research/script_disasm.py`; the layouts with `tools/ghidra/class_layout.py`; the enum value
names from `Engine.upk`; the native with Ghidra 12.1.4 headless, read-only, under the machine lock. Raw output stays
under ignored `local/analysis/pickuprb/`. No bl-sdk session was granted and no recorded evidence covers this native,
so every rule is UNVERIFIED. The note lists five bl-sdk checks.

Open: `InitializeRBPhysics`; the reattach's owner update; whether the slice gun's definition holds a physics asset;
whether the matched entries do anything script-visible beyond their fields.
