## 2026-11-25: GetSMNode refuses an anim node, or any object it reads, that the store has not loaded

AI-assisted (Claude Code, Opus 5.5). Lane GETSMNODE-FIND-NODE-LAZY (queue 0f), the follow-up named in #691's decision,
stacked on fix/rust-getsmnode-no-code-load. Off only. The rule is NATIVE_SPECIAL_MOVE_PLAY.md's, unchanged; no game
data and no native analysis was used.

- **The gap:** `SpecialMoveComponent.GetSMNode` read the component, the owner, the mesh, a cached node and each tree
  node through the loading helpers (`get` / `need`). A store handle there would be loaded, and a load is a write.
- **Reachability:**
  - **Not on today's Off slice paths.** The only Off writer of a component's `Animations` is the tree copy
    (`anim_tree_copy`), and every node it builds is a new, resident object.
  - **Reachable in general.** A component whose `Animations` (or `SMBlendNode`, `Mesh`) arrives from a loaded export
    holds the export's object references as store handles, and `GetSMNode` reads the field as stored.
  - The sibling lookup over the same trees (`tree_lookup`, #543's rule) already refuses such a handle. So the guard is
    added rather than argued away.
- **The change:** each object whose fields `GetSMNode` reads must be resident: the component, the owner, the mesh, a
  cached node, and every node the search visits. Otherwise it is `NotYet { NotRead }`, "SpecialMoveComponent.GetSMNode:
  an object the store has not loaded (a load is a write)". The check comes before the object's first field read, so
  before the node cache write.
- **Test** (`special_move_tests.rs`, synthetic): `a_lazy_anim_node_is_not_read`. A lazy node as the tree's root, and
  as a resident blend node's only child, each gives that `NotRead`. The handle stays unloaded (`defs.is_unloaded`), and
  #518's counters, the name table and the cache are unchanged. It fails without the guard in the node search.
- **Not verified:** anything in the running game; whether installed components carry a serialized `Animations`.
