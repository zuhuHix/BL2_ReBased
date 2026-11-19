# Native rules of InstanceDataHelper.InitializeBodyComposition (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-BODY-COMPOSITION (decision 2026-11-19-NNN, number pending).
Analyst only: a different lane implements it (clean room). This lane's author implemented the caller's Off rule earlier
(IMPL-INIT-PAWN-MESHES, #522), so that author must not implement this note.

**Sources:**
- the installed script, read with `research/script_disasm.py` (every function of the nine code packages);
- the installed packages, read with `ow-package --object-dump` (Maya's body class) and the class and struct layouts from
  `tools/ghidra/class_layout.py`;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock), for
  what neither settles.

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/bodycomp/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** `WillowPawn.InitializePawnMeshes` is a wrapper around this build (NATIVE_INIT_PAWN_MESHES.md). Its
Off rule (#522) refuses with `NotYet { LayerAbsent }` before the build. Once Maya's body class is loaded, that refusal
is slice links 1, 3, 4, 5, 13 and 14's stop. This note says what the build reads and writes, which parts a headless
port can carry, and which must stay with a render or scene layer. It also corrects NATIVE_INIT_PAWN_MESHES.md section
3 in three places (section 3 below, "Corrections").

## 1. Who calls it

- **Script** (`InstanceDataHelper.InitializeBodyComposition(Owner, BodyComposition, InstanceState, ReplicatedState,
  optional Mode)`, static, native). Six callers:
  - `Perch.InitBody`, `WillowPopulationPoint.PostBeginPlay`, `WillowVehicle.PostBeginPlay` and
    `WillowInteractiveObject.InitializeFromDefinition`: mode left at 0;
  - `PlayerStandIn.Initialize` (a menu stand-in): mode 5 in the foreground, else 4;
  - `VehicleStandIn.CreatePreviewMesh` (a menu preview): mode 1.
- **Native.** The pawn's `InitializePawnMeshes` calls the script function on the `InstanceDataHelper` class default
  object (NATIVE_INIT_PAWN_MESHES.md section 2), so it runs the same body, with mode 0. One more native caller serves the
  player pawn's menu state (`WillowPlayerPawn.SetInstanceDataMenuState`), off the slice. One further native caller (a
  virtual of a class that keeps its own instance state) was not identified; no slice class matched it.
- **On the slice:** only the pawn path, through `WillowPawn.ApplyBodyClassStartingValues` (script), whose first
  statement is `InitializePawnMeshes`. Links 1, 3, 4, 5, 13 and 14 reach it through Maya's set-up (`maya:8`).
- **Links 6 and 7 never call it.** Their `make` step goes through `WillowWeapon.AttachMeshToPickup`
  (NATIVE_SET_ANIM_TREE_TEMPLATE.md section 1). That makes a `SkeletalMeshComponent` from the weapon's
  `ThirdPersonMesh`, then calls `SetAnimTreeTemplate`, `SetForceRefPose`, `SetHidden`, `SetOwnerNoSee`,
  `HideBonesInMesh` and `Actor.AttachComponent`, and sets `CollisionComponent`. They need the same kind of layer
  (section 6): a component made from a template, attached to an actor, with primitive flags.

**After the build,** `ApplyBodyClassStartingValues` goes on (script) with `UpdateAnimSets`, `SetupSkeletalMeshPhysics`
(both script), the body class's `Mass`, relevance, `CrouchHeight`, behavior providers, death definition and rotation
flags. Which native those reach first was not probed.

## 2. What it does, in order

The entry unpacks owner, composition, the instance-data set, the replicated state and the mode, then runs the build.

1. **No owner:** nothing.
2. **The old entries.** A copy of the owner's `InstanceDataState.Data` is kept (for the replay step, 4.4).
3. **Capacity.** Room for `BodyComposition.MaxExpectedComponents` entries, or the attachment count when that is 0.
4. **Nothing more when the attachment list is empty.** Otherwise:
5. **Per-attachment records** (temporary, not script-visible), one per attachment, in order: no object yet, no entry
   index, "not made", and "removed" when the attachment's index is below 32 and its bit is set in
   `ReplicatedInstanceDataState.RemovedBodyCompositionPartsBitField`.
6. **The walk** (section 3).
7. **After the walk:**
   - **Authority** (owner `Role` 3): `ReplicatedInstanceDataState.SwitchStateBitField` and `ActiveSwitchValues` are
     **first set to 0**, then rebuilt. Each `IDT_Switch` attachment, in order and at most 32, takes the next bit. When
     its switch entry exists (an entry of type switch whose `BodyCompositionIndex` - 1 is that attachment), the bit is
     set, and the entry's `Int` is ORed into `ActiveSwitchValues` at a running offset, as long as the offset plus the
     width stays within 32. The width is the entry's `BitsRequiredForMaxSwitchValue`, or, when that is 0, its `Float`
     truncated to an integer. That answers NATIVE_INIT_PAWN_MESHES.md's open question 3.
     `RemovedBodyCompositionPartsBitField` and `bNeedToApplyThisState` are not touched. Then the owner's `bNetDirty`
     and `bForceNetUpdate` are set.
   - **Otherwise,** with `ReplicatedInstanceDataState.bNeedToApplyThisState` set: the replicated state is applied to
     the entries (not read further), and the flag is updated from the result.
8. The temporaries are freed.

## 3. The walk (each attachment once, in list order)

For attachment `i` (a `BodyInstanceDataUnion`: `Data`, an `InstanceDataUnion`; `ParentAttachmentIdentifier`;
`AttachmentBaseInstanceDataIndex`; `SwitchIdentifier`):
1. **Already made and removed:** it is torn down (not read further), and the walk moves on. On a first build, nothing is
   made yet, so this does not happen.
2. **Removed and not made:** skipped.
3. **No base** (`AttachmentBaseInstanceDataIndex` -1 or out of range): it is made, with **the owner** as its parent.
4. **A base:** the base's record, as it stands at this point, decides.
   - A base that is **not made** (later in the list, removed, or refused): this attachment is not made either. **There
     is no "base first" step:** a later base is not made early.
   - A base that is **a switch**: this attachment is made only when the switch entry's `Int` equals this attachment's
     `SwitchIdentifier`, and the entry's `Float` is about 0 or its `BitsRequiredForMaxSwitchValue` is 0. Otherwise it
     is not made.
   - Any other made base: it is made, with the base's made object as its parent.
5. **Making it**, by `Data.Type` (section 4). A made entry is appended to `InstanceDataState.Data` at once, with
   `BodyCompositionIndex` = `i` + 1. The record notes the object and the new entry's index.
6. **Mode toggles** (component entries only, and not on the owner itself): modes 1 and 2 hide or show the component
   and set flags on it; modes 4 and 5 do so differently. Mode 0, the slice's, does nothing here.
7. **The replay** (only when the call carries an old-state replayer): where the old entries held a matching entry,
   its value is carried onto the new one (not read further; open question 5).

**Corrections to NATIVE_INIT_PAWN_MESHES.md section 3:**
1. There is no "base first" creation (above, step 4).
2. `ParentAttachmentIdentifier` does not pick the parent. The parent is the base's made object, or the owner. The
   identifier matters in one case only: when it is one specific engine name (by fit `Mesh`), one object field of a
   component is set and a refresh runs after the attach (section 4.1, step 6).
3. Maya's switches have `BitsRequiredForMaxSwitchValue` 1, not 0, and her switched components are not made on a first
   build (section 5).

## 4. Making an entry, by type

### 4.1 `IDT_ActorComponent` (5)

1. **No template** (`Data.ComponentData.Component` None) or no owner: nothing is made.
2. **The component:** a new component is constructed from the template, owned by the owner (the engine's object
   construction with a template).
3. **A validity check** on the new component (a virtual of the component; not read further, open question 1). If it
   fails, the component is flagged for destruction and dropped, and nothing is recorded.
4. **The entry:** an `InstanceDataUnion` of type 5 with `Name`, `DeleteByOwner`, the template's `ComponentData` (with
   `Component` now the new component), and `BodyCompositionIndex`. It is appended to `InstanceDataState.Data`. The
   append always succeeds for a build entry (its index is never 0).
5. **Collision** from `ComponentData.CollisionType` (`Actor.ECollisionType`), on the new component when it is a
   primitive:
   - 0: nothing;
   - 1: the component's collision flags are cleared;
   - 2, 3, 6 and 9: collision flags are set on the component, and two flags on the owner;
   - the other values: not read in detail (open question 2).

   Which flags these are was named by fit only (`CollideActors`, `BlockActors` and the like). **UNVERIFIED.**
6. **The attach**, with a zero offset and rotation, at the attachment's socket name (`ComponentData.MeshSocketName`).
   It goes through the `IWorldBody` interface (`WorldBodyAttachComponent`) of the parent: the base's made object, or the
   owner. If that answers false, it is tried again on the owner.
   - **An actor as the target (the pawn):**
     - a component that is already being attached through this path is refused (a guard against recursion);
     - with a socket name: if the actor has one of its own named socket components of that name (one engine class,
       not identified; open question 3), the offset is taken relative to it. Otherwise the call is handed to the
       actor's attach base (`GetWorldBodyAttachmentBase`), which for a pawn is its `Mesh`;
     - with no socket name: the offset is made relative to the actor, added to the component's `Translation` and
       `Rotation`, and the component is attached to the actor (its `Components` list), or queued while a global
       "attaching later" flag is set.
   - **A `SkeletalMeshComponent` as the target:** a component is its own attach base. The name is matched against the
     mesh asset's **sockets** first (a socket adds its own offset and gives its bone), then its **bones**. With no mesh
     asset, no name, or a name that is neither, it answers false (so the owner gets the component instead). On success,
     the component is attached to that bone with unit scale. By fit, that is an entry in the skeletal component's
     `Attachments` list (`Component`, `BoneName`, relative location, rotation and scale). **UNVERIFIED.**
   - **The special name** (step 2 of the corrections above): after a successful attach of an attachment whose
     `ParentAttachmentIdentifier` is that name, one object field of a component is set and a refresh runs. Which
     component and which field is open question 4.

### 4.2 `IDT_Switch` (9)

An entry of type 9 is appended: a copy of `Data` with `Float` set to 0. No object is made.

### 4.3 `IDT_CustomHead` (11) and `IDT_CustomSkin` (12)

An entry of that type is appended (`Name`, `Replication`, type, `BodyCompositionIndex`). Then the owner's current
customization of that kind is looked up (a `WillowGame` class for head, another for skin) and applied through a
customization manager. What the apply writes was not read (open question 6). It changes how the body looks: a mesh or
material change.

### 4.4 Others

`IDT_Object` (4) loads an object and appends an entry; `IDT_ActorTemplate` (7) and `IDT_PickupTemplate` (10) spawn. None
occurs in Maya's body class; they were not read further. The plain value types (`IDT_Bool`, `IDT_Int`, `IDT_Float`,
`IDT_Vector`, `IDT_String`, `IDT_Actor`) make no entry.

## 5. What the slice's Maya takes (the installed packages)

`GD_Siren_Streaming.BodyClass_Siren` (a `BodyClassDefinition`): `MaxExpectedComponents` 0,
`HasHomingTargetComponents` false, 18 attachments:

| # | Type | Base | Socket | Parent identifier | Component template class | First build |
|---|---|---|---|---|---|---|
| 0 | component | none | none | `Mesh` | `SkeletalMeshComponent` | made, attached to the pawn |
| 1 | component | 0 | `Root` | `Parent` | `CustomSkeletalMeshComponent` | made, on 0 |
| 2-6, 9, 12 | component | 0 | `Spine3` (two), `Shield`, `ClassMod`, `R_Thigh`, `Hips`, `Launcher` | none | `GearLikenessMeshComponent` | made, on 0 |
| 7 | switch | 0 | `Head` | none | none | entry |
| 8 | component | 7 (value 1) | `Head` | none | `ParticleSystemComponent` | **not made** (switch at 0) |
| 10 | switch | 0 | `BlightPhoenixFX` | none | none | entry |
| 11 | component | 10 (value 1) | `BlightPhoenixFX` | none | `ParticleSystemComponent` | **not made** |
| 13 | component | none | none | `CylinderComponent` | `CylinderComponent`, collision 2 | made, attached to the pawn |
| 14 | switch | none | none | none | none | entry |
| 15 | component | 14 (value 1) | none | none | `StaticMeshComponent` | **not made** |
| 16 | custom head | none | none | `CustomHead` | none | entry, then the head customization |
| 17 | custom skin | none | none | `CustomSkin` | none | entry, then the skin customization |

All three switches have `MaxSwitchValue` 1, `BitsRequiredForMaxSwitchValue` 1 and `Int` 0; the two customization
entries have replication 2. Every component's `CollisionType` is 0, except attachment 13 (2). `bAttachToMesh` is false
on all.

**So the first build makes, in order:** 10 components and their entries (0, 1, 2-6, 9, 12, 13), 3 switch entries and
2 customization entries: 15 entries, in list order of the made attachments. Attachments 1 to 6, 9 and 12 attach to
attachment 0's new skeletal component at the named sockets or bones, through that mesh asset. Attachments 0 and 13
attach to the pawn. On authority (Maya is), `SwitchStateBitField` = 7 and `ActiveSwitchValues` = 0, then
`bNetDirty` and `bForceNetUpdate`. Attachment 0's identifier is the special name (by fit), so its step 6 refresh runs.

## 6. What a headless Off run needs, and what can stay with a render layer

**Script-visible results, and whether a headless port can carry them:**

| Result | Separable from rendering and physics? |
|---|---|
| `InstanceDataState.Data` entries (all types) | yes: plain data |
| the new component objects, from their templates (outer: the owner) | yes: object instancing, as the port does for templates elsewhere (#435) |
| a component attached to an actor: its relative `Translation`/`Rotation` and the actor's `Components` list | yes: data |
| a component attached to a skeletal component's bone or socket | yes, given the mesh asset's socket and bone names and the socket offsets, which are package data (a mesh-data read, not rendering). The render attachment itself is not |
| collision flags from `CollisionType` | the flag writes are data; what they do in a physics scene is not |
| switch entries, `SwitchStateBitField`, `ActiveSwitchValues`, `bNetDirty`, `bForceNetUpdate` | yes: data, fully read |
| the special-name field and its refresh | unknown until open question 4 is settled |
| the head and skin customization apply | not read; changes how the body looks (open question 6) |
| the component's validity check | not read (open question 1) |

**The smallest headless layer that gets links 1, 3, 4, 5, 13 and 14 past the build:**
1. component instancing from a template, owned by the owner;
2. the entry append;
3. the actor attach (no socket): relative transform and the `Components` list;
4. the skeletal attach by socket or bone name, from the mesh asset's socket and bone tables (a mesh-data read);
5. the switch rules and the authority packing;
6. the collision flags as data.

The customization apply, the validity check and the special-name refresh need either a reading (open questions 1, 4
and 6) or a hub ruling on a port answer.

**Links 6 and 7** use pieces 1 and 3 of the same layer (and the primitive flag writes) through their own natives. A
lane that builds this layer can serve both.

## 7. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** The hub rules on the choices marked **(hub)**.

**Keep #522's refusal until the layer exists:** `InitializePawnMeshes` refuses with `NotYet { LayerAbsent }` before
any write (NATIVE_INIT_PAWN_MESHES.md section 6). Nothing in this note changes that until a lane builds section 6's
layer.

**With the layer, for a build in mode 0** (the script native, and the pawn's wrapper):
1. **Preflight, write-free** (under the hub's write-tracker: a load counts as a write, decision of review r2 of #518):
   - the composition is a struct with an `Attachments` array of `BodyInstanceDataUnion`, and the owner's class
     declares the instance-data set and the replicated state as passed, else `VmError::Script`;
   - every attachment's `Data.Type` is one the port answers: component, switch, custom head or skin, or a plain value
     type (no entry). `IDT_Object`, `IDT_ActorTemplate` or `IDT_PickupTemplate` on an attachment that would be made:
     `NotYet { NotRead }`, "InitializeBodyComposition: object and template attachments";
   - a removed attachment that is already made (a rebuild): `NotYet { NotRead }`, "InitializeBodyComposition: tearing
     down a removed attachment";
   - a mode other than 0: `NotYet { NotRead }`, "InitializeBodyComposition: the menu modes' visibility toggles";
   - the replay step when an old-state replayer is passed: `NotYet { NotRead }`, "InitializeBodyComposition: the
     old-state replay onto new entries";
   - not on authority with `bNeedToApplyThisState` set: `NotYet { NotRead }`, "InitializeBodyComposition: the
     client-side apply of a replicated state";
   - a `CollisionType` other than 0, 1, 2, 3, 6 or 9 on a component that would be made: `NotYet { NotRead }`,
     "InitializeBodyComposition: collision type <n>";
   - a component that would be made, with a socket name, under a skeletal parent whose mesh asset is not loaded or
     not decoded: `NotYet { LayerAbsent }`, "InitializeBodyComposition: the socket and bone tables of <mesh>", layer
     "a skeletal mesh data reader (sockets and the reference skeleton)";
   - an actor target with a socket name that the actor itself would have to resolve (the named socket components):
     `NotYet { NotRead }`, "InitializeBodyComposition: an actor's own named socket components";
   - **(hub)** the validity check: either rule a port answer (every component made from a template is valid), or
     `NotYet { NotRead }`, "InitializeBodyComposition: the made component's validity check";
   - **(hub)** the special name's field and refresh: either rule "nothing" for the port, or `NotYet { NotRead }`,
     "InitializeBodyComposition: the special parent identifier's refresh";
   - **(hub)** a custom head or skin entry: the apply needs a customization layer. Either rule the port's answer
     (append the entry, apply nothing: a render-only change), or `NotYet { LayerAbsent }`, "InitializeBodyComposition:
     applying the owner's head or skin customization", layer "a customization layer (head and skin meshes and
     materials)". **Maya has both, so this ruling decides whether links 1, 3, 4, 5, 13 and 14 pass.**
2. **Writes, in section 2 and 3's order**, each its own commit under the write-tracker:
   - per made attachment, in list order: the component (made, then attached), or the entry; for a component, the entry
     is appended before the attach, as in the game;
   - after the walk, on authority: `SwitchStateBitField` and `ActiveSwitchValues` (zeroed, then packed), then
     `bNetDirty`, `bForceNetUpdate`.
3. A refusal after the first write is terminal, typed, kind and record kept, marked "stopped after partial writes: "
   by the tracker.

**Reference** keeps the stub policy.

## Open questions

1. **UNVERIFIED:** the made component's validity check: what it tests, and whether any template the slice uses fails it.
2. **UNVERIFIED:** the collision flags set for `CollisionType` 2, 3, 6 and 9, and what 4, 5, 7 and 8 do (named by
   fit).
3. **UNVERIFIED:** the engine class of an actor's own named socket components.
4. **UNVERIFIED:** the special parent identifier: which name it is (by fit `Mesh`), which component's field it sets,
   which field, and what the refresh does.
5. **UNVERIFIED:** when an old-state replayer is passed, and what it carries.
6. **UNVERIFIED:** what the head and skin customization apply writes, beyond how the body looks.
7. **UNVERIFIED:** the one unidentified native caller (section 1).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On Maya in Sanctuary, after spawn: read `InstanceDataState.Data` (expect 15 entries in the order of section 5, the
   switched particle and static-mesh components absent), `ReplicatedInstanceDataState` (expect switch bits 7, values
   0), and each component entry's object: its `Owner`, its attach parent and bone.
2. Read attachment 0's skeletal component's `Attachments` list: expect the gear and the custom mesh at the sockets of
   section 5.
3. Set switch 7's entry `Int` to 1 through the SDK and call the build again: expect the head particle component made.
4. Hook the customization manager's apply around the build and record what it changes.
