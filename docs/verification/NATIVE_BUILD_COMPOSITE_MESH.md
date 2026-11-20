# WillowWeapon.BuildCompositeMesh on link 8 (draft, 2026-11-20)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-BUILD-COMPOSITE-MESH (decision 2026-11-19-NNN, number
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

**On link 8**, `InitMeshes` opens with `NewMesh = BuildCompositeMesh(FirstPersonMesh, DefinitionData)`. Then it
walks the parts for `bUseWeaponMelee`, and, when the weapon type has a `GestaltMesh`, calls
`ThirdPersonMesh.InitGestaltMeshDataFromOther(FirstPersonMesh)`. After that come `SetSkeletalMesh(NewMesh)` on both
meshes (NATIVE_SET_SKELETAL_MESH.md, #583), `HideBonesInMesh`, the third-person scale, the material, and the part
effects.

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
   - **The write:** the gestalt definition applies itself to `C` with that name list. It sets `C`'s gestalt
     definition reference, and rebuilds `C`'s per-LOD gestalt section data: for each of the definition's gestalt
     infos, the sections whose part name is in the list, and the definition's bounds and socket data. Field names
     are by fit (UNVERIFIED): the layout tool cannot lay out `SkeletalMeshComponent`.
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

**Preflight (write-free, loads nothing), in the read order:**
1. `D.WeaponTypeDefinition` is an object field: otherwise a `VmError::Script`. None: the answer is None.
2. The type, its `GestaltMesh` when set, and every part the walk visits must be resident. An unloaded handle refuses
   `NotYet` with missing kind `NotRead`, "BuildCompositeMesh: a definition it reads is not loaded (a load is a
   write)".
3. Shapes: the type's `GestaltMesh`, the gestalt definition's `GestaltSkeletalMesh`, and each part's
   `bIsGestaltMode`, `GestaltModeSkeletalMeshName`, `AdditionalGestaltModeSkeletalMeshNames` and
   `NongestaltSkeletalMesh`. A wrong shape is a `VmError::Script` naming the field.
4. **The gestalt path** writes `C`'s gestalt section data, which only drawing reads (NATIVE_WEAPON_VISUALS.md). The
   port has no gestalt mesh data, since `GestaltInfos` is not decoded. Under NATIVE_ANIM_NODE_LOOKUP.md section 5,
   rule 4, a write that a later step reads stays refused. `InitGestaltMeshDataFromOther`, the next native, copies
   this data. So:
   - `C` an unloaded handle (the instancing ruling of #583 has the harness instance the weapon's components):
     `NotYet` with missing kind `NotRead`, as in check 2;
   - otherwise: `NotYet` with missing kind `LayerAbsent("a gestalt skeletal mesh data layer (the definition's
     per-LOD part sections)")`, "BuildCompositeMesh: the gestalt mesh data of the component", raised before any
     write.

   **For the hub:** the alternative is a ruling that the gestalt section data is render-side, so the port computes
   the name list, writes nothing to `C`, and returns `GestaltSkeletalMesh`. That moves link 8 on to
   `InitGestaltMeshDataFromOther`, which copies data the port then does not have (section 5).
5. **The non-gestalt path:** zero meshes answer None and one answers that mesh, both writing nothing. Two or more
   refuse `NotYet` with missing kind `LayerAbsent("a skeletal mesh merge")`, "BuildCompositeMesh: merging the parts'
   meshes", before any write.

**The counters:** the native's refusals all come before its first write, so they never carry the "stopped after
partial writes: " prefix inside it. The caller's earlier writes can mark it.

## 5. What link 8 meets after it (local probe, uncommitted)

The probe ran on a local, unpushed merge of #558, #534, #552 and #572, with throwaway stand-ins:
- for `GetParticleEffect`, `GetExpLevelFromManufacturerData` and `StaticGetWeaponDamageType`;
- for `SetSkeletalMesh`, the same-mesh rule, with its unloaded component handles loaded by the probe;
- for this native, the gestalt path's answer without its write to `C` (the hub alternative of section 4).

**Installed Off `--slice-gate` strict: 1 ok, 4 notyet, 9 error.** **Link 8 is 1 of 5 at `init`.** The stand-in took the
gestalt path and answered the gestalt skeletal mesh. The run then stopped at the unbound
**`SkeletalMeshComponent.InitGestaltMeshDataFromOther(object)`**, called on `ThirdPersonMesh` to copy the first-person
mesh's gestalt data. After that, `SetSkeletalMesh` with the gestalt mesh, a mesh different from the components' None,
would refuse `LayerAbsent` by NATIVE_SET_SKELETAL_MESH.md section 4 if reached.

## Open questions

1. **Not read:** the merge algorithm (non-gestalt, two or more meshes), and the detail of the gestalt section data
   the apply step writes (by fit).
2. **UNVERIFIED:** that the part walk is exactly `AllParts`' order without the name parts. The walk's slot iterator
   skips the two name-part slots, but its slot numbering was not matched to `AllParts` field by field.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `BuildCompositeMesh` (post) during a Bandit assault rifle's build: expect the gestalt skeletal mesh of
   `GestaltDef_AssaultRifle`.
2. Read `FirstPersonMesh`'s gestalt data after the call: expect the 4 part names of section 3 and no sight section.
3. On a weapon type without a gestalt mesh and one mesh part, expect that part's `NongestaltSkeletalMesh`.
