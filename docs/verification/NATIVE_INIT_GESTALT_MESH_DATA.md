# SkeletalMeshComponent.InitGestaltMeshDataFromOther on link 8 (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-INIT-GESTALT-MESH-DATA (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the script, read with `research/script_disasm.py` (callers in the nine code packages);
- the installed `Engine.upk` export table (the declared struct and its member names, through `ow-package --exports`;
  names only);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock);
- a local, uncommitted probe of the installed slice gate (section 6).

There is no bl-sdk session for this lane. One earlier live read is recorded (WEAPON_VISUALS.md section 2: a gun's
`GestaltData.PartMeshNames`). Raw output stays under ignored `local/analysis/igm/`. This note is in our own words: no
decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is UNVERIFIED** in the
running game.

**What was already covered.** NATIVE_BUILD_COMPOSITE_MESH.md (#588) reads the native that fills the gestalt data,
names both copy sites in `InitMeshes`, and refuses the fill with `LayerAbsent`. On r1 of #588 the hub overturned the
render-side ruling, so the gestalt component state is a port item. This note says what that state is, what this
native does with it, and what one implementer needs to port both natives as one layer. Nothing binds this native on
main or on any open head.

## 1. Callers (script)

`InitGestaltMeshDataFromOther(SkeletalMeshComponent ComponentToCopyFrom)` is a native with no return value. Its
callers are:
- `WillowWeapon.InitMeshes` (2 calls);
- `WillowInventory.CloneInventorySkelMeshComponent` (1).

**On link 8** both `InitMeshes` calls are guarded by the same two checks: the weapon type's `GestaltMesh` is set
**and** that definition's `GestaltSkeletalMesh` is set (NATIVE_BUILD_COMPOSITE_MESH.md section 1):
1. `ThirdPersonMesh.InitGestaltMeshDataFromOther(FirstPersonMesh)`, after `BuildCompositeMesh` has filled
   `FirstPersonMesh`;
2. in the `ExtraSlots` loop, for each slot with a `Mesh`: `Mesh.InitGestaltMeshDataFromOther(ThirdPersonMesh)`.

## 2. The state: the component's `GestaltData` (package declaration)

`SkeletalMeshComponent` declares a struct property **`GestaltData`** of type **`GestaltDataContainer`**. The package
declares the container's members in this order:

| Member | Declared type | Filled by `BuildCompositeMesh` on link 8's path (by fit) |
|---|---|---|
| `PartMeshNames` | array of names | the part name list. Not seen in the apply step that was read, but a live read found a gun's fragment names here (UNVERIFIED which step writes it) |
| `SkeletalMesh` | object | yes: the shared gestalt mesh reference, from the definition |
| `LODModelData` | array of `GestaltLODModel`, each an array of `GestaltLODModelFragment` (`FirstIndex`, `NumPrimitives`, `MaterialIndex`) | yes: rebuilt per LOD from the definition's gestalt infos, keeping the fragments whose part name is in the list |
| `SocketMappings` | array of structs | yes: emptied, then filled from the definition |
| `GestaltAccessoryList` | array of `GestaltAccessoryEntry` (`GPUBoneIndex`, `OriginalBoneIndex`, `TransformIndex`) | not by the path read. The script names an accessory twin, `GestaltSkeletalMeshDefinition.InitGestaltAccessoryMeshDataFromParts` |
| `GestaltPartMatricesContainer` | object (`GestaltPartMatrices`) | not by the path read |
| `ReferencePoseBounds` | bounds (origin, box extent, sphere radius) | yes: from the definition's part bounds, grown per kept part |

The members and their types come from the package declaration. Matching each executable write to a member is **by
fit** (UNVERIFIED): the matches are by position in the declared order and agree with the copy in section 3 member
for member, but `tools/ghidra/class_layout.py` cannot lay out `SkeletalMeshComponent`.

**Script visibility.** `GestaltData` is a declared property, so reflection reaches it (the recorded bl-sdk read).
**No script function in the nine code packages names `GestaltData` or any of its members.** The names checked were
`GestaltData`, `PartMeshNames`, `LODModelData`, `SocketMappings`, `GestaltAccessoryList`, `ReferencePoseBounds` and
`GestaltPartMatricesContainer`, with zero hits. Its property flags (const, transient and so on) were not read. Script
reaches the state only through natives: this copy, the fill, and possibly the socket queries (section 4).

## 3. What the native does (the rule, read from the executable)

1. It reads the object argument, `ComponentToCopyFrom`.
2. It **assigns the whole `GestaltData` container** of that component to the receiver's `GestaltData`, member by
   member:
   - the four arrays are copied as values (the receiver's old contents are replaced, nested fragment arrays
     included);
   - the two object references (`SkeletalMesh`, `GestaltPartMatricesContainer`) are copied as references, so both
     components share one mesh and one matrices object;
   - the bounds are copied as values.
3. Nothing else happens. It does not:
   - check the argument (a null argument is not handled; the native would dereference it);
   - touch the receiver's `SkeletalMesh`, bones, animation tree or render registration;
   - load or draw anything.

**Order on link 8.** The copy comes **before** `SetSkeletalMesh(NewMesh)` on both meshes. So the third-person mesh
holds the gestalt data before its mesh changes, and the extra slot's copy comes from the third-person mesh.

## 4. What one implementer needs (the layer)

Both natives work on the same state: the `GestaltData` container of a skeletal mesh component.
- **The home of the state:** the component's declared `GestaltData` property slot in the heap, not a side table. Then
  reflection, a future script read and both natives see one value. **To check first:** whether the port's heap
  already holds this struct slot whole, the nested `LODModelData` arrays included, for a resident component. That
  was not checked here.
- **The fill** (`BuildCompositeMesh`, NATIVE_BUILD_COMPOSITE_MESH.md section 2): write `SkeletalMesh`,
  `LODModelData`, `SocketMappings`, `ReferencePoseBounds` and, by the live read, `PartMeshNames`, from the gestalt
  definition and the part name list. The definition side is already decoded (`GestaltInfos` through
  `tools/gestalt-arrays.schema`, cross-checked live in RTSE_GUN_DATA_CROSSCHECK.md). The bounds growth and the
  socket fill were not read in detail.
- **The copy** (this native): a whole-value assignment of the slot. It needs no definition data.
- **The readers known so far:** drawing (`LODModelData` picks the fragments a gun draws, WEAPON_PART_ASSETS.md), and,
  **UNVERIFIED**, the socket natives (`GetSocketWorldLocationAndRotation`, used by `WillowWeapon.GetEffectLocation`,
  `GetWeaponMuzzleRotation` and `GetSocketLocationOnOwnerOrWeapon`). That they read `SocketMappings` was not read in
  the executable.

## 5. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `SkeletalMeshComponent.InitGestaltMeshDataFromOther(object)` in Off;
Reference keeps the stub policy.

**Preflight (write-free, loads nothing), in this order.** The first failure is the answer:
1. **The receiver** `this`: set and resident. An unloaded handle refuses `NotYet` with missing kind `NotRead`,
   "InitGestaltMeshDataFromOther: the component is an unloaded handle (a load is a write)". Under the instancing
   ruling of #583, link 8's harness instances the weapon's components, so this is not link 8's stop. Then a class
   with no `GestaltData` struct field is a `VmError::Script` naming the field.
2. **The argument** `ComponentToCopyFrom`:
   - null (`Object(None)`): a `VmError::Script`, "InitGestaltMeshDataFromOther: ComponentToCopyFrom is None". The
     native does not handle it, so there is no game answer;
   - not a `SkeletalMeshComponent`, or with no `GestaltData` field: a `VmError::Script` naming the class or the field;
   - an unloaded handle: `NotYet` with missing kind `NotRead`, as in check 1.
3. **Same component** (`this` is `ComponentToCopyFrom`): a self-assignment, so nothing changes and nothing is
   written. It is answered as success after checks 1 and 2. UNVERIFIED: the self-copy path was not read separately.

**The answer:**
- **Until the layer of section 4 exists:** `NotYet` with missing kind `LayerAbsent("a gestalt component data layer (a
  skeletal mesh component's GestaltData)")`, "InitGestaltMeshDataFromOther: the gestalt mesh data of the component",
  raised before any write. This is **the same layer name** `BuildCompositeMesh` refuses with on #588 r1, so both
  natives go to one implementer. On link 8 this native is not reached in Off while the fill refuses first.
- **With the layer:** assign the argument's `GestaltData` value to the receiver's slot whole, as section 3 says. The
  assignment is one heap write and it can fail only on a slot shape, which check 1 has already settled. Return
  nothing.

**The counters:** the refusals all come before the native's only write, so they never carry the "stopped after
partial writes: " prefix inside it. The caller's earlier writes can mark it.

## 6. What link 8 meets after it (local probe, uncommitted)

The probe ran on a local, unpushed merge of #558, #534, #552 and #572, with throwaway stand-ins:
- for `GetParticleEffect`, `GetExpLevelFromManufacturerData` and `StaticGetWeaponDamageType`;
- for `SetSkeletalMesh`, NATIVE_SET_SKELETAL_MESH.md's rule (the same mesh does nothing, a different mesh refuses),
  with its unloaded component handles loaded by the probe;
- for `BuildCompositeMesh`, the gestalt answer without the component write;
- for this native, a no-op.

**Installed Off `--slice-gate` strict: 1 ok, 5 notyet, 8 error.** **Link 8 is 1 of 5 at `init`, now `notyet`** (it
was an `error` at an unbound native; no other link changed).
- The stand-in was reached once, on `ThirdPersonMesh`, with `FirstPersonMesh` as the argument. The weapon had no
  extra slots.
- The run then stopped at **`SetSkeletalMesh(NewMesh)` on `FirstPersonMesh`**: the gestalt mesh against the
  component's None, a mesh change. By NATIVE_SET_SKELETAL_MESH.md section 4 that is `LayerAbsent("a skeletal mesh
  component's bone transforms")`, the stand-in's refusal, marked "stopped after partial writes: " by the script's
  earlier writes.

So link 8's next stop is **not a new unbound native**. It is the already-analysed mesh-change layer of #583, behind
the gestalt layer of section 4.

## Open questions

1. **UNVERIFIED:** the match of each write to a declared member (by fit; section 2).
2. **Not read:** which step writes `PartMeshNames` on the weapon path; the self-copy case; the bounds growth and the
   socket fill of the apply step.
3. **UNVERIFIED:** whether the socket natives read `GestaltData.SocketMappings` (section 4).
4. **To check before implementing:** whether the port's heap holds the `GestaltData` slot whole (section 4).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After `InitMeshes` on a Bandit assault rifle, read `GestaltData` on `FirstPersonMesh` and `ThirdPersonMesh`:
   expect equal containers, the same `SkeletalMesh` and `GestaltPartMatricesContainer` objects, and equal
   `PartMeshNames` and `LODModelData`.
2. Hook `InitGestaltMeshDataFromOther` (pre and post) on the third-person mesh: expect its `GestaltData` to change
   from its earlier value to the first-person mesh's, and its `SkeletalMesh` to be unchanged.
3. On a weapon with extra slots, expect each slot's `Mesh.GestaltData` to equal the third-person mesh's.
