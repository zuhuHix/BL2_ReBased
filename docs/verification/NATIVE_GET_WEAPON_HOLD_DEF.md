# The body class's weapon-hold lookup: BodyClassDefinition.GetWeaponHoldDef (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-GETWEAPONHOLDDEF (decision 2026-11-25-63), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/holddef/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule
here is UNVERIFIED in the running game.**

**Why.** With lane E's store fix, link 1 gets through the whole body composition. It then stops at an **error**, the
unbound native `BodyClassDefinition.GetWeaponHoldDef(name, object, bool, bool)`. Links 3, 4, 5, 13 and 14 stop there
too. `native_index_find.py GetWeaponHoldDef` has no row. NATIVE_INITIALIZE_BODY_COMPOSITION.md does not cover it.
NATIVE_BYTECODE_OPCODES.md mentions only that its result goes into an object temporary.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`):
  - `BodyClassDefinition.GetWeaponHoldDef(name HoldName, EnvironmentTagDefinition EnvironmentTag, bool
    bSearchParentClass, bool bFallbackToDefaultDef)` is a native returning a `BodyWeaponHoldDefinition`;
  - its script callers are four helpers of `BodyClassDefinition`, and `WillowPawn.RequiresEnvironmentTransitionAnimation`;
  - the helpers are `GetWeaponHoldAnimSets` (passing false, true), `GetAimOffsetProfileForHoldName`,
    `ShouldUseUpperBodyMovementBlendForHoldName` and `ShouldUseArmBoneBlendForHoldName` (each passing true, true).
- **Maya's call site,** `WillowPawn.UpdateAnimSets`, right after `InitializePawnMeshes` in
  `ApplyBodyClassStartingValues`:
  - with no `OffHandWeapon` and no `Weapon`, the hold name is `Unarmed`;
  - the tag is the pawn's `CurrentAnimEnvironmentTag`;
  - it calls `BodyClass.GetWeaponHoldAnimSets(name, tag, WeaponAnimSets)`. That helper first calls itself on
    `WeaponActionParentClass` when that is set, then `GetWeaponHoldDef(name, tag, false, true)` and the hold's
    `GetAnimSets`;
  - then `GetAimOffsetProfileForHoldName`, `ShouldUseUpperBodyMovementBlendForHoldName` and, with `ArmBoneBlend` set,
    `ShouldUseArmBoneBlendForHoldName` (only when a weapon is held).
- **The native:** Ghidra 12.1.4 headless, read-only, under the machine lock. The exec and the lookup routine it calls
  were read.
- **Field names:** the local copy of `tools/ghidra/class_layout.py` places `BodyClassDefinition`'s
  `WeaponActionParentClass`, `DefaultWeaponHoldDef` and `WeaponHoldDefs`, and `BodyWeaponHoldDefinition`'s `HoldName`
  and `EnvironmentTag`, exactly where the routine reads them.
- **Maya's data,** with `ow-package --def-dump ... --cooked <CookedPCConsole>` (output under ignored
  `local/analysis/link1next/` and `local/analysis/holddef/`):
  - `GD_Siren_Streaming.BodyClass_Siren`: `WeaponActionParentClass` is `GD_PlayerShared.Character.BodyClass_PlayerShared`,
    and `DefaultWeaponHoldDef` is `GD_Siren_Streaming.WeaponHolds.WeaponHold_Siren_Default` (`HoldName` None).
    `WeaponHoldDefs` holds 7 entries: `..._Pistol`, `..._Shotgun`, `..._Rifle`, `..._SMG`, `..._RocketLauncher`,
    `..._SniperRifle` and `WeaponHold_Siren_Unarmed`;
  - `WeaponHold_Siren_Unarmed`: `HoldName` `Unarmed`, `EnvironmentTag` None, `AimOffsetProfileName` `Unarmed`,
    `bUseUpperBodyMovementBlend` true, `bUseArmBoneBlend` false;
  - `BodyClass_PlayerShared`: no `WeaponActionParentClass`, no `WeaponHoldDefs`, and `DefaultWeaponHoldDef`
    `GD_PlayerShared.WeaponHolds.WeaponHold_Player_Default`;
  - `Pawn_Siren`: `CurrentAnimEnvironmentTag` None, `Weapon` and `OffHandWeapon` None.

## 2. What GetWeaponHoldDef returns

On the receiving body class, with `HoldName` N, `EnvironmentTag` T, `bSearchParentClass` P and
`bFallbackToDefaultDef` F:
1. **The own list.** `WeaponHoldDefs` is scanned in order, skipping None entries. Among entries whose `HoldName` is
   N:
   - **an entry whose `EnvironmentTag` is also T is the answer at once** (the exact match);
   - otherwise a **name-only candidate** is kept. The first name match becomes the candidate. A later name match
     replaces it only when that later entry's `EnvironmentTag` is None. So the candidate ends up as the last
     tag-less name match after the first, or the first name match when no later one is tag-less.
2. **The parent** (P true and `WeaponActionParentClass` set). The same lookup runs on the parent class, with N, T and
   P, and with F passed on **only when this class has no `DefaultWeaponHoldDef`**. Then:
   - with a candidate from step 1: the parent's answer stands only when it is an exact match (name and tag).
     Otherwise, or when the parent answers None, the candidate is the answer. **The mechanism is a field read:**
     the caller re-reads the returned hold's own `HoldName` and `EnvironmentTag` and compares them with N and T. The
     recursion does not report how it matched. So a parent answer that is the parent's **default**, with N and T as
     its own name and tag, also counts as exact, and stands over the candidate;
   - with no candidate: the parent's answer, when it is not None.

   Which object the parent lookup runs on comes from the field the routine tests (by fit). The register that carries
   the receiver was lost in the decompile.
3. **The candidate,** when there is one.
4. **The default:** `DefaultWeaponHoldDef` when F is true, else None.

**It writes nothing.** It reads three fields of the body class and two of each hold definition, and calls nothing
else.

**What Maya's `maya:8` gets** (with no weapon, `Unarmed`, tag None):
- `GetWeaponHoldAnimSets` first runs on `BodyClass_PlayerShared`, with (false, true). It has no entries and no
  parent, so the answer is its default, **`WeaponHold_Player_Default`**;
- then on `BodyClass_Siren`, with (false, true): entry 7 is an exact match (`Unarmed`, None), so the answer is
  **`WeaponHold_Siren_Unarmed`**, at once;
- `GetAimOffsetProfileForHoldName` and `ShouldUseUpperBodyMovementBlendForHoldName` use (true, true) on
  `BodyClass_Siren`: the same exact match, **`WeaponHold_Siren_Unarmed`**. They give the profile name `Unarmed` and
  the blend flag true.

## 3. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `BodyClassDefinition.GetWeaponHoldDef(name, object, bool, bool)` in Off.
Reference keeps the stub policy. Reads only, **load-free** (no load, no class-code load, nothing interned):
1. **The receiver** must be resident. A body class that is a store handle not loaded yet (`DefStore::is_unloaded`, or
   the slotless check) is `NotYet { NotRead }`, "GetWeaponHoldDef on a body class not loaded at set-up (a load is a
   write)". Its class must declare `WeaponHoldDefs` (an array of object references), `DefaultWeaponHoldDef` and
   `WeaponActionParentClass` (object references). Otherwise `VmError::Script` naming the field.
2. **Each list entry read** (step 1 of section 2, in order): None is skipped. A store handle not loaded yet is the same
   `NotRead`, with "a weapon hold definition" in the text. This is checked before any field of the entry is read. Each
   entry read declares `HoldName` (a name) and `EnvironmentTag` (an object reference). Otherwise `VmError::Script`.
   Names compare by id; objects by identity.
3. **The parent**, only when P is true and the field is set: the same rule recursively, with the not-loaded check on
   the parent. A chain that comes back to a class it has already visited is `VmError::Script`. That is a port-only
   guard; the game's routine has none.
   - **The returned hold's re-read** (only when this class has a candidate and the parent answers non-None): the
     parent's answer is read for `HoldName` and `EnvironmentTag`, as step 2 reads an entry. A store handle not loaded
     yet is the same `NotRead` ("a weapon hold definition"), checked before its fields are read. This can be an
     object no earlier step read, since the parent's `DefaultWeaponHoldDef` is returned without being read.
4. **The answer**, as section 2 orders it. An object reference, or None.
5. **Nothing is written.**

**The slice's data and the store.** `BodyClass_PlayerShared` and the seven hold definitions are references of Maya's
body class, and the VM's ordinary field reads load such handles. This native's answer must not. If they are still
handles when `UpdateAnimSets` runs, the answer is the not-loaded `NotRead`. That is the honest answer. Loading them
first is a labelled harness input (as #592's mesh preload): **the hub's call.**

**What `maya:8` meets next** (in `UpdateAnimSets`, then `ApplyBodyClassStartingValues`; a prediction, not a run):
1. `BodyWeaponHoldDefinition.GetAnimSets`: script. It walks `ParentDefinition` then `AnimSetList`, adding each set not
   already listed;
2. the anim sets appended to `Mesh.AnimSets` (script array writes);
3. `WillowPawn.SetAimOffsetProfile('Unarmed')` → `AimOffset.SetActiveProfileByName(...)`: **a native with no note**,
   reached when `AimOffset` is set (the body composition's hook sets it from `AimOffsetNodeName`);
4. `SetUpperBodyMovementBlendState(true)` (script) → `FindAllAnimNodes`, `SetBlendTarget` and `SetActiveChild`:
   natives with no notes. Then `UpdateAnimations` (NATIVE_BONE_TRANSFORM_LAYER.md);
5. `SetupSkeletalMeshPhysics` (script): `SetRBChannel`, `SetBlockRigidBody`, `SetActorCollision`, `SetTraceBlocking`,
   `SetNotifyRigidBodyCollision` and `InitializeRBPhysics`. Mostly no notes.

## 4. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each refusal leaves #518's three counters and the name table unchanged, and
`class_code_loads` unchanged:
1. **maya:8's shape:** a toy body class with entries (`Pistol`, None) and (`Unarmed`, None), a default, and a parent
   with no entries and its own default:
   - `GetWeaponHoldDef('Unarmed', None, false, true)` answers the `Unarmed` entry;
   - on the parent, the same call answers the parent's default;
2. no match, F true: the default. F false: None;
3. name matches with other tags only: the first name match is the candidate, ahead of the default. The shape of
   every armed-hold call on the slice: (`A`, None) asked against [(`A`, X)] answers (`A`, X), ahead of the default;
4. name matches (`A`, tag X), then (`A`, None), then (`A`, tag Y), asked (`A`, tag Z): the answer is the (`A`, None)
   entry (the candidate rule);
5. the exact match after a candidate: entries (`A`, None), (`A`, Z), asked (`A`, Z): the second;
6. **the parent:**
   - P true, no own match: the parent's exact match is the answer;
   - P true with an own candidate and a parent that matches by name only: the own candidate;
   - P false: the parent is not read;
   - this class has a default and the parent does too, with no matches anywhere: this class's default (F is not
     passed to the parent);
   - **F passed on:** this class has no default, no own match, F true: the parent's default;
   - the same with an own candidate, and a parent default whose name or tag differ from (N, T): the own candidate;
   - **the re-read quirk:** this class has a candidate and no default, F true, and the parent answers its own
     `DefaultWeaponHoldDef`, whose `HoldName` and `EnvironmentTag` equal (N, T): **the parent's default**, over the
     candidate;
   - the parent's returned default is a lazy handle while this class has a candidate: the not-loaded `NotRead`,
     nothing loaded;
7. None entries in `WeaponHoldDefs` are skipped;
8. a lazy entry, a lazy parent, or a lazy receiver: the not-loaded `NotRead`, nothing loaded;
9. a malformed declaration (`WeaponHoldDefs` not an array of object references, `HoldName` not a name):
   `VmError::Script`;
10. a parent chain that loops: `VmError::Script` (the port's guard);
11. no field of any object changes;
12. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above. The routine is read whole. The parent recursion's receiver is a fit by the field
   it tests.
2. **Not read:** `AnimNodeAimOffset.SetActiveProfileByName`, and the `SetUpperBodyMovementBlendState` natives (the
   next stops).
3. **The hub's call:** a harness preload of the body class's hold definitions and its parent, if they are store
   handles at `UpdateAnimSets`.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Call `Maya.BodyClass.GetWeaponHoldDef('Unarmed', None, true, true)`. Expect `WeaponHold_Siren_Unarmed`. Ask
   `('NoSuchHold', None, true, true)` and expect `WeaponHold_Siren_Default`.
2. Read `Mesh.AnimSets` right after spawn. Expect the parent's default hold's sets and the `Unarmed` hold's sets.
