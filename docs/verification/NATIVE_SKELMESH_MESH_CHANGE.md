# SkeletalMeshComponent.SetSkeletalMesh: the mesh change (2026-11-20)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-SKELMESH-MESH-CHANGE (decision 2026-11-19-NNN, number
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

**Answer: neither.** The mesh change empties the component's transform buffer and does not refill it. Neither the
reference pose nor the old transforms is copied into it. The buffer is refilled only by the pose update that a later
attach or tick runs (section 3). A saved copy of the old transforms is copied back by bone name **only if** something
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
    and its index is set to 0 (fitted: `InstanceWeightUsage` and `InstanceWeightIdx`). With a None mesh, `LODInfo` is
    left as it is.
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
record: `docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md`.

**Preflight, in this order** (write-free, load-free, RNG-free; reads with `vm.space.prop` / `vm.space.read`). Rules 1
to 3 are #599's and stay as they are:
1. the receiver resident (#599's `NotRead`);
2. the current mesh read, and the arguments decoded (#599);
3. equal meshes: nothing (#599).
4. **The new mesh, when set, must be resident:** an unloaded or failed handle is `NotYet { NotRead }`, "SetSkeletalMesh:
   the new mesh is an unloaded handle (a load is a write)". The native reads its origin, bones and LOD models.
5. **The old mesh, when set and the new one differs:** it must be resident too (its origin and bone count), else
   `NotYet { NotRead }`, "SetSkeletalMesh: the old mesh is an unloaded handle (a load is a write)".
6. **The paths the port has no layer for**, each before any write, in the native's order:
   - `bKeepSpaceBases` with a current mesh and a non-empty `SpaceBases`: `NotYet { LayerAbsent(BONES_LAYER) }`,
     "SetSkeletalMesh: keeping the bone transforms over a mesh change". An empty buffer saves nothing, so it passes;
   - a set `PhysicsAssetInstance`: `NotYet { LayerAbsent("a skeletal physics instance") }`;
   - a set `Animations`: `NotYet { NotPorted }`, "SetSkeletalMesh: the animation tree's per-bone state on a mesh
     change". The emptying branch is simple, but the port's tree instance (#561/#575) does not hold that array as a
     field this rule can write, so it is named rather than guessed;
   - `bAttached` true: `NotYet { NotRead }`, "SetSkeletalMesh: the re-attach's pose update (see
     ANALYST-BODYCOMP-SOCKET-BONE-TABLES)" (section 3);
   - a set `ParentAnimComponent`: `NotYet { LayerAbsent("the skeletal mesh data layer's bone names") }`, the parent
     bone map. It is liftable through #569's `reference_skeleton()` once that answers without a load;
   - a set `PhysicsAsset`: the same layer refusal, for `bHasValidBodies`;
   - a non-empty `MorphSets`: `NotYet { NotPorted }`, "SetSkeletalMesh: the morph-target index map";
   - **`LODInfo`** (step 16): with a new mesh whose LOD count differs from `LODInfo`'s length, `NotYet { LayerAbsent(
     "the skeletal mesh data layer's LOD models") }`. The count, and the per-LOD byte copied from each LOD model, come
     from native mesh data with no script declaration. With a None mesh, or an equal count, nothing is written there.
7. **The transform** (step 9) is native-only in the port: the probe found no `LocalToWorld` declaration on the port's
   layout (section 5), and an unattached component has no scene transform. **This needs a hub ruling**: either answer
   it as nothing in Off, because no script-visible field or slice step reads it, or refuse it as
   `NotYet { LayerAbsent("a component transform") }` whenever either mesh's origin offset or rotation is non-zero.
   Step 12's unidentified field and step 17's notification are native-only too, and fall under the same ruling.

**Writes**, only after the whole preflight passed, in the native's order:
1. `bAnimTreeInitialised` = false;
2. `SkeletalMesh` = `NewMesh`;
3. `SpaceBases` emptied;
4. `bHasValidBodies` = false (rule 6 has refused every case where it would be set);
5. `MorphTargetIndexMap` emptied, when the port holds it as a value (the probe saw a map declaration read as None:
   write nothing when it is already empty);
6. `LODInfo`: nothing (rule 6 has refused every case where it would change).

Each write is declared with its kind, else a `VmError::Script` naming the field, checked in the preflight. A write that
leaves a value unchanged is still made (the native writes it), except item 5's empty map. **After writes started:** a
`NotYet` raised despite the preflight is marked "stopped after partial writes: " by #518's counters, with its kind and
record kept.

**The clear** (a set mesh to None) needs no mesh data under these rules: rules 4 and 6's `LODInfo` row do not apply,
and rule 5 reads the old mesh's origin only for the transform, which follows the hub's ruling on rule 7. Link 8 makes
no such call (its resets are None to None, #583 section 5).

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

**So under section 4, link 8 stops first at rule 4**, the new mesh's `NotRead`. What would lift it: the mesh resident
before the run, through a harness preload as with the gestalt definitions (an UNVERIFIED harness input, a hub ruling),
or through the composite-mesh build once it is ported to produce a resident mesh. After that comes rule 6's `LODInfo`
row: the mesh has at least one LOD and `LODInfo` is empty, so it is `LayerAbsent` until #569's layer exposes the LOD
models. Rule 7 follows the hub's ruling. With all of those lifted, the next stop is `PrimitiveComponent.SetScale(float)`.

## Tests for the implementer

Synthetic packages through `package()`/`tag()`. Each refusal leaves the heap writes, draws and load transitions
unchanged:
1. #599's tests unchanged: an unloaded receiver, the argument shapes, the same mesh (None included) as a no-op;
2. a new mesh that is an unloaded handle, or a failed one: rule 4's `NotRead`, with nothing loaded;
3. an old mesh that is an unloaded handle, with a resident new one: rule 5's `NotRead`;
4. each rule 6 row on its own (keep with a non-empty buffer, a physics instance, a tree, attached, a parent component,
   a physics asset, morph sets, a differing LOD count): its refusal before any write. With two rows set, the first in
   the order above is reported;
5. keep with a current mesh and an **empty** buffer: no refusal from that row;
6. the minimal change (link 8's shape, with an equal LOD count, here 0 against a mesh whose LOD data the fixture
   leaves empty): the writes of section 4 in order, `SkeletalMesh` set to the new mesh and `SpaceBases` empty;
7. the clear (a set mesh to None) on a minimal component: `SkeletalMesh` None, `bAnimTreeInitialised` false,
   `SpaceBases` empty, `LODInfo` unchanged;
8. a field declared with the wrong kind (each written field): `VmError::Script` naming it, before any write;
9. under the hub's ruling on rule 7: the case it names (nothing, or the transform refusal with a non-zero origin);
10. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every field name of section 2 (by fit); the component flag of step 3; step 12's field; step 17's
   call; the LOD-model data behind step 16's byte.
2. **Not read in detail:** the bone-name search of steps 8, 10 and 14, and the tree's per-bone array layout.
3. **Hub ruling needed:** rule 7, the native-only transform.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `SetSkeletalMesh` (pre and post) during a weapon's `InitMeshes`: expect `SkeletalMesh` set, `SpaceBases` still
   empty on an unattached component, `bAnimTreeInitialised` false, and `LODInfo` sized to the composite mesh's LOD
   count.
2. On an attached weapon component, after a mesh change: expect `SpaceBases` filled to the new mesh's bone count (the
   re-attach's pose update).
