## 2026-11-25: The Parent link's identity-map flag is bIsGestaltAccessoryMesh

AI-assisted (Claude Code)

Lane ANALYST-CUSTOMHEAD-PARENT-FLAG, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** link 1's predicted next stop (#715) is Maya's attachment 1, the `Parent` link. It is `NotRead` under hub
  ruling 3 (the hub's rulings on #529), and its reading lacked one flag of the new component.
- **What:** section 8 of `docs/verification/NATIVE_INITIALIZE_BODY_COMPOSITION.md`, with markers in section 4.1 step
  7, section 7 and the open questions, plus one NATIVE_INDEX row and one GATES row.
- **Evidence:**
  - a local read-only Ghidra reading under the machine lock;
  - the layout tool;
  - `research/script_disasm.py`'s decoder over every script function;
  - `ow-package --def-dump` of Maya's head and body templates.

  Raw output stays under ignored `local/analysis/parentflag/` and `local/analysis/link1next/`.
- **Findings** (UNVERIFIED in game):
  - the flag is `SkeletalMeshComponent.bIsGestaltAccessoryMesh`, by fit, and only the component's own value sets it;
  - with the same mesh or the flag set, the map is the identity. Otherwise each bone maps to the parent mesh's
    same-name bone, or -1;
  - Maya's head template has the flag false and the mesh `CubeForCode`, against her body's `Skel_SirenBody`, so her
    map is the by-name one.
- **Proposed Off answer** (for the hub's revisit of ruling 3):
  - the link is planned from the two reference skeletons, with no load;
  - an unreadable skeleton refuses with its reader's `NotYet`;
  - the writes are `ParentAnimComponent`, then `ParentBoneMap`;
  - tests listed.
- **Clean room:** a different lane implements this note.
