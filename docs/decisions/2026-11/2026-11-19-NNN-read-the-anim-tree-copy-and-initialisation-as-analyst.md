## 2026-11-19: Read the animation tree's copy and initialisation as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-COPY-ANIM-TREE, analyst only, for slice links 1, 3, 4, 5, 13 and
14. The decision number is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_COPY_ANIM_TREE.md`, and one `NATIVE_INDEX.md` row. No
  code changes.
- **Sources:**
  - the installed packages through `ow-package` (Maya's tree template) and `tools/ghidra/class_layout.py`;
  - `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only, under the machine lock).

  Raw output stays under ignored `local/analysis/copytree/`. No bl-sdk session took place, so **every rule is
  UNVERIFIED** in the running game.
- **Clean room:** this lane's author wrote NATIVE_INITIALIZE_BODY_COMPOSITION.md (#529) and NATIVE_ANIM_NODE_LOOKUP.md
  (#543) as analyst. None of the three may be implemented by this author.
- **Findings:**
  - **The copy** has a pooled path, matched by template and by equal node, control and morph counts, and a fresh
    path. The fresh path duplicates each node in depth-first gather order with the component as outer and the template
    node as archetype. Children, ParentNodes, NextControl, the SkelControlLists heads and RootMorphNodes are rewired
    through old-to-new maps. The copy names the original template.
  - **The tick list:** the root first, then a depth-first walk in Children order. A node is appended once all its
    ParentNodes are in. On Maya's template, this rule reproduces the saved 66-entry tick list exactly, which settles
    #543's open question 1 by reading. The copy keeps classes and names through the archetype, which settles #543's
    open question 3.
  - **Not read:** the per-class link fix-ups beyond the base classes, the per-node initialisation, the morph and
    per-bone steps, and the owner event's name (by fit `PostInitAnimTree`).
  - **The proposed Off answer:** a write-free, load-free preflight, then the copy and tick list written in the heap.
    The unread steps refuse typed: `NotRead`, or `LayerAbsent` for the mesh-data reader. Maya's next stop is the
    unread link fix-up of her WillowGame and GearboxFramework node classes, then the per-node initialisation. Both
    are questions for the next analyst step or a hub ruling. No probe was run.
