## 2026-11-19: Rust AttachMeshToPickup part 2 in Off: SetLightEnvironment, the rigid-body stop, SpawnPickupParticles

AI-assisted (Claude Code). Lane IMPL-ATTACH-MESH-TO-PICKUP part 2 (the file number is a placeholder until the
maintainer gives one). Coded from `docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md` section 6 alone (analyst lane
ANALYST-ATTACH-MESH-TO-PICKUP part 2, the note at f32d69b, PR #507), with the hub's ruling on its section 6.6. This
lane did not read the natives. No real-game session was run. No dependency was added and no bounds check was
loosened. **Every rule is UNVERIFIED in the running game.** Stacked on #511 (part 1).

- **What was added** (`crates/ow-inventory/src/attach_mesh.rs`, Off only; Reference binds nothing):
  - `PrimitiveComponent.SetLightEnvironment(object)` (6.2):
    - the argument equal to the current `LightEnvironment`: nothing;
    - a component not `bAttached`: the one write `LightEnvironment` (the native's own rule);
    - an attached one: part 1's deferred-reattach preflight before any write, then `PreviousLightEnvironment`,
      `LightEnvironment` and `bNeedsReattach` = true.
    - The fields are **UNVERIFIED (fitted)**. The argument is checked against `LightEnvironment`'s declared class
      before any write.
  - `WillowPickup.InitializePickupForRBPhysics()` (6.3): `bUseRBPhysics` false is nothing (the native's own rule);
    otherwise `NotYet { LayerAbsent }`, "WillowPickup.InitializePickupForRBPhysics: the pickup's rigid-body set-up",
    layer "a rigid-body physics layer (the port has none)", before any write.
  - `WillowPickup.SpawnPickupParticles()` (6.4): `PickupParticles` None is `NotYet { NotRead }`, "WillowPickup.
    SpawnPickupParticles with no particle system: the particles' removal"; otherwise `NotYet { LayerAbsent }`,
    "WillowPickup.SpawnPickupParticles: the pickup's particle effect", layer "a particle-effects layer (the port has
    none)". Both come before any write of the native's own. The slice does not reach it while the rigid-body stop
    stands (6.5).
- **The hub's ruling on 6.6.** The rigid-body set-up's script-visible writes do not separate from its physics work
  (the note's per-effect table), so no part of it is a port answer and it stays `LayerAbsent`. No physics layer is
  added.
- **Partial writes.** Each `NotYet` is returned unmarked; the `make` step, which wrote before, adds the prefix.
- **Tests** (`attach_mesh_tests.rs`, 3 new, 12 in all): `SetLightEnvironment`'s three cases plus the
  owner-branch refusal with nothing written; the rigid-body skip and `LayerAbsent`; the particles' `NotRead` and
  `LayerAbsent`; Reference binds none of the eight.
- **Installed slice gate (Off, strict):** links 6 and 7 move from the unbound `SetLightEnvironment` (an error) to
  "stopped after partial writes: WillowPickup.InitializePickupForRBPhysics: the pickup's rigid-body set-up" (a
  `NotYet { LayerAbsent }`) in `make`, as the note's 6.5 foresees. The other 12 rows are byte-identical. Numbers in
  `docs/verification/GATES.md`. This is what the host did, not parity.
- **Open:** the note's open questions 6 and 7: the rigid-body entries, the physics mode's argument, the channels and
  the timers; the particle effect's removal, offset, template and start.

## Edits: review r1 of #520

AI-assisted (Claude Code). Sol's review r1 of #520.
- **First wins for part 2's keys.** `SetLightEnvironment`, `InitializePickupForRBPhysics` and `SpawnPickupParticles`
  are each bound only when no earlier pack bound that key, so a prebound key keeps its handler and the others are
  still registered (the `has_native` pattern of `definition.rs` and `globals.rs`).
- **`SetLightEnvironment`'s checks on their own paths:** the equal-value no-op comes before `bAttached` is read; on the
  attached path the deferred reattach's refusals run in owner-then-scene order, and only then are the write targets
  (`PreviousLightEnvironment`, `bNeedsReattach`, `LightEnvironment`) checked against the heap's write rules, before
  the first write.
- **Tests:** a prebound key; the no-op with a broken `bAttached`; the owner refusal ahead of broken `Scene` and
  `PreviousLightEnvironment` declarations; the Reference test asserts all eight keys unbound (14 tests in all).
