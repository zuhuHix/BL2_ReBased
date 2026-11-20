## 2026-11-19: the aim-offset fix-up and the per-node initialisation in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-ANIM-NODE-INIT, implementer. The decision number is pending from the hub
(three digits); NNN is a placeholder.

- **Sources:** `docs/verification/NATIVE_ANIM_NODE_INIT.md` (#566, accepted at 07b2dd68, analyst lane
  ANALYST-AIMOFFSET-REFRESH-AND-NODE-INIT, not this thread) and the hub's rulings recorded in it. No native code or
  analysis output was opened.
- **Base:** #561 (fix/rust-copy-anim-tree at d83f2fee).
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added and no bounds check was loosened. Off only, and Reference binds nothing.

**What changed:**
- **The aim-offset fix-up** (`anim_tree_copy.rs`) replaces #561's `AIM_OFFSET` refusal. For a copy of exact class
  `Engine.AnimNodeAimOffset` or `WillowGame.WillowAnimNodeAimOffset` whose outer differs from its source node's,
  `TemplateNode` := the source node and the copy's own `Profiles` := empty. It runs in gather order, after the copy's
  `ParentNodes`. The preflight checks both fields.
- **The per-node initialisation** (new `crates/ow-world/src/world/anim_node_init.rs`) replaces #551 rule 3's blanket
  refusal. It uses the note's exact-class table, which maps each class to one of the implementations A to H
  (`AnimNodeSlot` to H's slot part). A class not in the table is `NotRead`, "the per-node initialisation of <class>".
  - **The preflight** is write-free, loads nothing and draws nothing. It walks every node in tick order: the copy's
    tick rule, run read only on the template.
  - **Refusals:** the note's 1 to 7, with its texts:
    - F is #529's exact `MESH_LAYER` `LayerAbsent` while the mesh reader (#569) is not on this base;
    - E checks that the component's class declares `bUpdateComposeSkeletonPasses` as a bool (`VmError::Script`
      otherwise), and the write is the hub's fitted ruling, UNVERIFIED.
  - **H's owner test** uses a new read-only `CodeIndex::implements_resolved` in `crates/ow-object/src/reflect.rs`. It
    answers only from interface entries already resolved, so the preflight never calls `Vm::implements`, which
    resolves as it goes. Any other answer is the "not SpecialMoveInterface" refusal.
  - **The writes** follow the note's section 2 steps per node, in tick order, after the copy's two writes. H's slot
    part initialises its children past the first again.
- **The copy's remaining unread steps** (the component pass, the per-bone arrays and the owner's event) are unchanged
  and still refuse after the per-node initialisation. The test switch now answers three steps, not four.

**Tests** (invented classes and objects; ow-world 308 / 0 / 5 ignored, 28 in the copy's tests):
- A to H's writes, each implementation shown;
- the aim-offset fix-up, and nothing done when the outers match;
- each of refusals 1 to 7, write-free, and an unknown root class refusing by name;
- H's double initialisation.

ow-object adds one test, `implements_resolved_answers_only_from_resolved_entries`. The workspace (`-j 2`) passes
2,504 / 0 / 48 ignored.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on a throwaway local merge with #570
(fix/rust-mesh-rest at 2a7cad3f, with Maya's tree preload), #500 (c1ed82a9) and #528 (0e8e2312), deleted after:
- ok 1, NotYet 10, error 3 of 14;
- links 1, 3, 4, 5, 13 and 14 run 0 steps (link 1: 0 of 1, `maya:8`). They stop, unmarked, at refusal 5: "the per-bone
  weight build of Engine.AnimNodeBlendPerBone", `LayerAbsent` (the mesh data reader);
- that is the note's section 5 expectation. **What the run shows** (review r1 of #575, item 7; a local debug print,
  not committed): the preflight's first refusal is F, on the third node it checked. Before it, only E (the root,
  `WillowAnimTree`) and H (one `AnimNodeSpecialMoveBlend`) were checked and passed. No other implementation was
  reached, so the run does not show A, B, C, D or G passing on Maya's tree, nor the aim-offset fix-up's cast. The F
  refusal stops the preflight, so the fix-up's write never runs.

**Review r1 of #575:**
1. **The planned steps.** The preflight now plans exactly the steps the writes run, and the writes replay them in
   order, with no recursion:
   - the tick list, by the writes' own parent rule: a parent outside the tree counts as listed when its own tag is the
     component's next tag;
   - then, per node, the copy's two writes, the node's head, H's re-initialised children (each a child call), and H's
     tail.

   Every node those steps initialise is checked, including a child of H that is not on the tick list. The writes
   refuse with a `VmError::Script` if the list they build differs from the planned one.
2. **`implements_resolved`** returns no answer at the first unresolved entry met before a match. So a derived class's
   unresolved entry is never answered over its base's cached hit. A regression test covers it.
3. **A slot whose children lead back to it** (a cycle on the active slot call path) is a `VmError::Script` in the
   preflight. The plan is also capped at 65,536 steps.
4. **H's `AnimParameters`** must be an array of a defined struct, whose zero entry is read through the checked
   `struct_defaults`, in the preflight and in the writes.
5. **The aim-offset fix-up's checked cast:** when the fix-up will run, the source must derive from
   `Engine.AnimNodeAimOffset`, or it is a `VmError::Script` before the copy is made.
6. **Tests:**
   - H's child off the tick list, initialised by its step 2 only, with its `OnInit` refusal;
   - an outside parent with the next tag;
   - the slot cycle, the int-array parameters and the aim offset without its base;
   - a package-backed owner implementing `SpecialMoveInterface` with an interface-typed `SMI` (no test switch);
   - the write-free helper now also checks RNG draws and load transitions.

   ow-object passes 138 tests and ow-world 314 / 0 / 5 ignored. The workspace (`-j 2`) passes 2,511 / 0 / 48 ignored.

**Review r2 of #575:**
1. **H's new `AnimParameters` entries** are the element struct's zero value, taken from its checked layout, not its
   defaults. A struct default that names a `CustomAnimDef` no longer reaches the entries. A test covers a nonzero
   default.
2. **Child links:** a `Children[i].Anim` link that is not an object reference (an interface value, for one) is a
   `VmError::Script` in the preflight's gather. The copy remaps object links only, so the plan and the writes follow
   the same links. A test covers an interface-typed link.

ow-world now passes 316 / 0 / 5 ignored.
