## 2026-11-19: The tree initialisation's owner event is PostInitAnimTree, run as script on the owner

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-TREE-INIT-OWNER-EVENT, analyst only (clean room: a different lane
implements it). The decision number is pending from the hub (three digits); NNN is a placeholder.

- **What was decided:** `docs/verification/NATIVE_TREE_INIT_OWNER_EVENT.md` records:
  - **the event.** The event #551 and #598 had by fit is `PostInitAnimTree`. The executable's start-up name table and
    the one engine routine that sends that name, the tree initialisation, settle it. It is sent to the component's
    `Owner`, with the component as its argument (by the event's signature), after step 5 and before
    `bAnimTreeInitialised`;
  - **the three script versions.** `Actor`'s does nothing. `Pawn`'s calls `ClearAnimNodes` and the native
    `CacheAnimNodes`, only on its own `Mesh`. `WillowPlayerPawn`'s fills `ArmBoneBlend`, `AimOffset` and
    `LeftHandIK` while they are None, through `FindAnimNode` and `FindSkelControl`;
  - **Maya's path.** It takes `WillowPlayerPawn`'s three lookups, with `Pawn`'s branch skipped because her `Mesh` is
    set only later (by fit);
  - **a proposed Off answer:**
    - run the event through the VM;
    - a write-free preflight that refuses any unread override, an owner class whose code is not loaded, an unloaded
      owner, and `Pawn`'s own-`Mesh` branch (native `CacheAnimNodes` not read);
    - this lane binds `FindAnimNode` and `FindSkelControl` in Off from NATIVE_ANIM_NODE_LOOKUP.md;
    - the partial-writes mark applies after the copy's writes;
    - Reference is unchanged.
- **Sources:** the records; the script (`research/script_disasm.py`); `Borderlands2.exe` through
  `tools/ghidra/run.ps1 -ReadOnly` under the machine lock, used only for the name and the sending routine. Raw output
  stays under ignored `local/analysis/toie/`. No addresses, decompiler output or listing structure is in tracked
  files. No bl-sdk session was granted.
- **Verified:** nothing in the running game. Every rule is **UNVERIFIED**.
- **Probe:** with a local stand-in for the event, the six links (1, 3, 4, 5, 13, 14) stop next at the body
  composition's lookup refusal ("needs an animation-tree instance ..."), now unblocked by the copy.
- **Index:** a NATIVE_INDEX row was added.
