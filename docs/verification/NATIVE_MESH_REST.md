# The rest of the made-Mesh step of the body composition (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-MESH-REST (decision 2026-11-19-NNN, number pending). Analyst
only: a different lane implements it (clean room). This lane's author implemented #542 and #557, so it does not
implement this note.

**Sources:**
- the script, read with `research/script_disasm.py`, plus a local scan of every script function for the names below;
- the installed packages, read through `ow-package --def-dump` (Maya's pawn archetype, body class and mesh template)
  and the project's layout tool (`tools/ghidra/class_layout.py`, see "How the fields were named");
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock).

There is no bl-sdk session and no recorded live evidence on these steps. Raw output stays under ignored
`local/analysis/mesh-rest/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

**Prerequisites** (open): NATIVE_INITIALIZE_BODY_COMPOSITION.md (#529, `research/rust-body-composition` 386a65a), its
implementation #542 (`fix/rust-body-composition` b2b4de8a), NATIVE_ANIM_NODE_LOOKUP.md (#543, b09720f6) and its
implementation #557 (`fix/rust-anim-node-lookup` 6803b3b).

**Why it matters.** #557 settles the made `Mesh`'s animation fields from the template. When they all settle, the step
still has two parts neither earlier note answers, and #557 refuses them with `NotYet { NotRead }`, "the new Mesh's
unidentified flag and its ReattachWeapons event" (`MESH_REST`; hub ruling, 21:49Z). The step is on the path of links
1, 3, 4, 5, 13 and 14 (Maya's attachment 0 is her `Mesh`).

## 1. The flag #529 does not identify (question a)

- **Name:** `SkeletalMeshComponent.bSkipAllUpdateWhenPhysicsAsleep`, **by fit** (UNVERIFIED; see "How the fields were
  named"). The hook sets it to true on the new mesh. It never clears it.
- **Where it is set:** only in `WillowPawn`'s `PostInitBodyComposition`, on the made `Mesh` (section 3, step 3.2).
- **Who reads it:** no script function reads it (local scan of every script function). Native readers were not
  searched. By its name, it is the engine's per-tick skeletal update skip while the mesh's physics sleeps:
  render-side and physics-side, not a field the slice's script or steps read.
- **On Maya:** her mesh template (`GD_Siren_Streaming.Pawn_Siren.SkeletalMeshComponent_7134`, which is both her pawn
  archetype's `Mesh` and attachment 0's template component) already holds it **true**, with class defaults applied.
  So the write leaves her value as it was.

**The modes 1 and 2 step** (not on the port's path; the body composition runs mode 0): `WillowPlayerPawn`'s
version adds a foot-IK step. It sets another flag of the mesh (by fit `bOverrideAttachmentOwnerVisibility`;
UNVERIFIED), and works through the tree's `WillowFootIK` controls. It was not read further.

## 2. The `ReattachWeapons` event (question b)

- **Who raises it:** only `WillowPawn`'s `PostInitBodyComposition`, by name, with no arguments. It is raised for
  identifier `Mesh` in modes 0 and 3, **whether or not the made object is a skeletal mesh component**. No script
  function calls `ReattachWeapons` (local scan of every script function in the nine code packages).
- **What script runs:** `WillowPawn.ReattachWeapons`, the only definition in the packages, state functions included
  (`WillowPlayerPawn` does not override it, and no state of either class does; the local scan lists every function
  export, those of states too):
  1. when `Pawn.Weapon` is set: `WillowWeapon(Weapon).AttachThirdPersonMeshToComponent(Mesh, IsLocallyControlled() &&
     IsFirstPerson(), false)`;
  2. when `Pawn.OffHandWeapon` is set: the same with the off-hand flag true;
  3. nothing else.
- **What it writes:** with both weapons None, **nothing**: each check fails, and the script returns. With a held
  weapon, `AttachThirdPersonMeshToComponent` writes a lot. In brief, it sets the weapon's `Instigator` from the mesh's
  owner when that is unset, and then:
  - with no `ThirdPersonMesh` it returns;
  - otherwise it clears `bSwappingWeaponAfterPutDown`, rebuilds the extra meshes, detaches the old third-person mesh,
    sets `bAttachedToInstigator`, may start the mesh's animations and muzzle flash, then attaches the third-person mesh
    to the pawn mesh's socket (on-hand or off-hand). It sets the shadow parent, owner-no-see and light environment,
    attaches each extra weapon slot's mesh the same way, and on authority replays the barrel and magazine spin states.

  Most of those are natives the port does not have.
- **On Maya:** her pawn archetype holds no `Weapon` and no `OffHandWeapon`. Whether the spawned pawn holds one when the
  body composition runs is **not read**. The body composition runs from `WillowPawn.BodySelectClass`, through
  `ApplyBodyClassStartingValues`. The default inventory is given by a 0.01 s timer set in `WillowPawn.PostBeginPlay`
  and by `GameInfo.RestartPlayer`. Their order against the class selection was not read.

## 3. The made-Mesh step, in order (question c)

`WillowPawn`'s `PostInitBodyComposition`, for identifier `Mesh`, in modes 0 and 3. `i` is the attachment's index.
1. The old `Pawn.Mesh` is released (#529, section 4.5; refused in the port, `releasing_old`).
2. `Pawn.Mesh` = the made object, as a `SkeletalMeshComponent` (None when it is not one).
3. Only when the new `Pawn.Mesh` is set:
   1. `MeshBodyCompositionIndex` = `i`, stored as a byte (the low 8 bits);
   2. the new mesh's `bSkipAllUpdateWhenPhysicsAsleep` = true (section 1);
   3. the five animation fields, in #543's order: `ArmBoneBlend`, `AimOffset`, `LeftHandIK`, `HeadLookAt`,
      `bUsingFootIK` (NATIVE_ANIM_NODE_LOOKUP.md);
   4. `Pawn.SlotNodes` is emptied, then rebuilt (#543, section 2);
   5. on authority (`Role` 3, `ROLE_Authority`): `RepMeshScale` = the new mesh's `Scale`. Clients apply it through
      `WillowPawn.ReplicatedEvent` (`Mesh.SetScale(RepMeshScale)` when it is not −1), the field's only script reader.
      Its stored default is 0, not −1: `--def-dump` gives 0 for the `WillowPawn` class default and for Maya's pawn
      archetype. The −1 is only the client's sentinel.
4. When `BodyClass` is set and its `bUseMeshForCollision` is true: `CollisionComponent` = `Pawn.Mesh` (None included).
   This runs whether or not step 3 ran.
5. The `ReattachWeapons` script event (section 2). This also runs whether or not step 3 ran.

Then come the steps for every identifier, unchanged from #529 section 4.5: the gear-likeness step, `HeadMesh` for
`CustomHead`, the mode-0 rendering setting and the light environment.

**On Maya** (mode 0, authority):
- `MeshBodyCompositionIndex` 0;
- the flag true (already so on her template);
- `RepMeshScale` 1 (her template's `Scale`);
- `CollisionComponent` untouched: `BodyClass_Siren.bUseMeshForCollision` is false;
- the event writes nothing if she holds no weapon (section 2).

## 4. Proposed Off answer (question d; for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces #557's `MESH_REST` refusal. The rest of #529's and #543's Off contracts
are unchanged. Everything is decided in the body composition's write-free preflight, before any write. Nothing is
loaded there (#518: a lazy load is a write).

**Preflight** (after #557's lookups have settled every field; a refusal there comes first):
1. **The flag:** the made component's class (from the template) declares `bSkipAllUpdateWhenPhysicsAsleep` as a bool.
   If not, `VmError::Script` naming the class (a malformed package set).
2. **The fields:** the pawn's class declares `Mesh` and `MeshBodyCompositionIndex`, and `CollisionComponent` when
   `bUseMeshForCollision` is set (rule 3). On authority it also declares `RepMeshScale` as a float, and the made
   component's class declares `Scale`. Otherwise `VmError::Script` naming the class and the field. "On authority" is
   the authority the plan already holds (#529, section 7, step 3), not a fresh read of `Role`.
3. **The collision write:** read `BodyClass.bUseMeshForCollision` as #542 already does, from a loaded body class only.
4. **The event**, checked in this order:
   1. resolve `ReattachWeapons` the way the VM dispatches an event: the pawn's current state first, then its class
      chain. Anything other than `WillowPawn.ReattachWeapons` → `NotYet { NotRead }`, "InitializeBodyComposition: an
      override of ReattachWeapons". The packages have none, state functions included (section 2), so this only guards
      a changed package set. It comes first: when both refusals apply, this is the one raised;
   2. when `Pawn.Weapon` or `Pawn.OffHandWeapon` is set → `NotYet { NotRead }`, "InitializeBodyComposition:
      ReattachWeapons with a held weapon (AttachThirdPersonMeshToComponent)", record this note. The weapon attach is
      not read, and it calls natives the port lacks;
   3. otherwise (both None) the event is answered: it writes nothing.

**Writes**, once #529's release check has passed, in section 3's order, each a plain property write under #542's
tracker:
- `Pawn.Mesh` = the made component;
- `MeshBodyCompositionIndex` = `i` as a byte;
- the made component's `bSkipAllUpdateWhenPhysicsAsleep` = true;
- the five fields as #557 settled them (None, None, None, None, and `bUsingFootIK` as found);
- `SlotNodes` empty;
- on authority, `RepMeshScale` = the made component's `Scale`, read after it was made from the template;
- `CollisionComponent` = `Pawn.Mesh` when `bUseMeshForCollision`.

The event is not called: with both weapons None its script writes nothing (section 2), and the preflight has proved
that. An implementer who prefers to run it can call the resolved script function through the VM as the step's last
write. A refusal inside it would then be marked "stopped after partial writes", so not calling it is the smaller
answer.

**A made object that is not a skeletal mesh component** (#542's `MESH_NOT_SKELETAL`): by section 3 it would be
answerable too (`Pawn.Mesh` None, step 3 skipped, steps 4 and 5 as above). That is a scope change for the hub. This
note does not ask for it.

**Reference** keeps the stub policy.

**On Maya** this answer is reached only once #557's lookups settle every field. They do not on her tree: `AimOffset`
and `LeftHandGrip` are found, and the tree has 3 slot nodes (#543). So her links still stop at #529's animation-node
`LayerAbsent` until the tree is instanced. This note removes the next stop after that one.

## How the fields were named

The executable gives each field as a position in its object. The layout tool computes those positions from the
packages, and each name above is the computed field at the position the code uses:
- **The pawn's fields:** `Mesh`, `CylinderComponent`, `SlotNodes`, `Role`, `CollisionComponent`,
  `MeshBodyCompositionIndex`, `ArmBoneBlend`, `LeftHandIK`, `AimOffset`, `HeadLookAt`, `bUsingFootIK`, `BodyClass`,
  `HeadMesh` and `LightEnvironment` fit exactly. `RepMeshScale` fits one field slot after its computed place, near the
  end of `WillowPawn`, where the tool's layout is not checked; the name is taken by fit and by its single script
  reader.
- **The body class:** `bUseMeshForCollision` fits exactly.
- **The skeletal mesh component:** the tool's layout is short of the code by a constant shift. That holds at
  `SkeletalMesh`, at the animation-behaviour interface, at `Animations` (before the component's map field) and at
  `ClothSim` (after it). The tool does not size a map field, so this lane gave it the engine's usual map size in a
  local copy of the tool, and that size is what makes the constant shift hold past it (**fitted**). The flag word lies
  between those checks, so the word is a fit with the shift applied. `Scale`, in `PrimitiveComponent`, fits with the
  same shift.
- **The bit within the flag word:** the hook sets the word's second bit. Packed from the low bit in the order the class
  declares them, the word's bools put `bSkipAllUpdateWhenPhysicsAsleep` second, after `bHasValidBodies`. That packing
  order is the compiler's usual one, but no oracle in this lane checks it, so the bit is a fit too (UNVERIFIED).
- **The word's other 31 bools** run in declaration order from `bHasValidBodies` to
  `bOverrideAttachmentOwnerVisibility`. The local scan of every script function finds one script reader among them:
  `bDisableFaceFX`, read by `Pawn.GetActorFaceFXAsset` and `SkeletalMeshActor.GetActorFaceFXAsset`, far from the
  second bit. Five more are only written by script: `bUpdateSkelWhenNotRendered`, `bRootMotionModeChangeNotify`,
  `bUpdateKinematicBonesFromAnimation`, `bUpdateJointsFromAnimation` and `bOverrideAttachmentOwnerVisibility`. The
  flag and its neighbours (`bHasValidBodies`, `bComponentUseFixedSkelBounds`) have no script reader, so a misfit by a
  bit or two is harmless in Off: no script the port runs reads a different value. A misfit as far as `bDisableFaceFX`
  would not be.

## Open questions

1. **UNVERIFIED:** the flag's name and its bit within the word (both fits; section "How the fields were named").
2. **UNVERIFIED:** `RepMeshScale`'s name, a fit one field slot after its computed place (same section).
3. **Not read:** whether Maya's pawn holds a weapon when the body composition runs (section 2).
4. **Not read:** the native readers of the flag.
5. **Not read:** the modes 1 and 2 foot-IK step.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns: `Pawn.MeshBodyCompositionIndex` 0, `RepMeshScale` equal to `Mesh.Scale` (1), and
   `CollisionComponent` her cylinder.
   - `RepMeshScale` 1 tells the write from the stored default (0).
   - `MeshBodyCompositionIndex` 0 does not: her pawn archetype already holds 0 (the class default is 255), so a read
     before and after `BodySelectClass` shows no change either way. To see the write, set a sentinel (say 7) with the
     SDK just before `BodySelectClass` and read it after, or use a body whose `Mesh` attachment index is not 0.
2. Hook `WillowPawn.ReattachWeapons`: it fires once during `BodySelectClass`. Log `Weapon` and `OffHandWeapon` at that
   moment.
3. For the flag's name: find a body class whose mesh template holds `bSkipAllUpdateWhenPhysicsAsleep` false, spawn that
   pawn, and read the flag on its `Mesh`: true confirms the fit. Maya cannot settle it, because her template already
   holds true.
