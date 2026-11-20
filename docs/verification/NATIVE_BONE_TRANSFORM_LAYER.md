# The bone-transform layer: a skeletal component's current bone transforms (2026-11-20)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-BONE-TRANSFORM-LAYER (decision 2026-11-19-258, number
pending). Analyst only: a different lane implements it (clean room). This thread wrote the socket note (#636) and
implemented #627/#642 and an earlier SetSkeletalMesh mesh-change branch, which meet this layer; it implements nothing
here.

**Why.** Three notes stop at "the current bone transforms":
- NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md (#636, accepted, **open**: queued for landing batch 10): the socket attach's
  placement branch, run when the parent is itself attached;
- NATIVE_COMPONENT_BOUNDS.md (#639, **open**): a skeletal component's bounds on the physics-asset branch;
- NATIVE_SKELMESH_MESH_CHANGE.md (#635, on main): the mesh change empties the buffer, and only a re-attach refills it.
  Its implementation is #645 (**open**); main still has #599's blanket mesh-change refusal.

This note can land before or after any of them: it changes no code, and each of them refers to this layer by name
only.

This note settles how that buffer is filled, which slice links reach it, and a minimal Off answer. It does not repeat
those notes; they keep their own rules and refer here.

**Sources, in order:**
- no bl-sdk session was granted, and no recorded live evidence covers the buffer;
- the script, from the project's full script decode (`research/script_disasm.py`): the callers of
  `SkeletalMeshComponent.ForceSkelUpdate` and `UpdateAnimations`, and the script's bone readers;
- `Borderlands2.exe`, read locally with `tools/ghidra/run.ps1 -ReadOnly` under the machine lock. The reads were:
  - the execs of `ForceSkelUpdate` and `UpdateAnimations`;
  - the pose update both reach, its reference-pose copy and its compose step;
  - the skeletal component's attach, found among the pose update's callers by what it does (it builds the
    animation tree from `AnimTreeTemplate`, then runs the pose update);
- the installed packages: `SkeletalMeshComponent`'s layout and the `BoneAtom` struct through
  `tools/ghidra/class_layout.py`, with a size assumed for its one map property. Every field read lands on that layout
  with the component's known 16-byte shift: `SkeletalMesh`, `Animations`, `ParentAnimComponent`, `SpaceBases`,
  `LocalAtoms`, `RequiredBones`, `bForceRefpose`, `bSkipUpdateSkelPose`, `bRecentlyRendered`,
  `bForceDiscardRootMotion` and `RootBoneTranslation`. That is the structural check of the fit;
- a local probe of the installed slice gate (section 5), never pushed.

Raw output stays under ignored `local/analysis/btl/`. This note is in our own words: no decompiler output,
pseudo-code, addresses, offsets or listing structure. **Every rule here is UNVERIFIED** in the running game.

## 1. What the layer is

`SpaceBases` is the component's array of **component-space** bone transforms, one `BoneAtom` per bone of its mesh: a
rotation (`Quat`), a translation (`Vector`) and a uniform `Scale` (float). Everything that asks for "the current bone
transform" reads it:
- the socket attach's placement;
- the physics-asset bounds;
- the script's bone readers (`GetBoneLocation`, `GetBoneRotation`, `GetBoneMatrix`, socket world locations and the like:
  38 call lines in the script).

It is built from `LocalAtoms`, the same kind of array holding each bone's transform **relative to its parent**.

## 2. The pose update (one routine)

Every path that fills the buffer runs this one routine. In order:
1. **Skip:** nothing happens when the component has no `SkeletalMesh`, or when `bSkipUpdateSkelPose` is set.
2. `bRecentlyRendered` is refreshed from the world time.
3. **Sizing, only when `ParentAnimComponent` is None:**
   - when `SpaceBases`' length differs from the mesh's bone count, it is resized to the bone count and **every entry
     is set to the identity transform**. A native "the buffer was reset" flag is set;
   - `LocalAtoms` is resized to the bone count, its entries not filled here;
   - one more native array is resized when it disagrees (not read).

   With a `ParentAnimComponent`, the buffer is not sized here: the bones come from the parent (not read).
4. **An empty buffer ends the update.**
5. **Update-rate and not-rendered skips:** a component not recently rendered, or under an update-rate setting, can skip
   the rest. These are several conditions, not read in detail.
6. **The local atoms:**
   - **with no `Animations` (no tree instance), or with `bForceRefpose` set: the reference pose.** For each bone index
     in `RequiredBones`, that bone's local atom is its rotation and position from the mesh's reference skeleton,
     with scale 1;
   - **otherwise:** the tree's root node is asked for its bone atoms, which is the animation tree's evaluation. It is
     not read further; it is the tree's layer, not this one.
7. With `bForceDiscardRootMotion`, the root bone's local atom is reset to the identity. The root's translation away
   from the reference pose is kept in `RootBoneTranslation`.
8. **The compose:** for each required bone, its `SpaceBases` entry is its local atom composed with its parent's
   `SpaceBases` entry. The root's entry is its local atom. With a tree, its skeletal controls are applied during this
   step (not read in detail).
9. Afterwards come render-side updates and the attached components' update (not read).

## 3. Who runs it: the fill paths

| Path | When it runs the pose update | Notes |
|---|---|---|
| **The component's attach** (its registration in a scene) | always, once it reaches that step | Order: render data; then the tree: in a game world with no `Animations` and an `AnimTreeTemplate`, the tree is instanced from the template, otherwise it is re-initialised; then physics; the LOD choice; **the pose update**; each attached entry's update; the transform update |
| **The re-attach of a mesh change** (#635, step 13) | through the attach above | only for a component that was attached |
| **`ForceSkelUpdate`** (script native) | only when `bAttached` | it ticks the tree first when there is one (and a further condition holds, not read), then runs the LOD choice, **the pose update** and the transform update |
| **`UpdateAnimations`** (script native) | never itself | with `Animations` set: sets a tag field (by fit `InitTag`) from the tree's own tag plus one, then calls one virtual on each node of `AnimTickArray`. With no tree, nothing |
| about 25 other native callers | not read one by one | the per-frame tick path is among them (by fit) |

So **an unattached component's `SpaceBases` is never filled.** It keeps whatever it held, and is empty after a mesh
change. **An attached one is filled at attach,** and again on each forced update or tick:
- from the **reference pose** when it has no tree;
- from the **tree** when it has one.

## 4. What the slice meets

1. **The port never sets `bAttached` on a made primitive or skeletal component.** The port's actor attach (#511,
   `ow-inventory` `attach_mesh.rs`) writes `bNeedsReattach` and `bNeedsUpdateTransform` false, `Owner`, and the
   actor's `Components` list, and leaves `bAttached` as it was. Only the spawn's component pass (#605,
   `ow-world` `actors/component_attach.rs`) sets `bAttached` true, and only on copies of the two AI component classes
   (`GearboxFramework.AIComponent` and `WillowGame.WillowAIComponent`), which are not skeletal. So every path in
   section 3 that needs an attached skeletal component is unreachable in the port today.
2. **The six Maya links (1, 3, 4, 5, 13, 14):** the socket attach's placement branch needs the parent attached. The probe
   (section 5) logged attachment 0's `bAttached` **clear** at all eight socket attaches, so the branch is skipped and
   the layer is not reached. In the running game the pawn's components are attached at spawn, so the branch most
   likely runs there (UNVERIFIED). Maya's mesh has a tree, so her buffer would come from the tree, which this layer
   cannot answer.
3. **Links 6 and 7:** #639's skeletal entry is not reached (they refuse at the cylinder entry first).
4. **Link 8:** on main its two mesh changes meet #599's blanket refusal, which names this layer. Under #635's rule (its
   implementation #645, open) the component is unattached, so the change neither needs nor reaches the layer.
5. **Script callers on slice paths:** `WillowPawn.UpdateAnimSets` calls `Mesh.UpdateAnimations`, and
   `WillowWeapon.PlayWeaponEquip` calls `Arms.ForceSkelUpdate`. Both are **unbound** in the port, so each would
   stop as an unbound native once the stops before it lift. Their Off answers are simple (section 6).

**So today no slice link reaches the layer.** The nearest consumers are those two natives and, once the attach of a
skeletal component is ported, the attach itself.

## 5. Probe (local, never pushed)

**Installed Off `--slice-gate`, strict, on main 241a6c47** (with the psk export of Maya's body present): 4 ok / 9
NotYet / 1 error.
- Link 1 runs `maya:8` and stops at "the owner's event after the tree initialisation" (#612); links 3, 4, 5, 13 and 14
  are blocked behind it.
- Links 6 and 7 run `make` and `touch` and stop at `GetBoundingCylinder`'s bounds (#639).
- Link 8 runs `make` and `init` and stops at #599's mesh-change refusal.
- Link 9 is an error at the unbound `GearboxMind.GetAIComponent`.

**With local stand-ins** (behind one environment variable: #612's event, #622's hooks, the socket attach passed and
logged, and the `Parent` link passed), the gate is still 4 / 9 / 1:
- the six links next stop at "stopped after partial writes: AnimNodeSpecialMoveBlend: an owner that does not
  implement SpecialMoveInterface" (NATIVE_ANIM_NODE_INIT.md), not at this layer;
- the log shows the parent's `bAttached` clear at attachments 1 to 6, 9 and 12.

## 6. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** **Reference is unchanged.** The record of each new `NotYet` is this note. Two
separate answers, which must not be merged (review r1 of #646, item 1):
- **6.2, the read:** what a consumer gets when it asks for the current transforms. It is the stored buffer, never a
  computed pose;
- **6.3, the pose update:** what a **fill path** would write. Only the fill paths (section 3) may call it, and none of
  them is ported yet, so it ships with its tests only.

### 6.1 The data: the bind pose in #569's layer

`ow-world` `world/skeletal.rs`, today `SkeletonBone { name, parent }` and `ReferenceSkeleton { bones }`, both deriving
`Eq`:
- **`SkeletonBone` gains `bind: Option<BoneBind>`.** `BoneBind { rotation: [f32; 4], translation: [f32; 3] }` is the
  bone's reference transform relative to its parent: rotation `(x, y, z, w)`, translation in uu, in `ow-assets`'
  convention (`skeletal::BoneTransform`: `p_parent = q p q* + t`, Hamilton quaternions). "Unknown" lives **per bone**:
  `None` means the source gave no bind data for that bone.
- Because `f32` has no `Eq`, `SkeletonBone` and `ReferenceSkeleton` drop `Eq` and keep `PartialEq`. Every
  `Eq`-requiring use must change; none is known in the tree besides the derives.
- **The three struct literals that change** (each gains `bind`):
  - `ow-cli` `cmd/skeletal_source.rs` (the `.psk` source). It fills `bind: Some(..)` from `ow-assets`
    `skeletal::Bone::bind`, which that reader already parses and has checked against UModel's own export (its module
    documentation). That this bind pose equals the mesh's reference skeleton is **UNVERIFIED**;
  - `ow-world` `world/skeletal_tests.rs` and `world/anim_tree_copy_tests.rs`: `bind: None` unless a test needs it.

**The composed reference pose:** `skeletal::reference_space_bases(vm, mesh) -> Result<Rc<Vec<SpaceBase>>, VmError>`,
read-only. Here `SpaceBase { rotation: [f32; 4], translation: [f32; 3], scale: f32 }`.
- **The formula, root first:** for the root, `q_cs = q_local` and `t_cs = t_local`. For any other bone with parent
  `p`, `q_cs = q_cs(p) · q_local` and `t_cs = t_cs(p) + q_cs(p) t_local q_cs(p)*`, where `·` is the Hamilton
  product. The scale is 1 for every bone.
- **Every bone counts as required** (an UNVERIFIED simplification: `RequiredBones` depends on the LOD and is native).
- **Refusals:** any bone with `bind: None` is `NotYet { LayerAbsent }` with #569's layer text (`skeletal::LAYER`).
  The mesh and source checks are #569's own (`reference_skeleton`'s).
- **The cache:** a new field of the `SkeletalMeshes` service beside `reference_skeleton`'s cache: a `DetMap` by mesh
  identity holding the composed `Rc<Vec<SpaceBase>>`. It lives and is cleared with the service, so a new source starts
  with both caches empty. A refusal puts nothing in it. A second ask of the same mesh computes nothing and asks the
  source nothing, because `reference_skeleton` itself is cached.
- **The mapping onto `Object.BoneAtom`**, for a fill path that writes the buffer: `Rotation` is a `Quat` struct `{X, Y,
  Z, W}` from `rotation[0..4]`; `Translation` is a `Vector` `{X, Y, Z}` from `translation`; `Scale` is a float from
  `scale`. The member names and kinds are checked against the declared `Object.BoneAtom` (else `VmError::Script`).

### 6.2 The read: the buffer as stored (for consumers)

`skeletal::stored_space_bases(vm, component) -> Result<Vec<Value>, VmError>`. Read-only, load-free, RNG-free and
intern-free. Its only rule: `SpaceBases` declared on the component's class as an array of `Object.BoneAtom`, else a
`VmError::Script` naming it. It returns the entries **as stored**, possibly empty. It never computes a pose.

The consumers keep their own rules over what it returns. For example, #636's placement checks the bone index
against its length, and #639's physics-asset branch reads each body's bone from it. An unattached component's empty
buffer stays empty to them, as in the native.

### 6.3 The pose update (for fill paths only; tests only for now)

`skeletal::pose_update(vm, component) -> Result<PoseUpdate, VmError>`, write-free. It is the plan a fill path
writes in its own write phase, where `PoseUpdate` is `Skip` (write nothing) or `Write(Rc<Vec<SpaceBase>>)` (the new
`SpaceBases`).

**Field kinds** (review r1 of #646, item 2), each read by name on the component's class. **All seven are checked up
front, before rule 1, `SpaceBases` included, on every call, `Skip` answers included** (review r2 of #646, item 1; the
same pattern as `ow-inventory` `attach_mesh.rs`, which checks its declarations before any rule). Another declaration,
or none, is a `VmError::Script` naming the field, in this list's order:
- `SkeletalMesh`, `Animations` and `ParentAnimComponent`: objects;
- `bSkipUpdateSkelPose` and `bForceDiscardRootMotion`: bools;
- `bForceRefpose`: an **int**, as #511's `SetForceRefPose` declares and writes it (`ow-inventory` `attach_mesh.rs`):
  non-zero means set;
- `SpaceBases`: an array of `Object.BoneAtom` (6.2's check).

**Rules, in this order.** The first that applies is the answer:
1. no `SkeletalMesh`, or `bSkipUpdateSkelPose` set: `Skip`;
2. the mesh not resident (an unloaded or failed handle, or a slotless stand-in): `NotYet { NotRead }`, "the
   bone-transform layer: the mesh is an unloaded handle (a load is a write)". The residency check comes first, as in
   #635's rows 5 and 6, so this answer is `NotRead`. A direct call of #569's functions on an unresident mesh keeps
   #569's own kind (`LayerAbsent`, the store's load);
3. `ParentAnimComponent` set: `NotYet { LayerAbsent("the bone-transform layer's parent copy") }`, because the bones come
   from the parent;
4. `Animations` set and `bForceRefpose` zero: `NotYet { LayerAbsent("the animation tree's pose") }`, because the tree's
   evaluation is not ported. That is Maya's case, and every slice component with a tree;
5. `bForceDiscardRootMotion` set: `NotYet { NotPorted }`, "the bone-transform layer: the discarded root motion" (the
   root reset and `RootBoneTranslation`);
6. otherwise: `Write(reference_space_bases(mesh))` (6.1).

**What a fill path writes** (UNVERIFIED port answer, review r1 of #646, item 7): only `SpaceBases`, set to the
planned entries. The port does **not** write the native's `bRecentlyRendered` refresh, `LocalAtoms` (neither its resize
nor its entries), `RootBoneTranslation`, or the "buffer reset" flag (section 2, steps 2, 3 and 7). The update-rate and
not-rendered skips (step 5) are taken as not skipping. The skeletal controls come only with a tree, which rule 4
refuses.

### 6.4 The two script natives

- **`SkeletalMeshComponent.ForceSkelUpdate()`** in Off, through the keep-existing binding. `bAttached` is declared as
  a bool, else `VmError::Script`:
  - `bAttached` clear: nothing, as in the native. That is every skeletal component the port makes today;
  - `bAttached` set: `NotYet { NotRead }`, "ForceSkelUpdate on an attached component (the tree tick, the LOD choice,
    the pose update and the transform update)". It can be lifted to 6.3 for a tree-less component once the transform
    update has a port answer.
- **`SkeletalMeshComponent.UpdateAnimations()`** in Off. `Animations` is declared as an object, else
  `VmError::Script`:
  - `Animations` None: nothing;
  - `Animations` set: `NotYet { NotPorted }`, "UpdateAnimations: the tree nodes' ticks". The tag write comes first in
    the native, so it is part of the refused step: nothing is written.

**The attach** (registration) of a skeletal component stays unported. Making those components attached is a host
decision beyond this note. Until then, the consumers' placement branches stay unreached in the port, as section 4 says.

### 6.5 Test list

On synthetic classes and an invented skeleton source (`package()` / `tag()`, every value invented):
1. `reference_space_bases` on a three-bone chain with invented bind rotations and translations: each entry follows
   6.1's formula. The root equals its bind transform, every scale is 1, and a 90-degree parent rotation carries the
   child's translation;
2. a bone with `bind: None`: `LayerAbsent` with #569's layer text. A second ask of the same mesh refuses again with
   the same answer, and the composed-pose cache holds no entry for that mesh. A fresh `SkeletalMeshes` service with a
   source that has bind data then computes the pose;
3. the composed-pose cache: a second ask of the same mesh computes nothing (the source is asked once in total);
4. `stored_space_bases`: an empty buffer gives an empty answer (no pose is made); a filled one gives its entries; a
   `SpaceBases` of another kind is `VmError::Script`;
5. `pose_update`, each rule on its own, on a component whose seven fields are all declared with their kinds:
   - no mesh, and `bSkipUpdateSkelPose`, give `Skip`;
   - an unloaded mesh is `NotRead`;
   - `ParentAnimComponent` set is the parent copy's `LayerAbsent`;
   - `Animations` set is the tree's `LayerAbsent`;
   - `Animations` set with `bForceRefpose` 1 gives `Write`;
   - `bForceDiscardRootMotion` is `NotPorted`;
   - a tree-less component gives `Write` with 6.1's entries;
6. the field kinds, checked up front: on a component with `SkeletalMesh` **None** (rule 1 would give `Skip`), each of
   the seven fields in turn declared with a wrong kind (`bForceRefpose` as a bool, `bSkipUpdateSkelPose` as an int,
   `Animations` as an int, `SpaceBases` as an int, and so on), or undeclared, is still `VmError::Script` naming it. With
   two wrong, the first in 6.3's list is reported. A `bForceRefpose` of 2 counts as set;
7. `ForceSkelUpdate`: unattached is nothing (no counter moves); attached is the `NotRead`; `bAttached` of another kind
   is `VmError::Script`;
8. `UpdateAnimations`: no tree is nothing; a tree is `NotPorted`, with the tag unwritten; `Animations` of another kind
   is `VmError::Script`;
9. every refusal, every `VmError::Script` of test 6 and every `Skip` leaves #518's three counters and the name table's
   length unchanged (on test 5's and test 6's fixtures);
10. keep-existing binding in Off, and nothing bound under Reference.

## Open questions

1. **Not read:**
   - the per-frame tick path, among the pose update's other callers;
   - the update-rate and not-rendered skip conditions;
   - the tree's evaluation and the skeletal controls;
   - the copy from a `ParentAnimComponent`;
   - the native array resized beside `LocalAtoms`;
   - `ForceSkelUpdate`'s second tick condition.
2. **UNVERIFIED:**
   - that the pawn's components are attached at spawn in the running game, so that the placement branch runs there;
   - the port answer that a fill path writes only `SpaceBases` (6.3);
   - the `.psk` bind pose as the reference skeleton;
   - every bone as required;
   - `InitTag` as `UpdateAnimations`' tag field (by fit).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read `Mesh.bAttached` and the length of `Mesh.SpaceBases`: expect attached, and one entry per
   bone of her mesh.
2. On a tree-less skeletal component (a dropped gun's mesh, if it has none), compare `SpaceBases` with the reference
   pose composed from the mesh's bones.
3. On an unattached component after `SetSkeletalMesh`, expect `SpaceBases` empty until it is attached.
