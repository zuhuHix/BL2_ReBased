## 2026-11-19: The body composition's animation-node lookups search the planned copy

AI-assisted (Claude Code, Opus 5.5). Lane FIX-BODY-COMPOSITION-LOOKUP-STALE, implementer. The decision number is pending
from the hub (three digits); NNN is a placeholder.

- **Source:** `docs/verification/NATIVE_ANIM_NODE_LOOKUP.md` (#543), sections 1 and 2, which give the lookup rules on
  an instanced tree, and #557's implementation notes. This lane analysed neither note, and no native code or analysis
  output was opened (clean room). No new rule was needed.
- **Base:** #608 (`fix/rust-copy-component-pass` at 383c929c), stacked.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** Reference (the body composition is Off only). No dependency was added and no bounds check was
  loosened.

**Why.** #557 ran the lookups on the template, and refused every answer that needed the instance's node ("needs an
animation-tree instance for the skeletal mesh, which is not built yet"): the port had no instanced tree. Since #561,
#575 and #608 the attach copies and initialises the tree, so that refusal was stale. The probe of lane
ANALYST-TREE-INIT-OWNER-EVENT (#612) showed it as the six links' next stop after the owner's event.

**What changed:**
- `crates/ow-world/src/world/anim_node_lookup.rs`:
  - `check` takes the copy's preflight plan (`anim_tree_copy::Plan`) and runs the note's rules over it:
    - an animation node is the first node in the plan's tick order (the instance's tick list, on template objects)
      whose `NodeName` matches, a None name included;
    - a skeletal control is the first in `SkelControlLists` order, each head then its `NextControl` chain, whose
      `ControlName` matches; a None name answers None;
    - each find is kept only when it is of the field's class, else None. `bUsingFootIK` is whether a `WillowFootIK`
      control exists;
    - `SlotNodes` is the tick order's `AnimNodeSlot` nodes;
  - the copy keeps each node's class, name and slots, so the template's objects stand for their copies. The answers
    (`Settled`) are template objects, and `apply` writes them, mapped to the copies, into the pawn's six fields;
  - **refusals kept:**
    - an unloaded template component or body class is #529's refusal;
    - a tree with no planned copy is #529's refusal unchanged. This happens only when the copy's check is switched off
      in tests;
    - no body class is `NotRead`, as before;
  - **refusal added:** a planned copy that skips the initialisation (the component is already initialised) has no tick
    list, and the note's other path, the node gather, was not read: `NotYet { NotRead }`;
  - **rule removed:** the template-era saved-tick-order rule, since the copy's tick order is the instance's.
- `crates/ow-world/src/world/anim_tree_copy.rs`: `attach_plan` (`attach_check` that also returns its plan) and
  `instance_with_map` (`instance` that also returns the copy's map), for a caller that plans on the template.
- `crates/ow-world/src/world/body_composition.rs`: the made `Mesh`'s preflight passes the plan to the lookups. On this
  base the rest of the made-`Mesh` step is still `MESH_REST` (#570 implements it).
- **For #570 at landing:** its `write_mesh_rest` writes the four lookup fields as None and `SlotNodes` empty. With this
  lane, it should call `instance_with_map` and then `anim_node_lookup::apply` with the settled answers. Its `check`
  call takes the plan as a fourth argument.

**Write-free:** the lookups take `&Vm` and read only. The tests check heap writes, draws and load transitions
unchanged.

**Tests** (invented trees on the copy's fixture, 5 new in `anim_tree_copy_tests.rs`):
- the first node in tick order is found, and kept only as its class;
- controls follow the lists, a None name searches nothing, and `bUsingFootIK` is found;
- `SlotNodes` is the tick list's slots;
- no tick list and no body class are `NotRead`;
- `apply` writes the copies, outered to the component, through `instance_with_map`.

In `body_composition_tests.rs` the lookup tests are reduced to what that fixture can show, the no-copy refusal (its
classes declare none of the copy's fields, so the copy's check is off there). The two tests of the removed
template-era rules and one None-name test now on the copy's fixture were dropped.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on a throwaway local merge with #592
(5ab85711), #570 (d910fa13, its `check` call given the plan), #500 (c1ed82a9) and #528 (0e8e2312). The owner's event
(#612) was answered by a local stand-in; the merge was deleted after:
- ok 1, NotYet 10, error 3 of 14;
- links 1, 3, 4, 5, 13 and 14 run 0 of their driver's 1 step (`maya:8`). Their first stop moved from the lookups'
  refusal to "InitializeBodyComposition: the body's rendering hooks" (`LayerAbsent`, a rendering layer: #542's
  ruling 4).
