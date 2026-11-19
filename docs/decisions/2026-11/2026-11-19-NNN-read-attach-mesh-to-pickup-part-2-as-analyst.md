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
    effect. It is not reached on the slice in default Off.
  - **Where links 6 and 7 stop:** at `WillowPickup.InitializePickupForRBPhysics`, `LayerAbsent`, prefixed by the
    `make` step. Link 7 stops on its first gun.
- **Not verified:** everything above in the running game; the fitted identities (the `LightEnvironment` fields, by
  the same shift as part 1's bool word; the pickup's primitive component; the currency kind); the bodies' entries
  that were not read (section 6 and open questions 6 and 7).
