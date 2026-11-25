## 2026-11-25: Hook 1 is the simple-anim cache

AI-assisted (Claude Code)

Lane ANALYST-HOOK1-INTERFACE, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** the body composition's hook 1 was read as a gear-likeness step on the made object, with the interface name
  `IGearLikenessConsumer` taken by fit. Lane F's interface check (#716's hand-back) found its lookup registered under
  `ISimpleAnimPlayer`.
- **What:** section 8 of `docs/verification/NATIVE_BODYCOMP_RENDER_HOOKS.md`, with superseded markers in sections 1,
  2, 3, 4 and the open questions, plus the NATIVE_INDEX row's confidence and one GATES row.
- **Evidence:**
  - a local read-only Ghidra reading under the machine lock;
  - the interface tables and the tree's subobjects in the packages (`ow-package --exports`);
  - `ow-package --def-dump` of Maya's pawn and templates.

  Raw output stays under ignored `local/analysis/hook1/`.
- **Findings** (UNVERIFIED in game):
  - hook 1 is `CacheSimpleAnims(owner as ISimpleAnimPlayer, the made skeletal component)`, the owner by fit;
  - the cache registers the mesh's `WillowAnimNode_Simple` nodes in the owner's `SimpleAnimInfo`, and marks one
    dominant node per entry;
  - no component class implements either interface;
  - Maya's trees have no such node and her list is empty, so hook 1 writes nothing for her.
- **Proposed Off answer:**
  - a non-skeletal object, or a non-implementing owner: nothing;
  - an implementer with no node and an empty list: nothing;
  - otherwise `NotRead`;
  - the `GEAR_*` refusals go. Tests listed.
- **Clean room:** a different lane implements this note.
