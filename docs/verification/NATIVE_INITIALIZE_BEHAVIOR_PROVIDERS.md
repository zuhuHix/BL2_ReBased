# The player pawn's InitializeBehaviorProviders, and the material natives after it (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-INITIALIZE-BEHAVIOR-PROVIDERS (decision 2026-11-25-NNN), the analyst
half of the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/ibp/`
and `local/analysis/rbphys/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED in the running game.**

**Why.** NATIVE_LINK1_RBPHYSICS_AND_AFTER.md (#738) outlined `WillowPawn.InitializeBehaviorProviders` (its 2.B) and
`MeshComponent.GetInstancedMaterialInstanceConstant` (its 2.E) without naming their inputs. On `maya:8`'s
`ApplyBodyClassStartingValues`, the provider set-up comes once that note's 2.A (`InitializeRBPhysics`, the physics
`LayerAbsent`) answers. This note reads it in full, together with the material natives the player part reaches before
`ApplyBodyClassStartingValues` ends.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`):
  - `WillowPawn.ApplyBodyClassStartingValues` calls `InitializeBehaviorProviders`, then `SetDeathDefinition`, then
    `BehaviorKernel.BeginNondeterministicProviderRegistration` on the pawn's handle;
  - `WillowPlayerPawn.ApplyBodyClassStartingValues` goes on with `SetDefaultStance`, then the `Arms` (made from
    `BodyClass.FirstPersonHands`), then `UpdateAnimSets`. With `ArmsMaterialIndex` at least 0 and no instance yet, it
    then makes the arms material: a new `MaterialInstanceConstant` (script `new`) whose parent is
    `Arms.GetMaterial(ArmsMaterialIndex)`, set back with `Arms.SetMaterial`. It does the same for a customization
    instance parented to the first one. Then `UpdatePreferredColors` and the hook;
  - `WillowPlayerPawn.UpdatePreferredColors` runs only with a `BodyClass` **and** a `PlayerReplicationInfo`. Then it
    takes three colours through `GetLinearColor`. For each index of `Mesh.Materials` (the component's own override
    list) it calls `GetInstancedMaterialInstanceConstant(index)`. On a non-None answer it calls
    `SetVectorParameterValue` three times, with the body class's three colour parameter names;
  - **two script routes reach `ApplyBodyClassStartingValues`:**
    - `WillowPawn.PostBeginPlay` (with a `BodyClass`), which runs inside the spawn;
    - `WillowPlayerController.ApplyCharacterClassDefaults`, from `GameInfo.SetPlayerDefaults`.

    `GameInfo.RestartPlayer` spawns the pawn first, then `Possess` (where `Pawn.PossessedBy` copies the controller's
    `PlayerReplicationInfo` to the pawn), then `SetPlayerDefaults`. So **the spawn-time pass has no
    `PlayerReplicationInfo`**, and the second pass has one.
- **The natives:** Ghidra 12.1.4 headless, read-only, under the machine lock:
  - `WillowPawn.InitializeBehaviorProviders`' exec calls one pawn virtual. `WillowPlayerPawn` keeps `WillowPawn`'s body
    (#738). The body, its stance helper and the kernel routine every provider goes through were read whole;
  - the kernel routine is the **same one** the script-facing `BehaviorKernel.IntializeBehaviorProviderForConsumer`
    calls;
  - the routine the last step calls is the one `BehaviorKernel.ProcessReplicatedBehaviorConsumerState` calls. Its
    inner apply step was not read;
  - `MeshComponent.GetInstancedMaterialInstanceConstant`'s exec and its creation routine;
  - the component virtual both use (`GetMaterial`), read in `SkeletalMeshComponent`'s primary class table and
    checked equal in `GearboxSkeletalMeshComponent`'s, found as in NATIVE_COMPONENT_SKIP_FLAG.md section 6;
  - `MaterialInstanceConstant`'s parent setter and vector-parameter setter, read in its primary class table;
  - the execs of `MeshComponent.GetMaterial`, `MaterialInstance.SetParent` and
    `MaterialInstance.SetVectorParameterValue`.
    Each only calls one of those virtuals.
- **Field places** (the local copy of `tools/ghidra/class_layout.py`). Every field the bodies read lands on a declared
  property of the expected kind, which is the structural check of this reading:
  - `BodyClassDefinition`: `ScrapedStances`, `DefaultHitRegion`, `HitRegionList`, `DeathDef`, `PlayerEventDef`;
  - `BodyHitRegionDefinition`: `OverrideDeathDef` and `BehaviorProviderDefinition`, the last field of the class;
  - the `BehaviorProviderDefinition` fields of `BodyClassDeathDefinition`, `PlayerEventProviderDefinition` and
    `StanceTypeDefinition`;
  - the count of `BehaviorProviderDefinition.BehaviorSequences`;
  - the pawn's `InstanceDataState.Data`. The entry stride equals `InstanceDataUnion`'s declared size, and the field
    read is its `ComponentData.HitRegionDefinition`;
  - `MeshComponent.Materials`, `SkeletalMeshComponent.SkeletalMesh` and `SkeletalMesh.Materials`;
  - `MaterialInstance.Parent`, `MaterialInstanceConstant.VectorParameterValues`. The appended entry's size equals
    `VectorParameterValue`'s declared size;
  - `ConsumerHandle` and `ReplicatedBehaviorConsumerState.bNeedToApplyThisState`. On `WillowPawn` the layout tool
    places these fields one word earlier than the native reads them. That is the same skew as the earlier
    `WillowPawn` notes (it starts after the body-composition state). The fit holds under that skew.
- **Maya's data** (`ow-package --def-dump`, local):
  - `Pawn_Siren`:
    - `ConsumerHandle` `PID` -1, `bNeedToApplyThisState` false;
    - `InstanceDataState.Data` empty in the archetype. In the port, the body composition later fills 15 entries, and
      no attachment has a `HitRegionDefinition` (NATIVE_INITIALIZE_BODY_COMPOSITION.md section 6);
    - `PlayerReplicationInfo` None;
    - `Mesh`'s template `Materials` empty.
  - `BodyClass_Siren`:
    - `DefaultHitRegion` `HitRegion_Body`, `HitRegionList` empty;
    - five `ScrapedStances` (the run, patrol, sprint, injured and crouch stances);
    - `DeathDef` `Death_Siren`, `PlayerEventDef` `PlayerEvents_Siren`;
    - `ArmsMaterialIndex` 0, colour parameter names `PrimaryColor`, `SecondaryColor` and `ThirdColor`.
  - **The providers' sequence counts:**
    - each stance's definition, `Death_Siren`'s and `HitRegion_Body`'s: **0**, except the crouch stance's;
    - the crouch stance's: **1**, enabled on spawn, no condition;
    - `PlayerEvents_Siren`'s: **5**, four of them with a `CustomEnableCondition` (mission conditions).
  - `FirstPersonHands`:
    - a `GearboxSkeletalMeshComponent`, `Materials` empty, mesh `Hands_Siren`;
    - that mesh's `Materials` decodes as empty. Whether that is the package's value or a decoder gap was not checked
      (UNVERIFIED).

## 2. `WillowPawn.InitializeBehaviorProviders()`

### 2.1 What it does

**Only while `ConsumerHandle.PID` is -1** (exactly -1; any other value: nothing at all, including step 3):
1. **registers the pawn** as a behavior consumer and stores the kernel's handle in `ConsumerHandle`;
2. with a `BodyClass`, it hands **provider definitions** to the shared provider step (2.2), in this order:
   - **(a)** `DefaultHitRegion.BehaviorProviderDefinition`, only when `DefaultHitRegion` is set, `HitRegionList` is not
      empty and `DefaultHitRegion` is one of its entries (medium: the membership test was read by fit);
   - **(b)** for each non-None `HitRegionList` entry, in order: its `BehaviorProviderDefinition`, then, when its
      `OverrideDeathDef` is set, that death definition's `BehaviorProviderDefinition`.
      **So a `DefaultHitRegion` that passes (a) is handed over twice**: once there, once in this loop;
   - **(c)** for each non-None `ScrapedStances` entry, in order: the answer of its behavior-provider interface's
      `GetBehaviorProviderDefinition`. By fit, a stance's interface method answers its own
      `BehaviorProviderDefinition` field, as NATIVE_GET_BEHAVIOR_PROVIDER_DEFINITION.md section 2 reads for
      `ActionSkill` (**UNVERIFIED**: the stance's interface body was not read);
   - **(d)** for each `InstanceDataState.Data` entry whose `ComponentData.HitRegionDefinition` is set and is a
      `WillowGame.BodyHitRegionDefinition` (a class test): its `BehaviorProviderDefinition`;
   - **(e)** `DeathDef.BehaviorProviderDefinition`, when `DeathDef` is set;
   - **(f)** `PlayerEventDef.BehaviorProviderDefinition`, when `PlayerEventDef` is set;
3. when `ReplicatedBehaviorConsumerState.bNeedToApplyThisState` is set and the handle is valid: the kernel applies
   the replicated state (the step `ProcessReplicatedBehaviorConsumerState` uses; its inner part **not read**), and
   the flag becomes that step's answer. It is cleared when the kernel and the process exist, and kept otherwise.

Nothing else is written by the native itself.

### 2.2 The shared provider step (the kernel's gate)

Every definition above goes through one kernel routine. It does **nothing** unless all of these hold:
- the engine has a behavior kernel;
- the definition is not None;
- its `BehaviorSequences` is **not empty**;
- the handle names a consumer record that is in use.

Then it takes one of two branches on a property of the definition (**not read**). The first is the normal
registration that NATIVE_BEHAVIOR_POPULATION.md section C describes and the port's kernel rule implements (decode,
pass 1, then pass 2). The second consumes one of the consumer's deferred registrations (NATIVE_BEHAVIOR_PROVIDERS.md
section 1).

**Consequence:** a definition with no sequences is never registered, and a None one is skipped silently.

### 2.3 Maya's providers

With section 1's data, the provider step is reached seven times:
- the five stances;
- `Death_Siren`;
- `PlayerEvents_Siren`;
- none from the hit regions (the list is empty), and none from the instance data.

The gate passes **two**: the crouch stance's definition, then `PlayerEvents_Siren`'s. The second has sequences
with a `CustomEnableCondition`, which the port's kernel rule refuses in pass 2 (`NotYet { LayerAbsent }`, "pass 2 of
IntializeBehaviorProviderForConsumer: sequence ... has a CustomEnableCondition", layer "the condition's owner (the
mission tracker's observers)").

### 2.4 Off answer (proposed)

Bind under `WillowGame.WillowPawn.InitializeBehaviorProviders`, Off only. Reference keeps the stub policy. The rule
uses the kernel pack's own register and provider steps (`ow-behavior` `kernel::natives`), so it belongs in that crate or
calls into it.

**Preflight** (write-free and load-free), in this order:
1. **The receiver:** a `WillowAIPawn` (or a subclass) is `NotYet { NotRead }`, "WillowPawn.InitializeBehaviorProviders
   on a WillowAIPawn: the AI pawn's own body (NATIVE_BEHAVIOR_POPULATION.md section F)". The exec calls the pawn
   virtual, so an AI receiver reaches the AI body even through this key;
2. **declarations:** each of these declared with the kind named, else `VmError::Script` naming the field:
   - `ConsumerHandle` (a `BehaviorConsumerHandle` with an int `PID`);
   - `BodyClass` (an object);
   - `InstanceDataState` (`Data`, an array of `InstanceDataUnion` with `ComponentData.HitRegionDefinition`);
   - `ReplicatedBehaviorConsumerState.bNeedToApplyThisState` (a bool);
3. **`ConsumerHandle.PID` not -1:** nothing. Return with no write;
4. **no kernel service:** the kernel pack's `NotYet { LayerAbsent }`, "the behavior kernel natives", as
   `WillowWeapon.OnCreate` answers it;
5. **already registered:** a process that already holds this pawn as its consumer gets `RegisterBehaviorConsumer`'s own
   `NotRead`;
6. **the list** (reads only), in 2.1's order:
   - each object read must be resident. A store handle not loaded yet is `NotYet { NotRead }`,
     "InitializeBehaviorProviders: a definition the store has not loaded (a load is a write)";
   - the (d) class test is by name on the object's loaded class chain. No class code is loaded;
   - the gate of 2.2 is then applied: a None definition or an empty `BehaviorSequences` is dropped.
7. **the kernel step's own refusals, for every definition left, before any write:**
   - **the same definition twice** in the list is the kernel rule's `NotYet { NotRead }`, "a provider registered twice
     on one consumer". The game's handling of a repeat was not read. A `DefaultHitRegion` in `HitRegionList`
     always produces one;
   - **a sequence with a `CustomEnableCondition`** gives the kernel rule's pass-2 `NotYet { LayerAbsent }`, with the
     same text and layer. The test is a plain read of each entry's `CustomEnableCondition`.
8. **`bNeedToApplyThisState` set:** `NotYet { NotRead }`, "InitializeBehaviorProviders: applying the replicated
   behavior consumer state". A present kernel always gives a valid handle, so the game would run step 3.

**Commit:**
1. register the consumer by the kernel rule (`RegisterBehaviorConsumer`'s allocator, owner `Kernel`), and write
   `ConsumerHandle`;
2. each listed definition in order, by the kernel rule of `IntializeBehaviorProviderForConsumer` on that process
   (decode, pass 1);
3. step 3 does nothing (the flag is clear by preflight 8).

A refusal after commit step 1 (a behavior's own refusal in a provider's pass 1) carries the "stopped after partial
writes: " prefix, by #518's counters, with its kind and record kept. Before that step, no refusal has the prefix.

**Maya:** preflight 7 meets `PlayerEvents_Siren`'s conditional sequences: **the pass-2 `LayerAbsent`, with nothing
written** and `ConsumerHandle` still -1.

### 2.5 Tests (synthetic fixtures)

1. `ConsumerHandle.PID` 5: nothing written, no kernel process made;
2. no kernel service: the kernel `LayerAbsent`, nothing written;
3. `BodyClass` None: a process made, `ConsumerHandle` written, no provider;
4. a body class whose `DeathDef` definition has no sequences: a process with no provider;
5. a stance whose definition has one enabled-on-spawn sequence with no condition: one provider, pass 1 ran;
6. a `PlayerEventDef` definition with a conditioned sequence: the pass-2 `LayerAbsent`, `ConsumerHandle` still -1, no
   process made;
7. `DefaultHitRegion` listed in `HitRegionList`, with a non-empty definition: the repeat's `NotRead`, nothing written;
8. `DefaultHitRegion` set with `HitRegionList` empty: not registered;
9. a `HitRegionList` entry with an `OverrideDeathDef`: the entry's definition, then the death definition's, in that
   order;
10. an instance-data entry whose hit region is a `BodyHitRegionDefinition`: registered. One of another class of the
    same base: skipped;
11. `bNeedToApplyThisState` set: the `NotRead`, nothing written;
12. a stance handle not loaded: the not-loaded `NotRead`, nothing written;
13. the order: providers from (b), (c), (e) and (f), all non-empty: the process's providers in that order;
14. a later provider's pass-1 refusal after an earlier provider registered: the refusal with the partial-writes prefix;
15. a `WillowAIPawn` receiver: the AI-body `NotRead`.

## 3. The material natives

### 3.1 `MeshComponent.GetMaterial(int ElementIndex)` on a skeletal mesh component

The exec calls one component virtual. For `SkeletalMeshComponent` (and `GearboxSkeletalMeshComponent`, which keeps
it):
1. an index within the component's own `Materials` whose entry is not None: that entry;
2. otherwise, with a `SkeletalMesh` set and the index within the mesh's `Materials`, a non-None entry there: that;
3. otherwise None.

It writes nothing.

**Off answer:**
- the rule above, for a receiver whose first native class is `Engine.SkeletalMeshComponent` or
  `GearboxFramework.GearboxSkeletalMeshComponent`;
- any other first native class: `NotYet { NotRead }`, "MeshComponent.GetMaterial for <class>" (their bodies were not
  read);
- `Materials` and `SkeletalMesh` declared, and the mesh's `Materials` declared, else `VmError::Script`;
- a `SkeletalMesh` handle not loaded: `NotYet { NotRead }` (a load is a write).

**Tests:**
- an override entry wins;
- a None override entry falls through to the mesh's entry;
- an index past both lists gives None, as does a negative index;
- no mesh gives None.

### 3.2 `MaterialInstance.SetParent(MaterialInterface NewParent)` on a `MaterialInstanceConstant`

The exec calls one material virtual. `MaterialInstanceConstant`'s body: when `Parent` differs from the argument,
`Parent` := the argument and a material update follows (a render-side step, by fit). When it is the same, nothing.

**Off answer:**
- for a receiver whose first native class is `Engine.MaterialInstanceConstant`: that one write, or nothing when the
  value is the same. The update has no counterpart in the port (no render state in the VM), so nothing;
- any other first native class (for example a time-varying instance): `NotYet { NotRead }`,
  "MaterialInstance.SetParent for <class>";
- `Parent` declared as an object, else `VmError::Script`.

**Tests:**
- a new parent is written;
- the same parent writes nothing (no write counted);
- a non-constant instance class gives the `NotRead`.

### 3.3 `MeshComponent.GetInstancedMaterialInstanceConstant(int ElementIndex)`

1. `m` := `GetMaterial(ElementIndex)` (3.1);
2. when `m` is a `MaterialInstanceConstant` (a class test) **and its `Outer` is this component**: the answer is `m`.
   Nothing is written;
3. otherwise **a new `MaterialInstanceConstant` is made with this component as its `Outer`**, with no name given and
   no template (read from the construction call's arguments; its flags were not decoded). Then:
   - `SetParent(GetMaterial(ElementIndex))` on it (3.2's body, with the material asked again);
   - `SetMaterial(ElementIndex, it)` on the component: the same routine as the script's `MeshComponent.SetMaterial`,
     whose rule NATIVE_INITMESHES_MATERIAL_GLOW.md section 2 gives (#705);
   - the answer is the new instance. With a negative index the instance is still made, and `SetMaterial` does
     nothing.

**Off answer:**
- **preflight** (write-free and load-free):
  - 3.1's refusals;
  - on the creation branch, `Engine.MaterialInstanceConstant` ready (loaded). Otherwise `NotYet { NotRead }`: a
    class-code load is a write;
  - `SetMaterial`'s preflight for that index (its reattach refusals, `REATTACH_OWNER` and `REATTACH_SCENE`), before
    the object is made;
- **commit:**
  - make the object as the VM's `new` does with that `Outer` and no name (the made object and the name it interns
    count as writes);
  - 3.2's rule;
  - `SetMaterial`'s commit;
  - return it.

  A refusal after the object is made carries the partial-writes prefix.

**Tests:**
1. a material that is an instance owned by the component: returned, nothing written;
2. an instance owned by something else: a new instance, whose `Parent` is that instance, set at the index;
3. a plain material: a new instance with that parent;
4. an index past the end: `Materials` grows (by `SetMaterial`'s rule), with the new instance at the index;
5. a component with a static owner: `REATTACH_OWNER`, and no object made;
6. a negative index: a new instance is returned and `Materials` is unchanged.

### 3.4 `MaterialInstance.SetVectorParameterValue(name ParameterName, LinearColor Value)` on a `MaterialInstanceConstant`

The exec calls one material virtual. `MaterialInstanceConstant`'s body:
1. finds the **first** `VectorParameterValues` entry whose `ParameterName` equals the argument (name and number);
2. none found: appends one with that name, a zero `ExpressionGUID`, and the value;
3. found: when any of R, G, B, A differs (an exact f32 compare), writes the four. Otherwise nothing. Then a material
   update (render-side, by fit).

**Off answer:**
- for a receiver whose first native class is `Engine.MaterialInstanceConstant`: the rule above. The update is nothing
  in the port;
- any other first native class: `NotYet { NotRead }`, "MaterialInstance.SetVectorParameterValue for <class>";
- the array and its entry fields declared, else `VmError::Script`.

**Tests:**
- an append on an empty list;
- an update of the first match when two entries share the name;
- the same value: no write;
- a NaN component always writes, since NaN never compares equal.

### 3.5 What `maya:8` reaches of these

- **The arms** (on the path, after `SetDefaultStance`): script `new` of the instance, then 3.2 with `Arms.GetMaterial(0)`
  (3.1). With the decoded data, both `Materials` lists are empty, so the parent is None and 3.2 writes nothing. Then
  `SetMaterial` (#705, bound).
- **`UpdatePreferredColors`:**
  - on the spawn-time pass the pawn has no `PlayerReplicationInfo`, so the whole function is skipped;
  - on the second pass it would run, but `Mesh.Materials` (attachment 0's, from a template with none) is empty in
    the port, so the loop runs zero times. 3.3 and 3.4 are then not reached;
  - in the game the skin customization may fill it. That is NATIVE_INITIALIZE_BODY_COMPOSITION.md open question 6,
    UNVERIFIED;
  - the harness copies the controller's `PlayerReplicationInfo` onto the pawn only after the class switch
    (`ow-combat` `prepare_player_pawn`). Whether the port's class switch runs `RestartPlayer`'s possession first
    was not read.

## 4. What link 1 would meet (a prediction, not a run)

1. **2.A of #738 is the stop today:** the physics `LayerAbsent`;
2. once it answers (a physics layer, or a hub ruling): `InitializeBehaviorProviders`, unbound today. With section 2's
   rule: **the pass-2 `LayerAbsent` for `PlayerEvents_Siren`'s conditional sequences, nothing written**. That is the
   next stop;
3. once mission-condition sequences can register: `SetDeathDefinition` (script) and
   `BeginNondeterministicProviderRegistration` (bound);
4. then `SetDefaultStance` (indexed, Off `NotRead`, NATIVE_GO_DOWN_PATH.md);
5. then the `Arms`, `UpdateAnimSets`' player part (#738 2.C), the arms material (3.1, 3.2, `SetMaterial`),
   `UpdatePreferredColors` (skipped or empty, 3.5) and the hook (#738 2.F).

## 5. Side finding: the script-facing kernel native shares the gate

`BehaviorKernel.IntializeBehaviorProviderForConsumer`'s exec calls the same routine as 2.2. So in the game, a None
definition and a definition with no sequences are skipped there too. Two cases in the port's rule differ:
- a None provider is `NotYet { NotRead }`;
- an empty definition is registered as a provider with no sequences.

Neither is changed by this note. Whether to align the kernel rule is the hub's call, for a separate lane.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the provider step's second branch (the deferred registrations), and the definition property that picks it;
   - the replicated-state apply step;
   - the stance's interface method;
   - the game's handling of a definition handed over twice;
   - the material update after `SetParent` and `SetVectorParameterValue`;
   - the flags of the made instance;
   - `GetMaterial` for non-skeletal mesh components.
3. **The membership test** of 2.1 (a) was read by fit (medium).
4. **Maya's mesh materials:** whether `Hands_Siren` and `Skel_SirenBody` have an empty `Materials` list or the
   decoder misses it.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read her pawn's `ConsumerHandle`: expect a valid `PID`. List that consumer's providers: expect
   the crouch stance's and `PlayerEvents_Siren`'s only, in that order.
2. Hook `ApplyBodyClassStartingValues` and log `PlayerReplicationInfo` on each call: expect None on the first and set on
   the second.
3. Read `Arms.Materials[0]` after spawn: expect a `MaterialInstanceConstant` whose `Outer` is the pawn's `Outer` (script
   `new`), and read its `Parent`.
4. Read `Mesh.Materials` after spawn, and each entry's `VectorParameterValues`: expect the three colour parameters
   when the list is non-empty.
