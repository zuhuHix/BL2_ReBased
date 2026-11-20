# Actor.GetBoundingCylinder: an actor's collision radius and half height (draft, 2026-11-20)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-GET-BOUNDING-CYLINDER (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py` (every script function of the nine code packages
  searched for the call);
- the installed packages through `tools/ghidra/class_layout.py`, for field names and types;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling, read-only and under the machine lock. The native's
  exec step reads its two out arguments and calls one per-class virtual. That virtual was read in each of its three
  implementations, and every actor vtable in the image was counted by which implementation it holds;
- a local probe of the installed slice gate on a merge that is never pushed (section 5).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/gbc/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. Field names were fitted from the installed layouts; the fits stay under `local/`. **Every rule here is
UNVERIFIED** in the running game.

**Why it matters.** Links 6 and 7 stop at `touch` with an `error`, on the unbound native
`Actor.GetBoundingCylinder(float,float)`. The touch's `ValidTouch` reaches it through
`DroppedPickup.GetTouchTraceLocation`. This was observed with `InitializePickupForRBPhysics` and
`SpawnPickupParticles` stood in by no-write answers (NATIVE_SPAWN_PICKUP_PARTICLES.md section 5). The same rule is
link 9's stop "the actor's bounding-cylinder rule" in the spawn placement (NATIVE_SPAWN_PLACEMENT.md section 2,
`ow_world::placement::extent`), there asked of a template.

## 1. Where it is called

`GetBoundingCylinder(out float CollisionRadius, out float CollisionHeight)` writes only its two out arguments. Its
script callers:
- `DroppedPickup.GetTouchTraceLocation`, on the pickup itself. It returns the pickup's `Location`, raised by half the
  answered height. This is the call on links 6 and 7;
- `Actor.TakeRadiusDamage`, `Actor.CheckHitInfo`, `Actor.ApplyFluidSurfaceImpact` and `Actor.OnTeleport`;
- the `TakeRadiusDamage` overrides of `WillowPawn`, `WillowInteractiveObject` and `WillowVehicle`;
- `Action_LeapAtTarget.PlayIdle`, on its target.

The engine also asks it from native code: the spawn placement's extent, for a template whose collision component is
not a cylinder.

## 2. What it answers

The native table registers the same exec step for `Actor`, `NavigationPoint` and `Pawn`. The step calls the actor's
virtual. The executable has three implementations of that virtual. Of the actor vtables found by the bounding-box
routine they share, 282 hold the actor default, 21 the navigation-point one and 19 the pawn one. Which class holds
which was not mapped one by one. By the registrations, the last two are `NavigationPoint`'s and `Pawn`'s subtrees.

1. **The actor default** (`WillowPickup` holds it):
   - B := the bounding box of the actor's **colliding** components (the box routine, called with "colliding only");
   - radius := the length of B's horizontal half-diagonal, √(half-x² + half-y²);
   - height := B's vertical half size.

   An empty B gives 0 and 0.
2. **The box routine.** It starts from an empty box and walks the actor's `Components` in order. A component counts
   when:
   - it is not None and passes a component virtual (not read);
   - it is a `PrimitiveComponent`;
   - it is `bAttached`;
   - its `CollideActors` is set (the routine's "non-colliding too" argument is false here);
   - its `BulletListener` flag is clear (the fitted name; UNVERIFIED).

   Each counted component adds its `Bounds` box, which is `Bounds.Origin` ± `Bounds.BoxExtent`. When nothing counts,
   the box is empty.
3. **A pawn.** When the pawn is a template (a class default or an archetype, through its outer chain), its
   `CylinderComponent` is set and differs from its `CollisionComponent`, the answer is that `CylinderComponent`'s
   `CollisionRadius` and `CollisionHeight`. Otherwise, and so for **every live pawn**, the answer is the actor
   default's.
4. **A navigation point.** With its `CylinderComponent` set: that component's `CollisionRadius` and
   `CollisionHeight`, live or template. Without one: the actor default's.

**What it touches.** It writes nothing but its two out arguments. It loads nothing and draws nothing.

**Where the values come from.** `Bounds` is not a value the script sets. The engine computes it from the component's
geometry and transform when the component is attached to the scene and when it moves. The two `CollisionRadius` and
`CollisionHeight` paths read stored fields only.

## 3. For the slice's pickup and player

**The pickup.** It is a `WillowPickup`, so the actor default answers. In the probe
(section 5), at the touch, its `CollisionComponent` is its `SkeletalMeshComponent`. Its `Components` holds four
elements, in this order:
1. None (skipped);
2. a `CylinderComponent`: `CollideActors` set, `BulletListener` clear, `bAttached` clear, no owner, `Bounds` all zero.
   Its `CollisionRadius` and `CollisionHeight` are set (their source was not traced);
3. a `DynamicLightEnvironmentComponent`: not a `PrimitiveComponent` (skipped);
4. the `SkeletalMeshComponent`: `CollideActors` clear, `BulletListener` clear, `bAttached` clear, `Bounds` all zero.

These are the port's values after `InitializePickupForRBPhysics` and `SpawnPickupParticles` were stood in by no-write
answers. In the game, either native may change the mesh's collision flags, and both components would be attached with
engine-computed `Bounds`. So the game's counted set is not known from this probe (UNVERIFIED).

**What the rule answers for it.** In the game: the union of the counted components' `Bounds`, turned into a radius and a
half height (section 2). In the port, by section 4: the cylinder is a colliding primitive of a live actor, so the answer
is **refusal L**. The mesh is skipped on its clear `CollideActors`, as the port holds it today. If a later answer sets
that flag, the refusal stays L.

**The player.** Maya's pawn is a `WillowPlayerPawn`, under `Engine.Pawn`. Live, the pawn rule falls through to the actor
default (section 2, item 3), so she would get her colliding components' bounds, not her `CylinderComponent`'s radius and
height. In the port that is refusal L too. Links 6 and 7 do not ask it of her: `GetTouchTraceLocation` asks it of the
pickup only. The pawn was not probed.

## 4. A proposed Off answer, for the implementer

**Yes, it can be answered write-free, load-free and draw-free in Off**, as a rule over stored fields. The one
exception is component bounds, which the port does not compute.

**What the port lacks.** The port's `AttachComponent` (#520) leaves `bAttached` as it was and computes no `Bounds`.
That is the empty scene layer, as ruled. So on a live actor, the port's `bAttached` and `Bounds` do not mean what the
game's do. The answer must not read them as if they did.

**The answer, in order** (each step reads only loaded objects through the class table):
1. **Which implementation.** The receiver's class is a child of `Engine.Pawn`: the pawn rule. A child of
   `Engine.NavigationPoint`: the navigation-point rule. Otherwise: the actor default. This class mapping is the port's
   reading of the three registrations (UNVERIFIED). A class that overrides the box routine too would not show in the
   vtable count, so it is covered by the same UNVERIFIED label.
2. **The pawn rule.** Is the receiver a template, that is, a class default object or an archetype? The port's heap
   `Origin` decides:
   - `Origin::Default` is a template;
   - `Origin::Runtime` (spawned or `new`ed) is live;
   - `Origin::Export` or `Origin::Resource` could be either, because the port keeps no archetype flag: refusal T.

   The game also walks the outer chain. The port's spawned objects have no template outers, so that walk adds nothing
   here (UNVERIFIED).

   Then, when it is a template, its `CylinderComponent` is set and differs from its `CollisionComponent`: answer that component's `CollisionRadius` and
   `CollisionHeight`. Otherwise go to step 4.
3. **The navigation-point rule.** With `CylinderComponent` set: answer its `CollisionRadius` and `CollisionHeight`.
   Otherwise go to step 4.
4. **The actor default.** Walk `Components` in order and skip each element that is None, is not a
   `PrimitiveComponent`, has `CollideActors` clear, or has `BulletListener` set. Then:
   - **the receiver is a template** (as in step 2): its components are not attached to any scene in the game either
     (UNVERIFIED). None counts, so the answer is 0 and 0;
   - **a live receiver with any component left:** refusal L. The game would union those components' engine-computed
     `Bounds`, and the port has none;
   - **a live receiver with no component left:** 0 and 0.

   The component virtual the routine tests first was not read. The answer treats every component as passing it
   (UNVERIFIED).
5. **The out arguments** are written by the VM's out-parameter mechanism, which is this native's only effect.

**Declarations** checked before any read: `Components` (an array of component references), `CollisionComponent`,
`CylinderComponent` on the pawn and navigation-point classes, each counted component's `CollideActors` and
`BulletListener`, and `CollisionRadius` and `CollisionHeight` (floats) on `CylinderComponent`. A mismatch is a data
error.

**Refusals** (record `docs/verification/NATIVE_GET_BOUNDING_CYLINDER.md`):

| # | When | Kind | Text |
|---|---|---|---|
| L | a live actor with a colliding primitive component in `Components` | `LayerAbsent` | "Actor.GetBoundingCylinder: the colliding components' bounds", layer "component bounds (the headless world has none)" (the same layer text as the pickup targeting's) |
| T | the port cannot tell whether a pawn is a template | `NotRead` | "Actor.GetBoundingCylinder: whether the pawn is a template" |

**The spawn placement (link 9).** `ow_world::placement::extent` refuses "the actor's bounding-cylinder rule" for a
template without a cylinder collision component. Steps 2 to 4 answer that case: a pawn template with a separate
`CylinderComponent` gets its cylinder, and any other template gets 0 and 0. Whether that moves link 9 depends on its
templates. This note did not probe it.

**A port choice the hub could make instead of refusal L.** On links 6 and 7 the answer only feeds the touch trace's end
point. The next native, `DroppedPickup.TouchPickupTrace`, is refused `NotRead` in Off today (`ow_inventory::pickup`),
so no value of the cylinder changes the result yet. If the hub prefers to move the stop to that trace, the answer could
compute a `CylinderComponent`'s bounds from its own radius and height. That is the engine's usual cylinder bounds, but
it was not read here. It would still keep refusal L for meshes, whose bounds come from their geometry. Either way it
is UNVERIFIED.

## 5. The probe

A local probe of the installed slice gate (`ow-host --slice-gate`, Off) on a merge of `origin/main` with the open
attach-mesh fix branch. It is never pushed; its logs are under `local/analysis/gbc/probe/`. Stand-ins:
- `InitializePickupForRBPhysics`: a no-write `Ok`;
- `SpawnPickupParticles`: a no-write `Ok` that first printed the pickup's `CollisionComponent` and, for each element of
  `Components`, its class and the fields this rule reads. Nothing runs between that native and the touch's call, so
  the printed values are the ones the rule would see.

The gate: 1 of 14 links `ok` (link 2). Links 6 and 7 reach `touch` and stop on the unbound
`Actor.GetBoundingCylinder(float,float)`, three pickups in all (link 7 has a decoy), each with the same four components.
Link 9 still stops on "the actor's bounding-cylinder rule". The other links stop where `origin/main` stops them.

**After an implementation**, links 6 and 7 would stop on refusal L. With the hub's alternative (section 4), they would
reach `DroppedPickup.TouchPickupTrace`, refused `NotRead` in Off today. Neither was run, so both are UNVERIFIED.

## Open questions

1. **Not read:**
   - the component virtual the box routine tests first;
   - which classes hold which of the three implementations, one by one (only counted);
   - how and when the engine computes `Bounds`.
2. **UNVERIFIED:**
   - the `BulletListener` fit;
   - that a template's components are never `bAttached`;
   - the pawn's "differs from `CollisionComponent`" condition, as read.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Call `GetBoundingCylinder` on a dropped weapon pickup. Compare with its components' `Bounds`, `bAttached`,
   `CollideActors` and `BulletListener`: expect radius = √(half-x² + half-y²) and height = the half z size of the
   union of the counted boxes.
2. Call it on the player's pawn and on an enemy. Expect the same box rule, not the `CylinderComponent`'s values.
3. Call it on a pawn's class default object and on a navigation point. Expect the cylinder's `CollisionRadius` and
   `CollisionHeight`.
4. Call it on an actor with no colliding attached component. Expect 0 and 0.
