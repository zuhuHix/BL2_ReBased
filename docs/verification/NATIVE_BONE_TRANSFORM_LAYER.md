# The bone-transform layer: a skeletal component's current bone transforms (2026-11-20)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-BONE-TRANSFORM-LAYER (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room). This thread implemented #627/#642 and #641 and
wrote the socket note (#636), which end at this layer; it implements nothing here.

**Why.** Three accepted notes stop at "the current bone transforms":
- NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md (#636): the socket attach's placement branch, run when the parent is itself
  attached;
- NATIVE_COMPONENT_BOUNDS.md (#639): a skeletal component's bounds on the physics-asset branch;
- NATIVE_SKELMESH_MESH_CHANGE.md (#635): the mesh change empties the buffer, and only a re-attach refills it.

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

1. **The port never sets a made component's `bAttached`.** The actor attach writes `Owner` and the `Components` list
   only, so every path in section 3 that needs an attached component is unreachable in the port today.
2. **The six Maya links (1, 3, 4, 5, 13, 14):** the socket attach's placement branch needs the parent attached. The probe
   (section 5) logged attachment 0's `bAttached` **clear** at all eight socket attaches, so the branch is skipped and
   the layer is not reached. In the running game the pawn's components are attached at spawn, so the branch most
   likely runs there (UNVERIFIED). Maya's mesh has a tree, so her buffer would come from the tree, which this layer
   cannot answer.
3. **Links 6 and 7:** #639's skeletal entry is not reached (they refuse at the cylinder entry first).
4. **Link 8:** on main its two mesh changes meet #599's blanket refusal, which names this layer. Under #635's rule (#641)
   the component is unattached, so the change neither needs nor reaches the layer.
5. **Script callers on slice paths:** `WillowPawn.UpdateAnimSets` calls `Mesh.UpdateAnimations`, and
   `WillowWeapon.PlayWeaponEquip` calls `Arms.ForceSkelUpdate`. Both are **unbound** in the port, so each would
   stop as an unbound native once the stops before it lift. Their Off answers are simple (section 6).

**So today no slice link reaches the layer.** The nearest consumers are those two natives and, once attachment is
ported, the attach itself.

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

**Every rule below is UNVERIFIED.** **Reference is unchanged.** The record of each new `NotYet` is this note.

### 6.1 The layer: a read-only reference pose

Add to #569's skeletal-mesh data layer (`ow-world` `world/skeletal.rs`):
- **the bind pose per bone.** `SkeletonBone` gains each bone's reference rotation and position relative to its
  parent. ow-cli's `.psk` source already parses them: `ow-assets` `skeletal::Bone::bind`, "relative to the parent".
  That the export's bind pose equals the mesh's reference skeleton is **UNVERIFIED** (the export's mirroring is
  undone by the reader, as for the mesh layer). A source without bind data answers "unknown", which is
  `LayerAbsent` with the layer's own text.
- **`skeletal::reference_space_bases(vm, mesh) -> Result<Vec<BoneAtom>, VmError>`**, read-only, cached by mesh
  identity like `reference_skeleton`. It composes each bone's bind transform onto its parent's composed one, root
  first, with scale 1. It is section 2's steps 6 (reference pose) and 8 with no tree and no skeletal controls. Every
  bone counts as required: an UNVERIFIED simplification, because `RequiredBones` is LOD-dependent and native.

### 6.2 Who may use it, and when it refuses

A consumer that needs the component's current transforms (the three notes above, and the natives below) asks a
single gate, `skeletal::current_space_bases(vm, component)`. Its rules, in this order, are write-free, load-free and
RNG-free:
1. no `SkeletalMesh`, or `bSkipUpdateSkelPose` set: the caller's own "no update" case. The buffer stays as it is,
   and the gate returns the component's current `SpaceBases`, read only;
2. the mesh unresident: `NotYet { NotRead }`, "the bone-transform layer: an unloaded mesh (a load is a write)";
3. `ParentAnimComponent` set: `NotYet { LayerAbsent("the bone-transform layer's parent copy") }`, because the bones
   come from the parent;
4. `Animations` set and `bForceRefpose` clear: `NotYet { LayerAbsent("the animation tree's pose") }`, because the
   tree's evaluation is not ported. **That is Maya's case**, and every slice component with a tree;
5. otherwise, the reference pose of 6.1, as the buffer's new value. The caller writes it into `SpaceBases`, in its own
   write phase. The `SpaceBases` declaration must be an array of `Object.BoneAtom` (else `VmError::Script`).

**Not ported, and refused where they would matter:**
- the update-rate and not-rendered skips (section 2, step 5) are taken as not skipping;
- `bForceDiscardRootMotion`'s root reset and `RootBoneTranslation`: with the flag set, `NotYet { NotPorted }`, "the
  bone-transform layer: the discarded root motion";
- the skeletal controls: they come only with a tree, which rule 4 already refuses.

### 6.3 The two script natives

- **`SkeletalMeshComponent.ForceSkelUpdate()`** in Off, through the keep-existing binding:
  - `bAttached` clear: nothing, as in the native. That is every component the port makes today;
  - `bAttached` set: `NotYet { NotRead }`, "ForceSkelUpdate on an attached component (the tree tick, the LOD choice,
    the pose update and the transform update)". It can be lifted to rule 5 of 6.2 for a tree-less component once the
    transform update has a port answer.
- **`SkeletalMeshComponent.UpdateAnimations()`** in Off:
  - `Animations` None: nothing;
  - `Animations` set: `NotYet { NotPorted }`, "UpdateAnimations: the tree nodes' ticks". The tag write comes first in
    the native, so it is part of the refused step: nothing is written.

**The attach** (registration) stays unported. Making components attached is a host decision beyond this note. Until
then, the consumers' placement branches stay unreached in the port, as section 4 says.

### 6.4 Test list

On synthetic classes and an invented skeleton source (`package()` / `tag()`, every value invented):
1. `reference_space_bases` on a three-bone chain with invented bind rotations and positions: each component-space
   transform is the parent's composed with the bone's own. The root equals its bind transform, and every scale is 1;
2. a source without bind data: `LayerAbsent` with the layer's text; nothing cached, nothing written;
3. the gate, each rule on its own: no mesh, and `bSkipUpdateSkelPose`, give the current buffer unchanged; an unloaded
   mesh is `NotRead`; `ParentAnimComponent` set is the parent-copy `LayerAbsent`; `Animations` set is the tree's
   `LayerAbsent`; `Animations` set with `bForceRefpose` gives the reference pose; `bForceDiscardRootMotion` is
   `NotPorted`;
4. a `SpaceBases` declared as another kind: `VmError::Script`, before any write;
5. `ForceSkelUpdate`: unattached is nothing (no counter moves); attached is the `NotRead`;
6. `UpdateAnimations`: no tree is nothing; a tree is `NotPorted`, with the tag unwritten;
7. every refusal leaves #518's three counters and the name table's length unchanged;
8. keep-existing binding in Off, and nothing bound under Reference;
9. the cache: a second ask of the same mesh parses nothing (the source is asked once).

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
   - the `.psk` bind pose as the reference skeleton;
   - every bone as required;
   - `InitTag` as `UpdateAnimations`' tag field (by fit).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read `Mesh.bAttached` and the length of `Mesh.SpaceBases`: expect attached, and one entry per
   bone of her mesh.
2. On a tree-less skeletal component (a dropped gun's mesh, if it has none), compare `SpaceBases` with the reference
   pose composed from the mesh's bones.
3. On an unattached component after `SetSkeletalMesh`, expect `SpaceBases` empty until it is attached.
