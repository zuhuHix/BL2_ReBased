# The pawn's animation-node lookups, and the slot-node rebuild (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-ANIM-NODE-LOOKUP (decision 2026-11-19-154, number pending).
Analyst only: a different lane implements it (clean room).

**Sources:**
- the script (`research/script_disasm.py`): every function involved here is native, and the script only reads the
  fields afterwards;
- the installed packages through `ow-package` (Maya's animation tree template, its nodes and the class defaults);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock).

There is no bl-sdk session and no recorded live evidence on these steps. Raw output stays under ignored
`local/analysis/animlookup/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

**Prerequisites, in landing order** (all open):
1. #517, then #522 (NATIVE_INIT_PAWN_MESHES.md and its implementation), as NATIVE_INITIALIZE_BODY_COMPOSITION.md
   section 0 lists them;
2. #529, NATIVE_INITIALIZE_BODY_COMPOSITION.md, at `research/rust-body-composition` aa2fcb92 (its last round);
3. #495, the `SetAnimTreeTemplate` implementation, at its hub head 8dbc419, from NATIVE_SET_ANIM_TREE_TEMPLATE.md at
   `research/rust-set-anim-tree-template` 466350b6;
4. this note.

**Why it matters.** NATIVE_INITIALIZE_BODY_COMPOSITION.md section 4.5 (PR #529) has the pawn's
`PostInitBodyComposition` set five fields from the new `Mesh` and rebuild `Pawn.SlotNodes`. Maya's attachment 0 is
that `Mesh`, and the step is on the path of links 1, 3, 4, 5, 13 and 14. That note left the lookups and the slot
rebuild unread. This note reads them and proposes the Off answer.

## 1. The lookups the hook makes (question a)

From section 4.5, with the names made exact:

| Field written | Lookup | Name | Kept only as |
|---|---|---|---|
| `ArmBoneBlend` | animation node | `ArmBoneBlend` | `AnimNodeBlendPerBone` |
| `AimOffset` | animation node | `BodyClass.AimOffsetNodeName` (with no `BodyClass`: not read; unreachable, since the wrapper's gate needs a loaded `BodyClass`) | `WillowAnimNodeAimOffset` |
| `LeftHandIK` | skeletal control | `LeftHandGrip` | `SkelControlLeftHandGripWeapon` |
| `HeadLookAt` | skeletal control | `BodyClass.HeadLookAtName` (None with no `BodyClass`) | `WillowSkelControl_LookAtActor` |
| `bUsingFootIK` | skeletal control | `WillowFootIK` | true when any object is found |

A lookup's result that is not of the "kept only as" class is stored as None.

**The two lookups**, through the mesh component's animation-behaviour interface:
- **Animation node by name.** With no tree on the component (its `Animations` None), the answer is None. Otherwise:
  - when the component's tick list is filled and its tree is this component's own, the answer is the **first** node
    in the tick list whose `NodeName` equals the name;
  - otherwise the tree's nodes are collected by the node class's own gather step (not read; by fit, from the root
    through the children) and the first match in that list is the answer.

  The name is compared whole (name and number). **A None name is not special:** it matches the first node whose
  `NodeName` is None. On Maya's tree the root is the first such node, and the cast then gives None (section 4).
- **Skeletal control by name.** None when the component has no tree or its tree is not an `AnimTree`. **A None name
  answers None at once, with no search.** Otherwise: the tree's `SkelControlLists` in array order; for each list, its
  head control, then each control's `NextControl`, in order. The first control whose `ControlName` equals the name is
  the answer.

**What the hook writes**, in its order, all on the pawn: the five fields above, then `SlotNodes` (section 2). Each
lookup reads only; none writes the tree.

**The script does the same lookups too.** `WillowPlayerPawn.PostInitAnimTree`, the script event that runs after a
tree is initialised, sets `ArmBoneBlend`, `AimOffset` (with a `BodyClass`) and `LeftHandIK` through the script-facing
`FindAnimNode` and `FindSkelControl`, each only while the field is still None. The same rules apply to it, and an
implementer can share one lookup between the two.

## 2. The slot-node rebuild

`Pawn.SlotNodes` is emptied, then each node of the component's tick list that is an `AnimNodeSlot` (or a subclass) is
appended, in tick-list order. With no tree the list stays empty.

## 3. When the tree is instanced, and what the port has (question b)

**In the game:** the template is not what the lookups search. The mesh component gets its own copy of the tree when
it is attached, before the body composition's `PostInitBodyComposition` runs (NATIVE_INITIALIZE_BODY_COMPOSITION.md,
the attach step comes first). The attach sets the tree from `AnimTreeTemplate` through the same path as
`SetAnimTreeTemplate`: the template is copied (`CopyAnimTree`, not read), then the tree is initialised. That
initialisation puts the root first in the tick list, gives it the component, and then has the root add its
descendants (by fit, in the tick order; not read in full). So the lookups see the **instanced** nodes, and the fields
hold references to them.

**In the port:**
- the VM heap has no instanced animation tree. NATIVE_SET_ANIM_TREE_TEMPLATE.md's implementation (#495, open, at
  8dbc419) refuses a template with a typed `NotRead`, and no other code makes the tree objects;
- the host (`crates/ow-host/src/anim/`) reads the template through the definition store to pose a mesh. It is
  render-side, not in the VM heap, and its objects are not the game's instance;
- the template's nodes are readable through the definition store, **with class defaults applied**. In Off the store
  applies archetype values, and a template node's archetype is its class default object. That matters here:
  three of the four names found on Maya's tree (section 4) are not in the template's tagged properties at all. They
  come from the node classes' defaults, so a reader of the raw tags alone would find nothing.

## 4. Maya's attachment 0 (question d; names and counts only)

Attachment 0 is Maya's `Mesh` component. Its `AnimTreeTemplate` is `GD_PlayerShared.Character.AnimTree_Player` (a
`WillowAnimTree`), read from `GD_Siren_Streaming_SF`. Her body class (`BodyClass_Siren`) has `AimOffsetNodeName`
`AimOffset` and `HeadLookAtName` None.

- **The tree:** 71 objects reached from the root: 66 animation nodes (the root included) and 5 skeletal controls.
  The nodes are the closure of the root's `Children` (each child's `Anim`, then its own `Children`). The controls are
  those of `SkelControlLists`, each list's `ControlHead`, then the `NextControl` chain.
- **The saved tick indices:** 65 nodes store a `TickArrayIndex`, 1 to 65, all distinct. The root stores none, so it
  has the default, 0. The 66 nodes are a full order from 0 to 65. The game's runtime tick order is taken to equal it
  **by fit** (open question 1).
- **Names**, with class defaults applied: 27 nodes and controls are named in the template, 5 take a name from their
  class default, and 39 have none. For each of the 39, every class default up the parent chain to `AnimNode` was read
  (across packages), and none sets `NodeName`, so their name is None. None of them is named `ArmBoneBlend` or
  `AimOffset`; a None-name lookup is not on Maya's path, because her `AimOffsetNodeName` is `AimOffset`. Repeated names: `ScaleRateBySpeed` ×5, `WeaponAdd` ×5, `UBMoveBlend` ×4,
  `CrouchAnim` ×2, `WillowFootIK` ×2. The first match is the one with the lowest saved tick index.

**The five lookups on Maya's tree:**

| Field | Name | Found | Result |
|---|---|---|---|
| `ArmBoneBlend` | `ArmBoneBlend` | no node of that name | None |
| `AimOffset` | `AimOffset` | 1 node, a `WillowAnimNodeAimOffset`, named by its class default; saved tick index 5 | that node (the instance's) |
| `LeftHandIK` | `LeftHandGrip` | 1 control, a `SkelControlLeftHandGripWeapon`, named by its class default | that control (the instance's) |
| `HeadLookAt` | None | none: a None control name answers None at once | None |
| `bUsingFootIK` | `WillowFootIK` | 2 controls, `WillowSkelControl_FootPlacement`, named by their class default | true |

**SlotNodes:** 3 nodes, all `AnimNodeSpecialMoveBlend`, which extends `AnimNodeSlot`. In saved tick order:
`SpecialMoveBlend` (its class default name; index 1), `TurnBlend` (3), `WeaponAction` (8).

## 5. Proposed Off answer (question c; for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces one bullet of NATIVE_INITIALIZE_BODY_COMPOSITION.md section 7's
preflight, step 4 ("the pawn's `PostInitBodyComposition`"): the bullet for identifier `Mesh` with a made
`SkeletalMeshComponent`. In #529 that bullet refuses every such attachment. The rest of #529's Off contract is
unchanged, and this check runs **after** that step's release check for the old `Pawn.Mesh`.

**The smallest answer that does not invent an instance.** It is part of #529's write-free preflight, so it **loads
nothing** (hub ruling, review r1 of #543). A lazy load counts as a write under #518 (decision 2026-11-19-93), and it
would mark this refusal and every later one of the walk. The pattern is `ow-inventory/src/globals.rs`: check
`is_unloaded` and refuse without loading.
1. **What is read.** The attachment's **template component**, `Data.ComponentData.Component`, not the new component:
   the new component does not exist until write step 1.1. From it, its `AnimTreeTemplate`, read through the
   definition store with class defaults applied.
2. **An unloaded handle refuses.** If the template component, its `AnimTreeTemplate`, or any node or control the
   lookups visit is an unloaded handle: the refusal of rule 4, raised with nothing loaded. The instanced tree is
   missing anyway, so nothing is lost.
3. **Template-settled answers**, only for a template that is **already loaded** in full. By fit, the copy keeps each
   node's class and name, so the template's names stand for the instance's; this is **UNVERIFIED** (open question 3).
   The five lookups and the slot scan run against the template's names only:
   - node lookups visit the closure of the root's `Children` in the saved `TickArrayIndex` order, first match. If the
     saved values are not a full, distinct order 0 to n − 1 over those nodes, refuse with `NotYet { NotRead }`,
     "InitializeBodyComposition: the template's saved tick order", record this note;
   - control lookups visit `SkelControlLists` in order, each list's `ControlHead`, then the `NextControl` chain, and
     take the first match. A None name answers None;
   - a node lookup with a None name is matched as in section 1 (the root first).

   From those, these fields need no instance:
   - a lookup that finds nothing, or finds an object whose class is not the "kept only as" class: the field is None;
   - `bUsingFootIK`: true when a `WillowFootIK` control exists, else false;
   - `SlotNodes`: empty when the template has no `AnimNodeSlot` node.
4. **Everything else refuses** with #529's text, kept unchanged. A lookup that finds a kept object needs the field to
   hold the instance's node, and the port has none; a template with at least one slot node refuses too. The refusal
   is typed `VmError::NotYet` with `Missing::LayerAbsent("an animation-tree instance for the skeletal mesh")`, what
   "InitializeBodyComposition: the new Mesh's animation nodes (ArmBoneBlend, AimOffset, LeftHandGrip, the head
   look-at, WillowFootIK and the slot nodes)", and record NATIVE_INITIALIZE_BODY_COMPOSITION.md section 7 with this
   note. It is raised in the preflight, before any write.

   Under the hub's render-side ruling a field stays `NotYet` when script reads it: `SetAimOffsetProfile` reads
   `AimOffset`, and `UpdateAnimSets` reads `ArmBoneBlend`. For `SlotNodes`, no script reader was found:
   `Pawn.ClearAnimNodes` only sets its length to 0. Native readers were not searched.
5. **No tree:** a loaded template component with `AnimTreeTemplate` None answers None for all four lookups, false for
   `bUsingFootIK`, and an empty `SlotNodes`. That is the game's answer for a component without a tree.
6. **Writes:** none new beyond section 4.5's. Nothing here loads, so a refusal from this check never takes the
   "stopped after partial writes: " prefix on its own account.

**On Maya**, once the release check for the old `Pawn.Mesh` has passed (#529's "Where Maya stops", item 1):
- if her template is unloaded at that point, rule 2 refuses;
- if it is loaded, rule 4 refuses: `AimOffset` and `LeftHandIK` are found, and the tree has 3 slot nodes.

Either way the links named above move only once the tree is instanced. The next reading for that is `CopyAnimTree` and the rest
of the tree initialisation: what the copy duplicates, which outer the copies get, what each copy keeps of its
template, and the order in which the root adds its descendants. That reading would also settle open question 1. It
was not done in this lane.

**Reference** keeps the stub policy.

## Open questions

1. **UNVERIFIED:** that the runtime tick list is in the saved `TickArrayIndex` order. The initialisation's child step
   was not read in full. This decides which of a repeated name is found, and the `SlotNodes` order.
2. **UNVERIFIED:** the gather step used when the tick list is empty or belongs to another tree. Not on the body
   composition's path, by fit: the attach fills the tick list first.
3. **Not read:** `CopyAnimTree`, the tree's instancing (section 5), including that each copy keeps its template's
   class and name (by fit).
4. **UNVERIFIED:** the class-default names, read from each class's default object and its parents' (section 4).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On Maya after spawn: `Pawn.AimOffset` and `LeftHandIK` are set, their `Outer` is not the template's package, and
   their names are `AimOffset` and `LeftHandGrip`. `ArmBoneBlend` and `HeadLookAt` are None, and `bUsingFootIK` is
   true.
2. `Pawn.SlotNodes` holds 3 nodes, in the order `SpecialMoveBlend`, `TurnBlend`, `WeaponAction`.
3. On the mesh's tick list: its order against the template's saved `TickArrayIndex` values, and which `UBMoveBlend`
   a lookup by that name returns (the one with the lowest saved index).
