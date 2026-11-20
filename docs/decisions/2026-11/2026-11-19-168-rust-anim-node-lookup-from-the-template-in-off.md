## 2026-11-19: The new Mesh's animation-node lookups, from the template, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-ANIM-NODE-LOOKUP, links 1, 3, 4, 5, 13 and 14. Coded from
docs/verification/NATIVE_ANIM_NODE_LOOKUP.md alone, the note's text at b09720f6 (PR #543; clean room: lane
ANALYST-ANIM-NODE-LOOKUP wrote it), with the hub's rulings on its review r1. The executable and the analysis output
were not opened. Synthetic fixtures only. Stacked on #542 (fix/rust-body-composition b2b4de8a).

- **What changes (`ow-world` `world/anim_node_lookup.rs`, called from `world/body_composition.rs`'s preflight, Off
  only).** It replaces #529's bullet "identifier `Mesh` with a made `SkeletalMeshComponent` is `LayerAbsent`" (section
  7, step 4). It runs after that step's release check for the old `Pawn.Mesh`, which is unchanged. Reference is
  unchanged: the body composition is Off only.
- **What is read.** The attachment's template component (`Data.ComponentData.Component`), its `AnimTreeTemplate`, the
  tree's nodes and its controls, all as they stand:
  - the nodes are the closure of the root's `Children` (each child's `Anim`);
  - the controls are `SkelControlLists` in order, each list's `ControlHead`, then the `NextControl` chain. Only an
    `AnimTree` has them.
- **Nothing is loaded** (hub ruling). An unloaded or failed template component, tree, node or control is #529's
  refusal, raised with nothing loaded. So is an unloaded body class whose names the lookups need. The refusal keeps
  #529's text and layer string exactly. Its record now names both notes.
- **The lookups** (the note's sections 1 and 5):
  - node lookups take the first match in the saved `TickArrayIndex` order. Saved values that are not a full, distinct
    order 0 to n − 1 are `NotYet { NotRead }`, "InitializeBodyComposition: the template's saved tick order";
  - a None node name matches the first node whose `NodeName` is None;
  - a None control name answers None with no search;
  - names are compared whole (name and number);
  - the "kept only as" classes are matched by class name up the class chain.
- **What the template settles, and what refuses.** A lookup that finds nothing, or finds an object not of its kept
  class, settles its field as None. `bUsingFootIK` settles as whether a `WillowFootIK` control exists. `SlotNodes`
  settles as empty when no node is an `AnimNodeSlot`, and no tree settles everything. A kept find, or any slot node,
  is #529's refusal.
- **This lane's addition, for the hub.** When the template settles every field, the made-`Mesh` step still has two
  parts that neither note answers: the mesh flag that #529 section 4.5 does not identify, and the `ReattachWeapons`
  script event. They are `NotYet { NotRead }`, "InitializeBodyComposition: the new Mesh's unidentified flag and its
  ReattachWeapons event" (`MESH_REST`), so no made `Mesh` is written yet.
- **The AimOffset lookup with no body class.** The note leaves it unread, and it cannot be reached from the pawn
  wrapper. It is `NotYet { NotRead }`.
- **On Maya** (the note's section 4): her template either refuses under rule 2 (not loaded) or under rule 4
  (`AimOffset` and `LeftHandGrip` are found, and the tree has 3 slot nodes). Either way the refusal is the one #529
  gave. On the installed gate, with `Engine.IsSplitScreen()` bound false locally, links 1, 3, 4, 5, 13 and 14 stop
  there, unmarked, as #542 measured; without that binding they stop earlier, at `IsSplitScreen`.
- **Tests (`world/body_composition_tests.rs`).** The toy Engine package gains the animation classes the lookups read,
  and the template package gains a skeletal template whose tree is a store handle. The tests cover:
  - no tree;
  - a settled tree with the foot IK found;
  - each kept find and a slot node;
  - first match by saved tick order, against `Children` order;
  - None names;
  - a broken tick order;
  - an unloaded template and an unloaded tree, with nothing loaded;
  - the old-`Mesh` release coming first.

  The Maya-order test now uses a tree with a kept `AimOffset` node. Every refusal is checked write-free with #518's
  counters.
- **UNVERIFIED.** Every rule is unconfirmed in the running game. In particular, that the instanced tree keeps the
  template's classes and names, and that the runtime tick order is the saved one (the note's open questions 1 and 3).
