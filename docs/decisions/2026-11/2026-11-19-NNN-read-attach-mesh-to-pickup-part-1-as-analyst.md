## 2026-11-19: Read WillowWeapon.AttachMeshToPickup's natives, part 1 (analyst)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-ATTACH-MESH-TO-PICKUP, analyst only. The number NNN is a
placeholder; the hub gives it. This lane writes docs only. A fresh implementer codes from the note alone.

- **Why.** After #492's ruling (the `SetAnimTreeTemplate` registry walk answers "nothing" in Off), c376 expects links
  6 and 7 to stop next at the unbound `SkeletalMeshComponent.SetForceRefPose`, inside
  `WillowWeapon.AttachMeshToPickup`.
- **How.**
  - The script was read with `research/script_disasm.py`, and a scan of every script function found the callers.
  - The natives were read with Ghidra under the machine lock: the project query for the exec bodies, and the
    constructor-to-class-table method of `docs/NATIVE_ANALYSIS.md` for `SetHidden` and `SetOwnerNoSee`.
  - Field names come from the layout tool where it can lay the class out (`ActorComponent`, `Actor`,
    `PrimitiveComponent`), and by fit where it cannot (`SkeletalMeshComponent`).
  - Raw output stays under `local/analysis/attachmesh/`.
- **New record:** `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md`, part 1, with four `NATIVE_INDEX.md` rows. Its
  Off answers are Off only; Reference keeps the stub policy.
  - **The deferred reattach** (shared): `bNeedsReattach` = true. With an `Owner`, or with no `Owner` but a `Scene`,
    the answer is a preflight `NotRead`.
  - **`SetForceRefPose(bool)`:** `bForceRefpose` = 0 or 1, then the reattach.
  - **`SetHidden(bool)`, `SetOwnerNoSee(bool)`:** nothing when the flag is unchanged; otherwise the flag, then the
    reattach.
  - **`HideBoneByName(name,byte)`:** nothing with a None name or no skeletal mesh (the slice's case); otherwise
    `NotRead`.
  - **`Actor.AttachComponent(object)`:**
    - nothing for a None component or a `bDeleteMe` actor;
    - otherwise the default answer is `NotYet { LayerAbsent }`: the world's scene, which the port does not have;
    - section 5 puts the option of a script-visible-only port answer to the hub.
  - **Effect:** in default Off, links 6 and 7 stop at `AttachComponent`, with the `make` step's prefix.
- **Part 2** (section 6 of the note) lists what the rest of the `make` step reaches: `SetLightEnvironment`,
  `InitializePickupForRBPhysics` and `SpawnPickupParticles`. Not written up yet.
- **Not verified:** everything above in the running game. The fitted identities are `bForceRefpose`, the `HiddenGame`
  and `bOwnerNoSee` word, the `SkeletalMesh` read and the pending-kill flag. What a component made with a template
  holds in the game is open (the port's `new` ignores the template).
