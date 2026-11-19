## 2026-11-19: WillowPawn.InitializePawnMeshes read as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-INIT-PAWN-MESHES, analyst only (clean room: this thread does not
implement it). The decision number is pending; NNN is a placeholder.

- **What:** `docs/verification/NATIVE_INIT_PAWN_MESHES.md` and one NATIVE_INDEX row.
- **How:**
  - the installed script, with `research/script_disasm.py`;
  - read-only Ghidra queries under the machine lock;
  - fields named with the project's layout tool;
  - Maya's body class read from the installed packages through the definition store (a local probe, not committed).

  Raw output stays under ignored `local/analysis/pawnmesh/`. No bl-sdk evidence is on record for it, and no real-game
  session was run.
- **Found:**
  - both keys (`WillowPawn` and `WillowPlayerPawn`) run one rule;
  - it calls `InstanceDataHelper.InitializeBodyComposition` on the pawn's `BodyClass.BodyComposition`, then sets two
    root-motion fields on `Mesh` under two body-class flags;
  - it does nothing when the helper class, the body class or the attachment list is missing.
- **The slice's Maya:**
  - `BodyClass_Siren` has 18 attachments (13 components, 3 switches, a custom head and a custom skin);
  - both root-motion flags are false;
  - the pawn is the authority, so in game the build runs fully and raises `bNetDirty` and `bForceNetUpdate`;
  - at link 1's stop the body class is not loaded yet.
- **Hub rulings (2026-11-19):**
  1. Nothing is written before the component layer's `LayerAbsent`. The switch entries, the replicated switch fields
     and the network flags are not written without that layer, so the build is not split.
  2. In Off an unloaded `BodyClass` is not loaded: preflight refuses with `NotYet { NotRead }` naming its attachment
     list, before any load or write.
- **Proposed Off answer:**
  - a write-free preflight: the receiver, `Engine.InstanceDataHelper` in the class table (no loading lookup), the body
    class (its unloaded case refused), then the attachment list;
  - nothing for the empty cases;
  - for a non-empty list, `NotYet { LayerAbsent }` before any write.

  So Off writes nothing, and the slice's answer today is the unloaded body class's `NotRead`.
- **Review r1 of #517, folded in.**
  - the lazy-load exception and the partial-write instructions are removed;
  - the helper predicate, its package and the invalid-receiver errors are given;
  - the layout displacement is cut and the Maya listing reduced to counts (13 components, reconciled);
  - the per-attachment write order is stated, with the no-parent and zero-width cases carried into it as named
    `NotRead`s.
- Every rule is UNVERIFIED in the running game; the note lists three open questions and one hub ruling and one bl-sdk check.
