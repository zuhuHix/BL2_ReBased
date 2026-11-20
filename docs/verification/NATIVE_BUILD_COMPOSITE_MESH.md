# WillowWeapon.BuildCompositeMesh on link 8 (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-BUILD-COMPOSITE-MESH (decision 2026-11-19-200, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the script, read with `research/script_disasm.py` (callers in the nine code packages);
- the installed packages through `ow-package --def-dump` (the slice gun's type, parts and gestalt definition; names
  and counts only) and `tools/ghidra/class_layout.py`;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock);
- a local, uncommitted probe of the installed slice gate (section 5).

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/bcm/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule
here is UNVERIFIED** in the running game.

**What was already covered.** NATIVE_WEAPON_VISUALS.md section 3 (on main) outlines the native: none, one or a merged
mesh, and the gestalt path. It names the merge algorithm and the gestalt data as not read. Nothing binds the native
on main or on any open head. This note re-reads the selection, says what the gestalt path writes, places the native
on link 8, and gives the Off binding.

## 1. Callers (script)

`BuildCompositeMesh(SkeletalMeshComponent, WeaponDefinitionData)` is a native returning a `SkeletalMesh`. Its
callers are `WillowWeapon.InitMeshes` and `PlayerStandIn.CloneWeaponMesh`, plus the `WillowItem` twin
(`WillowItem.InitMeshes`, `PlayerStandIn.CloneItemMesh`).

**On link 8**, `InitMeshes` opens with `NewMesh = BuildCompositeMesh(FirstPersonMesh, DefinitionData)`, then walks
the parts for `bUseWeaponMelee`. It copies the gestalt data in **two places**, each guarded by **both** checks: the
weapon type's `GestaltMesh` is set **and** that definition's `GestaltSkeletalMesh` is set. A type with a gestalt
definition but no shared mesh makes no copy.
1. **First person to third person:** `ThirdPersonMesh.InitGestaltMeshDataFromOther(FirstPersonMesh)`, right after
   the part walk. Then come `SetSkeletalMesh(NewMesh)` on `FirstPersonMesh` and on `ThirdPersonMesh`
   (NATIVE_SET_SKELETAL_MESH.md, #583), `HideBonesInMesh`, the third-person scale, the material and the part effects.
2. **Third person to each extra slot:** in the loop over `ExtraSlots`, for each entry whose `Mesh` is set, under the
   same two checks, `Mesh.InitGestaltMeshDataFromOther(ThirdPersonMesh)`, just before that slot's own
   `SetSkeletalMesh(NewMesh)`.

The copy itself is NATIVE_INIT_GESTALT_MESH_DATA.md (the next lane).

## 2. The rule (read from the executable)

With `C` the component argument and `D` the definition data:
1. `D.WeaponTypeDefinition` None: the answer is **None**, and nothing is read or written.
2. **The part walk.** Both paths below walk `D`'s part slots in the definition data's slot order (the order
   NATIVE_EQUIP_PARTS.md's `AllParts` gives), **name parts excluded**, skipping empty slots.
3. **The gestalt path:** taken when the type's `GestaltMesh` is set **and** that `GestaltSkeletalMeshDefinition`'s
   `GestaltSkeletalMesh` is set.
   - For each part, its gestalt name:
     - with `bIsGestaltMode` set: its `GestaltModeSkeletalMeshName`;
     - otherwise, with a `NongestaltSkeletalMesh`: that mesh object's name;
     - otherwise: none.
   - A part with a name (not None) adds it, then adds each of its two `AdditionalGestaltModeSkeletalMeshNames`
     that is not None.
   - **The write:** the gestalt definition applies itself to `C` with that name list. The target is `C`'s declared
     struct property `GestaltData` (a `GestaltDataContainer`, declared in the package with `PartMeshNames`,
     `SkeletalMesh`, `LODModelData`, `SocketMappings`, `GestaltAccessoryList`, `ReferencePoseBounds` and
     `GestaltPartMatricesContainer`). It sets the object reference, rebuilds the per-LOD fragment data
     (`LODModelData`: for each of the definition's gestalt infos, the fragments whose part name is in the list),
     and fills the socket and bounds data from the definition. Which declared member each write lands in is **by
     fit** (UNVERIFIED): the layout tool cannot lay out `SkeletalMeshComponent`, so the match is by position in the
     declared order. A recorded live read (WEAPON_VISUALS.md section 2) saw `GestaltData.PartMeshNames` hold a gun's
     fragment names, which fits the name list landing there.
   - **The answer:** the gestalt definition's `GestaltSkeletalMesh`, a shared mesh, not a new one.
4. **The non-gestalt path** (no gestalt mesh): each part's `NongestaltSkeletalMesh`, when the part is **not** in
   gestalt mode and has one, is collected.
   - zero meshes: **None**;
   - exactly one: **that mesh**;
   - two or more: a **new merged skeletal mesh** is created (the merge algorithm was not read).

**Loads and draws:** none of its own. Every definition it reads (the type, its gestalt definition, each part) must be
readable, and the answer is returned as a reference. **Writes:** the gestalt path writes `C`; the merge path creates
an object; the other paths write nothing.

## 3. The slice gun (names and counts only)

Link 8's weapon (`EQUIP_WEAPON`, the Bandit assault rifle) takes **the gestalt path**:
- its type's `GestaltMesh` is `Weap_AssaultRifles.GestaltDef_AssaultRifle`, a `GestaltSkeletalMeshDefinition` with a
  `GestaltSkeletalMesh` (`..._GestaltSkeletalMesh`) and 47 part bounds;
- its parts give **4 names**: the barrel (`AR_Barrel_Bandit`), the body (`AR_Body_Bandit`), the grip
  (`AR_Grip_Jakobs`) and the stock (`AR_Stock_Jakobs`), each in gestalt mode with no additional names;
- two parts give none:
  - the sight is not in gestalt mode and has no non-gestalt mesh, so its `AR_Scope_Bandit` name is not used;
  - the material part has no mesh;
- the answer is the shared gestalt skeletal mesh, and `FirstPersonMesh` gets the gestalt data for those 4 names.

## 4. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `WillowWeapon.BuildCompositeMesh(object,weapondefinitiondata)` in Off;
Reference keeps the stub policy.

**Preflight (write-free, loads nothing).** Each value is checked for residency **before** its fields are read, and
each field's shape **as** it is read, in this order. The first failure is the answer, so a mixed case gives one
defined refusal:
1. **`D.WeaponTypeDefinition`:** an object field of `D`, otherwise a `VmError::Script`. None: the answer is None,
   with nothing else checked (the component included).
2. **The type:** resident, otherwise `NotYet` with missing kind `NotRead`, "BuildCompositeMesh: a definition it reads
   is not loaded (a load is a write)". Then its `GestaltMesh`: an object field, otherwise a `VmError::Script` naming
   the field.
3. **The path choice.** With `GestaltMesh` set: that definition resident (else `NotRead`, as in check 2), then its
   `GestaltSkeletalMesh` an object field (else a `VmError::Script`). Both set: the gestalt path. Otherwise: the
   non-gestalt path.
4. **The part walk**, in slot order. For each non-empty, non-name slot: the part resident (else `NotRead`), then its
   `bIsGestaltMode` (a bool) and `NongestaltSkeletalMesh` (an object), and on the gestalt path also
   `GestaltModeSkeletalMeshName` (a name) and `AdditionalGestaltModeSkeletalMeshNames` (two names). A wrong shape is a
   `VmError::Script` naming the field. **The name fallback** (gestalt path, a part not in gestalt mode) needs only
   the object name of the `NongestaltSkeletalMesh` handle. That name is the handle's existing name metadata (its
   export or import entry), read **without** loading the mesh, so an unloaded mesh handle is not a refusal there.
   On the non-gestalt path the mesh handles are only collected and returned, so they are not loaded either.
5. **The component `C`**, checked only on the gestalt path (the other paths never touch it):
   - null (`Object(None)`): a `VmError::Script`, "BuildCompositeMesh: the component is None". The native would
     dereference it; there is no game answer to copy;
   - not a `SkeletalMeshComponent`, or with no `GestaltData` struct field: a `VmError::Script` naming the class or
     the field;
   - an unloaded handle (the instancing ruling of #583 has the harness instance the weapon's components):
     `NotYet` with missing kind `NotRead`, as in check 2.

**The answer:**
- **The gestalt path:** `NotYet` with missing kind `LayerAbsent("a gestalt component data layer (a skeletal mesh
  component's GestaltData)")`, "BuildCompositeMesh: the gestalt mesh data of the component", raised before any
  write. It is not answered as render-side. `InitMeshes` reads and copies this state straight away (the two copy
  sites of section 1), and later script asks the weapon mesh for sockets (`GetSocketWorldLocationAndRotation`, used
  by `WillowWeapon.GetEffectLocation`, `GetWeaponMuzzleRotation` and `GetSocketLocationOnOwnerOrWeapon`).
  **UNVERIFIED:** that the socket natives read `GestaltData.SocketMappings` for a gestalt mesh; that dependency was
  not read in the executable.
- **The draw sections, on their own:** `LODModelData` (per-LOD fragments: first index, primitive count, material
  index) has no script reader in the nine code packages, and so far only drawing is known to use it. It is still
  state that the next native copies whole, so it belongs to the same layer. Whether an implementer can leave it out
  (a host that draws without it) is a later choice, to be settled with the draw path, not here.
- **What Off lacks** is the component state and the code that fills it, not the definition data. The package schema
  already decodes the definition's `GestaltInfos` (`tools/gestalt-arrays.schema`), checked against a live dump for
  every family (RTSE_GUN_DATA_CROSSCHECK.md).
- **The non-gestalt path:** zero meshes answer None and one answers that mesh, both writing nothing. Two or more
  refuse `NotYet` with missing kind `LayerAbsent("a skeletal mesh merge")`, "BuildCompositeMesh: merging the parts'
  meshes", before any write.

**The counters:** the native's refusals all come before its first write, so they never carry the "stopped after
partial writes: " prefix inside it. The caller's earlier writes can mark it.

## 5. What link 8 meets after it (local probe, uncommitted)

The probe ran on a local, unpushed merge of #558, #534, #552 and #572, with throwaway stand-ins:
- for `GetParticleEffect`, `GetExpLevelFromManufacturerData` and `StaticGetWeaponDamageType`;
- for `SetSkeletalMesh`, the same-mesh rule, with its unloaded component handles loaded by the probe;
- for this native, the gestalt path's answer without its write to `C`. That was the render-side alternative the
  hub first ruled for and then overturned on r1 of #588. It is a probe device only, not the proposed answer.

**Installed Off `--slice-gate` strict: 1 ok, 4 notyet, 9 error.** **Link 8 is 1 of 5 at `init`.** The stand-in took the
gestalt path and answered the gestalt skeletal mesh. The run then stopped at the unbound
**`SkeletalMeshComponent.InitGestaltMeshDataFromOther(object)`**, called on `ThirdPersonMesh` to copy the first-person
mesh's gestalt data. After that, `SetSkeletalMesh` with the gestalt mesh, a mesh different from the components' None,
would refuse `LayerAbsent` by NATIVE_SET_SKELETAL_MESH.md section 4 if reached.

## Open questions

1. **Not read:** the merge algorithm (non-gestalt, two or more meshes), and the match of each apply-step write to a
   declared `GestaltDataContainer` member (by fit).
2. **UNVERIFIED:** that the part walk is exactly `AllParts`' order without the name parts. The walk's slot iterator
   skips the two name-part slots, but its slot numbering was not matched to `AllParts` field by field.
3. **UNVERIFIED:** whether the socket natives (`GetSocketWorldLocationAndRotation`, `GetSocketByName`) read the
   component's gestalt socket data (section 4).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `BuildCompositeMesh` (post) during a Bandit assault rifle's build: expect the gestalt skeletal mesh of
   `GestaltDef_AssaultRifle`.
2. Read `FirstPersonMesh`'s gestalt data after the call: expect the 4 part names of section 3 and no sight section.
3. On a weapon type without a gestalt mesh and one mesh part, expect that part's `NongestaltSkeletalMesh`.
