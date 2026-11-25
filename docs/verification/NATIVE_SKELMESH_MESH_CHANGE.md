# SkeletalMeshComponent.SetSkeletalMesh: the mesh change (2026-11-20)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-SKELMESH-MESH-CHANGE (decision 2026-11-19-242, number
pending). **Analyst only: a different lane implements this note (clean room).**

This note covers what [NATIVE_SET_SKELETAL_MESH.md](NATIVE_SET_SKELETAL_MESH.md) (#583) left at one refusal: a call
whose `NewMesh` differs from the component's current mesh. That note keeps the callers, the arguments and the same-mesh
rule (a no-op), which #599 implements. Its section 2.2 is corrected here (section 2).

**Sources:**
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock): the
  exec of `SetSkeletalMesh`, the routine it calls and that routine's callees;
- a local, uncommitted probe of the installed slice gate (section 5).

`tools/ghidra/class_layout.py` cannot lay out `SkeletalMeshComponent` (it holds a map property), so the fields below
are named **by fit** from the script-visible declarations (UNVERIFIED). The probe confirmed which of them the port's
layout declares (section 5). There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored
`local/analysis/smmc/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED** in the running game.

## 1. The question

> What does a mesh change on a component with no bone transforms do: which fields does it read and write, and does it
> rebuild from the new mesh's reference pose or from the old transforms?

**Answer: neither.** The mesh change empties the component's transform buffer. Neither the reference pose nor the old
transforms is copied into it. **On an unattached component the buffer stays empty after the call**: it is refilled
only by the pose update that a later attach or tick runs (section 3). On an **attached** component, the call's own
re-attach (section 2, step 13) runs that pose update before it returns, so the buffer is refilled within the call, by
the attach side's rule and not by this native's. A saved copy of the old transforms is copied back by bone name **only if** something
refilled the buffer within the same call. That needs `bKeepSpaceBases`, a current mesh and an attached component. On a
component with no transforms (an empty buffer), nothing is saved, and the buffer is still empty after the change.

## 2. What the native does on a different mesh (in order)

Each step is by fit. The first test is unchanged from #583: when `NewMesh` is the current `SkeletalMesh` (None
included), it does nothing.

1. **Save.** With `bKeepSpaceBases` and a current mesh, the current transform buffer (`SpaceBases`) is copied to a local
   save.
2. **Physics.** The component's physics-asset instance, when there is one, is terminated and cleared.
3. **The old origin.** An origin-compensation matrix is built from the **old** mesh's origin offset and origin rotation
   (by fit `Origin` and `RotOrigin`). With no old mesh, or under one component flag (not identified), it is the identity.
4. **The tree flag.** `bAnimTreeInitialised` is cleared: the same flag the tree initialisation sets
   ([NATIVE_COPY_ANIM_TREE.md](NATIVE_COPY_ANIM_TREE.md) section 2).
5. **Deferred detach.** A scoped detach starts. Its end (step 13) re-attaches only a component that was attached.
6. **The mesh.** `SkeletalMesh` = `NewMesh`.
7. **The buffer.** `SpaceBases` is emptied.
8. **The tree's per-bone state** (only when `Animations` is an animation tree):
   - emptied when `bKeepSpaceBases` is false, when the tree's per-bone count differs from the old mesh's bone count, or
     when `NewMesh` is None;
   - otherwise rebuilt by matching bone names against the new mesh's reference skeleton (read through its bone-name
     search).
9. **The transform.** The component's local-to-world matrix is recomputed from its current value: the old
   compensation (step 3) is taken back out, and the new mesh's compensation (the same rule, from the new mesh's origin
   offset and rotation) is put in. Its inverse and its determinant's sign are stored beside it.
10. **The parent bone map.** Rebuilt from `ParentAnimComponent`'s mesh when that is set (a bone-name match), else
    emptied.
11. **The physics bodies flag.** `bHasValidBodies` is recomputed: set when one of the `PhysicsAsset`'s bodies names a
    bone the new mesh has; clear with no `PhysicsAsset` or no mesh.
12. **One native field** (not identified, no script declaration found) is zeroed.
13. **Deferred detach ends.** A component that was attached is re-attached, and the attach runs the pose update that
    refills `SpaceBases` (section 3). An unattached component stays detached, with its buffer empty.
14. **Restore.** If step 1 saved transforms and the buffer now holds the new mesh's bones, the saved ones are copied
    back by bone name. With an empty buffer there is nothing to copy into.
15. **The morph map.** The component pass runs, the same routine as the tree initialisation's
    ([NATIVE_COPY_ANIM_TREE.md](NATIVE_COPY_ANIM_TREE.md) section 2, step 5). It empties the morph-target index map
    (`MorphTargetIndexMap`) and refills it from `MorphSets`. With an empty `MorphSets` the map stays empty.
16. **LOD info.** With a new mesh, when the component's `LODInfo` length differs from the mesh's LOD count, `LODInfo` is
    emptied and refilled with one default entry per LOD: no hidden materials, both flags clear, and the index -1. Then,
    for each LOD whose model has data of a kind not identified, the entry's usage byte is copied from that LOD model
    and its index is set to 0 (fitted: `InstanceWeightUsage` and `InstanceWeightIdx`). **This per-LOD loop runs
    whether or not the lengths matched**, over every LOD of the new mesh. With a None mesh, `LODInfo` is left as it
    is.
17. **Notification.** A global object's virtual is called with the component (not identified; by fit, the
    render-scene update).

**What it reads:**
- from the component: its mesh, flags, buffer, tree, `ParentAnimComponent`, `PhysicsAsset`, `MorphSets`, `LODInfo`
  and transform;
- from the old mesh: its origin offset and rotation, and its bone count;
- from the new mesh: its origin offset and rotation, its bones (count and names, only on the paths named above) and
  its LOD models.

It draws nothing, and it loads nothing of its own, so both meshes must already be readable.

**Correction to NATIVE_SET_SKELETAL_MESH.md** (#583, section 2.2, step 8): the mesh change does **not** rebuild the
bone transforms from the new mesh's reference pose. Step 7 above empties the buffer, and only step 13's re-attach of an
attached component, or a later attach or tick, refills it. That note now points here.

## 3. Where this meets ANALYST-BODYCOMP-SOCKET-BONE-TABLES

That lane (c376) covers `AttachComponent`, the attachments, and the socket and bone tables. The scope split:
- **The pose update that refills `SpaceBases`** belongs to the attach side, not to this native. This note stops where
  step 13 calls into it, or where an unattached component stays detached. A port of step 13 for an **attached**
  component needs that lane's rule for what the attach writes.
- **The bone tables.** Steps 8 (rebuild), 10, 11 and 14 read the new mesh's reference skeleton by bone name: the same
  skeletal-mesh data layer (#569's `reference_skeleton()`) that the socket tables use. This note names those steps'
  inputs; the lookup itself is that layer's.
- Neither note covers the other's writes.

## 4. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces #599's single mesh-change refusal in
`crates/ow-world/src/world/skeletal_mesh.rs`. **Reference is unchanged.** Each new `NotYet` carries this note as its
record: `docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md`. The rows below are **the one authoritative Off order**
(review r1 of #635, item 1). The first that applies is the answer. Every row reads only (write-free, load-free,
RNG-free; `vm.space.prop` / `vm.space.read`), and nothing is written until every row has passed.

**A. #599's existing order, unchanged:**
1. the receiver resident, else #599's `NotRead`;
2. the arguments decoded: `NewMesh` an object (None allowed); `bKeepSpaceBases` through `NativeCall::has`, false when
   omitted, else a bool. Any other shape is #599's `VmError::Script`. **The arguments come before the current mesh slot
   is read;**
3. the current `SkeletalMesh` read: declared as an object field, else #599's `VmError::Script`;
4. equal meshes (both None, or the same handle): nothing, #599's no-op.

**B. The meshes' residency** (the native reads the old mesh at step 3 and the new one from step 8 on):

5. the old mesh, when set: resident, else `NotYet { NotRead }`, "SetSkeletalMesh: the old mesh is an unloaded handle (a
   load is a write)". An unloaded or a failed handle refuses alike;
6. the new mesh, when set: resident, else `NotYet { NotRead }`, "SetSkeletalMesh: the new mesh is an unloaded handle (a
   load is a write)". **A clear (`NewMesh` None) skips this row.**

**C. The unported paths, in the native's step order** (section 2's step number in brackets):

7. [1] `bKeepSpaceBases` with a current mesh and a non-empty `SpaceBases`: `NotYet { LayerAbsent(BONES_LAYER) }`,
   "SetSkeletalMesh: keeping the bone transforms over a mesh change". An empty buffer saves nothing, so it passes;
8. [2] a set `PhysicsAssetInstance`: `NotYet { LayerAbsent("a skeletal physics instance") }`;
9. [5] `bAttached` true: `NotYet { NotRead }`, "SetSkeletalMesh: an attached component's detach and re-attach (the
   pose update; see ANALYST-BODYCOMP-SOCKET-BONE-TABLES)". **The attached check sits here, at the deferred detach's
   start.** One test covers both halves: the detach at step 5 and the re-attach with its pose update at step 13 happen
   only for an attached component, and the refusal names the pair;
10. [8] a set `Animations`: `NotYet { NotPorted }`, "SetSkeletalMesh: the animation tree's per-bone state on a mesh
    change". The port's tree instance (#561/#575) does not hold that array as a field this rule can write;
11. [9] **the transform** (the hub's ruling, 05:35Z, UNVERIFIED). The probe found no `LocalToWorld` declaration on the
    port's layout, so the transform is native-only in the port. Read the origin offset and origin rotation of each mesh
    that is set: the old one, then the new one.
    - **Expected kinds** (by fit, `Engine.SkeletalMesh`): `Origin` a `Vector` struct (`X`, `Y`, `Z` floats), and
      `RotOrigin` a `Rotator` struct (`Pitch`, `Yaw`, `Roll` ints). Another declaration, or none, is a
      `VmError::Script` naming the field.
    - **Zero** means every member equals 0; `-0.0` counts as zero.
    - Either mesh non-zero: `NotYet { LayerAbsent("a component transform") }`, "SetSkeletalMesh: the component
      transform's origin compensation (a mesh with a non-zero origin)".
    - Both zero, or None: the transform is answered as nothing.
    - Step 3's unidentified component flag is not read, so a non-zero origin refuses even when the flag is set;
12. [10] a set `ParentAnimComponent`: `NotYet { LayerAbsent("the skeletal mesh data layer's bone names") }`, the parent
    bone map. It can be lifted through #569's `reference_skeleton()` once that answers without a load;
13. [11] a set `PhysicsAsset`: the same layer refusal, for `bHasValidBodies`;
14. [15] a non-empty `MorphSets`: `NotYet { NotPorted }`, "SetSkeletalMesh: the morph-target index map". A
    `MorphTargetIndexMap` that reads as anything but None is the same refusal. None is the only empty value the port
    is known to hold (section 5), and the native empties the map whatever it held;
15. [16] **`LODInfo`**, a new mesh only (**a clear skips this row**: the native leaves `LODInfo` alone with a None
    mesh). The native reads the mesh's LOD count, and for each LOD a usage byte from that LOD's model. Both are native
    mesh data with no script declaration, so they can come only from the skeletal-mesh data layer (#569):
    - **the layer gives no LOD count** (today's state: it does not expose the LOD models):
      `NotYet { LayerAbsent("the skeletal mesh data layer's LOD models") }`. `LayerAbsent`, not `NotRead`, because the
      native behaviour is read (section 2) and what is missing is the port's layer for the data. Nothing is unread or
      unloaded;
    - **a known count of zero** and an empty `LODInfo`: nothing (no resize, and no per-LOD update with zero LODs);
    - **any other known count** (positive, or zero against a non-empty `LODInfo`): the same `LayerAbsent`. A positive
      count runs the per-LOD usage update **whether or not the lengths match**, because the native runs that loop over
      every LOD after the resize test. Each entry's byte comes from the LOD model, which the layer does not expose
      either.

**D. The destinations.** Every planned write's field is checked before any is committed (review r1 of #635, item 6): the
field is declared with the kind below on the component's class, and the component is the resident receiver. Another
declaration, or none, is a `VmError::Script` naming the field, raised here with nothing written.

**Writes**, only after rows 1 to 15 and D have passed: **four**, in the native's order:
1. [4] `bAnimTreeInitialised` = false (a bool);
2. [6] `SkeletalMesh` = `NewMesh` (an object);
3. [7] `SpaceBases` = the empty array (an array);
4. [11] `bHasValidBodies` = false (a bool). Row 13 has refused every case where it would be set.

Each is written even when the value is unchanged, because the native writes it. **Not written:**
- `MorphTargetIndexMap`: row 14 lets it through only as None, the port's empty value, so the native's emptying leaves
  it as it is;
- `LODInfo`: row 15 lets it through only when nothing changes;
- the transform, step 12's field and step 17's notification: native-only, with no write proposed (row 11, open
  questions).

**After writes started:** a `NotYet` raised despite the preflight is marked "stopped after partial writes: " by #518's
counters, with its kind and record kept.

**The clear** (a set mesh to None):
- rows 6 and 15 are skipped, and the new mesh's origin counts as zero in row 11;
- the old mesh is still read: row 5 (resident) and row 11 (its origin);
- so an unloaded old mesh refuses `NotRead`, a zero-origin old mesh gives the four writes (`SkeletalMesh` None), and a
  non-zero one refuses the transform's `LayerAbsent`;
- link 8 makes no such call: its resets are None to None (#583 section 5).

## 5. What link 8 meets (local probe, uncommitted)

The probe ran on a local, unpushed merge of main 4c4522d7 with #577, #599, #596 and #613. Its throwaway stand-ins:
- a gestalt preload, which loads the definitions `BuildCompositeMesh` reads (seven handles named, eight loads counted
  with the gestalt definition);
- a stub for #599's mesh-change refusal, which wrote only `SkeletalMesh` and logged the component's state.

**Installed Off `--slice-gate` strict:**
- **Without stand-ins:** 4 ok, 10 NotYet, 0 errors. Link 8 runs 2 steps (`make`, `init`) and stops in `init` at
  "BuildCompositeMesh: a definition it reads is not loaded (a load is a write)", marked after partial writes.
- **With both stand-ins:** 4 ok, 9 NotYet, 1 error. Link 8 runs `make` and `init`. In `init` it passes
  `BuildCompositeMesh` and both `SetSkeletalMesh` calls (stop 4, stubbed), then stops with an error at the next
  unbound native, **`PrimitiveComponent.SetScale(float)`** (link 8 moves from NotYet to error). Every other link is as
  before.

**The two mesh changes link 8 reaches** (first-person, then third-person component) are the same case:
- from None to the composite mesh `GestaltDef_AssaultRifle_GestaltSkeletalMesh`, with `bKeepSpaceBases` false;
- the component is not attached, with no physics instance, no `ParentAnimComponent`, no `PhysicsAsset`, an empty
  `MorphSets`, no `Animations`, no `AnimTreeTemplate` and no `Owner`;
- `SpaceBases` and `LODInfo` are declared and empty, and `bAnimTreeInitialised` and `bHasValidBodies` are declared and
  false. The morph-target index map is declared and read as None. `LocalToWorld` is not declared on the port's layout;
- **the new mesh is an unloaded handle** (an `Engine.SkeletalMesh`).

**So under section 4, link 8 stops first at row 6**, the new mesh's `NotRead`. What would lift it: the mesh resident
before the run, either through a harness preload as with the gestalt definitions (an UNVERIFIED harness input, a hub
ruling), or through a ported composite-mesh build that produces a resident mesh. Then rows 7 to 10 and 12 to 14 pass on
the logged state.
- Row 11 refuses if the composite mesh's `Origin` or `RotOrigin` is non-zero. Those values were not read for this note.
- Row 15 refuses `LayerAbsent` today in any case, because the data layer gives no LOD count. That the installed mesh
  has a positive LOD count is a fit, not an observation: **UNVERIFIED**.

With all of those lifted, the next stop is `PrimitiveComponent.SetScale(float)`.

## Tests for the implementer

Synthetic packages through `package()`/`tag()`. Each refusal leaves the heap writes, draws and load transitions
(#518's counters) and the name table's length unchanged, and every component slot as it was:
1. #599's tests unchanged: an unloaded receiver, the argument shapes, the same mesh (None included) as a no-op. A
   malformed argument is reported ahead of a malformed `SkeletalMesh` declaration (row 2 before row 3);
2. rows 5 and 6: an unloaded or a failed old mesh (with a resident new one), and an unloaded or a failed new mesh (with
   no old one), each `NotRead` with nothing loaded. With both unloaded, the old mesh's refusal is reported;
3. each row of C on its own (7 keep with a non-empty buffer, 8 a physics instance, 9 attached, 10 a tree, 11 a non-zero
   `Origin` and, separately, a non-zero `RotOrigin`, on the new mesh and then on the old, 12 a parent component, 13 a
   physics asset, 14 morph sets, and a `MorphTargetIndexMap` that is not None): its refusal, before any write;
4. **mixed cases** (review r1 of #635, item 1): a non-zero origin together with a LOD blocker (a layer with no count)
   reports the transform's `LayerAbsent` (row 11 before row 15); attached with a tree reports row 9; morph sets with a
   LOD blocker report row 14;
5. keep with a current mesh and an **empty** buffer: no refusal from row 7;
6. row 11's declarations: `Origin` declared as another kind, or `RotOrigin` undeclared, is a `VmError::Script` naming
   it, before any write; `-0.0` members count as zero;
7. row 15: with no layer answer, `LayerAbsent`; with a layer answer that gives a **known count of zero** and an empty
   `LODInfo`, nothing; with a known count of 2, `LayerAbsent` both when `LODInfo` has 2 entries and when it is empty;
8. **the minimal change** (link 8's shape): an unattached component with no physics instance, tree, parent, physics
   asset or morph sets; an empty `SpaceBases` and `LODInfo`; no old mesh; a resident new mesh with zero `Origin` and
   `RotOrigin`; and a layer stand-in answering a **known LOD count of zero**. The four writes, in order:
   `bAnimTreeInitialised` false, `SkeletalMesh` the new mesh, `SpaceBases` empty, `bHasValidBodies` false. `LODInfo`
   and `MorphTargetIndexMap` stay as they were;
9. **the clear** (review r1 of #635, item 4), each on a minimal component with a set old mesh and `NewMesh` None:
   - an unloaded old mesh: row 5's `NotRead`, nothing loaded;
   - a resident old mesh at zero origin: the four writes with `SkeletalMesh` None, and `LODInfo` unchanged. This holds
     with no layer answer at all, since row 15 is skipped;
   - a resident old mesh with a non-zero origin: the transform's `LayerAbsent`;
10. **row D** (review r1 of #635, item 6): a component whose late destination is unwritable (`bHasValidBodies` declared
    as an int, or not declared) gives the `VmError::Script` naming it. `bAnimTreeInitialised`, `SkeletalMesh` and
    `SpaceBases` stay unchanged, and no counter moves;
11. Reference: unchanged.

## Amendment (2026-11-25): the entry the LOD-info refill constructs

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LODINFO-CONSTRUCTED-ENTRY (decision 2026-11-25-19),
analyst only: **lane A implements it.** Raw output stays under ignored `local/analysis/lodinfo/`. **UNVERIFIED in the
running game.**

**The question.** Step 16 (row 15) says the refill adds default entries: "no hidden materials, both flags clear, index
-1". The installed packages' script default of `SkelMeshComponentLODInfo` has `InstanceWeightIdx` 0 and
`InstanceWeightUsage` 0. Does the native use its own constructor, and what does each member get?

**Evidence.** Ghidra 12.1.4 headless, read-only, under the machine lock: `SetSkeletalMesh`'s LOD-info step, the same
routine section 2 step 16 read. The struct layout comes from `tools/ghidra/class_layout.py`, with
`SkeletalMeshComponent`'s `MapProperty` given a stand-in size in a local copy. The enum's value names come from the
`EInstanceWeightUsage` export of `Engine.upk`.

**The answer.** The native does not copy the script default. When the lengths differ, it empties `LODInfo`, reserving
room for the mesh's LOD count, then appends that many entries. Each new entry is set member by member:

| Member | Constructed value | Script default (for comparison) |
|---|---|---|
| `HiddenMaterials` | empty (no elements) | empty |
| `bNeedsInstanceWeightUpdate` | false | false |
| `bAlwaysUseInstanceWeights` | false | false |
| `InstanceWeightUsage` | the byte **0** = `IWU_PartialSwap` (the enum's names are `IWU_PartialSwap`, `IWU_FullSwap`, `IWU_MAX`) | 0 (`IWU_PartialSwap`) |
| `InstanceWeightIdx` | **-1** (confirmed) | 0 |

So the constructed entry differs from the script default in **one member only, `InstanceWeightIdx`: -1, not 0**.

The two flags share a bool word. The native clears just those two bits, and the word holds no other declared field, so
nothing else is affected. Then the per-LOD loop of step 16 runs over every LOD, whether the lengths differed or not. For
a LOD whose model has the unidentified data, it overwrites `InstanceWeightUsage` (from the model) and sets
`InstanceWeightIdx` to 0. A constructed entry keeps -1 only for a LOD without that data.

**For the implementer (lane A).** Build each refill entry explicitly with the five values above. Do not take
`SkelMeshComponentLODInfo`'s struct default from the packages: that would give index 0. Row 15's refusals are
unchanged.

**Test.** A synthetic component whose `LODInfo` is shorter than a known LOD count of 2, with no per-LOD data, gets two
entries, each with empty `HiddenMaterials`, both flags false, `InstanceWeightUsage` 0 and `InstanceWeightIdx` -1. The
same fixture with the struct default present in the synthetic package still gives -1, which proves the default is not
copied.

## Open questions

1. **UNVERIFIED:** every field name of section 2 (by fit); the component flag of step 3; step 12's field; step 17's
   call; the LOD-model data behind step 16's byte.
2. **Not read in detail:** the bone-name search of steps 8, 10 and 14, and the tree's per-bone array layout.
3. **Ruled by the hub (05:35Z):** row 11 (rule 7 before review r1 of #635), the native-only transform. Still open: steps 12 and 17 (native-only, no
   write proposed).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `SetSkeletalMesh` (pre and post) during a weapon's `InitMeshes`: expect `SkeletalMesh` set, `SpaceBases` still
   empty on an unattached component, `bAnimTreeInitialised` false, and `LODInfo` sized to the composite mesh's LOD
   count.
2. On an attached weapon component, after a mesh change: expect `SpaceBases` filled to the new mesh's bone count (the
   re-attach's pose update).
