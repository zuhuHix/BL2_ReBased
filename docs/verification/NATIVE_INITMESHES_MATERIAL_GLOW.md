# InitMeshes after the weapon material: SetMaterial and the glow read (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-INITMESHES-SETMATERIAL-GLOW (decision 2026-11-25-39), the analyst
half of the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/glow/`.
This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule
here is UNVERIFIED in the running game.**

**Why.** After `BuildWeaponMaterial` (NATIVE_WEAPON_VISUALS.md section 1.1, #698), link 8's `init` runs
`WillowWeapon.InitMeshes` on. Its next natives are:
1. `MeshComponent.SetMaterial(0, WeaponMaterial)` on `FirstPersonMesh`, then on `ThirdPersonMesh`;
2. `WeaponMaterial.GetScalarParameterValue(WeaponTypeDefinition.GlowScaleMaterialParamName, BaseGlowScale)`, with
   the weapon's `BaseGlowScale` as the out argument.

Neither native is recorded (`tools/native_index_find.py`: no rows) or bound in the port.

## 1. Evidence, in the order asked

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`): the `InitMeshes` calls above. Then come `InitPartParticles`,
  `InitEffects`, `InitVisibleAmmoBones`, and each extra weapon slot's mesh set-up (which calls `SetMaterial` again).
- **The native** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - `MeshComponent.SetMaterial`'s exec and the routine it calls;
  - `MaterialInterface.GetScalarParameterValue`'s exec, which calls one material virtual;
  - that virtual's bodies for `MaterialInterface`, `MaterialInstance`, `MaterialInstanceConstant` and `Material`. The
    tables were found as in NATIVE_COMPONENT_SKIP_FLAG.md section 6;
  - field names from `tools/ghidra/class_layout.py` (a local copy sizing `MapProperty` for `Material`). Every field
    the bodies read lands on a declared property of the expected kind: `Materials`, `ScalarParameterValues`, `Parent`,
    `ReentrantFlag`, `Expressions`, `ParameterName` and `DefaultValue`. That is a structural check of the reading.

## 2. `MeshComponent.SetMaterial(int ElementIndex, MaterialInterface Material)`

One rule for every mesh component (the exec is `MeshComponent`'s, and it calls a non-virtual routine):
1. a negative `ElementIndex`: nothing;
2. an index within `Materials` whose entry is already `Material`: nothing;
3. otherwise:
   - when the index is past the end, `Materials` grows so that it reaches the index. By fit the new entries are empty
     (None);
   - `Materials[ElementIndex]` = `Material`;
   - then the **deferred reattach** that NATIVE_ATTACH_MESH_TO_PICKUP.md section 2 already reads. It sets
     `bNeedsReattach`; with an `Owner`, the owner's component update; with none, the component's own detach and
     reattach.

`Materials` is the component's own override list. It is not the mesh's materials.

## 3. `MaterialInterface.GetScalarParameterValue(name ParameterName, out float OutValue)`

The answer is a bool. `OutValue` is written **only when the answer is true**; otherwise the caller's variable keeps
its value. The rule depends on the material's class:
- **`MaterialInterface`, and `MaterialInstance` (which keeps it):** false;
- **`MaterialInstanceConstant`:**
  1. with its `ReentrantFlag` set: false (a guard against a parent cycle);
  2. the first `ScalarParameterValues` entry whose `ParameterName` equals the name (names compared with their
     number): its `ParameterValue`, and true;
  3. otherwise, with a `Parent`: the flag is set, the parent is asked the same question, the flag is cleared, and the
     parent's answer is the answer;
  4. otherwise false;
- **`Material`:** the first entry of its `Expressions` that is a scalar-parameter expression (by fit
  `MaterialExpressionScalarParameter`, which declares `DefaultValue`) and whose `ParameterName` equals the name: its
  `DefaultValue`, and true. Otherwise false. Cooked materials may hold an empty `Expressions`, and then the answer is
  false. This is the installed data's concern, not a rule.

**Writes.** `MaterialInstanceConstant` sets and clears its own `ReentrantFlag` around the parent call. The net state
is unchanged, and nothing else is written.

## 4. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Record: `docs/verification/NATIVE_INITMESHES_MATERIAL_GLOW.md`. Bind both natives
in `ParityMode::Off`. Reference is unchanged: neither is bound there, so the stub policy answers.

**Common.** Reads are loaded-only, by name lookup on resident objects. A non-resident object that must be read is a
`NotYet` of kind `NotRead`, "<native>: an object the store has not loaded (a load is a write)". A field that is
missing or of the wrong kind is a `VmError::Script` naming it. Class tests use classes that are already ready
(`ow-inventory` `globals::ready_class`), so no class code is loaded.

### 4.1 `SetMaterial`

**Preflight:**
1. a negative index: nothing;
2. an index within `Materials` (declared as an array of objects) whose entry is `Material`: nothing;
3. otherwise, the deferred reattach's preflight (`component_setters::reattach_check`): `Owner` set is
   `REATTACH_OWNER`; no `Owner` with a `Scene` set is `REATTACH_SCENE`. Both are a `NotYet` of kind `NotRead`, with
   the record NATIVE_ATTACH_MESH_TO_PICKUP.md.

**Commit:** grow `Materials` with None entries to reach the index, set the entry, then `bNeedsReattach` = true (the
deferred reattach's write with no owner and no scene).

### 4.2 `GetScalarParameterValue`

**By the receiver's first native class** (the fitted `CLASS_NATIVE` walk):
- `Engine.MaterialInterface` or `Engine.MaterialInstance`: false;
- `Engine.MaterialInstanceConstant`: section 3's steps. The guard is kept **without writing**: the port walks the
  parent chain with a visited set, and a chain that returns to an instance already being asked answers false, as the
  flag would. An instance whose `ReentrantFlag` is already set answers false;
- `Engine.Material`: section 3's expression walk. Each `Expressions` entry must be resident. The scalar-parameter
  test is an `IsA` against a ready `Engine.MaterialExpressionScalarParameter`;
- any other first native class (for example `MaterialInstanceTimeVarying`, not read): a `NotYet` of kind `NotRead`,
  "GetScalarParameterValue: the scalar lookup of <class>".

The whole walk is decided read-only. The answer is then committed: on true, `OutValue` is written through the out
argument; on false, nothing is written.

**What link 8 would meet** (a prediction, not a run):
- **`SetMaterial` on `FirstPersonMesh`:** the instance from `BuildWeaponMaterial` is new, so it differs from what slot
  0 holds. With the mesh's `Owner` set (the weapon), the answer is the deferred reattach's `REATTACH_OWNER`, the next
  stop. With no `Owner` and no `Scene`, the entry is set and so is `bNeedsReattach`. The same then happens for
  `ThirdPersonMesh`;
- **the glow read,** once reached: the instance is a `MaterialInstanceConstant` with no scalar entries of its own. So
  it asks its `Parent`, the Material part's instance (`Mati_*`). That instance answers from its own
  `ScalarParameterValues` if it lists `p_EmissiveScale`, and otherwise its parents in turn, up to the master
  `Material`'s expressions. With no match anywhere, the answer is false, and `BaseGlowScale` keeps its value.

## 5. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each preflight refusal leaves #518's three counters and the name table
unchanged, and causes no class-code load (where #685 has landed):
1. `SetMaterial(-1, M)`: nothing;
2. `SetMaterial(0, M)` with `Materials[0]` already `M`: nothing, and no reattach refusal even with `Owner` set;
3. `SetMaterial(2, M)` on a component with an empty `Materials`, no `Owner` and no `Scene`: `Materials` is
   (None, None, `M`), and `bNeedsReattach` is true;
4. `SetMaterial(0, M)` with an `Owner`: `REATTACH_OWNER`, nothing written. With no `Owner` and a `Scene` set:
   `REATTACH_SCENE`;
5. `Materials` not declared as an array of objects: `VmError::Script` naming it;
6. `GetScalarParameterValue` on a toy `MaterialInstanceConstant` with an entry for the name: true, and the out
   variable is written. With two entries of the same name, the first wins. A name with a different number does not
   match;
7. no entry, with a `Parent` that has one: the parent's value and true. The instance's `ReentrantFlag` is unchanged
   afterwards;
8. a parent cycle (A's parent B, B's parent A), neither with the entry: false, nothing written, no loop;
9. `ReentrantFlag` already set on the receiver: false;
10. a toy `Material` whose `Expressions` holds a scalar-parameter expression with that name: its `DefaultValue` and
    true. A non-scalar expression with the same name is skipped;
11. no match anywhere: false, and the out variable keeps its prior value;
12. a lazy `Parent`, or a lazy expression: the "not loaded" `NotRead`, nothing loaded;
13. a receiver of an unlisted native class: the class `NotRead`;
14. Reference: unchanged (unbound).

## Open questions

1. Not read: the scalar lookup of other material classes (`MaterialInstanceTimeVarying` and the rest); whether the grown
   `Materials` entries are zeroed (by fit).
2. UNVERIFIED in game: everything above. In particular, the out argument is left untouched on false, and the scalar
   expression class is identified by fit.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After a gun spawns, read its `FirstPersonMesh.Materials[0]` and `ThirdPersonMesh.Materials[0]`: expect the same
   instance, `WeaponMaterial`.
2. Call `WeaponMaterial.GetScalarParameterValue('p_EmissiveScale', f)` and compare `f` with `BaseGlowScale`. Then
   walk `Parent` and read each `ScalarParameterValues` to see which level answered.
