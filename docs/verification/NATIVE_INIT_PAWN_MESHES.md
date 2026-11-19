# Native rules of WillowPawn.InitializePawnMeshes (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-INIT-PAWN-MESHES (decision 2026-11-19-NNN, number pending).
Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py`;
- the class and struct layouts, from the cooked packages through the project's layout tool;
- the installed packages, read through the port's definition store (Maya's body class, below);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/pawnmesh/`. This note is in our own words: no decompiler output, pseudo-code, addresses or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** With `WorldInfo.AllNavigationPoints` bound (#513) and `Engine.IsSplitScreen()` bound false
(c376's lane; a local binding for the probe), slice link 1 stops in `maya:8` at this native, unbound. Links 3, 4, 5, 13
and 14 sit behind it.

## 1. Script or native, and the caller

- `WillowPawn.InitializePawnMeshes()` and `WillowPlayerPawn.InitializePawnMeshes()` are both **natives**, with no
  parameters and no return value. Their keys are `WillowPawn.InitializePawnMeshes()` and
  `WillowPlayerPawn.InitializePawnMeshes()`.
- Both resolve to one body: the player pawn's override only forwards to the base rule. An implementer binds one rule
  under both keys.
- The only script caller is `WillowPawn.ApplyBodyClassStartingValues`, as its first statement. That function then calls
  `UpdateAnimSets`, `SetupSkeletalMeshPhysics` and the rest of the body-class set-up. `WillowPawn.BodySelectClass` and
  `WillowPlayerPawn.ApplyBodyClassStartingValues` (through the base call) reach it.

## 2. What it does, in order

The native is a thin wrapper around the engine's body-composition builder, the same one script calls as
`InstanceDataHelper.InitializeBodyComposition(Owner, BodyComposition, InstanceState, ReplicatedState, optional mode)` for
interactive objects, perches, vehicles and population points.

1. **The helper.** It takes the class default object of `Engine.InstanceDataHelper` and checks that it is one. If not,
   it returns with nothing done.
2. **The gate.** It reads the pawn's `BodyClass` (a `BodyClassDefinition`). If `BodyClass` is None, or
   `BodyClass.BodyComposition.Attachments` is empty, it returns with nothing done.
3. **The build.** It calls `InitializeBodyComposition` with:
   - the pawn as owner;
   - `BodyClass.BodyComposition`;
   - the pawn's `InstanceDataState` (an `IInstanceData.InstanceDataSet`) and `ReplicatedInstanceDataState`;
   - mode 0 (the optional byte left at zero).

   Section 3 describes the build.
4. **Root motion**, after the build, still inside the gate:
   - if `BodyClass.UseRootMotion` is set, the pawn's `Mesh` (`Pawn.Mesh`, a `SkeletalMeshComponent`) gets
     `RootMotionMode = RMM_Accel` (3);
   - if `BodyClass.UseRootRotation` is set, `Mesh.RootMotionRotationMode = RMRM_RotateActor` (1).

   The two mesh fields are named by fit: the written values match those two enums, and the gating flags are named for
   them. The layout tool places the component's fields 16 bytes off here. **UNVERIFIED.**

It writes nothing else on the pawn directly and calls no script function.

## 3. The body-composition build (`InitializeBodyComposition`)

Own-words outline; **UNVERIFIED** throughout.

1. **No owner:** nothing.
2. **The old state.** It keeps a copy of the owner's current `InstanceDataState.Data` (the previous entries).
3. **Capacity.** It reserves room for `BodyComposition.MaxExpectedComponents` entries (or the attachment count when
   that is 0) in `InstanceDataState.Data`.
4. **Bookkeeping.** It makes one record per attachment, in order:
   - not created yet, no entry index;
   - marked "removed" when the attachment's index is below 32 and its bit is set in
     `ReplicatedInstanceDataState.RemovedBodyCompositionPartsBitField`.
5. **The walk.** It visits the attachments (`BodyInstanceDataUnion` items) in order:
   - **Removed:** an attachment already marked removed is torn down if it was created.
   - **Parent first:** an attachment whose `AttachmentBaseInstanceDataIndex` names an earlier attachment waits for it
     (the parent is created first). An index of -1, or one out of range, means no base.
   - **Creation by type** (`Data.Type`, `IInstanceData.EInstanceDataType`). Each created entry is appended to
     `InstanceDataState.Data`, with `BodyCompositionIndex` = attachment index + 1, and its record notes the new entry
     index.
     - `IDT_ActorComponent` (5): a new component is made from the template `Data.ComponentData.Component`, owned by the
       owner. If it is not valid it is discarded and nothing is added. Otherwise an entry of type 5 is added holding the
       new component (name, `DeleteByOwner`, collision, socket, hit region and icons copied from the template data),
       then the component is attached. The attach goes to the parent the attachment names (`ParentAttachmentIdentifier`,
       or the base attachment's component), at `Data.ComponentData.MeshSocketName`. When there is no parent, it goes to
       the owner itself.
     - `IDT_Switch` (9): an entry of type 9 is added, with its object cleared (a switch state; no component).
     - `IDT_CustomHead` (11) and `IDT_CustomSkin` (12): an entry of that type is added, then the owner's current
       customization of that kind (`CustomizationType_Head` / `CustomizationType_Skin`, a `CustomizationDefinition`)
       is looked up and applied to the body.
     - `IDT_Object` (4) and `IDT_ActorTemplate` (7) and `IDT_PickupTemplate` (10): an object load and two actor-spawn
       paths. None occurs in Maya's body class, and they were not read further.
     - The plain value types (`IDT_Bool`, `IDT_Int`, `IDT_Float`, `IDT_Vector`, `IDT_String`, `IDT_Actor`): no entry.
   - **Mode 0.** In the mode the pawn passes, the visibility and collision toggles the other modes apply to created
     components are not taken. Where the old state held a matching entry, the old value is carried onto the new entry
     (a replay step, not read further).
6. **After the walk:**
   - **Authority** (owner `Role` = `ROLE_Authority`, 3):
     - `ReplicatedInstanceDataState.SwitchStateBitField` and `ActiveSwitchValues` are rebuilt from the switch entries.
       Each `IDT_Switch` attachment, in order and at most 32, sets the next bit of `SwitchStateBitField` when it has a
       state entry. That entry's `Int` value is packed into `ActiveSwitchValues` at the running bit offset, as wide as
       `BitsRequiredForMaxSwitchValue`, while the total stays within 32 bits.
     - `RemovedBodyCompositionPartsBitField` and `bNeedToApplyThisState` are left as they were.
     - Then the owner's `bNetDirty` and `bForceNetUpdate` are set.
   - **Otherwise,** when `ReplicatedInstanceDataState.bNeedToApplyThisState` is set: the replicated state is applied to
     the entries, and the flag is updated from the result.
7. **Temporaries:** the temporary records are freed.

## 4. What the slice's Maya takes

Read from the installed packages through the port's definition store, at link 1's stop (local probe, not committed).

- **The pawn:**
  - a `WillowPlayerPawn` with `Role` = 3 (authority);
  - a `Mesh` (a `SkeletalMeshComponent`);
  - an empty `InstanceDataState` and a zero `ReplicatedInstanceDataState`.
- **The body class:** `BodyClass` is `BodyClass_Siren`, which the store loads lazily (the read is a load, so it counts
  as a write for the partial-writes rule). It has:
  - `UseRootMotion` and `UseRootRotation` both false, so step 2.4 writes nothing;
  - `MaxExpectedComponents` 0;
  - **18 attachments**, so the gate passes and the build runs:
    - 13 `IDT_ActorComponent`: a skeletal mesh parented to `Mesh`, a `CustomSkeletalMeshComponent` (`CustomHead`) at
      `Root`, eight `GearLikenessMeshComponent`s at sockets (`Spine3` ×2, `Shield`, `ClassMod`, `R_Thigh`, `Hips`,
      `Launcher`), two `ParticleSystemComponent`s under switches, a `CylinderComponent` with collision type 2, and a
      `StaticMeshComponent` under a switch;
    - 3 `IDT_Switch` (`ColdBreath`, `BlightPhoenix`, `ViewingStatusMenu`), each the base of one switched component;
    - 1 `IDT_CustomHead` and 1 `IDT_CustomSkin`, both replication `IDR_Local`.
- **Branches taken:**
  - the helper check passes;
  - the gate passes;
  - the build creates components, switch entries and the two customization entries;
  - the authority branch rebuilds the switch fields and raises `bNetDirty` and `bForceNetUpdate`;
  - no root-motion write.

## 5. Script-visible state that results (in game, UNVERIFIED)

- **`InstanceDataState.Data`:** one entry per created attachment: component entries holding new components attached to
  the pawn's mesh, switch entries, and the two customization entries.
- **`ReplicatedInstanceDataState`:** `SwitchStateBitField` and `ActiveSwitchValues` set from the switches.
- **The owner:** `bNetDirty` and `bForceNetUpdate` true.
- **The mesh:** its attached components (render state). `Mesh.RootMotionMode` and `Mesh.RootMotionRotationMode` are
  unchanged for Maya.

## 6. Proposed ParityMode Off answer (for a separate implementer)

**Classes:**
- bind under both `WillowPawn.InitializePawnMeshes()` and `WillowPlayerPawn.InitializePawnMeshes()`, Off only;
- Reference keeps the stub policy;
- the receiver must be a `WillowGame.WillowPawn` (or a subclass).

**Preflight, write-free, in this order:**
1. **The helper class.** No `Engine.InstanceDataHelper` class in the class table: return None, nothing done (the native's
   own step 1).
2. **The body class.** Read `BodyClass`. If it is None, return None.
   - If the store reports it unloaded (`DefStore::is_unloaded`), the read needs a load. The load is a write: do it only
     after every check that needs no load. A failed load (`DefStore::is_failed`) is the store's error, unchanged.
3. **The attachments.** `BodyClass.BodyComposition.Attachments` empty: return None, nothing done. That covers the root
   motion too, which sits inside the gate.
4. **A non-empty list.** Refuse before any write except the lazy load. Answer `NotYet { LayerAbsent }`:
   - text: "WillowPawn.InitializePawnMeshes: the body composition build (InstanceDataHelper.InitializeBodyComposition:
     component instancing and attachment to the pawn's mesh)";
   - layer: "a component and mesh layer (instancing components from templates, socket attachment)";
   - record: `docs/verification/NATIVE_INIT_PAWN_MESHES.md`.

   This is the slice's case.

**Which parts need a layer the port does not have** (each `LayerAbsent` when a later lane splits the build):
- **`IDT_ActorComponent`:** instancing a component from its template and attaching it at a socket on a skeletal mesh
  (a scene and mesh layer).
- **`IDT_CustomHead` and `IDT_CustomSkin`:** applying the owner's customization (a customization layer, and the
  skeletal mesh it changes).
- **The root-motion writes:** these are plain field writes on the mesh component, possible today. They are not reached
  while step 4 refuses.

**Portable without a render layer, if a later ruling splits the build:**
- the switch entries (`IDT_Switch`);
- the authority branch's switch-field rebuild and the `bNetDirty` / `bForceNetUpdate` writes;
- the root-motion writes.

They come after the component entries in the walk, so a split needs its own ruling on order. Until then they stay behind
step 4's refusal.

**Not read** (named for a later analyst; `NotYet { NotRead }` if reached):
- "InitializeBodyComposition: IDT_Object, IDT_ActorTemplate and IDT_PickupTemplate creation";
- "InitializeBodyComposition: the old-state replay onto new entries";
- "InitializeBodyComposition: the client-side apply of a replicated state";
- "InitializeBodyComposition: tearing down a removed attachment".

**Writes in order, when every layer exists** (sections 2 and 3):
1. the body class load, if lazy;
2. the `InstanceDataState.Data` entries, attachment order, parents first;
3. the component attaches, each with its entry;
4. the authority branch's `ReplicatedInstanceDataState` fields, then `bNetDirty` and `bForceNetUpdate`;
5. `Mesh.RootMotionMode`, then `Mesh.RootMotionRotationMode`, each under its flag.

The "stopped after partial writes: " prefix is the caller's job, after a real write. Inside this native the first real
write is the lazy body-class load, so a refusal after it is terminal, with kind and record kept.

## Open questions

1. **UNVERIFIED:** the two root-motion fields and values (`RootMotionMode` = `RMM_Accel`, `RootMotionRotationMode` =
   `RMRM_RotateActor`), named by fit.
2. **UNVERIFIED:** the parent resolution for a component with no named parent: the owner itself, or a default
   attachment point the owner gives.
3. **UNVERIFIED:** the switch-field packing width when `BitsRequiredForMaxSwitchValue` is 0 (a fallback that was not
   settled).
4. **For the hub:** may a later lane split the build so the switch entries and network flags are written without the
   component layer, or does the whole build stay one `LayerAbsent` until a component layer exists?

## Live check that would confirm the rule (bl-sdk, the RTSE way)

On Maya in Sanctuary, read `InstanceDataState.Data`: expect one entry per created attachment, with component entries
for the 13 component attachments. Also read `ReplicatedInstanceDataState`'s switch fields, and
`Mesh.RootMotionMode` / `RootMotionRotationMode` (expect them unchanged from the mesh template, since both body-class
flags are false).
