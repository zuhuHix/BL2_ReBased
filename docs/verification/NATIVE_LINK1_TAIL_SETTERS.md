# maya:8's tail: the blend and collision setters (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK1-TAIL (decision 2026-11-25-NNN), the analyst half of the clean
room: **another lane implements this note.** Raw output stays under ignored `local/analysis/tail/`. This note is in
our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is
UNVERIFIED in the running game.**

**Why.** A look-ahead for link 1 past `AnimNodeAimOffset.SetActiveProfileByName` (NATIVE_AIMOFFSET_SET_PROFILE.md,
#730). Section 1 walks `maya:8`'s `ApplyBodyClassStartingValues` to its end. Section 2 reads the next unread natives
that are simple reads or field writes: one Off answer and tests each, so the implementer can chain through them.

## 1. maya:8's path past SetActiveProfileByName (on paper, not a run)

Read with `research/script_disasm.py`. Maya's data was read with `ow-package --properties` / `--def-dump` (local). In
order:
1. `WillowPawn.SetUpperBodyMovementBlendState(true)` (from `ShouldUseUpperBodyMovementBlendForHoldName`, true for
   `Unarmed`):
   - `Mesh.Animations.FindAllAnimNodes(Nodes, 'UBMoveBlend')`: **2.A**. Maya's tree has four nodes of that name, all
     `WillowAnimNodeFeatherBlend`;
   - for each: cast to `AnimNodeBlend` and `SetBlendTarget(1.0, 0.0)`: **2.B**. A node that is an `AnimNodeBlendList`
     would get `SetActiveChild` instead: **2.C**, not reached for Maya;
2. `ArmBoneBlend`'s `SetBlendTarget`: Maya's tree has no node named `ArmBoneBlend`, so `ArmBoneBlend` is None and this
   is skipped;
3. `Mesh.UpdateAnimations` (NATIVE_BONE_TRANSFORM_LAYER.md, bound);
4. `WillowPlayerPawn.UpdateAnimSets`' own part runs only with `Arms` set, which it is not yet;
5. `SetupSkeletalMeshPhysics` (script), on `Mesh`:
   - `SetRBChannel(1)`: **2.D**;
   - `SetBlockRigidBody(true)`: **2.E**;
   - `SetActorCollision(true, false)`: **2.F**;
   - `SetTraceBlocking(true, true)`: **2.G**;
   - `bUpdateJointsFromAnimation` := true (script);
   - with `BodyClass.RD_EffectsThreshold` above 0: `ScriptRigidBodyCollisionThresholdCompiled` (script), then
     `SetNotifyRigidBodyCollision(true)`: **2.H**. The same again under `MinimumRecognizedImpact`;
   - with `bUseMeshForCollision` (false for Maya): `CollisionComponent` := `Mesh`, and `SetActorCollision(true, true)`;
   - then `InitializeRBPhysics` (the pawn's; **not read**, the next unread item) and
     `SetupSkeletalMeshPhysicsForPhysXEffects` (script);
6. `Mass` and, on authority, `bAlwaysRelevant` (script); `CheckPainSounds` (script, no natives); `CrouchHeight`;
7. `InitializeBehaviorProviders` (a `WillowPawn` native; NATIVE_BEHAVIOR_POPULATION.md reads `WillowAIPawn`'s, not
   `WillowPawn`'s: unread for Maya);
8. `SetDeathDefinition` → `IntializeBehaviorProviderForConsumer` (bound), then
   `BeginNondeterministicProviderRegistration` (bound), then two script writes;
9. `WillowPlayerPawn.ApplyBodyClassStartingValues`' own part:
   - `SetDefaultStance` (a `WillowPlayerPawn` native);
   - the first-person `Arms` (`New`, `AttachComponent`, `SetHidden`, `SetLightEnvironment`, then `UpdateAnimSets`
     with `GetFirstPersonWeaponHoldDef`, a native with no note);
   - the arms materials (`GetMaterial`, `SetParent`, `SetMaterial`);
   - `UpdatePreferredColors` (`GetLinearColor`, `GetInstancedMaterialInstanceConstant`, `SetVectorParameterValue`);
   - `OnNativeApplyBodyClassStartingValuesHook` (native, no note).

## 2. The natives

**Evidence for all eight:** Ghidra 12.1.4 headless, read-only, under the machine lock:
- each exec;
- for the natives that call a virtual, its slot in the class tables of `PrimitiveComponent`,
  `SkeletalMeshComponent`, `AnimNodeBlend` and `WillowAnimNodeFeatherBlend`, found as in NATIVE_COMPONENT_SKIP_FLAG.md
  section 6;
- field places by the local copy of `tools/ghidra/class_layout.py`, each class at one constant shift that places
  every field the routine reads. There are no bl-sdk reads.

### 2.A `AnimNode.FindAllAnimNodes(out array<AnimNode> Nodes, name NodeName)`

**What it does.** On the receiving node (the tree root here), it collects the node list. That is the component's
tick list when it is filled and its tree is this one, else the node class's own gather step. It is the same choice
as the name lookup in NATIVE_ANIM_NODE_LOOKUP.md section 1. It then **appends every node whose `NodeName` equals the
name**, in that list's order, to `Nodes`. `Nodes` is not cleared first. It writes nothing else.

**Off answer.**
- The same list as the port's name lookup (`anim_node_lookup`), keeping every match instead of the first;
- a lookup that the port's name lookup would refuse refuses the same way;
- a None receiver is the script's null context;
- the out array's declaration is checked (an array of object references), else `VmError::Script`.

**Tests:**
1. a toy tree with three nodes named `X`, `Y`, `X`: two nodes, in list order;
2. no match: `Nodes` unchanged;
3. a pre-filled `Nodes`: the matches are appended;
4. the tick-list branch and the gather branch, each giving the same result as the name lookup's list.

### 2.B `AnimNodeBlend.SetBlendTarget(float BlendTarget, float BlendTime)`

`AnimNodeBlendPerBone` and `WillowAnimNodeFeatherBlend` share this exec. It calls one virtual, whose body is
`AnimNodeBlend`'s, and `WillowAnimNodeFeatherBlend` adds one step to it.

**What `AnimNodeBlend`'s body does:**
1. the target is clamped to 0 to 1, and `Child2WeightTarget` := that value;
2. **the blend is immediate** when `BlendTime` is not above 0. It is also immediate when `bSkipBlendWhenNotRendered`
   is set, the node's `SkelComponent` has `bRecentlyRendered` clear, and an engine-wide flag is clear. That flag is
   the one hook 1's gate tests (NATIVE_BODYCOMP_RENDER_HOOKS.md), not identified;
   - immediate: `Child2Weight` := the target, `Children[0].Weight` := 1 - target, `Children[1].Weight` := target, and
     `BlendTimeToGo` := 0 (the time, forced to 0 on the not-rendered path);
   - otherwise: `BlendTimeToGo` := `BlendTime`, and nothing else (the tick does the rest).

**`WillowAnimNodeFeatherBlend`** runs the same, **then `Children[0].Weight` := 1.0**.

**Off answer.**
- **Preflight:** the receiver's class declares `Child2Weight`, `Child2WeightTarget`, `BlendTimeToGo` (floats),
  `bSkipBlendWhenNotRendered` (a bool) and `Children` (an array of the child struct, with a float `Weight`), else
  `VmError::Script`;
- the immediate path needs `Children` to hold at least two entries. Fewer is the engine's array check, so the port
  answers `VmError::Script`;
- `BlendTime` above 0 with `bSkipBlendWhenNotRendered` set and `bRecentlyRendered` clear needs the unidentified engine
  flag: `NotYet { NotRead }`, "AnimNodeBlend.SetBlendTarget: the engine-wide flag of the not-rendered test". With
  `bSkipBlendWhenNotRendered` clear, or `bRecentlyRendered` set, the flag does not matter;
- **writes,** in order: `Child2WeightTarget`, then on the immediate path `Child2Weight`, the two child weights and
  `BlendTimeToGo`; else `BlendTimeToGo`. `WillowAnimNodeFeatherBlend` then writes `Children[0].Weight` := 1.0;
- `AnimNodeBlendPerBone`'s and other subclasses' slot bodies were not read: `NotYet { NotRead }`, naming the class.

**Maya:** each `UBMoveBlend` node (a `WillowAnimNodeFeatherBlend`) gets `SetBlendTarget(1.0, 0.0)`. That gives
`Child2WeightTarget` 1, `Child2Weight` 1, `Children[0].Weight` 1 (after the feather step) and `Children[1].Weight` 1,
`BlendTimeToGo` 0.

**Tests:**
1. `(0.25, 0.0)` on a toy `AnimNodeBlend` with two children: the five writes, weights 0.75 and 0.25;
2. `(2.0, 0.0)`: clamped to 1;
3. `(0.5, 0.3)` with the not-rendered flag clear: only `Child2WeightTarget` and `BlendTimeToGo` 0.3;
4. `(0.5, 0.3)` with `bSkipBlendWhenNotRendered` set and `bRecentlyRendered` clear: the flag's `NotRead`, nothing
   written;
5. the feather subclass: `Children[0].Weight` 1.0 last;
6. one child only on the immediate path: `VmError::Script`;
7. an unread subclass (a toy `AnimNodeBlendPerBone`): its `NotRead`.

### 2.C `AnimNodeBlendList.SetActiveChild(int ChildIndex, float BlendTime)`

Its exec calls one virtual, whose body was **not read**. It is not on Maya's path, since her `UBMoveBlend` nodes are
all feather blends. **Off answer:** `NotYet { NotRead }`, "AnimNodeBlendList.SetActiveChild (its body is not read)".

### 2.D `PrimitiveComponent.SetRBChannel(byte Channel)`

**What it does:** `RBChannel` := the channel, then the component's collision-filter update. That update recomputes
the filter data and applies it to the shapes of the component's physics bodies:
- for a `PrimitiveComponent`, its root body instance;
- for a `SkeletalMeshComponent` not in `bUseSingleBodyPhysics`, each body of its `PhysicsAssetInstance`.

With no body, the update touches nothing.

**Off answer:**
- the `RBChannel` write (a byte; the declaration checked);
- the filter update acts only on physics bodies. With the component's body instance and `PhysicsAssetInstance` both
  None (the port makes none), it does nothing, by the game's own gate. With either set: `NotYet { LayerAbsent }`,
  "the rigid-body collision filter update", layer "a physics layer (rigid bodies)". #574's at-rest ruling
  (NATIVE_PICKUP_RB_MINIMAL.md) already leaves the filter update out for the pickup.

**Tests:** a component with no bodies: `RBChannel` written, nothing else. With a toy `PhysicsAssetInstance` set: the
`LayerAbsent`, nothing written.

### 2.E `PrimitiveComponent.SetBlockRigidBody(bool bNewBlockRigidBody)`

**What it does:**
- the physics part: for a `PrimitiveComponent`, its root body; for a `SkeletalMeshComponent`, each body of its
  `PhysicsAssetInstance`. Each body's collision is enabled or disabled;
- then the `BlockRigidBody` bit := the argument.

**Off answer:** the bit write. Physics bodies present: the same `LayerAbsent` as 2.D. None: the write only.

**Tests:** the bit set and cleared on a component with no bodies; the `LayerAbsent` with a toy `PhysicsAssetInstance`.

### 2.F `PrimitiveComponent.SetActorCollision(NewCollideActors, NewBlockActors, optional NewAlwaysCheckCollision)`

**What it does,** in order:
1. `AlwaysCheckCollision` := the third argument (false when omitted);
2. when `CollideActors` differs from the first argument: `CollideActors` := it, then **the deferred reattach**
   (NATIVE_ATTACH_MESH_TO_PICKUP.md section 2 and its 2.1 owner rule). Then, when `CollideActors` and
   `AlwaysCheckCollision` are both set and the component has an `Owner`, one more engine step (**not read**; by fit,
   the owner's touch update);
3. `BlockActors` := the second argument.

The three bits are the ones NATIVE_COMPONENT_SKIP_FLAG.md section 1 anchors.

**Off answer:**
- **preflight:** the three bools and the deferred reattach's fields declared. On the reattach path, the 2.1.1 owner
  rule's own refusals. The unread owner step when it would run: `NotYet { NotRead }`,
  "PrimitiveComponent.SetActorCollision: the owner step when CollideActors and AlwaysCheckCollision are set";
- **writes:** `AlwaysCheckCollision`; then, on a change, `CollideActors` and the reattach's writes; then
  `BlockActors`. All-or-nothing by the preflight.

**Maya:** `(true, false)`. `AlwaysCheckCollision` := false; `CollideActors` set if it was clear, with the deferred
reattach (her pawn is an ordinary owner: `bNeedsReattach`); `BlockActors` := false. The owner step is not reached.

**Tests:**
1. no change of `CollideActors`: the two plain writes, no reattach;
2. a change with an ordinary owner: `bNeedsReattach` too;
3. a change with all three set and an owner: the owner step's `NotRead`, nothing written;
4. the third argument omitted: `AlwaysCheckCollision` false.

### 2.G `PrimitiveComponent.SetTraceBlocking(bool NewBlockZeroExtent, bool NewBlockNonZeroExtent)`

**What it does:** `BlockZeroExtent` := the first, `BlockNonZeroExtent` := the second. Nothing else: no reattach.

**Off answer:** the two bit writes, the declarations checked.

**Tests:** all four combinations; no other field changes.

### 2.H `PrimitiveComponent.SetNotifyRigidBodyCollision(bool bNewNotifyRigidBodyCollision)`

**What it does:**
- the physics part: for a `PrimitiveComponent`, its `BodyInstance`'s physics actor; for a `SkeletalMeshComponent`,
  each body of its `PhysicsAssetInstance`. Each gets the contact-notify flag;
- then the `bNotifyRigidBodyCollision` bit := the argument.

**Off answer:** as 2.E: the bit write, and `LayerAbsent` only with physics bodies present.

**Tests:** as 2.E.

## 3. What link 1 would meet (a prediction, not a run)

Once `SetActiveProfileByName` answers (it is a `LayerAbsent` today, #730):
- the four feather nodes take 2.B's writes, and `UpdateAnimations` runs (bound);
- `SetupSkeletalMeshPhysics` takes 2.D to 2.H on her `Mesh`. Her mesh has no `PhysicsAssetInstance` and no body at
  this point in the port, so each is its field writes only;
- then **`InitializeRBPhysics`** on her pawn, unread, is the next stop.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the engine-wide flag of 2.B's not-rendered test (shared with hook 1);
   - 2.C's body;
   - 2.F's owner step;
   - the physics bodies' own calls in 2.D, 2.E and 2.H;
   - `AnimNodeBlendPerBone`'s `SetBlendTarget` body.
3. **Next unread on `maya:8`:**
   - the pawn's `InitializeRBPhysics`;
   - `WillowPawn.InitializeBehaviorProviders`;
   - `WillowPlayerPawn.SetDefaultStance` (indexed in part);
   - `GetFirstPersonWeaponHoldDef`;
   - `UpdatePreferredColors`' natives;
   - `OnNativeApplyBodyClassStartingValuesHook`.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read each `UBMoveBlend` node's `Child2Weight` and child weights. Expect 1, (1, 1).
2. Read her `Mesh`'s `RBChannel`, `BlockRigidBody`, `CollideActors`, `BlockActors`, `AlwaysCheckCollision`,
   `BlockZeroExtent` and `BlockNonZeroExtent`. Expect 1, true, true, false, false, true and true.
