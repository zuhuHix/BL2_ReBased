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
  - the pawn is the authority, so the build runs fully and raises `bNetDirty` and `bForceNetUpdate`.
- **Proposed Off answer:**
  - nothing for the empty cases;
  - for a non-empty list, `NotYet { LayerAbsent }` before any write but the lazy body-class load, since the port has no
    component or mesh layer;
  - the parts that could be split out later are named for a hub ruling.
- Every rule is UNVERIFIED in the running game; the note lists four open questions and one bl-sdk check.
