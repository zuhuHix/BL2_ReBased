## 2026-11-26: InitializeRBPhysics makes no body for Maya

AI-assisted (Claude Code)

Lane ANALYST-LINK1-RIGID-BODY-LAYER, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** with #760, links 1, 3, 4, 5, 13 and 14 stop at 2.A's physics `LayerAbsent` in `InitializeRBPhysics`. The
  hub asked whether #574's at-rest ruling can extend here.
- **What:**
  - the new note `docs/verification/NATIVE_LINK1_RIGID_BODY_LAYER.md`;
  - two NATIVE_INDEX rows and a GATES row;
  - a dated pointer under NATIVE_LINK1_RBPHYSICS_AND_AFTER.md 2.A.
- **Evidence:**
  - a local read-only Ghidra reading under the machine lock: the component set-up in the class tables of nine classes,
    the shape virtual, and the primitive and skeletal bodies;
  - the layout tool, whose fields land on declared properties under the usual component shifts;
  - `research/script_disasm.py` and `ow-package --def-dump`.

  Raw output stays under ignored `local/analysis/rblayer/`.
- **Findings** (UNVERIFIED in game):
  - the actor components do nothing, and the cylinder has no shape;
  - the skeletal set-up writes `bSkelCompFixed` and empties the cloth index array. It makes a body only with
    single-body physics, the physics-asset instance flag or soft body, none of which Maya's meshes have;
  - so Maya gets no body.
- **Proposed Off answer:**
  - a class-table rule replacing 2.A's attached-entry `LayerAbsent`, with no physics layer: the game's own answer, not
    an at-rest stand-in;
  - typed refusals for the body-making cases and unread classes, all checked before the first write;
  - (b): the smallest record, if ever needed, makes the `BodyInstance` or `PhysicsAssetInstance` objects without
    simulation.
- **Next** (a prediction, not a run): `SetupSkeletalMeshPhysicsForPhysXEffects`' unbound `Engine.GetPhysXLevel`.
- **Review r1 (#763):**
  - `SetPushesRigidBodies`' refusal is folded into the preflight, so every refusal comes before the first write;
  - 2.A's refusal L for an unrecorded live entry, and the template receiver's physics `LayerAbsent`, stay; no write
    on a template's components;
  - the cloth bool's declaration is listed (fitted); the component count is corrected; three tests added.
- **Clean room:** a different lane implements this note.
