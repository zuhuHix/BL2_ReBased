## 2026-11-19: the per-bone weight builders in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-PER-BONE-WEIGHTS, implementer. The decision number is pending from the
hub (three digits); NNN is a placeholder.

- **Source:** `docs/verification/NATIVE_PER_BONE_WEIGHTS.md` (#580, accepted at 5b1a567, analyst lane
  ANALYST-PER-BONE-WEIGHT-BUILDERS, not this thread), section 6. No native code or analysis output was opened.
- **Base:** #575 (fix/rust-anim-node-init at e30b7fa9), with #569 (fix/rust-skeletal-mesh-layer at 10be0643)
  merged, since the builders call its `reference_skeleton`.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added and no bounds check was loosened. Off only, and Reference is unchanged.

**What changed:**
- **The builders** (new `crates/ow-world/src/world/per_bone_weights.rs`) replace #575's F refusal, "the per-bone weight
  build of <class>". `plan` runs at every F initialisation, H's nested ones included, and is write-free. In the note's
  order:
  1. **the component's `SkeletalMesh`** must be declared as an object and hold one, else a `VmError::Script`. A real
     null skips the build, and both arrays keep their values;
  2. **the skeleton:** #569's `reference_skeleton`, its errors passed through unchanged;
  3. **more than 256 bones** is `NotYet { NotRead }`, with the note's text;
  4. **the fields** and their element and member kinds must be as the builder reads them (a `VmError::Script`
     naming the class and the field);
  5. **the rules:** the weights (a branch bone 1 for the per-bone node, or the clamped `BlendWeight` with the first
     entry winning for the feather blend; otherwise the parent's weight, and the root 0); the byte list (a change
     from the parent, and for the feather blend also an equal partial weight); `bForceLocalSpaceBlend` leaving the
     list empty; then the parents pass.
- **The writes** (`anim_node_init.rs`, implementation F's head): `Child2PerBoneWeight` and `LocalToCompReqBones`, after
  implementation B and before `Children[0].Weight`.
- **Tests** (invented skeletons, in the copy's fixture):
  - a branch's chain and its ancestors listed;
  - the root as branch, and two names for one bone (an empty list);
  - a missing name and a None name;
  - the feather clamp (negative, above 1, not a number), the first entry winning, a listed partial weight, and
    `bForceLocalSpaceBlend`;
  - the parents pass inserting a chain;
  - a null mesh keeping both arrays;
  - more than 256 bones, a mesh field of another kind and a target member of another kind, each refused write-free;
  - #569's layer refusal passing through;
  - an F node under H, built at both initialisations.

  ow-world passes 337 / 0 / 5 ignored and ow-object 138 / 0. The workspace (`-j 2`) passes 2,570 / 0 / 49 ignored.
- **Test-only switch:** `skeleton_for_tests` answers every mesh's skeleton with an invented one, as the layer itself
  is tested in its own module.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on a throwaway local merge with #570
(2a7cad3f), #500 (c1ed82a9) and #528 (0e8e2312), deleted after:
- ok 1, NotYet 10, error 3 of 14;
- links 1, 3, 4, 5, 13 and 14 moved. They were at "the per-bone weight build of Engine.AnimNodeBlendPerBone"
  (`MESH_LAYER`). They now stop, unmarked, at #569's own refusal "the skeletal mesh data layer: the unloaded skeletal
  mesh Skel_SirenBody" (`LayerAbsent`, the store's load), passed through unchanged;
- the slice world loads no mesh and installs no skeleton source, so Maya's builds do not run yet. A harness preload of
  her mesh and the `ow-cli` skeleton source would be the next steps (a hub call).

**Review r1 of #585:**
1. **The feather blend's target struct:** its declared members (`StartBoneName` a name, `BlendWeight` a float) are
   checked before any entry is read, so an empty list is checked too. Each entry's values are checked after.
2. **The F builds:** every scheduled F head carries its own build, planned in the preflight, and the writes replay the
   two arrays from it. The builder is never rerun after A and B have written. A test asserts both heads of an F node
   under H.
3. **`bForceLocalSpaceBlend`** must hold a bool exactly. Anything else is a `VmError::Script` naming the class and the
   field. The target arrays' wrong-kind errors name the class too.
4. **A name index outside the name table** is a script error naming the field. The lookup is checked, so it no longer
   panics.
5. **Tests:**
   - a 256-bone chain whose list ends with byte 255;
   - a branching skeleton whose parents pass gives [0, 1, 3, 2, 4], not a sorted list.

ow-world now passes 341 / 0 / 5 ignored, and the workspace (`-j 2`) 2,574 / 0 / 49 ignored.
