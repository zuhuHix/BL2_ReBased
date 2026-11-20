## 2026-11-19: read the tree initialisation's component pass and per-bone control arrays as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-COPY-COMPONENT-PASS, analyst only (clean room: a different lane
implements the note). The decision number is pending from the hub (three digits); NNN is a placeholder.

**What:** `docs/verification/NATIVE_COPY_COMPONENT_PASS.md` (new) and one row in `docs/verification/NATIVE_INDEX.md`.
NATIVE_COPY_ANIM_TREE.md (#551 at 4a363c0c) section 2 step 5 left these steps unread, and NATIVE_SET_SKELETAL_MESH.md
(#583 at 231bbaed) does not read them either. The lane was first briefed to implement the step, then switched to
analyst when no note gave the rule (hub, 2026-11-20).

**Sources:**
- the script (the native `InitSkelControls` only);
- the installed packages (`--def-dump` of Maya's tree and controls, `--names` over her 107 packages, the class layout);
- Maya's reference skeleton from a local UModel export (bone names and indices only);
- `Borderlands2.exe`, read locally with `tools/ghidra/run.ps1` under the machine lock.

Raw output stays under ignored `local/analysis/ccp/`. No real-game session was run.

**Findings (all UNVERIFIED in the running game):**
- the step is three calls: the component pass, the morph nodes' initialisation (tree only), and the control-array
  builder. The owner's event and the initialised flag follow;
- the component pass rebuilds the component's native morph-target map from `MorphSets`, last set first, keeping the
  first name seen. Maya sets no `MorphSets`;
- the builder empties `SkelControlIndex`, `PostPhysSkelControlIndex` and `SkelControlTickArray`. With a mesh, a tree
  and lists, it then:
  - gives each bone 255;
  - raises `TickTag`;
  - gives each bone the index of the first list naming it;
  - lists each control of that list's chain once, tagged;
  - stops a chain at its first post-physics control, which also fills `PostPhysSkelControlIndex`;
- found on the way: the initialisation's gate field fits `ParentAnimComponent`; `InitTag` is set from the root's
  `NodeInitTag`; and two per-node arrays are sized before the per-node loop. These belong to #551's steps 2 to 4.

**Checked against the packages:** Maya's 4 lists name bones that her 27-bone skeleton holds once each, and none of
her 5 controls is post-physics. The rule gives indices 0 to 3 at bones 18, 4, 8 and 24, and a 5-control tick list.
This is a reading of the data, not a saved oracle.

**Proposed Off answer:**
- a write-free preflight, with #569's layer refusals unchanged;
- a NotPorted refusal for a non-empty `MorphSets`;
- NotRead refusals for more than 255 lists and for a looping chain;
- `VmError::Script` for a missing or mistyped field;
- the writes in the note's order, and 13 tests on invented skeletons.

Maya's next stop is then #551 rule 4, the owner's event.

**Clean room:** this thread implemented unrelated lanes earlier (#573, #584). It read no native code for them, and it
does not implement this note.
