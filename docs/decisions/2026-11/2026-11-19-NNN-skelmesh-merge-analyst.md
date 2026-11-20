## 2026-11-19: The skeletal mesh merge behind BuildCompositeMesh, read as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SKELMESH-MERGE. Docs only. Analyst lane: another lane implements
what it proposes (clean room).

**The read.** I read `BuildCompositeMesh`'s merge branch and the merge routine it calls locally, with the project's
Ghidra tooling (read-only queries, under the machine lock). The script was swept with `research/script_disasm.py`'s
decoder for the callers. Raw output stays under ignored `local/analysis/merge/` and `local/analysis/merge2/`. The
own-words note is `docs/verification/NATIVE_SKELMESH_MERGE.md`.

**Findings:**
- **Link 8 does not reach the merge.** The slice gun's type has a `GestaltMesh`, so `BuildCompositeMesh` takes the
  gestalt path. On main dba540a the installed Off gate stops link 8 in `init` at `SetSkeletalMesh`'s mesh change,
  after `BuildCompositeMesh`. No recorded run shows a stop at "a skeletal mesh merge".
- **What the merge is:**
  - it builds a new transient `SkeletalMesh` on every call, with the stock engine's mesh merge. The LOD count is the
    sources' smallest, the skeletons are combined, and per-LOD sections, vertices and render resources are built;
  - it answers None when the merge fails, after destroying the new object;
  - `WillowInventory.StaticCreateCompositeMesh` calls the same merge with no zero or one shortcut. No script in the
    nine code packages calls it.
- **It is a mesh-building layer on top of a native mesh decode** that the project does not have (rule 4). Its draw
  half belongs to the host.

**The proposal** (note sections 4 and 5, all UNVERIFIED):
- Link 8 needs no layer.
- Keep `LayerAbsent("a skeletal mesh merge")` for two or more meshes, before any write and without loading the
  source meshes.
- If `StaticCreateCompositeMesh` is ever bound, it refuses `LayerAbsent` at any length.
- A merged-skeleton stand-in in the skeletal mesh data layer is described as the narrowest honest option, but I don't
  recommend building it until a link needs it.

**What changes:** the note, a NATIVE_INDEX row, and a GATES row recording the gate run. No code.

**Not verified:**
- anything in the running game;
- the merge's bone-match key, its section grouping and its socket handling;
- the third native caller's identity;
- how many weapon types and items take the merge branch.
