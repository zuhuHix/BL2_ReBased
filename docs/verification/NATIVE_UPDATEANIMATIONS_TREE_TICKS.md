# UpdateAnimations' per-node step on Maya's tree (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-UPDATEANIMATIONS-TREE-TICKS (decision 2026-11-26-06), the analyst
half of the clean room: **another lane implements this note.** Raw output stays under ignored
`local/analysis/treeticks/`. This note is in our own words: no decompiler output, pseudo-code, addresses or listing
structure. **Every rule here is UNVERIFIED in the running game.**

**Why.** With the aim-offset rebuild (#745) and the feather blend's own `SetBlendTarget` key, links 1, 3, 4, 5, 13 and
14 stop in `maya:8` at `NotYet { NotPorted }`, "UpdateAnimations: the tree nodes' ticks" (NATIVE_BONE_TRANSFORM_LAYER.md
section 6.4), inside `WillowPawn.UpdateAnimSets`' `Mesh.UpdateAnimations()`. That note marks the per-node step not
ported because it was never read. This note reads it.

**The finding in one line:** the per-node step is **not a timed tick**. It takes no time argument, and on every node
class in Maya's tree but the sequence family it does nothing. On the sequence family it drops the resolved sequence and
keeps the name, the same step the tree's initialisation already runs (NATIVE_ANIM_NODE_INIT.md section 2 D). So a
narrow Off answer exists.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The executable** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - `UpdateAnimations`' exec calls one routine on the component, which the bone-transform note outlined. Read whole
    here: with `Animations` set, it sets one component field (by fit `InitTag`, as that note has it) to the tree's
    `NodeInitTag` plus one, then calls **one virtual on each node of `AnimTickArray`, in order, with no argument**;
  - that slot was read in the class tables of all 31 anim-node classes in the chain of Maya's tree (each class's own
    table, 31 distinct tables, found by `OwNativeQuery`'s `virtual:` form). **28 of them point it at the same empty
    body.** The other three, `AnimNodeSequence`, `WillowAnimNodeSequence` and `WillowAnimNodeSequenceAdditiveBlend`,
    share one short body;
  - that body calls the node's "set animation" virtual through the class table (so a subclass's override runs), and
    `WillowAnimNodeSequenceAdditiveBlend` overrides it. Both "set animation" bodies were read, as were the additive
    one's second call (its "set additive animation") and the component's sequence lookup it can reach;
  - the sequence family's initialisation slot was read beside it, to compare with NATIVE_ANIM_NODE_INIT.md D.
- **Field names:** `tools/ghidra/class_layout.py`. On `AnimNode` the reads land at the computed places up to
  `CachedNumDesiredBones`. From there on, every read on `AnimNode`, `AnimNodeSequence` and
  `WillowAnimNodeSequenceAdditiveBlend` sits exactly **4 bytes past** the computed place. That fits the 16-byte
  alignment of the `BoneAtom` struct `CachedRootMotionDelta` (the tool aligns structs to their largest member). The
  structural oracles:
  - both arrays the sequence step empties land on `CachedBoneAtoms` and `CachedCurveKeys` (its count, allocated size
    and data, in that order);
  - the name it saves and restores lands on `AnimSeqName`, and the two flags it tests land on `bIsIssuingNotifies` and
    `bReverseAnimSeqSearchOrder`;
  - the additive override's flag and name land on `bAutoGenerateAdditiveSeqName` and `AddAnimSeqName`;
  - the old sequence's list it walks lands on `AnimSequence.MetaData`.

  The shift itself is **UNVERIFIED (fitted)**. `SkeletalMeshComponent` and `AnimSet` cannot be laid out by the tool
  (each has a map property), so the component's tag field, its anim-set list and `AnimSet`'s sequence list are named by
  fit only.
- **Maya's data** (`ow-package --exports` / `--properties`, local, counts only):
  - her tree template (`GD_PlayerShared.Character.AnimTree_Player`, in `GD_Siren_Streaming_SF.upk`) holds 99 objects
    of 28 classes. 46 of these template objects are of the sequence family: 42 `AnimNodeSequence`, 1
    `WillowAnimNodeSequence` and 3 `WillowAnimNodeSequenceAdditiveBlend`. The tick list built from the copied tree may
    hold fewer: the port's init class table (`anim_node_init.rs`) has no row for `WillowAnimNodeSequence` and would
    refuse it, yet init passes on her tree, so that node is likely not in the tick list (not checked);
  - no sequence node tags `AnimSeq`, so each holds its archetype's value. NATIVE_ANIM_NODE_INIT.md section 2 D records
    `AnimSeq` None on her sequence nodes at that point;
  - of the three additive nodes, two tag `bAutoGenerateAdditiveSeqName` false, and **one does not tag it**. The class
    default (`Default__WillowAnimNodeSequenceAdditiveBlend` in `WillowGame.upk`) sets it **true**, so that node takes the
    auto-name path below. That node already tags `AddAnimSeqName` = `Add_None`, so the name is in the table and the
    intern is in practice a find; the other two tag `AddAnimSeqName` = `ADD_Idle`.

## 2. What UpdateAnimations does

On the component:
1. **The gate:** `Animations` None does nothing.
2. **The tag:** the component's tag field (by fit `InitTag`) := the tree's (`Animations`') `NodeInitTag` + 1. This
   is the one write before the nodes.
3. **The per-node step,** for each entry of `AnimTickArray` in order, with no time argument:
   - **every class but the sequence family: nothing.** In Maya's tree that is 22 of her 28 classes; three more are
     skeletal-control classes, which are not anim nodes and so not in the tick list;
   - **the sequence family:** the step below (section 3).

Nothing else is written: not the component's pose, `SpaceBases`, the tree's own tags or any timer.

## 3. The sequence step

On an `AnimNodeSequence`, `WillowAnimNodeSequence` or `WillowAnimNodeSequenceAdditiveBlend` node:
1. `AnimSeqName` is kept aside;
2. the node's **"set animation" with the name None**, through the class table:
   - **`AnimNodeSequence`'s** (the other two classes share it, the additive one calls it first in its override):
     - with `bIsIssuingNotifies` set **and** the node's current name not the asked one (here: not None): nothing at
       all. With the flag clear, or the name already None, the steps below run;
     - otherwise `AnimSeqName` := None, `AnimSeq` := None and `AnimLinkupIndex` := -1. With the name None no sequence is
       looked up;
     - only if `AnimSeq` was set before: the node's two caches are emptied (`CachedBoneAtoms`, `CachedCurveKeys`,
       `CachedNumDesiredBones` := 0), and one virtual is called on each non-None object of the old sequence's
       `MetaData` (not read);
   - **`WillowAnimNodeSequenceAdditiveBlend`'s override,** after the base above:
     - with `bAutoGenerateAdditiveSeqName` set: a name is built from the text "Add_" and the asked name's text ("None"
       here), so **`Add_None`, entered in the name table** (a name intern), and `AddAnimSeqName` := it;
     - then, **whatever the flag** (the call comes after the flag's branch, not inside it), its "set additive
       animation" with `AddAnimSeqName` (the auto-built name, or the node's own value when the flag is clear):
       `AddAnimSeqName` := the name, `AddAnimSeq` := None and `AddAnimLinkupIndex` := -1;
     - when the name is not None, the node's `SkelComponent` is set, that component's `SkeletalMesh` is set and its
       anim-set list (by fit `AnimSets`) is not empty: **the component's sequence lookup** (section 4). A found
       sequence becomes `AddAnimSeq`, then its link-up index for the mesh (not read) becomes `AddAnimLinkupIndex`;
3. `AnimSeqName` := the kept name;
4. `CachedNumDesiredBones` := 0.

**Net effect:** the node keeps its name and holds no sequence (link-up -1), unless it is issuing notifies under
another name. Every additive node also re-sets its additive sequence by `AddAnimSeqName` and looks that name up; with
the auto-name flag that name is first set to `Add_<asked name>`.

**A correction to NATIVE_ANIM_NODE_INIT.md section 2 D.** The sequence family's initialisation calls the same "set
animation" with None through the class table, as here. So for `WillowAnimNodeSequenceAdditiveBlend` the initialisation
also runs the override above: the auto-name, the intern and the lookup. That note's D lists the additive class with
the plain sequence and leaves the override out. In the port, the initialisation it implements (#575,
`anim_node_init.rs` kind D) does not write `AddAnimSeqName`, `AddAnimSeq` or `AddAnimLinkupIndex` for any of Maya's
three additive nodes (the override's re-set runs on all three, the auto-name on one). **A second disagreement:** that
note's D ("unless `bIsIssuingNotifies` is set: nothing at all") and the port's kind D skip the reset on the flag alone.
The guard read here also needs the current name to differ from the asked None: a node issuing notifies whose
`AnimSeqName` is None still gets `AnimSeq` := None and `AnimLinkupIndex` := -1 in the game, not in the port. **Both
fixes are the hub's call;** the rules in section 5 are the same for both callers.

## 4. The component's sequence lookup

Asked a name that is not None, on the component:
- the anim-set list in reverse order (last to first), or in order when the asking node's `bReverseAnimSeqSearchOrder`
  is set;
- None entries are skipped;
- for each set, the set's own lookup by name. It goes through a native name-to-index map (by its place, the set's
  `SequenceCache`, a map property no package value fills), and checks that the sequence found has that
  `SequenceName`. A miss is None. A fallback walk over the set's `Sequences` exists behind a global switch (not
  read);
- the first set that answers gives the sequence. No set: None.

## 5. Proposed Off answer (for the implementer)

This replaces `bone_transform`'s `TREE_TICKS` refusal for a tree whose tick list holds only these classes. **Every rule
is UNVERIFIED.** The preflight is write-free and load-free: no store load, no class-code load and nothing interned. A
name is matched by text through the name table's `find`. The step classes are told apart by **exact class**, by
path on loaded classes, as the init note's class table does.

**Preflight, in order** (over the component, then every tick-list node in order):
1. `Animations` declared as an object, else `VmError::Script`. None: nothing.
2. The tree root: resident, and declares `NodeInitTag` as an int; the component declares its tag field. Else the
   store's not-loaded `NotRead` or `VmError::Script`. The tag field's name is a fit: the implementer reuses the
   bone-transform note's `InitTag` reading, labelled UNVERIFIED.
3. Each tick-list entry: None skipped; a store handle not loaded is `NotYet { NotRead }`, "UpdateAnimations: a tick
   node not loaded at set-up (a load is a write)". Its exact class:
   - one of the 28 empty-step classes of section 1 (the list below): nothing;
   - `Engine.AnimNodeSequence` or `WillowGame.WillowAnimNodeSequence`: the plain step;
   - `WillowGame.WillowAnimNodeSequenceAdditiveBlend`: the plain step and the override;
   - any other class: `NotYet { NotRead }`, "UpdateAnimations: the per-node step of <class>".
4. Per sequence node, the fields read are declared with their kinds (`AnimSeqName` a name, `AnimSeq` an object,
   `AnimLinkupIndex` an int, `bIsIssuingNotifies` a bool, `CachedNumDesiredBones` a byte), else `VmError::Script`:
   - `bIsIssuingNotifies` set **and** `AnimSeqName` not None: on the plain step nothing changes, except
     `CachedNumDesiredBones` := 0. For the additive node with the auto-name flag set, the asked name's source is not
     settled in that case: `NotYet { NotRead }`, "UpdateAnimations: the additive auto-name while issuing notifies".
     With `AnimSeqName` None the node is treated as if the flag were clear;
   - `AnimSeq` set: `NotYet { NotRead }`, "UpdateAnimations: the steps after a sequence node's sequence changes" (the
     meta-data call). The init note refuses the same case with its own text.
5. Per additive node, **whatever `bAutoGenerateAdditiveSeqName`**:
   - the declarations (`bAutoGenerateAdditiveSeqName` a bool, `AddAnimSeqName` a name, `AddAnimSeq` an object,
     `AddAnimLinkupIndex` an int);
   - the name looked up: the text `Add_None` with the flag set, else the node's own `AddAnimSeqName`. None: no lookup;
   - the lookup's gate (`SkelComponent`, its `SkeletalMesh`, its anim-set list), read without loading;
   - when the gate passes and the name is not None, the lookup of that name, read only. Each set is resident (else the
     not-loaded
     `NotRead`), each of its `Sequences` resident, matched by `SequenceName` text, case-insensitively as names compare.
     A **match** is `NotYet { NotRead }`, "UpdateAnimations: an additive sequence's mesh link-up". No match: None.
     The set's native map is replaced by that walk, labelled UNVERIFIED (the map's agreement with the list is a fit).

**Writes** (only after the whole preflight; the partial-writes prefix applies to a refusal after the first write, and
by this order none can come):
1. the component's tag field := the root's `NodeInitTag` + 1;
2. per sequence node in tick order: `AnimSeqName` := None, `AnimSeq` := None and `AnimLinkupIndex` := -1 (unless
   `bIsIssuingNotifies` is set and `AnimSeqName` is not None); for **every** additive node: with the auto-name flag,
   intern `Add_None` and `AddAnimSeqName` := it; then `AddAnimSeqName` := that name (or the node's own value),
   `AddAnimSeq` := None and `AddAnimLinkupIndex` := -1; then `AnimSeqName` := the kept name and
   `CachedNumDesiredBones` := 0.

Writes that leave a field at the value it held still count as writes, as in the game.

**The 28 empty-step classes** (each checked in its own class table): `AnimNode`, `AnimNodeBlendBase`, `AnimTree`,
`WillowAnimTree`, `AnimNodeBlend`, `AnimNodeBlendList`, `AnimNodeBlendBySpeed`, `AnimNodeAdditiveBlending`,
`AnimNodeScaleRateBySpeed`, `AnimNodeSpecialMoveBlend`, `WillowAnimNodeFeatherBlend`, `AnimNodeBlendDirectional`,
`AnimNodeBlendByPosture`, `AnimNodeBlendByPhysics`, `AnimNodeBlendPerBone`, `AnimNodeAimOffset`,
`WillowAnimNodeAimOffset`, `AnimNodeSlot`, `WillowAnimNodeSlot`, `WillowAnimNodeBlendDirectional`,
`WillowAnimNodeBlendInjured`, `WillowAnimNodeBlendThirdPersonMenu`, `WillowAnimNode_Falling`,
`WillowAnimNode_AddCameraBone`, `WillowAnimNode_AimState`, `WillowAnimNode_ClimbLadder`, `WillowAnimNode_Condition`,
`WillowAnimNode_WeaponRecoil`. Their packages qualify them as in the init note's table (`Engine.`, `GearboxFramework.`
for `AnimNodeSpecialMoveBlend`, `WillowGame.` for the `Willow` ones).

**What Maya's tree does** (a prediction from the data, not a run): the tag; nothing on her empty-step nodes; on each
sequence-family node in her tick list (at most the 46 template objects of section 1) `AnimSeq` is None and
`bIsIssuingNotifies` clear, so the plain step's writes. On her three additive nodes, the re-set and the lookup over
her mesh's anim sets after `UpdateAnimSets` appended them:
- the auto-name node (`_76`): `Add_None`, already in the name table from its own tag, so the intern is a find. No
  sequence of hers is named `Add_None` (an expectation, not checked), so `AddAnimSeq` stays None;
- the two flag-false nodes (`_1`, `_10`): `ADD_Idle`, their tagged value. In the local name dump only
  `Anim_Siren.RocketLauncher_Siren` holds a sequence of that name. If that set is on her mesh's list at this point, the
  lookup matches and the stop is the link-up `NotRead`; if not, `AddAnimSeq` stays None. Which sets the list holds was
  not read (the holds' anim-set lists did not decode).

## 6. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each refusal leaves #518's three counters, the class-code loads and the name
table unchanged:
1. no tree: nothing; a tree with an empty tick list: only the tag (root's `NodeInitTag` + 1);
2. a tick list of empty-step classes (a toy `AnimNodeBlendList` and `WillowAnimNodeFeatherBlend`): only the tag;
3. a plain sequence node with `AnimSeq` None: `AnimSeqName` unchanged at the end, `AnimSeq` None,
   `AnimLinkupIndex` -1, `CachedNumDesiredBones` 0;
4. the same with `bIsIssuingNotifies` set and `AnimSeqName` a non-None name: `AnimSeq` and `AnimLinkupIndex`
   unchanged, `CachedNumDesiredBones` 0; with `bIsIssuingNotifies` set and `AnimSeqName` None: the writes of test 3
   (`AnimLinkupIndex` from a non -1 value to -1);
5. `AnimSeq` set: the meta-data `NotRead`, nothing written (the tag included);
6. an additive node with the auto-name flag clear, a toy `AddAnimSeqName` and no `SkelComponent`: the plain step, then
   `AddAnimSeqName` unchanged, `AddAnimSeq` None, `AddAnimLinkupIndex` -1, nothing interned; with it set and no
   `SkelComponent`: `AddAnimSeqName` `Add_None`, `AddAnimSeq` None, `AddAnimLinkupIndex` -1. The fixture's name
   table lacks `Add_None`, so the intern is seen once, at the write (in Maya's data it is a find);
7. the lookup, for an auto-name node and for a flag-clear node with a toy `AddAnimSeqName`: a mesh with two anim
   sets, the second holding a sequence of the looked-up name: the link-up `NotRead`; with
   `bReverseAnimSeqSearchOrder` set and the match only in the first set, the same; no match anywhere: None. A
   flag-clear node with `AddAnimSeqName` None: no lookup, so a lazy anim set is not read. A lazy anim set otherwise:
   the not-loaded `NotRead`;
8. an unknown node class in the tick list: its `NotRead`, nothing written;
9. a lazy tick-list entry: the not-loaded `NotRead`;
10. malformed declarations (`AnimSeqName` not a name, `NodeInitTag` not an int): `VmError::Script`;
11. Reference: unchanged.

## 7. What maya:8 meets next (a prediction, not a run)

After `UpdateAnimations`, `UpdateAnimSets` returns and `ApplyBodyClassStartingValues` goes on
(NATIVE_LINK1_TAIL_SETTERS.md section 1): `SetupSkeletalMeshPhysics`, its five collision setters (bound, #739), then
`InitializeRBPhysics` (bound, #742). Her components are in the attach record, so **the predicted next stop is
`InitializeRBPhysics`' physics `LayerAbsent`**, "WillowPawn.InitializeRBPhysics: a component's rigid body".
- the stop is earlier if the lookup meets something not resident. `GetWeaponHoldDef`'s port (`weapon_hold.rs`)
  already refuses a hold definition that is not resident, so the holds are loaded before this point; what can remain
  is an anim set on her mesh's list that is a store handle (a hold's set not loaded with its hold, or a set from the
  mesh template's own list), or a resident set whose `Sequences` hold a handle. Then the stop is the lookup's
  not-loaded `NotRead` on her first additive node in tick order. Preloading those would be a harness input, **the
  hub's call**;
- if `ADD_Idle` matches in a set on her list (section 5's prediction), the stop is the link-up `NotRead`;
- if a collision setter refuses first (a body present, a reattach refusal), that setter's refusal.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **By fit:** the 4-byte shift past `CachedNumDesiredBones`, and the component's tag field, anim-set list and the
   set's sequence list.
3. **Not read:**
   - the virtual called on the old sequence's `MetaData` objects;
   - the found sequence's mesh link-up;
   - the lookup's fallback switch;
   - which name the additive override builds from when `bIsIssuingNotifies` keeps the old name (issuing, name not
     None);
   - which anim sets her mesh's list holds at `UpdateAnimations` (it decides whether `ADD_Idle` matches).
4. **The hub's call:** NATIVE_ANIM_NODE_INIT.md D's missing additive override and its issuing-notifies guard (section
   3), and an anim-set preload if the lookup needs one.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read her additive sequence nodes' `AddAnimSeqName` and `AddAnimSeq`. Expect `Add_None` on the
   one with the class default's flag, and the template's `ADD_Idle` on the other two; `AddAnimSeq` set on those two
   only if a set on her list holds `ADD_Idle`.
2. Read a sequence node's `AnimSeq` right after `UpdateAnimSets`. Expect None until the first real tick binds it.
