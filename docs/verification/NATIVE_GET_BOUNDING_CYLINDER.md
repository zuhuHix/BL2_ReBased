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
`SpawnPickupParticles` stood in by no-write answers (NATIVE_SPAWN_PICKUP_PARTICLES.md section 5, **pending**: that
note is #581, not yet on main). The same engine method also gives the spawn's extent for a template without a cylinder
collision component. NATIVE_SPAWN_EXTENT.md reads it for that use, and main implements that reading
(`ow_world::placement::extent`, lane IMPL-LINK9-EXTENT). Section 2a sets this note against it.

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
   - its skip flag is clear. That is the collision-flag bit the trace filters skip on. The layout tool's name for it
     is `BulletListener`; section 2a says why the answer does not read it.

   Each counted component adds its `Bounds` box, which is `Bounds.Origin` ± `Bounds.BoxExtent`. When nothing counts,
   the box is empty.
3. **A pawn.** The answer is the `CylinderComponent`'s `CollisionRadius` and `CollisionHeight` when all three hold:
   - its `CylinderComponent` differs from its `CollisionComponent`;
   - the pawn is a template, that is, the pawn itself or an object on its outer chain carries the class-default or
     archetype flag;
   - its `CylinderComponent` is set.

   Otherwise the answer is the actor default's, and so for **every live pawn**. No class test is made on the
   component.
4. **A navigation point.** With its `CylinderComponent` set: that component's `CollisionRadius` and
   `CollisionHeight`, live or template. Without one: the actor default's.

**What it touches.** It writes nothing but its two out arguments. It loads nothing and draws nothing.

**Where the values come from.** `Bounds` is not a value the script sets. The engine computes it from the component's
geometry and transform when the component is attached to the scene and when it moves. The two `CollisionRadius` and
`CollisionHeight` paths read stored fields only.

## 2a. Against NATIVE_SPAWN_EXTENT.md (main)

NATIVE_SPAWN_EXTENT.md (decision 2026-11-19-09) reads the same method for the spawn's extent, and `ow_world::placement`
implements it. The two readings meet on four points:

| Point | NATIVE_SPAWN_EXTENT.md | This note | Which wins, and why |
|---|---|---|---|
| The skip flag | the bit's name is unresolved, and the extent refuses it (`trace_filter::SKIP_FLAG`). NATIVE_SPAWN_TRACE_FILTER.md later infers `BulletListener` for the trace filter only | the layout tool's fit, `BulletListener` | **main's.** This lane has no evidence for the name beyond the same layout fit. An entry that reaches the flag refuses `NotRead` with main's `SKIP_FLAG` text |
| The pawn rule | the `CylinderComponent` set, not the `CollisionComponent`, and a cylinder component; no template condition | the same first two conditions, **plus "the pawn is a template"**, and no class test on the component | **this note's**, on new evidence. Re-read for this lane: the body's flag test runs on the pawn itself, not on the component, and walks its outer chain for the class-default and archetype flags. No class test on the component was found. Both readings give the same answer for a template (link 9's case), so main's extent needs no change. They differ only for a live pawn, which the spawn never asks about |
| Classes not read | refused: "the bounding-cylinder rule of <class>", by exact class | the three bodies counted over all vtables, but not mapped to classes one by one | **main's.** The census shows no fourth body among the vtables found, but it does not say which class holds which body. So every class whose body was not read refuses, by exact class (section 4, step 1) |
| A template's `bAttached` | read from the loaded entry, not assumed | assumed clear (an earlier draft) | **main's.** The answer reads the entry's own `bAttached` |

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
half height (section 2). In the port, by section 4: `WillowPickup` is a class whose body was read (the actor default),
the pickup is live (`Origin::Runtime`), and its first primitive entry is the resident cylinder. So step 5.4 answers
**refusal L**, before the mesh is reached. Any later change to the mesh's flags leaves the answer at L.

**The player.** Maya's pawn is a `WillowPlayerPawn`, under `Engine.Pawn`. Live, the pawn rule falls through to the actor
default (section 2, item 3), so she would get her colliding components' bounds, not her `CylinderComponent`'s radius and
height. In the port that is refusal L too. Links 6 and 7 do not ask it of her: `GetTouchTraceLocation` asks it of the
pickup only. The pawn was not probed.

## 4. A proposed Off answer, for the implementer

**It can be answered write-free, load-free and RNG-free in Off**, as a rule over stored fields, with two exceptions:
- component `Bounds`, which the port does not compute;
- attachment, which the port does not track: its `AttachComponent` (#520) leaves `bAttached` as it was.

So on a live actor the port's `bAttached` and `Bounds` do not mean what the game's do, and the answer does not read them
there.

**Nothing is loaded.** A lazy handle (an entry the store has not loaded, with no slots) is never loaded here. Class
tests use only the classes already in the class table (`Engine.Pawn`, `Engine.NavigationPoint`,
`Engine.PrimitiveComponent` and the classes of step 1). An absent class counts as "not a child", and nothing is loaded
through `find_class`.

**The answer, in order.** It computes both values first and writes the two out arguments only at the end, so a refusal
or a data error writes neither.
1. **Which body, by the receiver's exact class:**
   - `Engine.Pawn`, `WillowGame.WillowPawn`, `WillowGame.WillowAIPawn`, `WillowGame.WillowPlayerPawn`: the pawn rule
     (step 3);
   - `Engine.NavigationPoint`: the navigation-point rule (step 4);
   - `Engine.Actor` and `WillowGame.WillowPickup`: the actor default (step 5);
   - any other class: `NotYet { NotRead }`, "the bounding-cylinder rule of <class>", main's text and record. Its body
     was not read for this lane or for NATIVE_SPAWN_EXTENT.md.

   The pawn classes are main's `PAWN_RULE_CLASSES`. `WillowPickup` is added on this lane's reading of its vtable, which
   holds the actor default.
2. **Template or live**, from the receiver's heap `Origin`:
   - `Origin::Default` is a template;
   - `Origin::Runtime` (spawned or `new`ed) is live;
   - `Origin::Export` or `Origin::Resource` could be either (a placed actor, or an archetype), because the port keeps no
     archetype flag: refusal T.

   The answer needs this only where it is used: in step 3, and in step 5's `bAttached` test. A receiver that never
   reaches either use is not refused. The game also walks the outer chain. The port's spawned objects have no template
   outers, so that walk adds nothing here (UNVERIFIED).
3. **The pawn rule.** It needs the `CylinderComponent` differing from the `CollisionComponent`, the receiver a template
   (step 2), and the `CylinderComponent` set. Then it answers that component's `CollisionRadius` and `CollisionHeight`.
   The component must be resident: an unloaded one refuses N. Otherwise go to step 5.
4. **The navigation-point rule.** With `CylinderComponent` set (and resident, else N): its `CollisionRadius` and
   `CollisionHeight`. Otherwise go to step 5.
5. **The actor default.** For each entry of `Components`, in order:
   1. None: skipped;
   2. not a `PrimitiveComponent`, decided from the entry's class (a lazy handle carries it): skipped. Nothing is loaded
      for it;
   3. a lazy handle: refusal N, never a load;
   4. `bAttached`:
      - on a template, read from the entry, and clear means skipped (main's reading);
      - on a live receiver, refusal L, because the port does not track attachment;
      - on an `Export` or `Resource` receiver, refusal T;
   5. `CollideActors` clear: skipped;
   6. the skip flag: `NotYet { NotRead }` with main's `trace_filter::SKIP_FLAG` text;
   7. `Bounds`: not reached while step 6 refuses. When the flag is read, refusal L, because the port has no component
      bounds.

   With no entry left: 0 and 0, through main's `placement::box_cylinder` of no boxes.

   The component virtual that the routine tests first was not read. The answer treats every component as passing it
   (UNVERIFIED).
6. **The out arguments:** both are written last, through the VM's out-parameter mechanism. That is the native's only
   effect.

**Malformed data is a `VmError::Script` naming the field.** That covers:
- `Components` that is not an array of component references;
- `CollisionComponent` or `CylinderComponent` that is not an object;
- an entry's `bAttached` or `CollideActors` that is not a bool;
- a cylinder's `CollisionRadius` or `CollisionHeight` that is not a float;
- a field the class does not declare.

These are the same errors as main's actor field reads (`ow_world::actors`).

**Refusals** (record `docs/verification/NATIVE_GET_BOUNDING_CYLINDER.md` unless named):

| # | When | Kind | Text |
|---|---|---|---|
| C | a class whose body was not read (step 1) | `NotRead` | "the bounding-cylinder rule of <class>" (main's, record NATIVE_SPAWN_EXTENT.md) |
| T | the receiver's `Origin` is `Export` or `Resource` where step 2 is used | `NotRead` | "Actor.GetBoundingCylinder: whether the actor is a template" |
| N | a cylinder component or a primitive `Components` entry that is a lazy handle | `NotRead` | "Actor.GetBoundingCylinder: a component the store has not loaded (a load is a write)" |
| L | a live receiver's primitive entry at `bAttached`, or a counted entry's `Bounds` | `LayerAbsent` | "Actor.GetBoundingCylinder: the colliding components' bounds", layer "component bounds (the headless world has none)" (the pickup targeting's layer text) |
| S | an entry that reaches the skip flag | `NotRead` | main's `trace_filter::SKIP_FLAG` text |

Every refusal comes before the native's only write, so none carries the "stopped after partial writes: " prefix inside
it. The caller's earlier writes can mark it.

**Reference is unchanged.** The C++ reference binds no `GetBoundingCylinder`, so Reference keeps the VM's stub policy.

**Tests the implementer owes** (synthetic fixtures, the existing helpers):
- a live (`Runtime`) pickup with a colliding cylinder entry: L, and neither out argument written;
- a class-default (`Default`) actor whose loaded entries are unattached: 0 and 0;
- a class-default entry that is attached and colliding: S;
- an `Export`-origin pawn with a separate `CylinderComponent`: T;
- a live pawn with a separate `CylinderComponent`: the actor default, not the cylinder;
- a class-default pawn with a separate `CylinderComponent`: that cylinder's values;
- a navigation point with a cylinder (its values) and without one (the actor default);
- a lazy-handle primitive entry: N, and the handle is still unloaded afterwards;
- a non-primitive lazy-handle entry: skipped, with nothing loaded;
- a class outside step 1: C;
- `Components` that is not an array, and an entry's `CollideActors` that is not a bool: `VmError::Script` naming the
  field;
- on every refusal, no slot of any object written and no store load (a heap fingerprint before and after).

**The spawn placement (link 9).** Main answers the spawn's extent itself (`placement::extent`). Its template rule and
this note's steps 3 and 5 give the same answer for a template. So nothing here changes link 9, and this native need
not share code with it. Sharing `box_cylinder` and the `SKIP_FLAG` text keeps the two in step.

**No cylinder-bounds stand-in is proposed.** An earlier draft offered to compute a cylinder entry's bounds from its
radius and height in place of refusal L. The gain is small. For links 6 and 7 only the half height matters, and no value
changes the result until `DroppedPickup.TouchPickupTrace` (refused `NotRead` in Off today) is answered. It would also
need rules that were not read: the bounds' centre, whether scale and rotation apply, and a stand-in for `bAttached`.
So refusal L stays.

## 5. The probe

A local probe of the installed slice gate (`ow-host --slice-gate`, Off) on a merge of an earlier `origin/main` with
the open attach-mesh fix branch (**pending**: that branch is not on main, so the probe's pickup state rests on it). It
is never pushed; its logs are under `local/analysis/gbc/probe/`. Stand-ins:
- `InitializePickupForRBPhysics`: a no-write `Ok`;
- `SpawnPickupParticles`: a no-write `Ok` that first printed the pickup's `CollisionComponent` and, for each element of
  `Components`, its class and the fields this rule reads. Nothing runs between that native and the touch's call, so
  the printed values are the ones the rule would see.

The gate: 1 of 14 links `ok` (link 2). Links 6 and 7 reach `touch` and stop on the unbound
`Actor.GetBoundingCylinder(float,float)`, three pickups in all (link 7 has a decoy), each with the same four components.
Link 9 stopped on "the actor's bounding-cylinder rule" there, because that merge predates IMPL-LINK9-EXTENT. On main
now, link 9 no longer stops there. The other links stopped where that base stopped them.

**After an implementation**, links 6 and 7 would stop on refusal L, at the live pickup's cylinder entry. That was not
run, so it is UNVERIFIED.

## Open questions

1. **Not read:**
   - the component virtual the box routine tests first;
   - which classes hold which of the three implementations, one by one (only counted);
   - how and when the engine computes `Bounds`.
2. **UNVERIFIED:**
   - the skip flag's name (shared with NATIVE_SPAWN_EXTENT.md open question 2);
   - whether any class default has an attached component (NATIVE_SPAWN_EXTENT.md open question 3);
   - the pawn's template condition, read here and not in NATIVE_SPAWN_EXTENT.md (section 2a).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Call `GetBoundingCylinder` on a dropped weapon pickup. Compare with its components' `Bounds`, `bAttached`,
   `CollideActors` and `BulletListener`: expect radius = √(half-x² + half-y²) and height = the half z size of the
   union of the counted boxes.
2. Call it on the player's pawn and on an enemy. Expect the same box rule, not the `CylinderComponent`'s values.
3. Call it on a pawn's class default object and on a navigation point. Expect the cylinder's `CollisionRadius` and
   `CollisionHeight`.
4. Call it on an actor with no colliding attached component. Expect 0 and 0.
