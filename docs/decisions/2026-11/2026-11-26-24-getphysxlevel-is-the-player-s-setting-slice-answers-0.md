## 2026-11-26: GetPhysXLevel is the player's setting; slice answers 0

AI-assisted (Claude Code)

Lane ANALYST-GETPHYSXLEVEL-AND-AFTER, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** NATIVE_LINK1_RIGID_BODY_LAYER.md (#763) predicts maya:8's next stop after `InitializeRBPhysics` is the
  unbound `Engine.GetPhysXLevel`, which gates `SetupSkeletalMeshPhysicsForPhysXEffects`.
- **What:**
  - the new note `docs/verification/NATIVE_GETPHYSXLEVEL_AND_AFTER.md`;
  - three NATIVE_INDEX rows and a GATES row.
- **Evidence:**
  - a local read-only Ghidra reading under the machine lock: the exec, the field's readers and writers, the engine
    start-up copy, the PhysX start-up clamp, the system-settings tables, and the `SetHasPhysicsAssetInstance` and
    `DoInitRBPhys` execs;
  - the layout tool;
  - `research/script_disasm.py`;
  - the install's shipped engine configuration (one value read).

  Raw output stays under ignored `local/analysis/physx/`.
- **Findings** (UNVERIFIED in game):
  - the native returns the engine's transient `PhysXLevel`, copied at start-up from the system settings'
    `PhysXLevel`;
  - the PhysX start-up clamps that setting to 0 to 2. It is the player's choice, with no hardware query; the shipped
    default is 0;
  - the PhysX-effects set-up tests "greater than 0" only. At 0 it writes nothing. Above 0 it needs:
    - the physics-asset instance (`SetHasPhysicsAssetInstance`);
    - channel setters;
    - for a player's pawn, two foot force fields attached at the foot bones.
- **Proposed Off answer:**
  - the native reads the engine object's field (no engine object: `LayerAbsent`);
  - the slice worlds' set-up writes it from a harness setting, default 0, limited to 0 to 2 and logged as a harness
    setting, UNVERIFIED.
- **Next** (a prediction, not a run): at 0, `InitializeBehaviorProviders`' pass-2 `LayerAbsent` for
  `PlayerEvents_Siren`.
- **Superseded row:** #763's NATIVE_INDEX row for `Engine.GetPhysXLevel` ("body not read") is replaced by this note's
  row. Whichever lands second drops it.
- **Clean room:** a different lane implements this note.
