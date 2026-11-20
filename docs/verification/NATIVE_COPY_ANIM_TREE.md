# The animation tree's copy and initialisation (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-COPY-ANIM-TREE (decision 2026-11-19-162, number pending).
Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed packages through `ow-package` (Maya's tree template: names, counts, its saved tick list) and
  `tools/ghidra/class_layout.py` (the field names of `AnimTree`, `AnimNode`, `SkelControlBase`, `MorphNodeBase`);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock);
- the script declares these classes and the actor event `PostInitAnimTree`, but neither the copy nor the
  initialisation is script.

There is no bl-sdk session and no recorded live evidence on these steps. Raw output stays under ignored
`local/analysis/copytree/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

**Prerequisites, in landing order** (all open except #543):
1. #529, NATIVE_INITIALIZE_BODY_COMPOSITION.md, at `research/rust-body-composition` 386a65a6 (accepted), and #542,
   its implementation;
2. #543, NATIVE_ANIM_NODE_LOOKUP.md, at `research/rust-anim-node-lookup` b09720f6 (accepted);
3. NATIVE_SET_ANIM_TREE_TEMPLATE.md at `research/rust-set-anim-tree-template` 466350b6, and #495, its implementation
   (hub head 8dbc419): its template path is where the copy and the initialisation run;
4. this note.

**Why it matters.** Under #543, Maya's link 1 stops at attachment 0's `PostInitBodyComposition` with
`NotYet { LayerAbsent }` ("an animation-tree instance for the skeletal mesh"), because the port has no instanced
tree. In the game, the instance is made when the mesh component is attached, before that hook runs. The attach takes
the same path as `SetAnimTreeTemplate` with a template: copy the template, then initialise the tree. This note reads
both steps.

## 1. The copy (question a)

The copy is called on the template tree with an outer. `SetAnimTreeTemplate` passes the mesh component
(NATIVE_SET_ANIM_TREE_TEMPLATE.md), so **every copy's outer is the component**. There are two paths.

### 1.1 The pooled path

The engine keeps a global pool of released trees. When the pool exists and a component is given, the copy looks for
a pooled tree whose `AnimTreeTemplate` is this template. If it finds one, it takes it out of the pool and compares
the node, control and morph-node counts of the pooled tree and the template:
- if all three agree, each pooled node gets its template node's values copied over, node by node in gather order
  (section 1.3), and the path joins the common tail (section 1.4);
- otherwise the pooled tree is dropped and the fresh path runs.

Which trees enter the pool, and when, was not read. **Port answer, UNVERIFIED:** the port has no pool, so the fresh
path always runs. A first build of a just-spawned pawn is the case where the pool is least likely to hold a match.

### 1.2 The fresh path

1. **The root** (the tree object itself) is made by a per-class virtual on the template, with the outer. By fit it
   is a copy of the template: step 3 finds the template's child in it. Whether it takes every property of the
   template was not read in full (**UNVERIFIED**, open question 5). The new tree's `AnimTreeTemplate` becomes the
   template's own `AnimTreeTemplate` when that is set, and otherwise the template itself. So a copy of a copy still
   names the original.
2. **The nodes.** The template's nodes are gathered (section 1.3), the root is removed from the list, and each
   remaining node is duplicated, in that order:
   - class: the template node's class;
   - outer: the outer (the component);
   - archetype: **the template node**. So every property the copy does not set reads through to the template node,
     and its class default behind it. That is how a node keeps a `NodeName` the template itself leaves to its class
     default (NATIVE_ANIM_NODE_LOOKUP.md section 4).

   Each new object also gets one post-construction call (its effect was not read). Each pair (template node, new
   node) goes into a map from the template's objects to the copies. When all nodes are made, the `Children[i].Anim`
   links of every new node that is an `AnimNodeBlendBase` are mapped to their copies. A child not in the map stays as
   it is.
3. **The root's first child** (`Children[0].Anim`) is mapped to its copy.
4. **For each new node**, in the same order: its `ParentNodes` is rebuilt from the template node's `ParentNodes`, each
   entry mapped to its copy. Then the node's per-class link fix-up runs with the map. It is a virtual: a class that
   does not override it uses its parent's. It was checked class by class for every node class in Maya's tree
   (review r1 of #551):
   - it does nothing for `AnimNode`, `AnimNodeBlendBase`, `AnimNodeSequence`, `AnimNodeSlot` and every other node
     class of her tree, except one;
   - `WillowAnimNodeAimOffset` has its own. When the copy's `Outer` differs from its template node's, which is always
     so for a copy outered to the component, it sets the copy's `TemplateNode` to the template node, then makes one
     further call that was not read (by fit, a refresh of its aim profiles).

   The fix-ups of classes outside her tree were not checked.
5. **The tree's own fix-up**, with the map:
   - each `AnimGroups` entry's run-time state is cleared: its `SeqNodes` list emptied, `SynchMaster` and
     `NotifyMaster` None;
   - unless `bRebuildAnimTickArray` is set on the copy, it takes the template's cached tick list (`AnimTickArray`), with
     each entry mapped to its copy. This was read only in part; by fit it is the tree's tick-list cache. The flag is
     read here, before step 6 copies it, so its value at this point depends on the root copy of step 1, which was not
     read in full. By fit it equals the template's (UNVERIFIED);
6. **One flag:** the copy's `bRebuildAnimTickArray` is set to the template's.

**Which links are rewired:** `Children[i].Anim` (step 2 for the nodes, step 3 for the root's first child),
`ParentNodes`, and the tree's cached tick list. Links are rewired only
through the map. A link to an object outside the tree, such as an `AnimSet` or a sequence, stays as it is.

### 1.3 The gather order

Nodes are gathered depth first from the root: a node is taken the first time it is reached, then its `Children` in
order, each child's `Anim` in turn. Empty child slots are skipped. A node reached again (it has several parents) is
not taken twice. **This is not the tick order** (section 2). On Maya's tree the two orders differ, because 4 nodes
have more than one parent.

### 1.4 The common tail (both paths): controls and morph nodes

1. **The controls** are gathered from the template's `SkelControlLists` in order: each list's `ControlHead`, then
   its `NextControl` chain. Each control is duplicated as the nodes are (outer the outer, archetype the template
   control) into a second map. Then each copy's `NextControl` is mapped to its copy.
2. **The morph nodes** are gathered from `RootMorphNodes` and their own children (each morph node class's gather step,
   not read in detail). Each is duplicated the same way into a third map, with the same link rewiring.
3. **The heads:** on the new tree, each `SkelControlLists[i].ControlHead` and each `RootMorphNodes[i]` are mapped to
   their copies.

While the copies are made, an engine-wide "duplicating" flag is set and then restored. Its effect is not read.

## 2. The initialisation (question b)

The initialisation is called on the mesh component. It runs when the call forces it, or when the component's
initialised flag (by fit `bAnimTreeInitialised`, which `SetAnimTreeTemplate` clears) is clear. It also needs the
component's `Animations` set and one component field (not identified) clear. In order:

1. **The rebuild flag.** If the tree's `bRebuildAnimTickArray` is set, the tree's nodes of one engine class (not
   identified) lose every child link after the first. Then the flag is cleared. Maya's template does not set it.
2. **The parent lists.** Unless the tree's `bParentNodeArrayBuilt` is set (and no rebuild happened), each node's
   `ParentNodes` is rebuilt from the `Children` links, by each class's own step (not read in detail). Then
   `bParentNodeArrayBuilt` is set. Maya's template sets it, so the copy keeps the `ParentNodes` it got in section 1.2.
3. **The tick list.** It is taken from the tree's cache when an engine setting allows that, the cache is not empty and
   no rebuild happened. Which setting this is was not read. Otherwise it is built:
   - the component's tick counter goes up by one, and its tick list (`AnimTickArray`) is emptied;
   - **the root comes first:** it is appended, its `TickArrayIndex` = its position (0), its `SkelComponent` = the
     component, and its `NodeTickTag` = the counter;
   - **then, depth first from the root:** a node visits its `Children` in order. For each child that is set and not
     already tagged: the child's `SkelComponent` is set to the component, and **the child is appended only when every
     node in its `ParentNodes` already carries the current tag**. When it is appended, it gets its `TickArrayIndex`
     and the tag, and its own children are visited. A child with a parent not yet listed is skipped for now, and the
     visit from its last parent appends it;
   - the built list is stored in the tree's cache.
4. **Each node in tick order** gets `SkelComponent` = the component and `NodeTickTag` = the counter. Then its class's
   per-node initialisation runs: `AnimNode`, `AnimNodeBlendBase`, `AnimNodeSequence`, `AnimTree` and `AnimNodeSlot`
   each have their own, and **none was read**.
5. **What follows was not read:** a pass over the component's own state; the morph nodes' initialisation, one call
   per morph node with the component; and a step that sizes the component's per-bone control arrays from the
   skeletal mesh's bones and the tree's `SkelControlLists`, which needs the mesh asset's data.
6. **The owner's event.** With a tree and an owner, one script event is sent to the owner with the component as its
   argument. By fit it is `PostInitAnimTree`: the actor event that takes a `SkeletalMeshComponent`, and
   `WillowPlayerPawn`'s version does the lookups NATIVE_ANIM_NODE_LOOKUP.md section 1 lists. The name was not read
   (**UNVERIFIED**).
7. The component's initialised flag is set.

**What each node gets:** `SkelComponent`, `TickArrayIndex`, `NodeTickTag` and whatever its class's initialisation
sets (step 4, not read). The component gets its tick list and its counters.

## 3. Maya's attachment 0 (names and counts only)

Her mesh component's template is `GD_PlayerShared.Character.AnimTree_Player` (a `WillowAnimTree`):
- **66 nodes**, the root included: the closure of the root's `Children`. The root has exactly one child slot. 16 child slots are empty, and 4 nodes have
  more than one parent;
- **5 controls** in 4 `SkelControlLists` (bones `L_Hand`, `L_Foot`, `R_Foot`, `R_Hand`);
- **no morph nodes** (`RootMorphNodes` is not set);
- the template is saved with `bParentNodeArrayBuilt` set, `bRebuildAnimTickArray` clear, a saved `AnimTickArray` of
  66 entries, and `AnimGroups` entries whose run-time lists are empty.

**The tick rule against the saved data.** Section 2 step 3, run on the template's `Children`, gives a tick list that
**equals the template's saved `AnimTickArray`, all 66 entries in order**. Each node's saved `TickArrayIndex` is its
position in that list. The `ParentNodes` rebuilt from `Children` equal the saved ones for all 65 non-root nodes. This
is a structural oracle on the template, not a run of the game.

## 4. What this settles in NATIVE_ANIM_NODE_LOOKUP.md (question d)

- **Open question 1** (the runtime tick order is the saved order): **settled by reading.** The tick list is built by
  the rule in section 2 step 3, and on Maya's tree that rule gives the saved order exactly. So among repeated names
  the first match is the one with the lowest saved index, and `SlotNodes` is in the order `SpecialMoveBlend`,
  `TurnBlend`, `WeaponAction`. This is still UNVERIFIED in the game.
- **Open question 3** (the copy keeps class and name): **settled by reading.** Each copy has its template node's class,
  and its template node as archetype (section 1.2), so its names are the template's.
- **Open question 2** (the gather step when the tick list is not the component's): the lookup's fallback and the
  copy call the same per-class gather virtual. Its `AnimNode` and `AnimNodeBlendBase` versions were read (the tree
  uses the latter), and they give the order of section 1.3. Overrides in other node classes were not checked, so for
  those this is by fit (**UNVERIFIED**).

## 5. Proposed Off answer (question c; for a separate implementer)

**Every rule below is UNVERIFIED.** It is the template path of `SetAnimTreeTemplate` (#495's "SetAnimTreeTemplate
with a template" refusal) and of the attach. Like the rest of #529's walk, it is planned in a preflight that writes
nothing and loads nothing, then written.

**Preflight (write-free, loads nothing):**
1. The template, and every node, control and morph node the gather reaches, must already be loaded. An unloaded
   handle refuses `NotYet { LayerAbsent("an animation-tree instance for the skeletal mesh") }`, which is #543's
   refusal, kept unchanged, with nothing loaded.
2. **The link fix-ups** (section 1.2 step 4). The check is per class, by the class's own virtual, inheritance
   included: a subclass that does not override a checked fix-up uses that fix-up. Refuse `NotYet { NotRead }`,
   "CopyAnimTree: the link fix-up of <class>", for:
   - a node class whose fix-up was not checked (any class outside Maya's tree);
   - `WillowAnimNodeAimOffset`, whose fix-up is read except for its last call: "CopyAnimTree: the aim-offset node's
     refresh after its TemplateNode is set";
   - a morph node of any class, since the morph nodes' gather step was not read.
2a. **The root** (review r1 of #551). The root copy comes from an unread per-class virtual (open question 5), and
   section 1.2 step 3 remaps only its first child. A root with more than one child slot refuses `NotYet { NotRead }`,
   "CopyAnimTree: a root with more than one child". Maya's root has exactly one child slot.
2b. **The initialisation's gates** (section 2 steps 1 to 3):
   - the template's `bRebuildAnimTickArray` set: `NotYet { NotRead }`, "the animation tree's rebuild step (the
     unidentified class whose extra children it drops)";
   - the template's `bParentNodeArrayBuilt` clear: `NotYet { NotRead }`, "the animation tree's parent node array
     build".

   **Port answer, UNVERIFIED:** the port always builds the tick list by the section 2 rule. The unidentified gate
   field of the component and the setting that allows the cached tick list are both treated as clear. Maya's template
   passes both gates: `bRebuildAnimTickArray` is clear and `bParentNodeArrayBuilt` is set.
3. **The initialisation's unread steps** (section 2 steps 4 and 5): a typed refusal, unless the hub rules otherwise.
   - The per-node initialisation of every node class: `NotYet { NotRead }`, "the animation tree's per-node
     initialisation". This affects every tree, Maya's included.
   - The per-bone control arrays need the mesh asset's bones: `NotYet { LayerAbsent("a skeletal mesh data reader
     (sockets and the reference skeleton)") }`, the layer #529 already names.

   **For the hub:** these steps write native state on the nodes and the component. Whether any of it is a field that
   the script or a slice step reads was not read. Under NATIVE_ANIM_NODE_LOOKUP.md section 5, rule 4, they stay
   refused until it is.
   One alternative is a ruling that the port leaves them out, labelled UNVERIFIED. Only then does the instance below
   advance link 1.
4. **The owner's event** (section 2 step 6): the event's name was not read. Run it as `PostInitAnimTree` only under a
   hub ruling; otherwise `NotYet { NotRead }`, "the owner's event after the tree initialisation".

**Writes, in order** (only when the preflight passed), all in the VM heap:
1. the new tree object, outer the component, with its `AnimTreeTemplate` set as section 1.2 step 1 says. "Archetype
   the template" is by fit and **UNVERIFIED**: the root copy comes from an unread per-class virtual (open question
   5);
2. one new object per node in gather order (section 1.3; the root excluded), then per control (in `SkelControlLists`
   order and `NextControl` order), then per morph node. Each has outer the component and archetype its template
   object;
3. the links through the maps: `Children[i].Anim`, `ParentNodes`, `NextControl`, `SkelControlLists[i].ControlHead`,
   `RootMorphNodes`, and the tree's cached `AnimTickArray`;
4. the `AnimGroups` run-time lists emptied, and `bRebuildAnimTickArray` copied;
5. the component's `Animations` = the new tree, written by #495's `SetAnimTreeTemplate` path between the copy and
   the initialisation (section 2 needs it set);
6. the initialisation of section 2:
   - no rebuild and no parent-list build: both are refused in the preflight unless the template's flags skip them;
   - the tick list, always built by the section 2 rule (the port answer above);
   - each node's `SkelComponent`, `TickArrayIndex` and `NodeTickTag`;
   - the component's tick list and its initialised flag;
7. the owner's event, if ruled.

**The counters.** Object creation and link writes are heap writes under #518. A refusal raised after any of them
takes the "stopped after partial writes: " prefix exactly when `committed_writes()`, `draws()` or
`load_transitions()` moved. The preflight raises every refusal above before the first write, so none of them takes the
prefix.

**The pool:** absent in the port (section 1.1), so the fresh path always runs. UNVERIFIED.

**Maya's next stop.** Rule 1 passes when her template is loaded. If it is not loaded, that refusal stays her stop, as
in #543. In rule 2, every node class in her tree has a checked fix-up. The one exception is the last call of
`WillowAnimNodeAimOffset`'s (her tree has one such node), so **rule 2's aim-offset refusal is her next stop**. Rules
2a and 2b pass. After them comes rule 3. Reading that refresh call and the per-node initialisation is the next analyst
step. No local probe was run in this lane.

**Reference** keeps the stub policy.

## Open questions

1. **UNVERIFIED:** which trees enter the pool and when (section 1.1). The port runs the fresh path always.
2. **Not read:** the link fix-up of node classes outside Maya's tree, the last call of `WillowAnimNodeAimOffset`'s
   fix-up, and the morph nodes' gather step.
3. **Not read:** the per-node initialisation of each class, the component pass, the morph initialisation and the
   per-bone arrays (section 2 steps 4 and 5).
4. **UNVERIFIED:** the owner event's name (by fit `PostInitAnimTree`).
5. **UNVERIFIED:** how the root copy is made (section 1.2 step 1), and the post-construction call on each copy.
6. **Not identified:** the component field that must be clear for the initialisation to run, the engine class whose
   extra children a rebuild drops, the setting that allows the cached tick list, and the effect of the duplicating
   flag.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On Maya after spawn: `Mesh.Animations` is not the template. Its outer is the mesh component, its
   `AnimTreeTemplate` is `AnimTree_Player`, and its nodes' outers are the component with archetypes the template's
   nodes.
2. `Mesh.AnimTickArray` holds 66 nodes in the template's saved order, each with `TickArrayIndex` its position and
   `SkelComponent` the component.
3. `Pawn.SlotNodes` and the `AimOffset` and `LeftHandIK` fields hold nodes of that instance.
4. A hook on `PostInitAnimTree` fires once per mesh component during the spawn, before the body composition's hook.
