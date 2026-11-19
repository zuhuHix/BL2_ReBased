## 2026-11-19: Read the rest of the pickup's make step, part 2 (analyst)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-ATTACH-MESH-TO-PICKUP, part 2, analyst only. The number NNN is a
placeholder; the hub gives it. Stacked on part 1 (#505). This lane writes docs only.

- **Why.** The hub ruled on part 1's section 5: a slice world in Off gets an explicit, empty scene layer, in which
  `Actor.AttachComponent` makes only its script-visible writes. So links 6 and 7 go past `AttachMeshToPickup`, and
  the rest of the `make` step needed Off answers.
- **How.**
  - The script was read first (`InventoryAssociated`, `SetPickupability`, `TryToSetInteractParticles`,
    `SetInteractParticles`, the `Pickup` state).
  - The natives were read with Ghidra under the machine lock: the project query for the exec bodies, and the
    constructor-to-class-table method for `SetLightEnvironment` and `SpawnPickupParticles`.
  - The class defaults of `WillowPickup` were read with `ow-package --properties`.
  - Raw output stays under `local/analysis/attachmesh/`.
- **Added to `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md` (section 6)**, with three `NATIVE_INDEX.md` rows:
  - **`SetLightEnvironment`:** nothing when unchanged. Otherwise the new environment; and only while attached, the
    previous one and a deferred reattach. On the slice, under the scene layer (which leaves `bAttached` false), that
    is one write.
  - **`InitializePickupForRBPhysics`:** nothing without `bUseRBPhysics`. Otherwise `NotYet { LayerAbsent }`, the
    rigid-body set-up. `WillowPickup`'s class default sets `bUseRBPhysics`.
  - **`SpawnPickupParticles`:** `NotRead` with no particle system; otherwise `NotYet { LayerAbsent }`, the particle
    effect. Both are prefixed, since `SetInteractParticles` has written by then. It is not reached on the slice in
    default Off while the rigid-body stop stands; if the hub rules a port answer for that, the stop moves to it, and
    it needs a ruling of its own.
  - **Where links 6 and 7 stop:** at `WillowPickup.InitializePickupForRBPhysics`, `LayerAbsent`, prefixed by the
    `make` step. Link 7 stops on its first gun.
- **The physics-layer question (section 6.6).** The hub asked for the native's script-visible writes, apart from
  the physics work, as a candidate answer for an empty physics layer. Review r1 of #507 asked for one table row per
  effect. **They do not separate from the physics:**
  - raising `CollideActors` (class default false) runs a deferred reattach, which under the scene layer answers the
    owner-branch `NotRead` inside the native;
  - the physics mode entry writes `Physics`, which `DroppedPickup.Pickup.ValidTouch` reads, and its argument was not
    recovered;
  - several component and pickup entries were not read, and the spin moves the engine's random stream.

  Newly read: the sleep set-up entry (`bForceRBToSleep` and two timers), the three pickup fields (fitted as
  `MaxImpactEffectPlayCount`, `ImpactEffectDelay`, `ImpactVelocityAkRtpc`), the channel bit helper, and the timer's
  name (`EnableRagdollCollision`, 0.3 s; the harness does not advance the world before the touch). Nothing earlier on
  the route writes `bUseRBPhysics`. So the note offers no candidate, the answer stays `LayerAbsent`, and links 6 and 7
  leave the queue for now.
- **Not verified:** everything above in the running game; the fitted identities (the `LightEnvironment` fields, by
  the same shift as part 1's bool word; the pickup's primitive component; the currency kind; the field names and the
  timer pairing in 6.6's table); the bodies' entries that were not read (section 6 and open questions 6 and 7).
