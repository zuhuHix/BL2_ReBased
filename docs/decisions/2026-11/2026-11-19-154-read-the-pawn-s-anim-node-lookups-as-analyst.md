## 2026-11-19: Read the pawn's animation-node lookups as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-ANIM-NODE-LOOKUP, analyst only, for slice links 1, 3, 4, 5, 13 and 14.
The decision number is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_ANIM_NODE_LOOKUP.md`, and two `NATIVE_INDEX.md` rows. No
  code changes.
- **Sources:**
  - the script (`research/script_disasm.py`);
  - the installed packages through `ow-package`, for Maya's tree template and the node classes' defaults;
  - `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only, under the machine lock).

  Raw output stays under ignored `local/analysis/animlookup/`. The note has no addresses, offsets, listing structure
  or decompiler names. No bl-sdk session took place, so **every rule is UNVERIFIED** in the running game.
- **Clean room:** this lane's author wrote NATIVE_INITIALIZE_BODY_COMPOSITION.md (#529) as analyst. Neither note may
  be implemented by this author.
- **Findings:**
  - **The lookups:** a node lookup returns the first node in the component's tick list with that name, and a None
    name is not special. A control lookup walks `SkelControlLists` and each `NextControl` chain, and a None name
    answers None at once. With no tree, both answer None. `SlotNodes` is rebuilt from the tick list's `AnimNodeSlot`
    nodes, in order.
  - **In the game, the lookups search the instanced tree:** the mesh's attach copies the template before the body
    composition's hook runs. The port has no instanced tree.
  - **Maya** (`AnimTree_Player`, 66 nodes and 5 controls): `AimOffset`, `LeftHandGrip` and `WillowFootIK` exist. Each
    is named by its class's default, not by the template's own tags. `ArmBoneBlend` does not exist, and `HeadLookAt`
    is None (her body class names none). She has 3 slot nodes. The template's saved tick indices form a full order,
    and the runtime order is taken to equal it by fit.
  - **The proposed Off answer** replaces #529 section 7's preflight bullet for a made `Mesh`, and runs after its
    old-Mesh release check. It reads the attachment's template component and its `AnimTreeTemplate`, and **loads
    nothing** (hub ruling, review r1). An unloaded handle refuses `LayerAbsent` with #529's text. A loaded template
    settles the fields that find nothing (None) and `bUsingFootIK`, by the template's names (by fit, the copy keeps
    classes and names). Anything that needs an instanced node, including any slot node, refuses with #529's
    `LayerAbsent` text before writing. That is Maya's stop after the release check. The next reading is
    `CopyAnimTree` and the rest of the tree initialisation.
