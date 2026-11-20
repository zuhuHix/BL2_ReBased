## 2026-11-19: The pawn's PostInitBodyComposition is mapped; its old-component release is the pawn's own detach

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-POSTINITBODYCOMP-SLOTNODE, analyst only (clean room: a different
lane implements it). The decision number is pending from the hub (three digits); NNN is a placeholder.

- **What was decided:** `docs/verification/NATIVE_POSTINIT_BODYCOMP_SLOTNODE.md`:
  - maps each step of the pawn's `PostInitBodyComposition` (#529 section 4.5, open question 7) to the note that reads it
    and the port's state. The lookups and `SlotNodes` are #543's; the mesh flag and `ReattachWeapons` are
    NATIVE_MESH_REST.md's; the rendering hooks are #622's;
  - reads the one step still unread. Releasing the old `Mesh` or `CylinderComponent` is the pawn's own component
    detach: every copy is removed from the pawn's `Components` (correcting #529's "a global list"); when one was
    there, a render-side notice is sent, then the component's detach clears its scene pointer and `Owner`;
  - says what links 1, 3, 4, 5, 13 and 14 need. Nothing new from the hook on the port's path. The stop after the hooks
    is the slot nodes' own `AnimNodeSpecialMoveBlend` owner test (NATIVE_ANIM_NODE_INIT.md, read in full). It refuses
    because the port's interface test answers only from already-resolved entries: an implementer or hub question, not
    an unread native.
- **Proposed Off answer:** replace the two "releasing the old <field>" refusals. An old component not in `Components`
  is nothing. One in it, unattached, is removed (every copy) and its `Owner` cleared. An attached or unresident one is
  refused `NotRead` before any write. Reference is unchanged; a test list of nine.
- **Sources:**
  - the records;
  - the port on main dba540a3;
  - `Borderlands2.exe` through `tools/ghidra/run.ps1 -ReadOnly` and a call-site listing under the machine lock. Raw
    output stays under ignored `local/analysis/pib/`.

  No addresses, decompiler output or listing structure is in tracked files. No bl-sdk session was granted.
- **Verified:** nothing in the running game. Every rule is **UNVERIFIED**.
- **Probe:** not run (docs only). The stop cited is the recorded probe of NATIVE_BONE_TRANSFORM_LAYER.md section 5.
- **Index:** a NATIVE_INDEX row and a GATES row were added.
