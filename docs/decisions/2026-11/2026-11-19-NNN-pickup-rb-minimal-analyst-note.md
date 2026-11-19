## 2026-11-19: A minimal answer for the pickup's rigid-body set-up, as analyst

AI-assisted (Claude Code, Opus 5.5).

Lane ANALYST-PICKUP-RB-MINIMAL, analyst only. Another lane implements it. The number is a placeholder (NNN) for the
hub to assign. Links 6 and 7 stop at `make` on #520's refusal of `WillowPickup.InitializePickupForRBPhysics` (layer
absent, rigid-body physics). #520's note (NATIVE_ATTACH_MESH_TO_PICKUP.md 6.6) ruled that the native's writes do not
separate from the physics work. This lane re-opens that question. The new own-words note
`docs/verification/NATIVE_PICKUP_RB_MINIMAL.md` and one `NATIVE_INDEX.md` row record, as behaviour only:

- **The entries 6.6 left unread are the engine's script-callable physics natives:** `SetBlockRigidBody`,
  `SetNotifyRigidBodyCollision`, `SetRBChannel`, `SetRBCollidesWithChannel`, `WakeRigidBody`, `SetRBPosition` and
  `Actor.SetPhysics`. Each was matched to the script native whose exec step calls the same entry, and the arguments
  were read from the image. Two physics-asset setters were identified by the fields they write. Their bodies were
  matched, not read.
- **The native's script-visible writes, in order:** the sleep flag and two timers; `RotationRate`; the component's
  scale, collision flags, impact-effect copies from the globals definition, threshold byte, channel and nine channel
  bits; `Physics` := `PHYS_RigidBody`; three random draws for the spin; the ragdoll-collision timer.
- **What links 6 and 7 read afterwards:** `ValidTouch` reads `Physics` only together with `Instigator` and a positive
  `Velocity.Z`; the harness has none, and in the game the mode is rigid body, not falling. The look and key steps read
  `Location`, `bPickupable` and the controller's choice. The native writes none of them.
- **Proposed port answer (UNVERIFIED, a hub ruling):** the pickup at rest at its spawn location with no simulation.
  Every field write is done, plus the existing `SetPhysics` byte write and three draws on the port's game random
  stream. The physics-body work is the empty physics layer, the same shape as the empty scene layer ruled for
  `AttachComponent`. Dropping the pickup to the floor is not available, because the headless pickup world has no
  collision.
- **Typed refusals kept,** all raised in a preflight before any write: the physics-asset branch (layer absent), the
  branch with no physics asset (not read), and part 1's deferred-reattach owner update. No physics crate is needed
  for the slice.
- **Next stop (local probe, never pushed):** a merge of origin/main with #520, with a stand-in that answers this native
  with no write. Links 6 and 7 then stop at `make` on #520's `WillowPickup.SpawnPickupParticles` (particle layer
  absent, `PickupParticles` set). The other links did not change.

Method: the script with `research/script_disasm.py`; the layouts with `tools/ghidra/class_layout.py`; the enum value
names from `Engine.upk`; the native with Ghidra 12.1.4 headless, read-only, under the machine lock. Raw output stays
under ignored `local/analysis/pickuprb/`. No bl-sdk session was granted and no recorded evidence covers this native,
so every rule is UNVERIFIED. The note lists three bl-sdk checks.

Open: the set-up without a physics asset; the inventory entry that names the physics asset; the deferred reattach's
owner update; whether the matched entries do anything script-visible beyond their fields.
