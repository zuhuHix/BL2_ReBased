## 2026-11-26: UpdateAnimations' per-node step is a re-bind reset, not a timed tick

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-UPDATEANIMATIONS-TREE-TICKS, the analyst half of the clean room
(another lane implements the note), on main 1709b8a8. Ghidra 12.1.4 headless, read-only, under the machine lock; raw
output under ignored `local/analysis/treeticks/`. No bl-sdk session was granted. **Every rule is UNVERIFIED in the
running game.**

**What was read.** `SkeletalMeshComponent.UpdateAnimations`: with a tree, one component tag field (by fit `InitTag`)
:= the tree's `NodeInitTag` + 1, then one argument-less virtual per `AnimTickArray` node. That slot in all 31 anim-node
classes of the chain of Maya's tree: 28 share an empty body. The sequence family (`AnimNodeSequence`,
`WillowAnimNodeSequence`, `WillowAnimNodeSequenceAdditiveBlend`) runs "set animation" with None through the class table,
keeping the name, and zeroes `CachedNumDesiredBones`. The additive blend's override auto-names `Add_<name>` (an intern)
and looks it up through the component's anim sets.

**What it settles.** NATIVE_BONE_TRANSFORM_LAYER.md section 6.4 refused the step as `NotPorted` because it was unread.
It is not a tick: a narrow Off answer exists (NATIVE_UPDATEANIMATIONS_TREE_TICKS.md section 5), with refusals for a set
`AnimSeq`, an unknown node class and a lookup match.

**A correction.** NATIVE_ANIM_NODE_INIT.md section 2 D groups `WillowAnimNodeSequenceAdditiveBlend` with the plain
sequence. Its initialisation also calls the overridden "set animation", so Maya's one auto-name additive node (its class
default sets `bAutoGenerateAdditiveSeqName`) gets `Add_None` there too. The port's init (#575, kind D) does not write
it. Whether to fix it is the hub's call.

**Field names.** `class_layout.py`, with every read past `CachedNumDesiredBones` 4 bytes beyond the computed place (by
fit the 16-byte alignment of the `BoneAtom` struct). It is checked by the arrays, names and flags landing on declared
properties. The shift is UNVERIFIED (fitted). `SkeletalMeshComponent` and `AnimSet` are named by fit (map properties
stop the tool).

**Predicted next stop:** `InitializeRBPhysics`' physics `LayerAbsent` once this is implemented. If Maya's anim sets
are store handles at `UpdateAnimations`, the earlier stop is the lookup's not-loaded `NotRead`. No code changed.
