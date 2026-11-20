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

**Review r1 of #612** (FIX; the script account confirmed, the proposed Off run tightened, note section 3):
1. the event is called with `vm.call(f, Some(owner), ...)` and the preflight's own `f`, as `notify_owner` does, not
   through the world-event path, whose frame set-up writes and can load; an owner in a state is refused until a
   read-only state-chain lookup is planned;
2. the three casts' classes are resolved read-only and each cast's answer computed in the preflight, so no `NotYet`
   comes after the writes;
3. the Off binding of `FindAnimNode` / `FindSkelControl` is scoped to #543's tick-list path, with "own tree" defined,
   the gather path `NotRead`, whole-name matching, and a strict `--slice-gate` re-run asked of the implementer;
4. the call's entry checks are planned with `Vm::check_call`;
5. the port evidence for "`Pawn.Mesh` is not the made component" is cited, and the write path must call `instance`
   after the attach writes `Owner`;
6. a prerequisites list in landing order (#570 for `find_loaded_method`, #608, the probe's other branches), and the new
   owner parameter of `plan` and `attach_check`;
7. five more tests (a state, a state function, an unloaded `BodyClass`, each native unbound, the partial-writes mark,
   the binding's gather path), one `NotPorted` text per native, and this note named as every new `NotYet`'s record.
