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
   it returns with nothing done. In the shipped game this branch is not reachable (UNVERIFIED): the class is compiled
   into the executable and registered in package `Engine`, and a class default is always an instance of its class.
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
   them. The layout tool does not place these two fields where the native writes, so the names rest on that fit.
   **UNVERIFIED.**

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
5. **The walk.** It visits the attachments (`BodyInstanceDataUnion` items) once each, in list order. For each
   attachment, in this order:
   1. **Removed:** an attachment marked removed is torn down if it was created, and the walk moves on.
   2. **Its base first:** `AttachmentBaseInstanceDataIndex` names the attachment whose result is this one's attach
      base (-1 or out of range: none). A base not created yet is created first, by the same steps. In the installed
      shape the base always comes earlier in the list: the first attachment is the base of most others, and each
      switched component names its switch as its base.
   3. **Creation by type** (`Data.Type`, `IInstanceData.EInstanceDataType`). Each created entry is appended to
      `InstanceDataState.Data` at once, with `BodyCompositionIndex` = attachment index + 1, and its record notes the
      new entry index.
      - `IDT_ActorComponent` (5): a new component is made from the template `Data.ComponentData.Component`, owned by
        the owner. If it is not valid it is discarded and nothing is added. Otherwise an entry of type 5 is added
        holding the new component (name, `DeleteByOwner`, collision, socket, hit region and icons copied from the
        template data), then the component is attached. The attach goes to the parent the attachment names
        (`ParentAttachmentIdentifier`) or its base's component, at `Data.ComponentData.MeshSocketName`.
        **No-parent fallback (open, question 2):** with neither, the component goes to a default attach point the
        owner gives, read here as the owner itself, UNVERIFIED. An implementer refuses this case with
        `NotYet { NotRead }` until it is settled.
      - `IDT_Switch` (9): an entry of type 9 is added, with its object cleared (a switch state; no component).
      - `IDT_CustomHead` (11) and `IDT_CustomSkin` (12): an entry of that type is added, then the owner's current
        customization of that kind (`CustomizationType_Head` / `CustomizationType_Skin`, a `CustomizationDefinition`)
        is looked up and applied to the body.
      - `IDT_Object` (4), `IDT_ActorTemplate` (7) and `IDT_PickupTemplate` (10): an object load and two actor-spawn
        paths. None occurs in Maya's body class, and they were not read further.
      - The plain value types (`IDT_Bool`, `IDT_Int`, `IDT_Float`, `IDT_Vector`, `IDT_String`, `IDT_Actor`): no entry.
   4. **Mode 0.** In the mode the pawn passes, the visibility and collision toggles the other modes apply to created
      components are not taken. Where the old state held a matching entry, the old value is carried onto the new entry
      (a replay step, not read further).
6. **After the walk:**
   - **Authority** (owner `Role` = `ROLE_Authority`, 3):
     - `ReplicatedInstanceDataState.SwitchStateBitField` and `ActiveSwitchValues` are rebuilt from the switch entries.
       Each `IDT_Switch` attachment, in order and at most 32, sets the next bit of `SwitchStateBitField` when it has a
       state entry. That entry's `Int` value is packed into `ActiveSwitchValues` at the running bit offset, as wide as
       `BitsRequiredForMaxSwitchValue`, while the total stays within 32 bits.
     - **Zero width (open, question 3):** when `BitsRequiredForMaxSwitchValue` is 0, the width comes from another
       field of the entry, and that fallback was not settled. Maya's three switches have 0 there, so this is the
       slice's case. An implementer refuses it with `NotYet { NotRead }` until it is settled.
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
  - an empty `InstanceDataState` and a zero `ReplicatedInstanceDataState` (no removed parts, no state to apply).
- **The body class:** `BodyClass` is `BodyClass_Siren`. **At link 1's stop the store has not loaded it yet**; the probe
  had to load it to read the fields below. It has:
  - `UseRootMotion` and `UseRootRotation` both false, so step 2.4 writes nothing;
  - `MaxExpectedComponents` 0;
  - **18 attachments**, so the gate passes and the build runs:
    - 13 `IDT_ActorComponent` (3 of them under a switch);
    - 3 `IDT_Switch`, each with `BitsRequiredForMaxSwitchValue` 0;
    - 1 `IDT_CustomHead` and 1 `IDT_CustomSkin`, both replication `IDR_Local`;
  - every base index is -1 or an earlier attachment.
- **Branches taken (in game, UNVERIFIED):**
  - the helper check passes, and the gate passes;
  - the build creates components, switch entries and the two customization entries;
  - the authority branch rebuilds the switch fields (the zero-width case) and raises `bNetDirty` and `bForceNetUpdate`;
  - no root-motion write.
- **In the port's Off answer** (section 6), the unloaded body class stops it first: preflight step 3's
  `NotYet { NotRead }`.

## 5. Script-visible state that results (in game, UNVERIFIED)

- **`InstanceDataState.Data`:** one entry per created attachment: component entries holding new components attached to
  the pawn's mesh, switch entries, and the two customization entries.
- **`ReplicatedInstanceDataState`:** `SwitchStateBitField` and `ActiveSwitchValues` set from the switches.
- **The owner:** `bNetDirty` and `bForceNetUpdate` true.
- **The mesh:** its attached components (render state). `Mesh.RootMotionMode` and `Mesh.RootMotionRotationMode` are
  unchanged for Maya.

## 6. Proposed ParityMode Off answer (for a separate implementer)

**Hub rulings (2026-11-19):**
1. **No writes before the refusal.** The switch entries, the replicated switch fields and the network flags are not
   written without the component layer. Nothing is written before the `LayerAbsent` of step 5.
2. **No lazy load.** In Off an unloaded `BodyClass` is not loaded: preflight step 3 refuses with `NotYet { NotRead }`
   before any load or write.

So the native writes nothing at all in Off today. Each step either answers "nothing done" or refuses, and the
"stopped after partial writes: " prefix never applies inside it. A caller that wrote before calling it adds the prefix
itself.

**Classes:**
- bind under both `WillowPawn.InitializePawnMeshes()` and `WillowPlayerPawn.InitializePawnMeshes()`, Off only;
- Reference keeps the stub policy.

**Preflight**, write-free, in this order. No class lookup that can load, no definition-store load, no class default
made.
1. **The receiver.** It must be an object whose class is `WillowGame.WillowPawn` or a subclass of it. Otherwise:
   - no receiver: `VmError::Script`, function `WillowPawn.InitializePawnMeshes`, message "called with no pawn";
   - another class: `VmError::Script`, same function, message "<class path> is not a WillowPawn";
   - a receiver that is no longer in the heap: the heap's own error (`VmError::Heap`), unchanged.
2. **The helper class.** The predicate: `Engine.InstanceDataHelper` (package `Engine`, read from the packages and the
   class registration) is already in the class table. It is looked up in the table alone, not through `Vm::find_class`
   (which can load a class) and not under `GearboxFramework`.
   - Absent: `NotYet { NotRead }`, "WillowPawn.InitializePawnMeshes: the Engine.InstanceDataHelper class is not
     loaded". This is a port state; in the game the class is always present (section 2, step 1).
   - The native's own check, that the class default is an `InstanceDataHelper`, holds by construction (a class default
     is an instance of its class), so the port makes no default and adds no check for it. A wrong-type default cannot
     arise.
3. **The body class.**
   - The pawn's class must declare `BodyClass`, else `VmError::Script`, "<class path> declares no BodyClass".
   - Its value must be an object reference, else `VmError::Script`, "BodyClass holds <kind>, not an object".
   - None: return None, nothing done.
   - **Unloaded** (`DefStore::is_unloaded`) or a handle whose load already failed (`DefStore::is_failed`):
     `NotYet { NotRead }`, "WillowPawn.InitializePawnMeshes: the attachment list of the unloaded body class <object
     path>", record `docs/verification/NATIVE_INIT_PAWN_MESHES.md`. Nothing is loaded. **This is the slice's
     answer today** (section 4).
4. **The attachments.** `BodyComposition` missing on the body class, or not a struct holding an `Attachments` array, is
   `VmError::Script`, "<class path> has no BodyComposition.Attachments array". Empty: return None, nothing done. That
   also covers the root motion, which sits inside the gate.
5. **A non-empty list:** `NotYet { LayerAbsent }`, before any write:
   - text "WillowPawn.InitializePawnMeshes: the body composition build (InstanceDataHelper.InitializeBodyComposition:
     component instancing and attachment to the pawn's mesh)";
   - layer "a component and mesh layer (instancing components from templates, socket attachment)";
   - record `docs/verification/NATIVE_INIT_PAWN_MESHES.md`.

**Which parts need a layer the port does not have:**
- `IDT_ActorComponent`: instancing a component from its template and attaching it at a socket on a skeletal mesh (a
  scene and mesh layer);
- `IDT_CustomHead` and `IDT_CustomSkin`: applying the owner's customization (a customization layer, and the skeletal
  mesh it changes).

Under ruling 1, nothing else is written ahead of these.

**Not read** (named for a later analyst; `NotYet { NotRead }` if a later implementation reaches them):
- "InitializeBodyComposition: a component attachment with no parent and no base";
- "InitializeBodyComposition: the switch packing width when BitsRequiredForMaxSwitchValue is 0";
- "InitializeBodyComposition: IDT_Object, IDT_ActorTemplate and IDT_PickupTemplate creation";
- "InitializeBodyComposition: the old-state replay onto new entries";
- "InitializeBodyComposition: the client-side apply of a replicated state";
- "InitializeBodyComposition: tearing down a removed attachment".

**The write sequence once a component layer exists** (for a later lane; sections 2 and 3):
1. **Per attachment, in list order,** its base first when not yet made:
   - its `InstanceDataState.Data` entry is appended;
   - for a component, the component is made and attached to its parent or base, right after its own entry.

   So a switch's entry comes before the components it is the base of.
2. **After the walk, on authority:** `ReplicatedInstanceDataState.SwitchStateBitField` and `ActiveSwitchValues`, then
   `bNetDirty` and `bForceNetUpdate`.
3. **Last, under their flags:** `Mesh.RootMotionMode`, then `Mesh.RootMotionRotationMode`.

## Open questions

1. **UNVERIFIED:** the two root-motion fields and values (`RootMotionMode` = `RMM_Accel`, `RootMotionRotationMode` =
   `RMRM_RotateActor`), named by fit.
2. **UNVERIFIED:** the parent resolution for a component with no named parent: the owner itself, or a default
   attachment point the owner gives.
3. **UNVERIFIED:** the switch-field packing width when `BitsRequiredForMaxSwitchValue` is 0 (a fallback that was not
   settled).
4. **Ruled by the hub (2026-11-19):** the build is not split. The switch entries and network flags are not written
   without the component layer (section 6, ruling 1).

## Live check that would confirm the rule (bl-sdk, the RTSE way)

On Maya in Sanctuary, read `InstanceDataState.Data`: expect one entry per created attachment, with component entries
for the 13 component attachments. Also read `ReplicatedInstanceDataState`'s switch fields, and
`Mesh.RootMotionMode` / `RootMotionRotationMode` (expect them unchanged from the mesh template, since both body-class
flags are false).
