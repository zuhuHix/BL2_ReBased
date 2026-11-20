## 2026-11-19: the animation tree's copy and initialisation in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-COPY-ANIM-TREE, implementer. The decision number is pending from the hub
(three digits); NNN is a placeholder.

- **Sources:** `docs/verification/NATIVE_COPY_ANIM_TREE.md` (#551, accepted at 4a363c0c, analyst lane ANALYST-COPY-ANIM-TREE, not
  this thread) and NATIVE_ANIM_NODE_LOOKUP.md (#543 at b09720f6), with the hub's rulings and port answers. No native code
  or analysis output was opened.
- **Base:** #542 (fix/rust-body-composition at b2b4de8a).
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added and no bounds check was loosened. Off only, and Reference binds nothing.

**What changed:** `crates/ow-world/src/world/anim_tree_copy.rs` (new) and one module line in `world.rs`.
- **`plan`, the write-free preflight.** It writes nothing and loads nothing, and checks in this order:
  1. an unloaded template or node or control is #543's refusal, unchanged: `LayerAbsent`, an animation-tree instance;
  2. the declared fields; a missing one is `VmError::Script`;
  3. **the link fix-ups (rule 2),** each `NotRead`:
     - a node class outside the checked set: "CopyAnimTree: the link fix-up of <class>". The set is `AnimNode`,
       `AnimNodeBlendBase`, `AnimNodeSequence`, `AnimNodeSlot` and the node classes of Maya's tree. The tree's 15
       classes were taken from the class names of `AnimTree_Player`'s node exports, names only;
     - `WillowAnimNodeAimOffset`: "CopyAnimTree: the aim-offset node's refresh after its TemplateNode is set";
     - any morph node.

     The check is by exact class. The note's inheritance rule needs to know which classes override, which the port
     cannot tell, so a subclass outside the set is refused;
  4. rule 2a and rule 2b, each `NotRead` with the note's text: a root with more than one child, the template's
     `bRebuildAnimTickArray` set, and its parent node array not built;
  5. a component already initialised, `NotRead`. This is this lane's check: whether the attach forces the
     initialisation was not read;
  6. the per-node initialisation, `NotRead`. This refuses every tree, as the hub ruled. The pass over the component
     (this lane's text), the per-bone arrays (`LayerAbsent`, the mesh data reader) and the owner's event come after it
     and are named in the module.

  A test-only switch answers step 6 with nothing, to keep the write path covered, as #542 does for its render hooks.
- **`instance`, the writes** (only when the preflight passed), under #518's counters:
  1. **the copy**, on the fresh path, since the port has no pool. The tree is made first, then the nodes in gather
     order.
     - Each copy keeps its template's class and name, and takes the component as outer.
     - Each copy takes its template's values at copy time. This is the port's archetype: the heap has no archetype
       link.
     - `AnimTreeTemplate` names the original.
     - Links are rewired through the maps: `Children[i].Anim` of blend nodes, the root's first child, `ParentNodes`,
       and the cached `AnimTickArray`.
     - The `AnimGroups` run-time lists are emptied, and `bRebuildAnimTickArray` is copied.
     - The controls are copied too, with `NextControl` and the list heads mapped.
  2. **the component's `Animations`** is set to the copy. This is #495's `SetAnimTreeTemplate` write, made before the
     initialisation, as the hub's port answer says;
  3. **the initialisation:** `TickTag` goes up by one. The tick list is always built, by the note's rule, as the hub
     answered. Each node gets `SkelComponent`, `TickArrayIndex` and `NodeTickTag`. The list is stored on the component
     and in the tree's cache, and `bAnimTreeInitialised` is set.
- **The root in the map** (a port reading, UNVERIFIED): the template root maps to the new tree. Without that, the root's
  children would keep the template root as a parent, and the tick rule could never list them.

**Maya's expected stop** (the note's section 5): the aim-offset refusal.

**Tests** (invented classes and objects; the class and field names are the packages'):
- the gather order on a diamond: root → A; A → B, C; B → D; C → D;
- the tick order on the same diamond: the root, A, B, C, D, with D waiting for its second parent. This is not the
  gather order;
- each node's index, component and tag;
- the copy's class, name, outer, links, groups and controls, with the template left untouched;
- a copy of a copy names the original;
- a checked class copied and listed;
- each refusal, the aim-offset node's included, with nothing written and nothing made;
- an unloaded store template, with nothing loaded.

**Wiring** (after the merge of #557 at 9694cc30, no rebase): the body composition's preflight, for a made `Mesh`, runs
`anim_tree_copy::attach_check` before #557's lookups. The attach instances the tree first, so the copy's preflight runs
on the component template's `AnimTreeTemplate`, with the template standing for the component. The unloaded-handle
refusal is #557's own `refusal()`, so both lanes refuse with one text, layer and record. The instance's writes join the
made `Mesh`'s writes when that step has an Off answer past the lookups, which it does not yet (#557's `MESH_REST`).
#557's lookup tests skip the check through a test-only switch, because their fixture tree has no copy fields.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on a throwaway local merge with #500
(fix/rust-go-down-natives at c1ed82a9) and #528 (fix/rust-spawn-complete at 0e8e2312), deleted after:
- with #500 alone, link 1 stops earlier, at #522's unloaded body-class refusal; #528 is what loads the body class, as
  it did in #542's probe;
- **with both:** ok 1, NotYet 10, error 3 of 14. Link 1 runs 0 of its driver's 1 step (`maya:8`) and stops at #543's
  `LayerAbsent` "the new Mesh's animation nodes", raised by the copy's rule 1. A local debug print showed the
  unloaded handle is the tree template, `GD_PlayerShared.Character.AnimTree_Player`. Nothing in the slice world loads it,
  so the note's "if it is not loaded, that refusal stays her stop" holds;
- **with the tree preloaded** (a local-only probe that loaded its 71 objects, 66 nodes and 5 controls, never pushed):
  link 1 stops at "CopyAnimTree: the aim-offset node's refresh after its TemplateNode is set", unmarked, as the note
  expects. Every other node class of her tree passed the checked set;
- loading the tree for the slice is a harness or set-up question for the hub; this lane does not add it.

**Review r1 of #561:**
1. **Copied tags:** the copies take their templates' `NodeTickTag`. So a gathered node whose tag already equals the
   component's next tick tag is refused, `NotRead`. How the game keeps them apart was not read (hub ruling,
   UNVERIFIED).
2. **The four unread steps** of the initialisation are raised in order:
   - the per-node initialisation;
   - the pass over the component;
   - the per-bone arrays (`LayerAbsent`, the mesh data reader);
   - the owner's event.

   The test switch can now answer the first n of them.
3. **The component's class:** a component that is not a `SkeletalMeshComponent` is a `VmError::Script`.
4. **Morph nodes:** an unloaded morph node is rule 1's refusal, raised before its class check.
5. **Control chains:** #557's rule (6803b3b) applies. A repeat within one chain is a `VmError::Script`, and a control
   shared with an earlier list keeps its first place.
6. **An initialised component:** the already-initialised refusal is gone. When `bAnimTreeInitialised` is set (the call
   does not force it), the copy is made and `Animations` set, and the initialisation is skipped, as the note says.
7. **Left out, port answers, UNVERIFIED:**
   - the post-construction call on each copy (open question 5);
   - the engine-wide "duplicating" flag.

   The #495 integration must not write `Animations` a second time.
8. **New tests:**
   - an unloaded node and an unloaded control in a loaded tree;
   - the body composition with the copy check on, refusing before the lookups;
   - a looping control chain, and a control shared between two lists;
   - the colliding-tag copy;
   - the skipped initialisation;
   - the unread steps in order;
   - the non-skeletal component.

   ow-world now passes 298 / 0 / 5 ignored.
