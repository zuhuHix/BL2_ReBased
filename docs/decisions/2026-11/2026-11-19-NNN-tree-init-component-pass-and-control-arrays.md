## 2026-11-19: The tree initialisation's component pass and per-bone control arrays in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-COPY-COMPONENT-PASS, implementer. The decision number is pending from
the hub (three digits); NNN is a placeholder.

- **Source:** `docs/verification/NATIVE_COPY_COMPONENT_PASS.md` on `research/rust-copy-component-pass` at 5196fcec
  (#598, accepted after its r1; the analyst lane ANALYST-COPY-COMPONENT-PASS), section 6. This lane did not write it,
  and no native code or analysis output was opened (clean room). Its texts, kinds and check order live in one file,
  `crates/ow-world/src/world/control_arrays.rs`.
- **Base:** #585's accepted head, `fix/rust-per-bone-weights` at ce3be0ac, not origin/main. The code this step
  extends (#561's copy, #575's per-node initialisation, #585's builders and #569's skeletal layer) is not on main yet.
- **No real-game session:** every rule is **UNVERIFIED** in the running game. The note's bl-sdk checks settle it.
- **Unchanged:** Reference (the copy is an Off path; nothing new is registered). No dependency was added and no bounds
  check was loosened.

**What changed:**
- `crates/ow-world/src/world/control_arrays.rs` (new):
  - `plan` is the preflight. The copies exist only in #561's write phase, so it reads the **template** tree and
    controls, which the copy maps one to one; a copy keeps its template's slots. It writes and loads nothing. In order:
    1. **the init tag and the per-node arrays:**
       - `InitTag` must be declared an int, `AnimTickRelevancyArray` an array of ints and `AnimTickWeightsArray` an
         array of floats, and the root must have a `NodeInitTag` int; else `VmError::Script`;
    2. **the component pass:**
       - `MorphSets` must be declared a dynamic array of object references, else `VmError::Script`;
       - any set entry is `NotYet { NotPorted }`, "the tree initialisation's component pass: the morph-target map of
         MorphSets". An empty list, or one of None entries, writes nothing;
    3. **the control arrays:**
       - `SkelControlIndex`, `PostPhysSkelControlIndex` (bytes), `SkelControlTickArray` (objects), `TickTag` and
         `SkeletalMesh` must be declared with those kinds;
       - with no mesh or no lists the build is skipped;
       - otherwise the bone count comes from #569's `reference_skeleton`, through `per_bone_weights::skeleton` (now
         `pub(crate)`, so both builders share the layer and the tests' skeleton). Its refusals pass through unchanged;
       - more than 255 lists is `NotYet { NotRead }`;
       - a `BoneName` that is not a name, or a `ControlHead` that is not an object, is `VmError::Script`. Names are
         found with `ReferenceSkeleton::bone_index`, and None finds nothing;
       - each template control reached must declare `ControlTickTag`, `NextControl` and `bPostPhysicsController`
         with their kinds;
       - a reached template control whose saved `ControlTickTag` already equals the new control tag is
         `NotYet { NotRead }`, the note's collision text, as #561's node rule.
  - Residency and a looping chain are #561's gather's checks (#557's rule, a script error), before this runs; this
    lane adds no loop refusal.
  - **The walk** simulates the tags from the template controls' saved `ControlTickTag`s. The new tag is the
    component's `TickTag` + 2. A control not carrying the new tag gets it and is appended. A post-physics control is
    tagged and listed first, then sizes `PostPhysSkelControlIndex` (once), sets its bone and ends the chain, whether
    or not it was newly listed.
  - `write_init` runs after the tick list and before #575's per-node loop: `InitTag` = the root's `NodeInitTag` + 1;
    `AnimTickRelevancyArray` = n zeros and `AnimTickWeightsArray` = n entries of 0.0, n being the tick list's length,
    each emptied first. The 0.0 values are a port convention, UNVERIFIED.
  - `write_arrays` runs after the per-node loop: the three arrays emptied (always); then, when the build runs,
    `SkelControlIndex`, `TickTag` + 1, each newly tagged control's `ControlTickTag` on its copy and the tick array,
    and `PostPhysSkelControlIndex` when a post-physics control was met.
- `crates/ow-world/src/world/anim_tree_copy.rs`:
  - `plan` calls `control_arrays::plan` after the per-node initialisation and keeps the result in `Plan::arrays`;
  - the copy adds the controls' copies to its map;
  - `initialise` calls `write_init` before the per-node steps and `write_arrays` after them;
  - #575's two refusals, `COMPONENT_PASS` and `BONE_ARRAYS`, are removed. The owner's event (#551 rule 4) is the one
    step left `NotYet { NotRead }`, so the test-only switch answers one step.
- **Every refusal is raised in the preflight,** before the copy's first write, so none takes the "stopped after partial
  writes: " prefix.

**Tests** (invented skeletons and trees, `anim_tree_copy_tests.rs`, 14 new). The note's cases; each refusal goes
through `refused_clean`, which checks heap writes, objects, draws and load transitions unchanged:
1. no mesh, or no lists: the three arrays are emptied of stale entries and `TickTag` is the tick list's only;
2. to 4. list indices, the first list on a bone wins, and None or unknown names are skipped; `TickTag` + 1; walk order
   with the new tag;
5. and 6. a control shared by two chains is listed once and walked past in the later chain;
7. post-physics in mid-chain: tagged, listed and stopping; a second list keeps the first entry; and a shared
   post-physics control is not listed again but still sets the later bone and stops that chain;
8. 256 lists: `NotRead`;
9. a looping chain: the copy's `VmError::Script`;
10. a set `MorphSets`: `NotPorted`, while None entries pass;
11. #569's own error, passed through (a mesh that is not a `SkeletalMesh`; a cold mesh is the layer's own test);
12. Maya-shaped: section 5's arrays;
13. Reference binds nothing new (nothing is registered);
14. the control tag collision: `NotRead`;
15. `InitTag` and the two per-node arrays at the tick list's length, stale entries removed.

One more test checks that a non-name `BoneName` is `Script`. The copy's fixture classes gained the new component,
node and control fields, and the two tests that expected the old refusals now expect the owner's event.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on a throwaway local merge with #592
(5ab85711), #570 (d910fa13), #500 (c1ed82a9) and #528 (0e8e2312, the head the earlier probes used; its newer head
conflicts widely with this base), deleted after:
- ok 1, NotYet 10, error 3 of 14;
- links 1, 3, 4, 5, 13 and 14 run 0 of their driver's 1 step (`maya:8`). The first stop moved from "the animation
  tree initialisation's pass over the component" to "the owner's event after the tree initialisation" (`NotRead`,
  NATIVE_COPY_ANIM_TREE.md rule 4). The note's section 7 predicts that stop.
