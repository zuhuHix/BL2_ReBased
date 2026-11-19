# SkeletalMeshComponent.SetSkeletalMesh on link 8 (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-SET-SKELETAL-MESH (decision 2026-11-19-NNN, number pending).
Analyst only: a different lane implements it (clean room).

**Sources:**
- the script, read with `research/script_disasm.py` (callers in the nine code packages);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock);
- a local, uncommitted probe of the installed slice gate (section 5).

`tools/ghidra/class_layout.py` cannot lay out `SkeletalMeshComponent` (it holds a map property), so the fields below
are named **by fit** from the script-visible declarations (UNVERIFIED). There is no bl-sdk session and no recorded
live evidence. Raw output stays under ignored `local/analysis/ssm/`. This note is in our own words: no decompiler
output, pseudo-code, addresses, offsets or listing structure. **Every rule here is UNVERIFIED** in the running game.

**What was already covered.** Nothing binds `SetSkeletalMesh` on main or on any open head. The neighbours:
- **#569** (`fix/rust-skeletal-mesh-layer`): a skeletal-mesh data layer, the reference skeleton read from the
  exported meshes. It reads mesh data and binds no component native.
- **#570** (`fix/rust-mesh-rest`, NATIVE_MESH_REST.md): the rest of the body composition's made-`Mesh` step (a mesh
  flag and `ReattachWeapons`), on Maya's pawn. Not this native.
- **#575** (`fix/rust-anim-node-init`): the animation tree's node initialisation. This native clears the tree's
  initialised flag and runs part of the same component pass when the mesh changes (section 2). It does not replace
  #575.

## 1. Callers and the arguments on link 8 (script)

`SetSkeletalMesh(SkeletalMesh NewMesh, optional bool bKeepSpaceBases)` is a final native. Its callers include
`WillowWeapon.ClientInitializeFromWeaponTypeDefinition` (3 calls), `WillowWeapon.InitMeshes` (3), `WillowItem.InitMeshes`,
`WillowInventory.CloneInventorySkelMeshComponent`, `PlayerStandIn.CloneWeaponMesh` and `CloneItemMesh`,
`SkeletalMeshActor.OnSetMesh` and `ReplicatedEvent`, and `KAsset.SetMeshAndPhysAsset`.

**On link 8**, in order:
1. `ClientInitializeFromWeaponTypeDefinition`, right after `FireOffset`, resets the meshes:
   `FirstPersonMesh.SetSkeletalMesh(None)`, then `ThirdPersonMesh.SetSkeletalMesh(None)`, then the same for each
   `ExtraSlots` entry's `Mesh`. Each call is made only when that component is set, and `bKeepSpaceBases` is omitted
   (false).
2. Later, `WillowWeapon.InitMeshes` sets the composite mesh: `NewMesh` = `BuildCompositeMesh(FirstPersonMesh,
   DefinitionData)`, on `FirstPersonMesh`, then `ThirdPersonMesh`, then each extra slot's mesh.

## 2. What the native does (question: writes, reads, loads, draws)

1. **The same mesh:** when `NewMesh` is the component's current `SkeletalMesh` (None included), it **does nothing**:
   no write, no load, no draw.
2. **A different mesh**, in order (each step by fit):
   1. with `bKeepSpaceBases` and a current mesh, the current bone transforms are saved;
   2. the component's physics-asset instance, if any, is terminated and cleared;
   3. the component's animation-tree-initialised flag is cleared (the flag `SetAnimTreeTemplate` also clears;
      NATIVE_COPY_ANIM_TREE.md section 2);
   4. the component is detached for an update (by fit);
   5. `SkeletalMesh` = `NewMesh`;
   6. the component's bone-transform buffer is emptied;
   7. with an animation tree (the component's `Animations` is an `AnimTree`), the tree's per-bone state is resized
      to the new mesh's reference-skeleton bone count, unless `bKeepSpaceBases` holds and the count is unchanged;
   8. the bone transforms are rebuilt from the new mesh's reference pose, using the bone names, and several
      per-component bone and LOD arrays are rebuilt;
   9. part of the component pass of the tree initialisation runs (the same step NATIVE_COPY_ANIM_TREE.md section 2
      step 5 names);
   10. the component is re-registered with the render scene (by fit).

   It reads the new mesh's reference skeleton (bone count, names, reference pose). It draws nothing, and it loads
   nothing of its own: a mesh handle it is given must already be readable for step 8.

## 3. The layers the port lacks

| Part (section 2.2) | Layer the port lacks | Typed refusal |
|---|---|---|
| steps 1, 6, 8: bone transforms from the reference pose | a component bone-transform layer. #569's data layer reads the reference skeleton, but nothing holds a component's bones | `NotYet` with missing kind `LayerAbsent("a skeletal mesh component's bone transforms")` |
| step 2: the physics-asset instance | physics (no skeletal physics instance in the port) | covered by the row above, since it is in the same native |
| steps 3, 7, 9: the animation tree's flag and per-bone state | #561/#575 (tree instance and init) | covered by the row above |
| steps 4, 10: detach and re-register | the render scene (render-side; the host has no component registry) | covered by the row above |

Every row is one native call, so a separate refusal per row would not change what the caller sees. One refusal
covers the whole mesh change.

## 4. Proposed minimal Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `SkeletalMeshComponent.SetSkeletalMesh(object,bool)` in Off; Reference keeps
the stub policy.

**Preflight (write-free, loads nothing):**
1. `this` is an unloaded definition handle: `NotYet` with missing kind `NotRead`, "SetSkeletalMesh: the component is
   an unloaded handle (a load is a write)". On link 8 this is **the first stop** (section 5): the weapon's
   `FirstPersonMesh` is the class default's component, not an instance of its own.
2. `this` has no `SkeletalMesh` object field: a `VmError::Script` naming the field.
3. `NewMesh` (an object or None) and `bKeepSpaceBases` (a bool, default false): otherwise a `VmError::Script`.

**The answer:**
- `NewMesh` equals the current `SkeletalMesh`: nothing, the native's own answer;
- otherwise: `NotYet` with missing kind `LayerAbsent("a skeletal mesh component's bone transforms")`, "SetSkeletalMesh:
  a mesh change (bones, the animation tree's per-bone state, physics and render state)", raised before any write.

**The counters:** the native writes nothing on either path, so its refusals never carry the "stopped after partial
writes: " prefix inside it. The caller's earlier writes can mark it, as the probe shows.

**For the hub:** rule 1 is an instancing question, not this native's. In the game every spawned weapon has its own
mesh components. The equip run builds its weapon from class defaults, so the components stay the shared template
handles. Two choices:
- the harness instances the weapon's components, through the subobject instancing the world's spawn path already
  does (#435), logged as an UNVERIFIED harness input;
- rule 1 stays the link 8 stop.

## 5. What link 8 meets (local probe, uncommitted)

The probe ran on a local, unpushed merge of #558, #534, #552 and #572 (3d948c43), with throwaway stand-ins:
- for `GetParticleEffect` (handles loaded, evaluators false) and `GetExpLevelFromManufacturerData` (the interpolating
  path);
- for `StaticGetWeaponDamageType`, its rule;
- for this native, the same-mesh rule.

**Installed Off `--slice-gate` strict: 1 ok, 4 notyet, 9 error.** **Link 8 is 1 of 5 at `init`.** It showed:
1. **Without loading:** `FirstPersonMesh` is the unloaded handle `FirstPersonWeaponMesh` (an
   `Engine.SkeletalMeshComponent` of the class default), so rule 1 is the stop, marked by the script's earlier
   writes.
2. **With the probe loading it:** `FirstPersonWeaponMesh` and `ThirdPersonWeaponMesh` both hold `SkeletalMesh` None,
   so both resets are None→None no-ops. There are no extra slots. The next stop is the unbound
   **`WillowWeapon.BuildCompositeMesh(object,weapondefinitiondata)`**, the first call of `InitMeshes`, which builds
   the composite mesh that `SetSkeletalMesh` then receives. That later call is a real mesh change and would refuse
   by section 4.

## Open questions

1. **UNVERIFIED:** the field names of section 2.2 (by fit); the layout tool cannot lay out the class.
2. **Not read in detail:** steps 8 to 10 (the bone-array rebuild, the component pass and the re-registration).
3. **For the hub:** the instancing question of section 4.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On a freshly spawned weapon, read `FirstPersonMesh` and `ThirdPersonMesh`: expect per-weapon component instances,
   outered to the weapon, with `SkeletalMesh` None before `InitMeshes`.
2. Hook `SetSkeletalMesh` (pre and post) during the build: expect the None resets to change nothing, and the
   composite-mesh call to set `SkeletalMesh` and clear the tree-initialised flag.
