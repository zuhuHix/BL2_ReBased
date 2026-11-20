## 2026-11-19: The bone-transform layer is one pose update, filled only on an attached component

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-BONE-TRANSFORM-LAYER, analyst only (clean room: a different lane
implements it). The decision number is pending from the hub (three digits); NNN is a placeholder.

- **What was decided:** `docs/verification/NATIVE_BONE_TRANSFORM_LAYER.md` reads the layer that #636, #639 and #635
  end at:
  - `SpaceBases` is filled by one pose update: sized to the mesh's bones (identity entries when the size changes);
    local atoms from the reference pose (no tree, or `bForceRefpose`) or from the tree; then composed root first;
  - it runs at the component's attach, at a mesh change's re-attach, and from `ForceSkelUpdate` on an attached
    component. `UpdateAnimations` only ticks the tree's nodes;
  - an unattached component's buffer is never filled.
- **What the slice meets:** the port never sets a made component's `bAttached`, so no slice link reaches the layer
  today. The probe shows the socket attaches' parent unattached, links 6 and 7 stopping before the skeletal entry, and
  link 8's mesh change on an unattached component. The nearest consumers are the unbound `UpdateAnimations` (Maya's
  `UpdateAnimSets`) and `ForceSkelUpdate` (`PlayWeaponEquip`).
- **Proposed Off answer:**
  - #569's layer gains each bone's bind pose (from the `.psk` reader, UNVERIFIED as the reference skeleton) and a
    read-only composed reference pose;
  - one gate for consumers: no mesh or `bSkipUpdateSkelPose` keeps the buffer; an unloaded mesh is `NotRead`; a
    parent animation component and a tree (without `bForceRefpose`) are `LayerAbsent`; a discarded root motion is
    `NotPorted`; otherwise the reference pose;
  - `ForceSkelUpdate`: nothing when unattached, `NotRead` when attached;
  - `UpdateAnimations`: nothing with no tree, `NotPorted` with one;
  - Reference unchanged; a test list of nine.
- **Sources:**
  - the records (#636, #639, #635, #569);
  - the script (the callers and the bone readers);
  - the packages (the component's layout, `BoneAtom`);
  - `Borderlands2.exe` through `tools/ghidra/run.ps1 -ReadOnly` under the machine lock. Raw output stays under ignored
    `local/analysis/btl/`.

  No addresses, decompiler output or listing structure is in tracked files. No bl-sdk session was granted.
- **Verified:** nothing in the running game. Every rule is **UNVERIFIED**.
- **Probe:** main 241a6c47, 4 ok / 9 NotYet / 1 error, with and without local stand-ins (never pushed).
- **Index:** a NATIVE_INDEX row and a GATES row were added.
