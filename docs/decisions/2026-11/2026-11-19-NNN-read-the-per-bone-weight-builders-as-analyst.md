## 2026-11-19: read the per-bone weight builders as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-PER-BONE-WEIGHT-BUILDERS, analyst only (clean room: a different lane
implements the note). The decision number is pending from the hub (three digits); NNN is a placeholder.

**What:** `docs/verification/NATIVE_PER_BONE_WEIGHTS.md` (new) and one row in `docs/verification/NATIVE_INDEX.md`. The
note reads the two weight builders that #566's note (NATIVE_ANIM_NODE_INIT.md at 07b2dd68, implementation F) left
unread, and proposes their Off answer against #569's `reference_skeleton`.

**Sources:**
- the script (neither class has a script builder);
- the installed packages (`--def-dump` of Maya's tree nodes, `class_layout.py`);
- Maya's reference skeleton from a local UModel export (counts and bone names only);
- `Borderlands2.exe`, read locally with `tools/ghidra/run.ps1` under the machine lock.

Raw output stays under ignored `local/analysis/perbone/`. No real-game session was run.

**Findings (all UNVERIFIED in the running game):**
- both classes' shared `InitAnim` override calls the builder after `AnimNodeBlend`'s initialisation;
- the builder runs only with a component and a mesh. Weights are 1.0 at the branch bones (per-bone node) or the
  clamped entry weight (feather blend), are inherited down the skeleton, and are 0 at an unlisted root;
- the byte list holds the bones whose weight changes from their parent's. The feather blend also lists partially
  weighted bones, and lists nothing when `bForceLocalSpaceBlend` is set. A parents pass then inserts every missing
  ancestor ahead of its bone.

**Checked against the packages:** Maya's per-bone node saved 27 weights and a 3-bone list, and the rule reproduces
both exactly. That supports the read (it is an editor build, not a game run). Her five feather blends saved empty
arrays, so their builder has no package oracle.

**Proposed Off answer:**
- a write-free preflight, using #569's layer refusals unchanged;
- one new refusal, for a mesh of more than 256 bones;
- `VmError::Script` for a missing or mistyped field;
- two writes per built node.

It replaces #575's "the per-bone weight build of <class>". Maya's next stop is then #551 section 2 step 5 (the
component pass and the per-bone control arrays).

**Clean room:** this thread implemented unrelated lanes earlier (#570 and others). It read no native code for them,
and it does not implement this note.
