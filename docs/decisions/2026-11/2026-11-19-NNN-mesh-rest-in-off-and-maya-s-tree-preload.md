## 2026-11-19: the rest of the made-Mesh step in Off, and Maya's tree as a harness input

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-MESH-REST, implementer. The decision number is pending from the hub
(three digits); NNN is a placeholder.

- **Source:** `docs/verification/NATIVE_MESH_REST.md` (#563, accepted at 9ebde2bb, analyst lane, not this thread),
  section 4, with the hub's relays of its review r1. No native code or analysis output was opened.
- **Base:** #561 (fix/rust-copy-anim-tree at 07d53556), with its accepted r1 fix d83f2fee merged (no rebase).
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added and no bounds check was loosened. Off only, and Reference binds nothing.

**What changed:**
- `crates/ow-world/src/world/body_composition.rs`: #557's `MESH_REST` refusal is replaced by the note's Off answer.
  - **The preflight** (after #557's lookups, writing and loading nothing):
    1. the made component's class declares `bSkipAllUpdateWhenPhysicsAsleep` as a bool. The name and its bit (a fit
       by declaration order) are UNVERIFIED;
    2. the pawn's class declares `Mesh` and `MeshBodyCompositionIndex`, and `CollisionComponent` when
       `bUseMeshForCollision` is set. On the plan's authority (#529 section 7 step 3, not a fresh read of `Role`) it
       declares `RepMeshScale` as a float (the name a fit, UNVERIFIED), and the made component's class declares `Scale`.
       This port also requires the five animation fields and `SlotNodes`, since it writes them. Each destination is
       checked against the write it takes: its kind, and for `Mesh` and `CollisionComponent` that it takes the made
       component's class, and `Scale` must be a float (review r1 of #570). So no write fails after the mesh is made.
       Otherwise `VmError::Script`;
    3. `ReattachWeapons`, in the note's order. It is resolved first, as Off event dispatch makes it: the active state,
       its parent states, then the class chain (`ow_vm::states::find_loaded_event`), with nothing loaded. Anything
       but `WillowPawn.ReattachWeapons`, a state function or no function at all included, is `NotRead` "an override of
       ReattachWeapons" (review r1 of #570). A held `Weapon` or `OffHandWeapon` is `NotRead` "ReattachWeapons with a
       held weapon (AttachThirdPersonMeshToComponent)". With both None the event writes nothing, and it is not called.
  - **This port's own refusal:** a resolution that would need a read or a write is `NotRead` "ReattachWeapons, whose
    dispatch needs class code or a state not read yet". That covers a chain class whose package code is not loaded,
    a state not read yet, and a pawn with no state frame whose heap names a state. The preflight may not load.
  - **The writes,** right after the attach: the tree instance (#561) first, then section 3's order. That is `Mesh`, the
    index as a byte, the flag true on the made component, the four fields None and `bUsingFootIK` as #557 settled it,
    `SlotNodes` empty, `RepMeshScale` = the made component's `Scale` on authority, and `CollisionComponent` = the mesh
    when `bUseMeshForCollision`. A second `Mesh` attachment is the release `NotRead`, as a second cylinder is.
- `crates/ow-vm/src/interp/load.rs`: `Vm::find_loaded_method`, `find_method`'s answer without loading: `None` when a
  chain class's code is not loaded. `Vm::declare_method` gives a class declared by hand (no package code) a method
  that `find_method` sees; the body composition's fixture uses it for its pawn class. It only adds a name no lookup
  from the class already answers, after loading the chain's package code, and a hand-declared entry never stands
  for a package ancestor's load (review r2 of #570).
- `crates/ow-vm/src/states.rs`: `find_loaded_event`, `find_event`'s walk (state, parent states, class chain) over the
  states and class code already read, writing nothing.
- `crates/ow-cli/src/cmd/combat.rs`: **Maya's tree preload**, a harness input (the hub's ruling), sits in
  `apply_class_inputs` between the archetype preload and the class select, so no #528 merge was needed. It loads
  `GD_PlayerShared.Character.AnimTree_Player` and what it reaches through `Children`, `SkelControlLists` and
  `NextControl`, and logs the count. It is an UNVERIFIED stand-in for the streamed player package; the natives load
  nothing.

**Tests** (invented values; the class and field names are the packages'):
- the final field values on authority with mesh collision, the attachment at index 1;
- off authority, `RepMeshScale` is left alone;
- a held weapon in either hand is write-free `NotRead`;
- a second `Mesh` is a release;
- the kind checks give `Script`, and an integer `Scale` fails before any write;
- no `ReattachWeapons`, and a subclass override with a held weapon, are the override refusal; a frameless pawn whose
  heap names a state, with a held weapon, is this port's refusal. Each is write-free with no partial-writes prefix;
- `find_loaded_method` loads nothing and answers like `find_method`, a state's function over the class's;
- `find_loaded_event` finds a parent state's event, as `find_event` does, where the state name alone would not;
- `declare_method`'s refusals.

The ow-world fixture's classes are declared by hand, so it cannot build a package state or a chain class with
unloaded code. Those two paths rest on the ow-vm tests.

**The installed Off `--slice-gate`** (strict, under the machine lock, zuhu desktop), on a throwaway local merge with
#500 (c1ed82a9) and #528 (0e8e2312), deleted after:
- ok 1, NotYet 10, error 3 of 14. The preload logs "loaded 71 objects" (66 nodes and 5 controls, as #561's local
  probe counted);
- link 1 runs 0 of its driver's 1 step (`maya:8`) and stops, unmarked, at "CopyAnimTree: the aim-offset node's refresh
  after its TemplateNode is set", the stop NATIVE_COPY_ANIM_TREE.md section 5 expects. Links 3, 4, 5, 13 and 14 are
  blocked behind it, with the same first stop;
- the made-Mesh rest is therefore not reached on Maya yet. The copy's refusal comes before it, in the attach.
