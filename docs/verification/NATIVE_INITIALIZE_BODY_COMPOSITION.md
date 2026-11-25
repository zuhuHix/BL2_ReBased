# Native rules of InstanceDataHelper.InitializeBodyComposition (draft, 2026-11-19, review r1 of #529 folded in)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-BODY-COMPOSITION (decision 2026-11-19-140, number pending).
Analyst only: a different lane implements it (clean room). This lane's author implemented the caller's Off rule
earlier (IMPL-INIT-PAWN-MESHES, #522), so that author must not implement this note.

**Sources:**
- the installed script, read with `research/script_disasm.py` (every function of the nine code packages);
- the installed packages, read with `ow-package --object-dump` (Maya's body class) and the class and struct layouts from
  `tools/ghidra/class_layout.py`;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries and a local call-site
  listing, under the machine lock), for what neither settles. The engine's fixed name table was read from the image to
  name the identifiers the code compares against (`Parent`, `CylinderComponent` and `Mesh`).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/bodycomp/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

## 0. Prerequisites, what this note supersedes, and the hub's rulings

**Land order.**
1. #517 first: NATIVE_INIT_PAWN_MESHES.md, at `research/rust-init-pawn-meshes` 3217b58b.
2. Then #522, the pawn wrapper's Off rule, at `fix/rust-init-pawn-meshes` 1551f039.
3. Then the implementation of this note.

NATIVE_SET_ANIM_TREE_TEMPLATE.md is cited at `research/rust-set-anim-tree-template` 466350b6. None of these is on
`main` at the time of writing.

**Superseded in NATIVE_INIT_PAWN_MESHES.md** (at 3217b58b) by this note:
- section 3, steps 5.2 and 5.3: the "base first" creation and the parent picked by `ParentAttachmentIdentifier`. Both
  are wrong (section 3 below);
- section 3, step 6: the zero-width switch fallback is open no more (section 5 below);
- section 4: "`BitsRequiredForMaxSwitchValue` 0" for Maya's switches (it is 1), and "13 components made" (10 are);
- section 6, "Not read": the zero-width packing, the no-parent fallback and the replay. Section 7 below gives the Off
  answer for each;
- the live check's expectation of "component entries for the 13 component attachments": it is 10.

The rest of that note (the wrapper's gate, its preflight and its root-motion writes) stands. Section 7 keeps the
root-motion writes.

**The hub's rulings on #529 (2026-11-19), all UNVERIFIED port answers:**
1. A component made from a template counts as valid (the validity check, section 4.1 step 3).
2. A custom head or skin entry is appended and nothing is applied (section 4.3).
3. The `Parent` refresh stays `NotRead` (it was the special-name refresh). Section 4.1 step 7 now reads it whole; the
   hub may revisit this ruling with that reading.
   *Lifted 2026-11-25 by lane IMPL-CUSTOMHEAD-PARENT-LINK (decision 2026-11-25-NNN): the link is answered from section 8.1.*

## 1. Who calls it

- **Script** (`InstanceDataHelper.InitializeBodyComposition(Owner, BodyComposition, InstanceState, ReplicatedState,
  optional Mode)`, static, native). Six callers:
  - `Perch.InitBody`, `WillowPopulationPoint.PostBeginPlay` and `WillowVehicle.PostBeginPlay`: their own
    instance-state fields, mode 0;
  - `WillowInteractiveObject.InitializeFromDefinition`: its replicated state is a member of a nested struct
    (`ReplicatedState.ReplicatedInstanceDataState`), mode 0;
  - `PlayerStandIn.Initialize` (a menu stand-in; replicated state is a local): mode 5 in the foreground, else 4;
  - `VehicleStandIn.CreatePreviewMesh` (a menu preview; replicated state is a local): mode 1.
- **Native.** The pawn's `InitializePawnMeshes` runs the script function on the `InstanceDataHelper` class default
  object, so it runs the same body with mode 0 (NATIVE_INIT_PAWN_MESHES.md section 2). One more native caller serves the
  player pawn's menu state (`WillowPlayerPawn.SetInstanceDataMenuState`), off the slice. One further native caller (a
  virtual of a class that keeps its own instance state) was not identified, and no slice class matched it.
- **On the slice:** only the pawn path, through `WillowPawn.ApplyBodyClassStartingValues` (script; its first statement
  is `InitializePawnMeshes`). Links 1, 3, 4, 5, 13 and 14 reach it through Maya's set-up (`maya:8`).
- **Links 6 and 7 never call it.** Their `make` step goes through `WillowWeapon.AttachMeshToPickup`
  (NATIVE_SET_ANIM_TREE_TEMPLATE.md section 1). They need only part of the same layer (section 6).

**After the build,** `ApplyBodyClassStartingValues` goes on (script) with `UpdateAnimSets`, `SetupSkeletalMeshPhysics`
(both script), the body class's `Mass`, relevance, `CrouchHeight`, behavior providers, death definition and rotation
flags. Which native those reach first was not probed.

## 2. What it does, in order

The entry unpacks owner, composition, the instance-data set (`InstanceState`), the replicated state and the mode, then
runs the build.

1. **No owner:** nothing.
2. **The existing entries.**
   - The owner's `InstanceState.Data` is copied into a temporary old list.
   - Then a per-entry step runs over the existing entries, from the last to the first. For a component entry it
     involves the entry's component and its `DeleteByOwner` flag; it was not read in full.
   - Then `Data` is emptied, and room is reserved for `BodyComposition.MaxExpectedComponents` entries, or the
     attachment count when that is 0.
   - **New entries are appended from index 0.** With `Data` empty on entry, as for a first build, none of this
     changes anything.
3. **Nothing more when the attachment list is empty.** Otherwise:
4. **The replayer:** the owner's `IBodyCompositionInstance` interface, when the owner implements it (`WillowPawn`
   does). It is used per made attachment (section 3, step 6).
5. **Per-attachment records** (temporary, not script-visible), one per attachment, in order: no object, no entry index,
   "not made", and "removed" when the attachment's index is below 32 and its bit is set in
   `ReplicatedInstanceDataState.RemovedBodyCompositionPartsBitField`.
6. **The walk** (section 3).
7. **After the walk:** on authority (owner `Role` 3), the switch packing (section 5), then the owner's `bNetDirty` and
   `bForceNetUpdate` are set. Otherwise, with `ReplicatedInstanceDataState.bNeedToApplyThisState` set, the replicated
   state is applied (not read) and the flag is updated from the result.
8. The temporaries are freed.

## 3. The walk (each attachment once, in list order)

For attachment `i` (a `BodyInstanceDataUnion`: `Data`, an `InstanceDataUnion`; `ParentAttachmentIdentifier`;
`AttachmentBaseInstanceDataIndex`; `SwitchIdentifier`):
1. **A record already "made" and "removed"** (a rebuild only): torn down (not read).
2. **"Removed" and not made:** skipped.
3. **No base** (`AttachmentBaseInstanceDataIndex` -1 or not below the attachment count): it is made, with **the owner**
   as its parent.
4. **A base**, by the base's record as it stands at this point (there is **no "base first" step**: a later base is not
   made early):
   - **not made** (later in the list, removed, or refused): this attachment is not made;
   - **a switch** (the base attachment's `Data.Type` is 9): made only when the switch's entry has `Int` equal to this
     attachment's `SwitchIdentifier` **and** either |`Float`| < 0.00000001 (strictly) or
     `BitsRequiredForMaxSwitchValue` = 0. Otherwise it is not made. The switch entry's `Float` is 0 when it is made
     (section 4.2), so on a first build the test is `Int` = `SwitchIdentifier`;
   - **any other made base:** made.
   - **The parent** is the base record's object. A switch's record holds **its own parent's object**: the owner for a
     baseless switch, else its base's object. So a switch child's parent is the switch's parent.
5. **Making it**, by `Data.Type` (section 4). A made entry is appended to `InstanceState.Data` at once. The record
   notes the object, the new entry's index, and the object's `IWorldBody` interface.
6. **After making a component** (a switch is made too, but skips this step; a custom head or skin appends its entry
   yet counts as **not made**, so it skips it as well, section 4.3):
   - the menu modes' visibility toggles (modes 1, 2, 4 and 5; mode 0 does nothing here);
   - when the owner has the replayer (section 2, step 4):
     1. the entry's per-entry step. It registers a hit region when `ComponentData.HitRegionDefinition` is set and the
        owner takes hit regions; with a global "attaching later" state set, another step runs first (not read);
     2. the replayer's `PostInitBodyComposition(ParentAttachmentIdentifier, the made object, i, mode)`. For a pawn,
        that is section 4.5.

## 4. Making an entry, by type

**The entry contract.** Each new entry is an `InstanceDataUnion` whose fields start at their zero values (`Name`
None, `Type` 0, `Replication` 0, `MaxSwitchValue` 0, `BitsRequiredForMaxSwitchValue` 0, `Bool` false, `DeleteByOwner`
false, `Int` 0, `Float` 0, `Vector` (0, 0, 0), `Object` None, `ComponentData` all zero or None, `String` empty,
`BodyCompositionIndex` 0). Each type then sets only what its table lists. `BodyCompositionIndex` is always **copied
from `Data`**: the packages hold `i` + 1 there, so the entry carries `i` + 1.

### 4.1 `IDT_ActorComponent` (5)

| Entry field | Value |
|---|---|
| `Name` | copied from `Data` |
| `Type` | 5 |
| `DeleteByOwner` | copied from `Data` |
| `ComponentData` | copied from `Data` (`CollisionType`, `bAttachToMesh`, `bIsUsable`, `bIsSecondaryUsable`, `MeshSocketName`, `HitRegionDefinition`, `InteractIcon`, `SecondaryInteractIcon`), with `Component` **replaced** by the new component |
| `BodyCompositionIndex` | copied from `Data` |
| everything else (`Replication`, `MaxSwitchValue`, `BitsRequiredForMaxSwitchValue`, `Bool`, `Int`, `Float`, `Vector`, `Object`, `String`) | zero |

1. **No template** (`Data.ComponentData.Component` None) or no owner: nothing is made, and the record stays "not made".
2. **The component:** a new component is constructed from the template, with the owner as its outer.
3. **The validity check** (not read). **Hub ruling 1:** a component made from a template is valid.
4. **The entry** is appended.
5. **Collision**, when the new component is a primitive, from `ComponentData.CollisionType` (`Actor.ECollisionType`):

   | Value | The component's flags | The owner's flags |
   |---|---|---|
   | 0 | unchanged | unchanged |
   | 1 | `CollideActors`, `BlockActors`, `BlockRigidBody` cleared | unchanged |
   | 2 | `CollideActors`, `BlockActors`, `BlockNonZeroExtent`, `BlockRigidBody`, `BlockZeroExtent` set | `bCollideActors`, `bBlockActors`, `bPushedByEncroachers` set |
   | 3 | `CollideActors`, `BlockActors`, `BlockZeroExtent` set; `BlockNonZeroExtent`, `BlockRigidBody` cleared | `bCollideActors`, `bBlockActors` set |
   | 6 | `CollideActors`, `BlockActors`, `BlockNonZeroExtent`, `BlockRigidBody` set; `BlockZeroExtent` cleared | `bCollideActors`, `bBlockActors`, `bPushedByEncroachers` set |
   | 9 | `CollideActors`, `BlockActors`, `BlockNonZeroExtent`, `BlockZeroExtent` set; `BlockRigidBody` cleared | `bCollideActors`, `bBlockActors` set |
   | 4 | `CollideActors`, `BlockNonZeroExtent`, `BlockRigidBody`, `BlockZeroExtent` set; `BlockActors` cleared | `bCollideActors`, `bPushedByEncroachers` set |
   | 5 | `CollideActors`, `BlockZeroExtent` set; `BlockActors`, `BlockNonZeroExtent`, `BlockRigidBody` cleared | `bCollideActors` set |
   | 7 | `CollideActors`, `BlockNonZeroExtent`, `BlockRigidBody` set; `BlockActors`, `BlockZeroExtent` cleared | `bCollideActors`, `bPushedByEncroachers` set |
   | 8 and above 9 | `CollideActors` cleared | unchanged |

   The component's flags are `PrimitiveComponent`'s, named by matching their bit pattern against the layout tool's
   declaration (shifted in the engine); the pattern fits exactly. The owner's flags are `Actor`'s, at the
   tool's places. **UNVERIFIED.**
6. **The attach** (only after the entry is appended):
   1. **First attempt:** the parent's `IWorldBody.WorldBodyAttachComponent`, with the component, a zero location, a
      zero rotation and `ComponentData.MeshSocketName`.
   2. **If that answers false:** the owner's `WorldBodyAttachComponent`, with the component, zero location and rotation,
      and **no socket name** (None).
   3. **If that answers false too,** nothing more happens: the component and its entry stay, unattached.

   What a target does:
   - **An actor (the pawn):**
     - a component already being attached through this path is refused;
     - with a socket name: when the actor has a named socket component of its own (one engine class, not
       identified), the attach is relative to it; otherwise the call goes to the actor's attach base. For a pawn that
       base is its `Mesh`, with that mesh's answer;
     - with no socket name: the component's `Translation` and `Rotation` stay as they are (the offsets are zero),
       the component is attached to the actor (by fit, its `Owner` set), and it is appended to the actor's `Components`
       list if not already there (by fit). While the global "attaching later" state is set, it is queued instead (not
       read). It answers true.
   - **A `SkeletalMeshComponent`:**
     - the name is matched against its mesh asset's sockets first, then its bones;
     - with no mesh asset, no name, or no match, it answers false;
     - on a match, the component is attached to that bone, with the socket's offset and unit scale. By fit, that is an
       entry in the skeletal component's `Attachments` list. It answers true.
7. **The `Parent` link:** when `ParentAttachmentIdentifier` is `Parent`, the parent object is a
   `SkeletalMeshComponent` and the new component is one too:
   - the new component's parent-animation field (by fit `ParentAnimComponent`) is set to the parent;
   - then its parent-bone map (by fit `ParentBoneMap`) is rebuilt:
     - it is emptied, and filled only when both components have a skeletal mesh asset;
     - it gets one slot per bone of the new component's mesh;
     - when both use the same mesh asset, or a flag of the new component is set (not identified), slot `k` holds `k`
       *(identified 2026-11-25 in section 8: `bIsGestaltAccessoryMesh`)*;
     - otherwise slot `k` holds the parent mesh's bone index with the same name as bone `k`, or the no-match index;
   - **hub ruling 3:** `NotRead` in Off (section 7).

### 4.2 `IDT_Switch` (9)

The whole `Data` is copied: every field, `Object` and `Replication` included. Then **`Float` is set to 0**. The earlier
note's "its object cleared" is wrong: the object is copied. No object is made. The record's object is the switch's
parent's object (section 3, step 4).

### 4.3 `IDT_CustomHead` (11) and `IDT_CustomSkin` (12)

| Entry field | Value |
|---|---|
| `Name` | copied from `Data` |
| `Type` | 11 or 12 |
| `BodyCompositionIndex` | copied from `Data` |
| everything else, **`Replication` and `Object` included** | zero |

Then the owner's customization of that kind is looked up and applied, using `Data.Object` (the default head or skin
definition). **Hub ruling 2:** the entry is appended and nothing is applied. The type step gives back no object, so the
attachment's record counts as **not made**: no `PostInitBodyComposition` runs for it, and nothing can be based on it.

### 4.4 Others

`IDT_Object` (4) loads an object and appends an entry (with `DeleteByOwner` set and the socket name copied);
`IDT_ActorTemplate` (7) and `IDT_PickupTemplate` (10) spawn. None occurs in Maya's body class; they were not read
further. The plain value types (`IDT_Bool`, `IDT_Int`, `IDT_Float`, `IDT_Vector`, `IDT_String`, `IDT_Actor`) make
no entry.

### 4.5 The pawn's `PostInitBodyComposition` (the replayer, for a `WillowPawn` owner)

`WillowPlayerPawn`'s version calls `WillowPawn`'s, and in modes 1 and 2 adds a foot-IK step. In mode 0 it is
`WillowPawn`'s alone. Its arguments are the attachment's identifier, the made object, the index `i` and the mode. In
modes 0 and 3:
1. **Identifier `Mesh`:**
   - the old `Pawn.Mesh` is released: if it was attached, it is detached and removed from a global list. **That list
     was not read:** neither what it holds nor what the removal changes. Maya's pawn archetype
     (`GD_Siren_Streaming.Pawn_Siren`) sets `Mesh`. Whether the spawned pawn still holds it, attached, when the build
     runs is not read (**UNVERIFIED**). If it does, the release runs; in the port as #542 (b2b4de8a) measured it, the
     pawn holds no `Mesh` and nothing is released;
   - `Pawn.Mesh` = the made object, as a `SkeletalMeshComponent` (None when it is not one).
   - **When the new mesh is set:**
     - `MeshBodyCompositionIndex` = `i`;
     - one flag of the mesh is set (not identified);
     - `ArmBoneBlend` = the mesh's animation node named `ArmBoneBlend` (as an `AnimNodeBlendPerBone`);
     - `AimOffset` = the node named by `BodyClass.AimOffsetNodeName` (as a `WillowAnimNodeAimOffset`);
     - `LeftHandIK` = the skeletal control named `LeftHandGrip` (as a `SkelControlLeftHandGripWeapon`);
     - `HeadLookAt` = the control named by `BodyClass.HeadLookAtName` (as a `WillowSkelControl_LookAtActor`), None
       with no `BodyClass`;
     - `bUsingFootIK` = whether a control named `WillowFootIK` exists;
     - `Pawn.SlotNodes` is emptied, then rebuilt from the mesh's animation nodes (by fit, its slot nodes; not read in
       detail);
     - on authority, `RepMeshScale` = the mesh's scale.
   - `CollisionComponent` = the new `Mesh`, when `BodyClass.bUseMeshForCollision` is set.
   - The pawn's script event **`ReattachWeapons`** is called.
2. **Identifier `CylinderComponent`:**
   - the old `Pawn.CylinderComponent` is released as above. Stock `Pawn` defaults give the pawn a cylinder, and
     Maya's archetype sets `CylinderComponent`, so this release runs on a first build too;
   - `Pawn.CylinderComponent` = the made object, as a `CylinderComponent`;
   - `CollisionComponent` = it, when `BodyClass.bUseMeshForCollision` is clear.
3. With either identifier or none, when two flags allow it (not read, including whose flags they are): a
   gear-likeness step on the made object (a `WillowGame` interface; render-side, by fit; not read).

Then, in every mode:
4. When the attachment's **`Data.Name`** is `CustomHead`: `HeadMesh` = the made object, as a `SkeletalMeshComponent`.
5. A primitive made object in mode 0, when a pawn flag and an engine flag allow it (neither read): one rendering
   setting on it (not read).
6. With the **pawn's `LightEnvironment`** set: the made object, when it is a mesh component, gets it as its light
   environment. Maya's archetype sets it (`MyLightEnvironment`, a `DynamicLightEnvironmentComponent`).

**Hub ruling (last hub read of #529), the port's answer, UNVERIFIED:** the unread flags of steps 3 and 5 are taken as
allowing the hook.

## 5. The switch packing (authority, after the walk)

1. `SwitchStateBitField` = 0 and `ActiveSwitchValues` = 0.
2. Each attachment of `Data.Type` 9, in list order, takes the next position `k` (0, 1, 2, ...); after 32 switches,
   the packing stops.
3. Its entry is the first entry of `InstanceState.Data` of type 9 whose `BodyCompositionIndex` − 1 is the attachment's
   index.
   - **No such entry** (the switch was not made): no bit is set, and the offset does not move. Position `k` is still
     used up.
   - **An entry:** bit `k` of `SwitchStateBitField` is set. The width is the entry's `BitsRequiredForMaxSwitchValue`,
     or, when that is 0, its `Float` truncated to an integer.
   - **When offset + width ≤ 32:** `ActiveSwitchValues` |= `Int` << (offset mod 32), with no mask to the width, so a
     value too wide for its width runs into the next fields; then offset += width. Otherwise the value is not packed
     and the offset stays.
   - **A width of 0** packs `Int` at the current offset and leaves the offset where it is.
4. `RemovedBodyCompositionPartsBitField` and `bNeedToApplyThisState` are not touched.

For Maya, the three switches each have an entry with width 1 and `Int` 0, so `SwitchStateBitField` = 7 and
`ActiveSwitchValues` = 0.

## 6. What the slice's Maya takes

`GD_Siren_Streaming.BodyClass_Siren` (a `BodyClassDefinition`): `MaxExpectedComponents` 0, `AimOffsetNodeName`
`AimOffset`, and `HeadLookAtName` and `bUseMeshForCollision` at their defaults (None, false; from the class default,
UNVERIFIED). There are 18 attachments; every `Replication` is 0 except the two customizations (2), and no attachment has
a `HitRegionDefinition`.

| # | Type | `Data.Name` | Base | Socket | Identifier | Template | First build |
|---|---|---|---|---|---|---|---|
| 0 | component | None | none | none | `Mesh` | `SkeletalMeshComponent` | made; attached to the pawn; becomes `Pawn.Mesh` (4.5) |
| 1 | component | `CustomHead` | 0 | `Root` | `Parent` | `CustomSkeletalMeshComponent` | made on 0; the `Parent` link (4.1 step 7); becomes `HeadMesh` |
| 2-6, 9, 12 | component | None | 0 | `Spine3` (2, 3), `Shield`, `ClassMod`, `R_Thigh`, `Hips`, `Launcher` | none | `GearLikenessMeshComponent` | made on 0 |
| 7 | switch | `ColdBreath` | 0 | `Head` | none | | entry |
| 8 | component | None | 7 (value 1) | `Head` | none | `ParticleSystemComponent` | **not made** |
| 10 | switch | `BlightPhoenix` | 0 | `BlightPhoenixFX` | none | | entry |
| 11 | component | None | 10 (value 1) | `BlightPhoenixFX` | none | `ParticleSystemComponent` | **not made** |
| 13 | component | None | none | none | `CylinderComponent` | `CylinderComponent`, collision 2 | made; attached to the pawn; becomes `Pawn.CylinderComponent` and `CollisionComponent` |
| 14 | switch | `ViewingStatusMenu` | none | none | none | | entry |
| 15 | component | None | 14 (value 1) | none | none | `StaticMeshComponent` | **not made** |
| 16 | custom head | None | none | none | `CustomHead` | | entry |
| 17 | custom skin | None | none | none | `CustomSkin` | | entry |

**So the first build appends 15 entries, at indices 0 to 14**: those of attachments 0 to 7, 9, 10, 12, 13, 14, 16
and 17, in that order. 16 and 17 have entries but count as not made (section 4.3). Attachments 0 and 13 attach to the pawn. 1 to 6, 9 and 12 attach to attachment 0's new skeletal
component at their sockets or bones. Attachment 13's collision (value 2) sets the cylinder's and the pawn's flags as
section 4.1's table says. The pawn's `PostInitBodyComposition` runs for the made components only: 0, 1, 2 to 6, 9,
12 and 13. On authority: switch bits 7, values 0, then `bNetDirty` and `bForceNetUpdate`.

## 7. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** **Scope:** this contract covers the pawn wrapper only:
`WillowPawn.InitializePawnMeshes` and `WillowPlayerPawn.InitializePawnMeshes`, mode 0, with the pawn's own
`InstanceDataState` and `ReplicatedInstanceDataState` as the destinations. The script native, for the other callers
(nested or local destinations), stays outside it: **hub ruling**, it is `NotYet { NotPorted }`, "InitializeBodyComposition
called from script (its destinations)".

**The wrapper,** as NATIVE_INIT_PAWN_MESHES.md section 6 (at 3217b58b) and #522 code it: the receiver, the helper class,
the loaded `BodyClass`, the attachment gate. Its `LayerAbsent` refusal is replaced by the build below. After the build,
inside the gate, it keeps the root-motion writes: `Mesh.RootMotionMode` = 3 when `BodyClass.UseRootMotion`, then
`Mesh.RootMotionRotationMode` = 1 when `BodyClass.UseRootRotation`. That `Mesh` is the new `Pawn.Mesh`.

**Preflight**, before any write (under the write-tracker, a load counts as a write: decision of review r2 of #518):
1. **Shapes:** the composition is a struct with an `Attachments` array of `BodyInstanceDataUnion`. The pawn declares
   `InstanceDataState` (an `InstanceDataSet`) and `ReplicatedInstanceDataState`. Otherwise `VmError::Script`.
2. **A rebuild:** `InstanceDataState.Data` not empty → `NotYet { NotRead }`, "InitializeBodyComposition: rebuilding
   an existing body composition (the old entries' teardown and replay)".
3. **Authority:** `Role` not 3 with `bNeedToApplyThisState` set → `NotYet { NotRead }`, "InitializeBodyComposition:
   the client-side apply of a replicated state".
4. **Plan the walk** (section 3), without writing, deciding for each attachment "made" or "not made" and its parent.
   Then, for each attachment that will be made:
   - `Data.Type` 4, 7 or 10 → `NotYet { NotRead }`, "InitializeBodyComposition: object and template attachments";
   - **a component:**
     - `CollisionType` outside 0 to 9 is still answered by the table (it is read whole);
     - `HitRegionDefinition` set → `NotYet { NotRead }`, "InitializeBodyComposition: registering a hit region";
   - **the attach**, both attempts resolved now:
     - a skeletal parent with a socket name, whose mesh asset's socket and bone tables are not loaded or not decoded →
       `NotYet { LayerAbsent }`, "InitializeBodyComposition: the socket and bone tables of <mesh>", layer "a skeletal
       mesh data reader (sockets and the reference skeleton)";
     - an actor parent with a socket name: the actor's named socket components are not read → `NotYet { NotRead }`,
       "InitializeBodyComposition: an actor's own named socket components";
     - a first attempt that answers false: the owner retry with no socket name, as an actor attach;
   - **the `Parent` link** (identifier `Parent`, both skeletal) → `NotYet { NotRead }`, "InitializeBodyComposition:
     the Parent link's parent-bone map" (hub ruling 3). Maya's attachment 1 meets this, but only after attachment 0
     and attachment 1's socket attach have passed ("Where Maya stops", below). The reading in section 4.1, step 7 is whole except for one flag of the component. A port answer is
     possible if the hub revisits ruling 3: set the link, and fill the map from the two meshes' bone tables, with the
     same mesh-data layer as the socket attach. *Section 8.1 (2026-11-25) gives that answer, with the flag
     identified, for the hub's revisit;*
   - **the pawn's `PostInitBodyComposition`** (section 4.5):
     - **the release of the old field**, for identifier `Mesh` or `CylinderComponent`, when the old `Pawn.Mesh` or
       `Pawn.CylinderComponent` is set and attached (or the port cannot tell whether it is attached) →
       `NotYet { NotRead }`, "releasing the old Mesh/CylinderComponent" (hub ruling, 20:03Z: the global list it is
       removed from is not read, section 4.5). An old field that is None, or set but not attached, releases nothing.
       With stock `Pawn` defaults the old cylinder is set, so a first build meets this;
     - after the release check: identifier `Mesh` with a made `SkeletalMeshComponent` → `NotYet { LayerAbsent }`, "InitializeBodyComposition:
       the new Mesh's animation nodes (ArmBoneBlend, AimOffset, LeftHandGrip, the head look-at, WillowFootIK and the
       slot nodes)", layer "an animation-tree instance for the skeletal mesh". The `ReattachWeapons` script event and the
       unread mesh flag follow it. **Maya's attachment 0 meets this;**
     - identifier `CylinderComponent`, after the release: `Pawn.CylinderComponent` and `CollisionComponent` are plain
       writes, answered;
     - the gear-likeness step, the mode-0 rendering setting and the light environment: render-side. **Hub ruling
       (19:58Z):** a hook answers "nothing" in Off only when it writes no field that the script or a slice step reads;
       a hook that writes such a field stays `NotYet { LayerAbsent }`, "InitializeBodyComposition: the body's
       rendering hooks". None of the three was read far enough to list its fields, so each is `LayerAbsent` until it
       is. **With the unread flags taken as allowing** (hub ruling, section 4.5), the gear-likeness step refuses for
       **every made component**, the rendering setting for **every primitive made in mode 0**, and the light
       environment for every made mesh component when the pawn's `LightEnvironment` is set (Maya's is).
       **UNVERIFIED**, with a live check;
     - `Data.Name` `CustomHead`: `HeadMesh`, a plain write, answered;
   - **custom head or skin:** answered (hub ruling 2).
5. **The global "attaching later" state** is never set in the port, so neither its queued attach nor its extra
   per-entry step is taken. That is a port answer, UNVERIFIED.

**Writes, in order**, each its own commit under the write-tracker:
1. per made attachment, in list order:
   1. a component: construct it; append the entry (4.1's table); collision (4.1's table); the attach (the first attempt,
      then the owner retry); the `Parent` link (refused above);
   2. a switch: append the copied entry with `Float` 0;
   3. a custom head or skin: append the entry (4.3's table);
   4. then, for a made component, the pawn's `PostInitBodyComposition` writes (4.5), in its order;
2. after the walk, on authority: `SwitchStateBitField` and `ActiveSwitchValues` (section 5), then `bNetDirty`,
   `bForceNetUpdate`;
3. the wrapper's root-motion writes.

A refusal after the first write is terminal, typed, kind and record kept, marked "stopped after partial writes: " by
the tracker.

**Where Maya stops under these rulings**, all found in the preflight, before any write, in the walk's order. Each
item is reached only once everything before it has an answer:
1. attachment 0's `PostInitBodyComposition`: the release of the old `Pawn.Mesh` (`NotRead`), only when the pawn
   still holds a `Mesh`, attached, at the build. Her archetype `Pawn_Siren` sets `Mesh`, but whether the spawned pawn
   still holds it then is not read (**UNVERIFIED**). If it does, this is her first stop. In the port, as #542
   (b2b4de8a) measured, the pawn holds none, so this item passes;
2. then attachment 0's animation nodes (`LayerAbsent`, the animation-tree instance): **in the port as measured, her
   first stop**;
3. then attachment 0's rendering hooks (`LayerAbsent`): the gear-likeness step, the mode-0 rendering setting (it is a
   primitive), and the light environment (`Pawn_Siren` sets `LightEnvironment`);
4. attachment 1's socket attach to its skeletal parent with socket `Root` (`LayerAbsent`, the skeletal mesh data
   reader), then its `Parent` link (`NotRead`, ruling 3), then its rendering hooks;
5. attachments 2 to 6, 9 and 12 (gear-likeness components on socket attaches): each socket attach (`LayerAbsent`), and
   each one's rendering hooks;
6. attachment 13's `PostInitBodyComposition`: the release of the old `Pawn.CylinderComponent` (`NotRead`; `Pawn_Siren`
   sets it), then its rendering hooks.

**Reference** keeps the stub policy.

## 8. Amendment (2026-11-25): the `Parent` link's flag, identified

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-CUSTOMHEAD-PARENT-FLAG (decision 2026-11-25-51), analyst only:
**another lane implements this section.** Raw output stays under ignored `local/analysis/parentflag/`. **Every rule
here is UNVERIFIED in the running game.**

**Why.** NATIVE_BODYCOMP_RENDER_HOOKS.md section 7 (#715) predicts that link 1's next stop is Maya's attachment 1
(`CustomHead`): the `Parent` link, `NotYet { NotRead }` under hub ruling 3 (section 0, the hub's rulings on #529).
Section 4.1 step 7 had read the link whole except one flag of the new component. That flag decides whether the
parent-bone map is the identity.

**Evidence.**
- Ghidra 12.1.4 headless, read-only, under the machine lock:
  - the body composition's component case calls the engine's own `SetParentAnimComponent` routine, when the
    identifier is `Parent` and both objects are `SkeletalMeshComponent`s. That routine is the one the script natives
    `SkeletalMeshComponent.SetParentAnimComponent` and `UpdateParentBoneMap` call. It is also called by the mesh change
    (the routine `SetSkeletalMesh`'s exec reaches) and by one more component routine (not read);
  - the field names come from the local copy of `tools/ghidra/class_layout.py` that sizes `MapProperty`, with one
    constant shift that also places `SkeletalMesh`, `ParentAnimComponent` and `ParentBoneMap` at the places the
    routine reads. So the names are a strong fit. The tested bit is one bool of the 20-bool word that begins with
    `bEnableClothSimulation`. `bIsGestaltAccessoryMesh` is the 16th bool declared in that word, and the same shift
    lands the tested bit exactly on it. Its neighbours (`bCacheAnimSequenceNodes` before it,
    `bNeedsInstanceWeightUpdate` after it) are ruled out only by that exact bit, not by a separate reading;
- An instruction search found no immediate set or clear of the flag's bit in the executable. The search covered two
  forms each: the word and the byte, set and clear. It is **not exhaustive**.
- `research/script_disasm.py`'s decoder was run over every script function of the nine code packages. No script
  function reads or writes the flag or `ParentAnimComponent`, and none calls `SetParentAnimComponent` or
  `UpdateParentBoneMap`.
- Maya's template was read with
  `ow-package --def-dump GD_Siren_Streaming.Pawn_Siren.CustomSkeletalMeshComponent_191 --cooked <CookedPCConsole>`, the
  merged values from the installed packages (output under ignored `local/analysis/link1next/`). Her body template was
  read the same way: `GD_Siren_Streaming.Pawn_Siren.SkeletalMeshComponent_7134`.
- There are no bl-sdk reads.

**The flag is `SkeletalMeshComponent.bIsGestaltAccessoryMesh`** (by fit, high). This replaces "a flag of the new
component (not identified)" in section 4.1 step 7.

**What sets it.**
- Only the component's own value: its template's value as made, or the class default. The made component copies its
  template's slots.
- Nothing in the body composition, the attach or the link writes it, as far as read. No script touches it. The
  instruction search found no native write (not exhaustive).

**The link, in order** (section 4.1 step 7, now whole):
1. the new component's `ParentAnimComponent` = the parent component;
2. its `ParentBoneMap` is emptied;
3. when the new component's `SkeletalMesh`, the parent and the parent's `SkeletalMesh` are all set, the map gets
   one slot per bone of the new component's mesh (its reference skeleton's count). Otherwise it stays empty;
4. **when the two `SkeletalMesh` objects are the same, or `bIsGestaltAccessoryMesh` is set:** slot `k` holds `k`;
5. **otherwise:** slot `k` holds the index of the bone of the parent's mesh with the same name as bone `k`, looked up
   through the parent mesh's name-to-index table. A name it lacks gives the engine's no-index value, -1.

When a skeleton has two bones with the same name, which index the table keeps was not read. The port's `find_bone`
takes the first, an **UNVERIFIED** choice.

**What Maya takes.**
- Attachment 1's template (`CustomSkeletalMeshComponent_191`) has `bIsGestaltAccessoryMesh` false.
- Its `SkeletalMesh` is `CD_Heads_Assassin_MainGame.CubeForCode`, the placeholder cube decision 2026-11-25-09 names.
  Attachment 0's is `Char_Siren.Mesh.Skel_SirenBody`.
- So the meshes differ and the flag is clear: **her map is the by-name one** (step 5), over `CubeForCode`'s bones
  against `Skel_SirenBody`'s. No stand-in is needed for the flag.
- The custom head entry (attachment 16) applies nothing in the port (ruling 2). In the game, a later head-mesh change
  would rebuild the map through the mesh change; that is not on `maya:8`'s path in the port.

### 8.1 The Off answer (proposed for the hub's revisit of ruling 3)

**Every rule below is UNVERIFIED.** If the hub revisits ruling 3, it replaces the `PARENT_LINK` refusal
(`body_composition::PARENT_LINK`, section 7's walk). The gate is unchanged: identifier `Parent`, the parent a made
`SkeletalMeshComponent` and the new component one too.

**Preflight** (read only, after the attachment's socket attach is planned, before the pawn's hooks):
1. **Declarations:** the new component's class declares `ParentAnimComponent` (an object or component reference that
   takes the parent), `ParentBoneMap` (an array of ints) and `bIsGestaltAccessoryMesh` (a bool). Otherwise
   `VmError::Script` naming it.
2. **The meshes:** the new component's `SkeletalMesh` from its template, as made; the parent's from the parent's
   template. Either None: the map will be empty, and nothing more is read.
3. **The same mesh, or the flag set:** the new mesh's reference skeleton (`skeletal::reference_skeleton`) for its
   count. The map is `0..n`.
4. **Otherwise:** both reference skeletons. Slot `k` is `skeletal::find_bone(parent mesh, name of bone k)`, or -1 when
   it answers None. A name text the name table does not hold is no match, as `find_bone` does.
5. A skeleton that cannot be read refuses with that reader's own `NotYet`, before any write. For example: no skeleton
   source is installed, the mesh is not loaded, or a name is not flat. The reader's kind and record are kept, and
   nothing is loaded.

**Writes** (in the walk's writes, after the attachment's socket attach and before the pawn's `PostInitBodyComposition`
writes for it):
1. `ParentAnimComponent` = the parent made object;
2. then `ParentBoneMap` = the planned ints.

A failure after the first write is marked "stopped after partial writes: " by #518's counters.

**Not in scope:** the script natives `SetParentAnimComponent` and `UpdateParentBoneMap` share this rule, and could be
bound from it; no slice link calls them. The mesh change's own rebuild belongs to NATIVE_SKELMESH_MESH_CHANGE.md.

**What link 1 would meet** (a prediction, not a run). With the link answered, attachment 1 needs `CubeForCode`'s
reference skeleton as well as `Skel_SirenBody`'s. If the slice's skeleton source does not hold `CubeForCode`, that
reader's refusal is her next stop. Otherwise she goes on to attachment 1's rendering hooks.

### 8.2 Tests for the implementer

Synthetic, with `package()` / `tag()` and a test skeleton source. Each refusal leaves #518's three counters and the
name table unchanged, with nothing loaded:
1. a `Parent` attachment whose child and parent templates share one mesh of 3 bones: after the build,
   `ParentAnimComponent` is the parent made object and `ParentBoneMap` is `[0, 1, 2]`;
2. different meshes, the flag clear: the child's bones `A, B, C`, the parent's `C, A`: the map is `[1, -1, 0]`;
3. different meshes, the flag set: the identity over the child's count;
4. the child's or the parent's `SkeletalMesh` None: the link is set and the map is empty. A template map with entries
   is emptied too;
5. the child's skeleton not readable (no source installed): the reader's `LayerAbsent`, before any write;
6. a child bone name the name table lacks: -1 in that slot, and nothing is interned;
7. `ParentBoneMap` not an int array, or `bIsGestaltAccessoryMesh` not a bool: `VmError::Script` naming it;
8. a non-skeletal parent or child, or another identifier: no link (as now);
9. order: the link's two writes come after the socket attach's writes and before the hooks' writes for the same
   attachment;
10. the existing `PARENT_LINK` refusal test is replaced by tests 1 to 4;
11. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** the made component's validity check (ruled valid for the port).
2. **UNVERIFIED:** the component flags of the collision table: named by bit position against the layout tool.
3. **UNVERIFIED:** the engine class of an actor's own named socket components.
4. **UNVERIFIED:** the `Parent` link: the component flag that forces the identity map (`bIsGestaltAccessoryMesh`, by
   fit, section 8), and the field names (`ParentAnimComponent`, `ParentBoneMap`) by fit. Which index the parent's
   name table keeps for a duplicated bone name was not read.
5. **UNVERIFIED:** the per-entry step on the existing entries of a rebuild, and the replay onto new entries (refused in
   Off).
6. **UNVERIFIED:** what the head and skin customization apply writes (ruled "nothing" for the port).
7. **UNVERIFIED:** the pawn's `PostInitBodyComposition`: the slot-node rebuild, the mesh flag it sets, the
   gear-likeness step and the two rendering steps (section 4.5).
8. **UNVERIFIED:** the one unidentified native caller (section 1).
9. **UNVERIFIED side finding, a separate open item:** while checking the gear-likeness interface for section 8, the
   pawn's hook 1 was found to look up an interface registered as `WillowGame.ISimpleAnimPlayer`, not
   `IGearLikenessConsumer`. What was read for that:
   - the lookup's class registration;
   - the routine it then calls, which the script native `WillowAnimNode_Simple.CacheSimpleAnims` also calls;
   - the packages' interface tables: only `WillowPawn`, `WillowInteractiveObject` and `WillowPopulationPoint`
     implement it, and no component class carries any interface table.

   Hook 1 and decision 2026-11-25-03 are unchanged by this note until a separate analyst lane reads it
   (ANALYST-HOOK1-INTERFACE, NATIVE_BODYCOMP_RENDER_HOOKS.md section 8).
   *Read 2026-11-25 in NATIVE_BODYCOMP_RENDER_HOOKS.md section 8 (decision 2026-11-25-55): hook 1 is the simple-anim
   cache, `CacheSimpleAnims(the pawn, the made skeletal component)`, the pawn by fit.*

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On Maya in Sanctuary, after spawn:
   - read `InstanceDataState.Data`: expect 15 entries in section 6's order;
   - read each entry's `Replication`, `Object` and `BodyCompositionIndex`: expect 0, None and `i` + 1 for the
     customization entries;
   - read `ReplicatedInstanceDataState`: expect switch bits 7, values 0;
   - check that `Pawn.Mesh` is attachment 0's component, `HeadMesh` attachment 1's, and `CylinderComponent` and
     `CollisionComponent` attachment 13's.
2. Read attachment 0's skeletal component's `Attachments` list: expect attachments 1 to 6, 9 and 12 at the sockets of
   section 6. Read attachment 1's `ParentAnimComponent` (expect attachment 0's component) and its `ParentBoneMap`.
3. Read the cylinder's `CollideActors`, `BlockActors`, `BlockZeroExtent`, `BlockNonZeroExtent` and `BlockRigidBody`
   (expect all true for value 2), and the pawn's `bCollideActors`, `bBlockActors` and `bPushedByEncroachers`.
4. Set switch 7's entry `Int` to 1 through the SDK and rebuild: expect the head particle component made.
