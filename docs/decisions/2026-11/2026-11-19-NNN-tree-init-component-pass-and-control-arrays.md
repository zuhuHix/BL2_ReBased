## 2026-11-19: The tree initialisation's component pass and per-bone control arrays in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-COPY-COMPONENT-PASS, implementer. The decision number is pending from
the hub (three digits); NNN is a placeholder.

- **Source:** `docs/verification/NATIVE_COPY_COMPONENT_PASS.md` on `research/rust-copy-component-pass` at 6809f93a
  (#598, the analyst lane ANALYST-COPY-COMPONENT-PASS), section 6. This lane did not write it, and no native code or
  analysis output was opened (clean room). The note's r1 review was still open when this was coded; its texts, kinds
  and check order live in one file, `crates/ow-world/src/world/control_arrays.rs`, so a change lands in one place.
- **Base:** #585's accepted head, `fix/rust-per-bone-weights` at ce3be0ac, not origin/main. The code this step
  extends (#561's copy, #575's per-node initialisation, #585's builders and #569's skeletal layer) is not on main yet.
- **No real-game session:** every rule is **UNVERIFIED** in the running game. The note's bl-sdk checks settle it.
- **Unchanged:** Reference (the copy is an Off path; nothing new is registered). No dependency was added and no bounds
  check was loosened.

**What changed:**
- `crates/ow-world/src/world/control_arrays.rs` (new):
  - `plan` is the step's preflight, on the template tree, writing and loading nothing. In the note's order:
    1. **the component pass:** `MorphSets` must be declared a dynamic array of object references, else
       `VmError::Script`. Any set entry is `NotYet { NotPorted }`, "the tree initialisation's component pass: the
       morph-target map of MorphSets". An empty list, or one of None entries, writes nothing;
    2. **the control arrays:**
       - `SkelControlIndex`, `PostPhysSkelControlIndex` (arrays of bytes), `SkelControlTickArray` (objects),
         `TickTag` and `SkeletalMesh` must be declared with those kinds;
       - with no mesh or no lists the build is skipped;
       - otherwise the bone count comes from #569's `reference_skeleton`, through `per_bone_weights::skeleton` (now
         `pub(crate)`, so both builders share the layer and the tests' skeleton). Its refusals pass through unchanged;
       - more than 255 lists is `NotYet { NotRead }`;
       - a `BoneName` that is not a name, or a `ControlHead` that is not an object, is `VmError::Script`. Names are
         found with `ReferenceSkeleton::bone_index`, and None finds nothing;
       - each control reached must be loaded (#543's refusal) and declare `ControlTickTag`, `NextControl` and
         `bPostPhysicsController` with their kinds;
       - a chain that reaches a control twice is `NotYet { NotRead }`. The copy's own control gather (#557's rule)
         refuses such a chain earlier, as a script error, so on the copy's path this guard is a second one, tested on
         the plan alone.
  - The walk simulates the controls' tags. The copies take their templates' `ControlTickTag`, so the template's tags
    are the copies' tags before the build. The new tag is the tick list's tag plus one.
  - `write_arrays` performs the writes in the note's order: the three arrays emptied (always); then, when the build
    runs, `SkelControlIndex`, `TickTag` + 1, each newly tagged control's `ControlTickTag` on its copy and the tick
    array, and `PostPhysSkelControlIndex` when a post-physics control was met.
- `crates/ow-world/src/world/anim_tree_copy.rs`:
  - `plan` calls `control_arrays::plan` after the per-node initialisation, and keeps the result in `Plan::arrays`;
  - the copy adds the controls' copies to its map;
  - `initialise` writes the arrays after the per-node steps, before `bAnimTreeInitialised`;
  - the owner's event (#551 rule 4) is the one step left `NotYet { NotRead }`, so the test-only switch now answers
    one step. `COMPONENT_PASS` and `BONE_ARRAYS`, the old refusal texts, are kept as constants but no longer raised.
- **Every refusal is raised in the preflight,** before the copy's first write, so none takes the "stopped after partial
  writes: " prefix.
- **Not done here (the note's "found on the way"):** `InitTag`, `AnimTickRelevancyArray` and `AnimTickWeightsArray`
  belong to #551's steps 2 to 4. That is a separate lane.

**Tests** (invented skeletons and trees, `anim_tree_copy_tests.rs`, 11 new). The note's cases 1 to 12; each refusal
goes through `refused_clean`, which checks heap writes, objects, draws and load transitions unchanged:
1. no mesh, or no lists: the three arrays are emptied of stale entries and `TickTag` is the tick list's only;
2. lists on bones 1 and 3: their indices there, 255 elsewhere, `TickTag` + 1, the controls listed in walk order with
   the new tag;
3. a second list on bone 1: neither tagged nor listed;
4. a None name and an unknown name: skipped, and later lists keep their own index;
5. a shared control: listed once;
6. a pre-tagged control: walked but not listed;
7. post-physics: `PostPhysSkelControlIndex` is sized and set, the chain ends, and a second list keeps the first entry;
8. 256 lists: `NotRead`;
9. a loop: `NotRead`, on the plan, with the counters unchanged;
10. a set `MorphSets`: `NotPorted`, while None entries pass;
11. #569's own error, passed through;
12. Maya-shaped: 27 bones, lists on 18, 4, 8 and 24, and 5 controls, giving section 5's arrays and `TickTag` two
    above its start.

One more test checks that a non-name `BoneName` is `Script`. The copy's fixture classes gained the new component and
control fields, and the two tests that expected the old refusals now expect the owner's event.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on a throwaway local merge with #592
(5ab85711), #570 (d910fa13), #500 (c1ed82a9) and #528 (0e8e2312, the head the earlier probes used; its newer head
conflicts widely with this base), deleted after:
- ok 1, NotYet 10, error 3 of 14;
- links 1, 3, 4, 5, 13 and 14 run 0 of their driver's 1 step (`maya:8`). The first stop moved from "the animation
  tree initialisation's pass over the component" to "the owner's event after the tree initialisation" (`NotRead`,
  NATIVE_COPY_ANIM_TREE.md rule 4). The note's section 7 predicts that stop.
