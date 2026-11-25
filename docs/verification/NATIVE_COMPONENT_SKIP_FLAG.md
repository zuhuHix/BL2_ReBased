# The component skip flag and the check before it (2026-11-20)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SKIP-FLAG (decision 2026-11-19-262, number pending), the analyst
half of the clean room: **another lane implements this note.** Raw output stays under ignored
`local/analysis/skipflag/`. This note is in our own words: no decompiler output, pseudo-code, addresses, table byte
offsets or listing structure. **Every rule here is UNVERIFIED in the running game.**

**The question.** `Actor.GetBoundingCylinder`'s live path (NATIVE_COMPONENT_BOUNDS.md section 7 B, #639; ported by #643)
stops at refusal S whenever a recorded, attached, colliding component reaches the **skip check**. Before it, a
**per-component check** that was never read is treated as passing. What does each one read, what does it mean, and
which value skips the component?

## 1. Evidence, in the order asked

- **bl-sdk live reads:** none. No real-game session was granted, and no recorded capture holds a component's flag
  word or the bounding cylinder's per-component walk.
- **The script** (`research/script_disasm.py`; every script function of the nine code packages scanned):
  - `PrimitiveComponent` declares the bool `BulletListener` in its collision bool word. Nothing in script reads or
    writes it.
  - WillowGame has a native class `BulletListenerComponent`, a `CylinderComponent` with an `OnBulletTouch` behavior
    array. `WillowProjectile` holds one (`BulletListenerComponent`). Its `TraceTouch` runs that component's
    `OnBulletTouch` behaviors. Its `HandleTouch` first asks the native `ShouldIgnoreTouchFrom(Other, OtherComp, ...)`.
  - The class default of `BulletListenerComponent` carries no tagged properties, so it does not set `BulletListener`
    itself. Who sets it on an instance was not found (section 5).
- **The native** (Ghidra 12.1.4 headless, read-only, under the machine lock; a local table reader for vtable slots):
  - the box routine that the bounding cylinder and `GetComponentsBoundingBox` both reach;
  - every function that tests the skip bit directly. There are eight, and none of them names the bit: the string
    `BulletListener` does not occur in the executable;
  - the native setters that write the same bool word;
  - the vtables of the component classes, read from their internal constructors, for the per-component check.

## 2. The skip check

**The rule.** The box routine skips a component whose collision bool word has **bit 10 (value 1024) set**. A clear
bit lets it through to the box.
- *Confidence:* high that this bit is the skip test (read in the routine). UNVERIFIED in game.

**The bit's name is `BulletListener`.** Confidence: high by structure; it is still a fit, so UNVERIFIED:
- **Declaration order.** In the packages, the word's bools are declared in this order: `bUsePrecomputedShadows`,
  `bInstancedStaticRB`, `CollideActors`, `AlwaysCheckCollision`, `BlockActors`, `BlockZeroExtent`,
  `BlockNonZeroExtent`, `CanBlockCamera`, `BlockRigidBody`, `bBlockFootPlacement`, `BulletListener`, ... and then
  `AbsoluteTranslation`, `AbsoluteRotation`, `AbsoluteScale` at bits 24 to 26. Bit 10 is `BulletListener`
  (`tools/ghidra/class_layout.py`).
- **Native anchors in the same word:**
  - `SetActorCollision` writes exactly bits 2, 3 and 4, that is `CollideActors`, `AlwaysCheckCollision` and
    `BlockActors`;
  - `SetTraceBlocking` writes exactly bits 5 and 6, `BlockZeroExtent` and `BlockNonZeroExtent`;
  - the transform update reads bits 24 to 26 as the three Absolute flags (NATIVE_COMPONENT_BOUNDS.md section 3, #639).

  The declaration order holds at both ends of bit 10. A native class's script bool word holds its C++ members in the
  same order; that is an engine-convention assumption (UNVERIFIED), not read here. So bit 10 is the declared bool between `bBlockFootPlacement` and
  `bDisableAllRigidBody`. The layout tool places the word a few bytes early, but the setters pin it as the same word,
  so the bit order is what counts.
- **Behaviour that fits the name** (each read in its native):
  - `WillowProjectile`'s touch filter (behind `ShouldIgnoreTouchFrom`) ignores a touch from a component with the bit
    set, ahead of every other test. A projectile passes through a bullet listener and does not stop on it;
  - the collision walkers skip a primitive with the bit set, except in a query that sets one particular trace-flag
    bit, which is presumably the bullet trace;
  - the bounding boxes leave it out (this section's rule).

  A primitive that notices bullets passing, and is left out of ordinary collision and of the actor's bounds, is
  what "bullet listener" names. WillowGame's own `BulletListenerComponent` is a cylinder whose behaviors run on a
  bullet touch.

**So bit 10 / `BulletListener` is corroborated as far as packages and code can, not rejected.** This agrees
with NATIVE_SPAWN_TRACE_FILTER.md section 4, which inferred the same name from the layout alone (medium), and raises
it to high.

## 3. The per-component check before it

**A different predicate.** The box routine's first test on each non-None entry is a **component virtual**, asked
before the cast to a primitive. It does not read bit 10 or any field the skip check reads.

What the classes answer, read from the vtables their internal constructors store:

| Class | Answer | Confidence |
|---|---|---|
| `PrimitiveComponent` | always true | medium-high: its slot is a one-line "return true" body shared by many unrelated functions, so identical code was folded |
| `CylinderComponent` | always true (the same body) | medium-high |
| `WillowGame.BulletListenerComponent` | always true (the same body) | medium-high |
| `ActorComponent` (the base) | always false (a shared "return nothing" body) | medium: this table may be a base or a secondary one. A non-primitive fails the cast right after anyway |
| `SkeletalMeshComponent`, `StaticMeshComponent`, `ParticleSystemComponent` and any other class | **not read**: the scan of their constructors did not identify their primary table | — |

**So for the cylinder family the check always passes.** That is a static read of their vtables, medium-high and
still UNVERIFIED. For every other primitive class it is unread (section 5: `NotRead`).

*Superseded on 2026-11-25 by section 6:* the virtual was read for all 93 native primitive classes (true) and for 52
non-primitive component classes (false), so the "not read" row above no longer holds.

## 4. The box routine, in order (for the record)

Per entry of the actor's `Components`, in order:
1. the entry is not None;
2. the per-component virtual answers true (section 3);
3. the entry is a `PrimitiveComponent`;
4. its `bAttached` is set. This is bit 0 of `ActorComponent`'s first bool word; that class's size matches its native
   registration, so the field is not shifted;
5. either the routine's argument says "non-colliding too", or `CollideActors` (bit 2) is set. `GetComponentsBoundingBox`
   asks for colliding components only. NATIVE_GET_BOUNDING_CYLINDER.md (#590) read the bounding cylinder's call as
   colliding only too, so `CollideActors` is required there (not re-read by this lane; *historical:* section 6
   re-read it on 2026-11-25 and confirms colliding only);
6. bit 10 (`BulletListener`) is clear;
7. then its `Bounds` box (`Origin` ± `BoxExtent`, read at that field's place) joins the union.

This agrees with NATIVE_COMPONENT_BOUNDS.md section 4's order. It adds that step 2 is a virtual of the component,
asked before the primitive cast, and that step 5 depends on the routine's argument.

## 5. The Off answer (for a separate implementer)

*Historical in part since 2026-11-25:* the component check below (its `NotRead` for every class outside three, the
"what links 6 and 7 would meet" item 2, and tests 4 and 6's component-check refusals) is superseded by section 6.1. The
skip check and the path orders still stand.

**Every rule below is UNVERIFIED.** It carries the hub's rulings on #653 (review r1): refusal S is lifted on every
path that has it, and unread component classes refuse `NotRead`. Reference is unchanged. New refusals carry this note
as their record, `docs/verification/NATIVE_COMPONENT_SKIP_FLAG.md`. Existing ones keep theirs.

**The skip check, wherever S stands today:** read `BulletListener` from the entry. It must be declared as a bool,
else `VmError::Script` naming it.
- **Set:** the entry is skipped, with no refusal.
- **Clear:** the entry goes on, as each path below says.

**The component check (the virtual of section 3).** The hub's ruling: the virtual is only a predicate. It has no side
effects, an assumption that stays **UNVERIFIED**: only its answers were read, for three classes. So it is asked at
the last moment, only for an entry that every other test would count: after the skip check and just before the
entry's box.
- **Exact class `Engine.PrimitiveComponent`, `Engine.CylinderComponent` or `WillowGame.BulletListenerComponent`:**
  it passes (read, section 3).
- **Any other class** (`SkeletalMeshComponent`, `StaticMeshComponent`, `ParticleSystemComponent`, a toy class):
  `NotYet { NotRead }`, "Actor.GetBoundingCylinder: the component check of <class> (not read)".

**`Actor.GetBoundingCylinder`'s actor default, per entry of `Components`, in order** (`ow-world`
`bounding_cylinder`):

*Live receiver:*
1. None: skipped.
2. Not a `PrimitiveComponent`, judged by the class the handle carries: skipped.
3. Not resident (a lazy handle, or a stand-in with no slots): refusal N.
4. `CollideActors` clear: skipped.
5. Not attached by the attach record: refusal L.
6. **The skip check:** set is skipped; clear goes on.
7. **The component check:** pass, or the `NotRead` above.
8. The entry's box (NATIVE_COMPONENT_BOUNDS.md section 7 B.7, `entry_box`). For a `CylinderComponent` that is its
   (R, R, H) box at the owner's `Location`, or that rule's world-transform `NotRead`. Every other class's B.7 answer
   is reached only after step 7, so today the class refusal comes first.

   `entry_box` matches `Engine.CylinderComponent` exactly. So a `WillowGame.BulletListenerComponent` passes step 7
   and then meets B.7's "the bounds update of WillowGame.BulletListenerComponent" `NotRead`.

*Template receiver* (#595's path):
1. None: skipped.
2. Not a `PrimitiveComponent`: skipped.
3. Not resident: refusal N.
4. `bAttached` clear: skipped.
5. `CollideActors` clear: skipped.
6. **The skip check:** set is skipped; clear goes on.
7. **The component check:** pass, or the `NotRead` above.
8. The bounds. A template's components have no computed `Bounds` in the port, so this is refusal L, as
   NATIVE_GET_BOUNDING_CYLINDER.md's step 7 says.

The box union and the "no entry gives 0 and 0" answer are unchanged.

**`placement::actor_rule`, the spawn extent** (NATIVE_SPAWN_EXTENT.md; NATIVE_GET_BOUNDING_CYLINDER.md keeps the two in
step). The ruling lifts S there too. Per entry, in order:
1. None, or not a primitive: skipped.
2. Unloaded at the spot search: `VmError::Script` (the caller skipped `load_extent_components`), as now.
3. A stand-in with no slots: `NotRead`, as now.
4. `bAttached` clear, or `CollideActors` clear: skipped.
5. **The skip check:** set is skipped; clear goes on.
6. **The component check:** pass, or the `NotRead` above.
7. **The bounds (its step 7):** the port computes none for a template's components. So step 7 answers
   `NotYet { LayerAbsent }` with the bounding cylinder's refusal L text and layer, "the colliding components' bounds"
   and "component bounds (the headless world has none)", on `EXTENT_RECORD`.

   For the installed class defaults, every loaded entry is expected unattached (NATIVE_SPAWN_EXTENT.md section 4), so
   none reaches step 5 and the answer stays (0, 0, 0) (UNVERIFIED).

**`trace_filter::SKIP_FLAG`'s wording.** The bit is now settled, so the text drops "unresolved": "the trace filter's
skip flag (`BulletListener`, bit 10 of a primitive's collision flag word)". The comment on `SKIP_FLAG_FIELD` changes
too: it says the spawn extent "still answers its own `NotYet` with `SKIP_FLAG`", which is no longer true. It now
reads the flag by this note's rule. After the lift, no refusal is raised with `SKIP_FLAG` on these paths.

**Trace filters:** `ow_world::trace_filter` already reads the bit as `BulletListener` (NATIVE_SPAWN_TRACE_FILTER.md).
This note raises that reading's confidence to high. Nothing else changes there.

**What links 6 and 7 would meet** (a prediction from the recorded probe values, not a run; the probe saw the bit clear
on all three entries):
1. The pickup's `CylinderComponent` is recorded and colliding, and its bit is clear. It passes the component check
   (read) and gives its box: R 64 and H 20 at the owner's `Location`, unless the world-transform rule refuses.
2. The `SkeletalMeshComponent` is colliding and recorded, and its bit is clear. It then meets the component check:
   `NotYet { NotRead }`, "the component check of Engine.SkeletalMeshComponent (not read)". That is the next stop, in
   `touch`, after `make`.
3. The `ParticleSystemComponent` does not collide, so it is skipped at step 4 and never reached.

**Tests for the implementer** (synthetic, `package()` / `tag()`). Each refusal leaves #518's three counters and the
name table unchanged:
1. a live entry, and a template entry, with `BulletListener` set: skipped, with no refusal;
2. with it clear: a live recorded `CylinderComponent` gives its box; a template entry meets refusal L at step 8;
3. `BulletListener` declared as another kind: `VmError::Script` naming it;
4. the component check, on a live recorded colliding entry with the bit clear:
   - a `CylinderComponent` passes and gives its box;
   - a `BulletListenerComponent` passes, then meets B.7's "the bounds update of WillowGame.BulletListenerComponent"
     `NotRead`;
   - a `SkeletalMeshComponent`, and a toy primitive class, are the component check's `NotRead`;
   - with the bit set, or not colliding, none of them is asked;
5. order: an unrecorded colliding entry is still L before the skip check is read; a lazy entry is N before
   `CollideActors`;
6. `placement::actor_rule`: an attached colliding template entry with the bit set is skipped (the answer stays (0, 0,
   0)). With it clear: a `CylinderComponent` meets step 7's `LayerAbsent`, and another class meets the component
   check's `NotRead`;
7. Reference: unchanged.

## 6. Amendment (2026-11-25): the component check, read for every primitive class

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SKELMESH-COMPONENT-CHECK (decision 2026-11-25-11),
analyst only: **another lane implements this section.** Raw output stays under ignored `local/analysis/skelcheck/`.
**Every rule here is UNVERIFIED in the running game.**

**Why.** Main's measured stop for links 6 and 7 (`pick_up_gun`, `pick_up_aimed`) on 2026-11-25 is in `touch`:
"Actor.GetBoundingCylinder: the component check of Engine.SkeletalMeshComponent (not read)", section 5's refusal for
a class section 3 had not read.

**Evidence, in the order asked.**
- **bl-sdk live reads:** none. No session was granted, and the check is a C++ virtual with no script face to call.
- **The script:** nothing. No script function stands for the check.
- **The native** (Ghidra 12.1.4 headless, read-only, under the machine lock; a throwaway table tracer kept under
  `local/analysis/skelcheck/scripts/`, not tracked):
  - the actor default's bounding cylinder, its box routine and the call between them, reached through `Actor`'s own
    table. The box routine is the one section 4 describes. The bounding cylinder asks it for **colliding components
    only**, which settles the call argument this note took from #590;
  - the check is one slot of the entry's **primary** table, called with no argument. Only its answer is tested,
    zero or not;
  - for each class, its primary table, found by the documented method (`docs/NATIVE_ANALYSIS.md`, "When `virtual:`
    refuses a class"). For the three classes section 3 missed, the table is stored one constructor call deeper than
    that scan looked;
  - which classes were examined, from two sides:
    - **the executable:** every registered class name ending in `Component`. That is 145 component classes, plus one
      non-component class the filter also caught (not counted);
    - **the packages:** the superclass chains of the nine code packages, without a name filter. They declare 96
      descendants of `PrimitiveComponent`, `PrimitiveComponent` itself included. The only one outside the name filter,
      `Engine.ApexComponentBase`, was then read the same way. So the primitive side is covered by hierarchy, not by
      name. They also declare 64 descendants of `ActorComponent` outside the primitive branch.

**What the check answers.** Two bodies, nothing else:
- **every class that descends from `Engine.PrimitiveComponent` answers true.** That is 93 native classes, and the body
  is the folded "return true" of section 3. Among them are `Engine.SkeletalMeshComponent`,
  `Engine.StaticMeshComponent`, `Engine.ParticleSystemComponent`, `Engine.MeshComponent`,
  `Engine.InstancedStaticMeshComponent`, `GearboxFramework.GearboxSkeletalMeshComponent`,
  `GearboxFramework.GearboxStaticMeshComponent`, `WillowGame.CustomSkeletalMeshComponent`,
  `WillowGame.GearLikenessMeshComponent`, and the three of section 3. The full list is at the end of this section;
- **every non-primitive component class examined answers false.** That is 52 of the packages' 64 (section 6's scope:
  the 12 not examined are the `RB_Handle` / `RB_Spring` and vehicle-simulation classes, `WillowAIEncounterComponent`
  and `WillowPointLight`, which the name filter missed; a non-primitive never reaches the check on the port's paths,
  so they do not bear on the Off answer). The 52 are `Engine.ActorComponent` itself, the light,
  fog, scene-capture, audio and wind components, `GearboxFramework.AIComponent`, `GearboxFramework.SpecialMoveComponent`,
  `WillowGame.MovementComponent`, the waypoint components, and others. The body is the folded "return nothing" of
  section 3, which settles that row (its "base or secondary table" doubt is gone).

Neither body reads a field, a flag or the mesh, and neither writes anything. The answer is a constant of the class.
In effect the check asks "is this a primitive component?". The executable does not name it, and it is not named here.

**The oracle.** The split matches the packages class by class: all 93 true classes have `PrimitiveComponent` in their
superclass chain, and none of the 52 false classes do. Of the packages' 96 descendants of `PrimitiveComponent`, the
other three (`WillowGame.DebugArrowComponent`, `WillowGame.NxFootForceFieldComponent` and
`WillowGame.RotationArrowComponent`) have no registration in the executable, so they are script-only.

**Two extrapolations, both UNVERIFIED:**
- a script-only class's instance uses its nearest native parent's table. That is an engine convention, not read here,
  so those three would answer true too;
- every native class has its script class in the nine packages, so no native primitive class exists outside the 93.
  That is also an engine convention.

**A second check of the table rule.** The same "last store into the first word" rule gives `Actor`'s table. Its two
slots there are the box routine and the bounding cylinder named by the exec stubs of `GetComponentsBoundingBox` and
`GetBoundingCylinder`, and `WillowPickup`'s table holds the same two. So the pickup takes the actor default, as
NATIVE_GET_BOUNDING_CYLINDER.md says.

**Confidence:**
- **high** for the answers: every slot was read, and the 93 / 52 split agrees with the hierarchy without one
  exception;
- **medium-high** that the table found is the primary one in each case: the store-order rule, plus the two checks
  above;
- **high** that the check has no side effects. Section 5 had to assume that.

### 6.1 The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces section 5's component check. The skip check, the order of steps and
the entry boxes are unchanged.

**Recommended: the component check passes for every entry that reaches it.** On all three paths (the live receiver,
the template receiver and `placement::actor_rule`), an entry gets there only once it is known to be a
`PrimitiveComponent`, by the class its handle carries. All 93 native primitive classes read answer true. The
unconditional pass then rests on the two UNVERIFIED extrapolations above: script-only classes inherit, and no native
primitive class lies outside the 93. So the check never refuses:
- `COMPONENT_CHECK_PASSES`, the exact-class list, and the `COMPONENT_CHECK_OF` refusal retire. Alternatively,
  `component_check` stays as a documented pass, to mark the step;
- it stays read-only, and no counter or name-table change follows;
- its place in the order (asked last, just before the box) changes no answer now, because it cannot refuse and has no
  side effects. It may stay where it is.

A non-primitive entry is skipped at the class test, as now. The game's false answer skips it too, one test earlier, so
the outcomes agree.

**An alternative, not recommended:** keep a refusal for an unread class. Walk up from the entry's class to its first
native class, by the fitted `CLASS_NATIVE` bit, as `ow-combat`'s `pawn_interface` read-only walk does. Pass when that
class is one of the 93 below, and keep the `NotRead` otherwise. No class in the nine packages would reach that
refusal, and it would cost a 93-name list, so it is offered only in case the hub wants the read set spelled out in
code.

**What links 6 and 7 would meet** (a prediction from main's measured stop and the recorded probe values, not a run):
1. The pickup's `CylinderComponent` passes and gives its box, as on main. The measured stop lies past it.
2. The `SkeletalMeshComponent` passes the component check. It then reaches its box,
   NATIVE_COMPONENT_BOUNDS.md section 7 B.7's skeletal case, a `NotYet` of kind `NotRead`, "Actor.GetBoundingCylinder: a
   skeletal mesh component's bounds source" (the `SKELETAL_SOURCE` text already on main, record
   NATIVE_COMPONENT_BOUNDS.md). That is the next stop, still in `touch`.
3. The `ParticleSystemComponent` does not collide. It is skipped before the check, as now.

The next analyst question is therefore the skeletal bounds update's two early returns and its source selector
(NATIVE_COMPONENT_BOUNDS.md section 3, "read at the level of its branches only").

**Tests for the implementer** (synthetic, `package()` / `tag()`). Each refusal leaves #518's three counters and the
name table unchanged:
1. a live, recorded, attached, colliding entry with `BulletListener` clear, of exact class:
   - `Engine.SkeletalMeshComponent`: the `SKELETAL_SOURCE` `NotRead`, with record NATIVE_COMPONENT_BOUNDS.md, and no
     component-check refusal;
   - `Engine.StaticMeshComponent`: the `STATIC_BOUNDS` / `STATIC_LAYER` `LayerAbsent`;
   - `Engine.ParticleSystemComponent`: "Actor.GetBoundingCylinder: the bounds update of
     Engine.ParticleSystemComponent" `NotRead`;
2. a toy script-only subclass of `PrimitiveComponent`, and one of `CylinderComponent`: both pass the check, then meet
   "the bounds update of <toy class>" `NotRead`. `entry_box` stays exact-class;
3. unchanged: a `CylinderComponent` gives its box, and a `BulletListenerComponent` meets B.7's `NotRead`;
4. the template receiver: the same `SkeletalMeshComponent` entry, attached and colliding with the bit clear, meets
   refusal L at step 8, not the check;
5. `placement::actor_rule`: the same entry meets step 7's `LayerAbsent`. The test that today expects
   "the component check of Engine.SkeletalMeshComponent (not read)" changes to expect that;
6. a non-primitive entry (a toy `ActorComponent` subclass) is still skipped at the class test;
7. no answer on any path carries the "the component check of" text;
8. Reference: unchanged.

**Live checks that would confirm it** (bl-sdk; none run, no session granted). The virtual cannot be called from script.
It needs a discriminating pickup: its skeletal component attached, colliding, with `BulletListener` clear, and with
bounds that reach past the cylinder's in a dimension the answer returns. On such a pickup, call `GetBoundingCylinder`
and compare it with the cylinder alone (√2 × R, H). A larger answer means the skeletal component was counted, so its
check passed. An unchanged answer is inconclusive: the skeletal bounds may simply lie inside the cylinder's.

**The 93 native classes that answer true:** `AkAudio.AkAmbientSoundRenderingComponent`,
`AkAudio.WwiseSoundGroupRenderingComponent`, `Engine.ApexComponentBase`, `Engine.ApexDynamicComponent`,
`Engine.ApexStaticComponent`, `Engine.ApexStaticDestructibleComponent`, `Engine.ArrowComponent`,
`Engine.BlockingMeshComponent`, `Engine.BrushComponent`, `Engine.CameraConeComponent`,
`Engine.CoverGroupRenderingComponent`, `Engine.CoverMeshComponent`, `Engine.CylinderComponent`,
`Engine.DecalComponent`, `Engine.DrawBoxComponent`, `Engine.DrawCapsuleComponent`, `Engine.DrawConeComponent`,
`Engine.DrawCylinderComponent`, `Engine.DrawFrustumComponent`, `Engine.DrawLightConeComponent`,
`Engine.DrawLightRadiusComponent`, `Engine.DrawPylonRadiusComponent`, `Engine.DrawQuadComponent`,
`Engine.DrawSoundRadiusComponent`, `Engine.DrawSphereComponent`, `Engine.FluidInfluenceComponent`,
`Engine.FluidSurfaceComponent`, `Engine.FoliageComponent`, `Engine.GBXNavMeshRenderingComponent`,
`Engine.HomingTargetComponent`, `Engine.HybridNavigationAreaDebugRenderingComponent`,
`Engine.HybridNavigationVisualizationComponent`, `Engine.InstancedStaticMeshComponent`,
`Engine.InteractiveFoliageComponent`, `Engine.LandscapeComponent`, `Engine.LandscapeHeightfieldCollisionComponent`,
`Engine.LensFlareComponent`, `Engine.LevelGridVolumeRenderingComponent`, `Engine.LineBatchComponent`,
`Engine.MeshComponent`, `Engine.ModelComponent`, `Engine.NavMeshRenderingComponent`, `Engine.NxForceFieldComponent`,
`Engine.NxForceFieldCylindricalComponent`, `Engine.NxForceFieldGenericComponent`, `Engine.NxForceFieldRadialComponent`,
`Engine.NxForceFieldTornadoComponent`, `Engine.ParticleSystemComponent`, `Engine.PathRenderingComponent`,
`Engine.PrimitiveComponent`, `Engine.RB_ConstraintDrawComponent`, `Engine.RB_RadialImpulseComponent`,
`Engine.RouteRenderingComponent`, `Engine.SkeletalMeshComponent`, `Engine.SocketComponent`, `Engine.SphereComponent`,
`Engine.SplineComponent`, `Engine.SplineMeshComponent`, `Engine.SpriteComponent`, `Engine.SpriteRTTComponent`,
`Engine.StaticMeshComponent`, `Engine.TerrainComponent`, `GearboxFramework.GearboxAIMoveNodeRenderComponent`,
`GearboxFramework.GearboxSkeletalMeshComponent`, `GearboxFramework.GearboxStaticMeshComponent`,
`GearboxFramework.PopulationEncounterRenderingComponent`, `GearboxFramework.PopulationOpportunityAreaRenderingComponent`,
`GearboxFramework.PopulationOpportunityCombatRenderingComponent`,
`GearboxFramework.PopulationOpportunityPointRenderingComponent`, `GearboxFramework.PopulationPointRenderingComponent`,
`WillowGame.BoundaryTurretRenderingComponent`, `WillowGame.BulletListenerComponent`,
`WillowGame.CustomSkeletalMeshComponent`, `WillowGame.GearLikenessMeshComponent`,
`WillowGame.LevelLandmarkAnchorRenderingComponent`, `WillowGame.PerchEditorComponent`,
`WillowGame.PerchPreviewComponent`, `WillowGame.PhysicsJumpNodeEditorComponent`,
`WillowGame.PhysicsJumpPreviewComponent`, `WillowGame.PopulationOpportunityDenRenderingComponent`,
`WillowGame.TravelStationNozzleComponent`, `WillowGame.WillowAIMoveNodePathComponent`,
`WillowGame.WillowDynamicNavMeshConnectionPointRenderingComponent`, `WillowGame.WillowElevatorRenderingComponent`,
`WillowGame.WillowFormationComponent`, `WillowGame.WillowInteractiveSwitchRenderingComponent`,
`WillowGame.WillowObstacleRenderComponent`, `WillowGame.WillowPopulationPointEditorComponent`,
`WillowGame.WillowPopulationPointPreviewComponent`, `WillowGame.WillowPreviewComponent`,
`WillowGame.WillowReplicatedAmbientSoundRenderingComponent`, `WillowGame.WillowVehicleCombatAreaRenderingComponent`,
`WillowGame.WorldDiscoveryAreaRenderingComponent`.

## Open questions

1. **Not read:**
   - ~~the per-component virtual of `SkeletalMeshComponent`, `StaticMeshComponent`, `ParticleSystemComponent` and the
     other primitive classes~~: read on 2026-11-25 (section 6);
   - who sets `BulletListener` on an instance (no script writes it, the class default does not, and no native writes
     the bit directly in the forms searched). Perhaps a projectile definition's component template, or a native
     setter through a register;
   - ~~the bounding cylinder's own call argument, which this lane takes from #590~~: colliding only (section 6);
   - the skeletal bounds update's early returns and source selector (NATIVE_COMPONENT_BOUNDS.md section 3), now links
     6 and 7's predicted next stop.
2. **UNVERIFIED:**
   - the bit's name (a structural fit plus fitting behaviour, no name in the executable);
   - the folded-body readings of sections 3 and 6;
   - that a script-only class uses its native parent's table (section 6).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On a live projectile, read its `BulletListenerComponent`'s `BulletListener`: expect it set.
2. On a dropped pickup, read each component's `BulletListener`: expect clear. Then call `GetBoundingCylinder` and
   compare with the cylinder's √2 × R and H.
3. Set `BulletListener` on a pickup's cylinder and call `GetBoundingCylinder` again: expect that component left out.
