# A primitive component's `Bounds`: how they are computed, and what links 6 and 7 need

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-COMPONENT-BOUNDS (decision 2026-11-19-246), the analyst half of the
clean room: another lane implements this note. This lane read the natives in `Borderlands2.exe` locally, with Ghidra
12.1.4 read-only through `tools/ghidra/run.ps1` and a local copy of the table-read helper. The script was read with
`research/script_disasm.py`. Raw output stays under ignored `local/analysis/component_bounds/`. No bl-sdk session was
granted, and no recorded bl-sdk capture holds a component's `Bounds`, so **every rule here is UNVERIFIED in the
running game.** Section 8 gives the live checks that would confirm it.

**Why now.** On main (df0970ec), slice links 6 and 7 pass `make` and stop in `touch`:

> stopped after partial writes: Actor.GetBoundingCylinder: the colliding components' bounds: needs component bounds
> (the headless world has none)

That is refusal L of NATIVE_GET_BOUNDING_CYLINDER.md (#590, ported by #595), at the live pickup's first primitive
entry.

## 1. Where the call sits (script)

The pickup is a `WillowPickup`, so it uses `DroppedPickup`'s pickup state. The chain the touch runs is:
1. the state's `Touch(Other)` casts `Other` to a pawn and, for a pawn that is not a vehicle, asks `ValidTouch(Pawn)`;
2. `ValidTouch` first checks the pawn (`bCanPickupInventory`, no driven vehicle, and so on) and the pickup's falling
   state. It then asks `TouchPickupTrace(Pawn.Location, GetTouchTraceLocation())`, and when that answers false it
   sets a 0.5 second `RecheckValidTouch` timer and returns false;
3. `GetTouchTraceLocation()` copies the pickup's `Location`, calls `GetBoundingCylinder(Radius, Height)`, adds
   `Height × 0.5` to the copy's Z, and returns it.

So only the **half height** of the answer is used here, and it moves the trace's end point up.

## 2. The "after partial writes" mark (question c)

- **Nothing on the touch path writes before `GetBoundingCylinder`.** `Touch`, `ValidTouch` and
  `GetTouchTraceLocation` assign only locals and the return value before the call (the pawn cast, the controller, the
  trace location copy). None of them writes an object property, and nothing is loaded or drawn.
- **The mark is the pickup driver's** (`ow-cli` `slice_pickup`, the hub's rule from reviews r2 and r3 of #452). The
  run's first write is the gun's creation in `make`. From then on every `NotYet` is marked "stopped after partial
  writes: ", with its kind and record kept. `touch` runs after `make` in the same world, so its first refusal carries
  the mark whatever the touch itself did.
- **A refusal cannot move before it in any useful way.** `make` must write before `touch` can run, because the pickup
  it touches is made there. Inside `touch`, the refusal already comes before any write of the touch path itself. So
  the mark is honest as it stands: it says the world was written by `make`, not by the touch. Nothing needs to change
  for this.

## 3. How a primitive component gets its `Bounds` (question a)

`Bounds` is a `BoxSphereBounds`: `Origin`, `BoxExtent` (half sizes) and `SphereRadius`. The script never sets it. The
engine computes it in a per-class virtual, the **bounds update**.

**When it runs.** These are the paths read; the deferred reattach's own scheduling was not followed further:
1. **When a primitive component is attached.** The attach does these steps, in this order:
   - the base component attach sets `bAttached` and registers the component with its owner;
   - then the **transform update** computes `LocalToWorld`;
   - then the **bounds update**;
   - then scene, collision and light registration (not read for this note).
2. **When the component is reattached.** `CylinderComponent.SetCylinderSize(Radius, Height)` writes the two fields and
   then asks for a **deferred reattach**:
   - it sets `bNeedsReattach`;
   - a component with no owner is detached and attached again at once, so its bounds are recomputed;
   - a component with an owner leaves the reattach to the owner's component update.

   The same deferred reattach is reached from many other setters (for example `SetActorCollision`, `SetForceRefPose`
   and the scene-capture parameter setters).
3. **When the transform is updated.** A moved owner updates its components' transforms. Whether every transform
   update is followed by a bounds update was not read separately (UNVERIFIED).

**The bounds update, per class.**
- **`PrimitiveComponent`** (the base rule, for every class that does not override it): `Origin` = 0,
  `BoxExtent` = (262144, 262144, 262144), and `SphereRadius` = √3 × 262144. That is an "unbounded" box the size of the
  world's half extent.
- **`CylinderComponent`:**
  - `Origin` = the translation of the component's `LocalToWorld` (its world position);
  - `BoxExtent` = (`CollisionRadius`, `CollisionRadius`, `CollisionHeight`);
  - `SphereRadius` = √(R² + R² + H²).

  No scale or rotation enters the extent, and nothing is added. It is an axis-aligned box around the cylinder's world
  position, whatever the component's rotation or scale.
- **`StaticMeshComponent`:**
  - **With no static mesh:** the base rule.
  - **Otherwise, the mesh's own stored bounds transformed by `LocalToWorld`:**
    - the `Origin` through the full matrix;
    - each axis of `BoxExtent` as the largest value of that axis over the eight transformed corners of the mesh's
      box, measured from the transformed origin. That is the sum of the absolute values of the matrix's axis rows,
      weighted by the mesh's extent;
    - `SphereRadius` as the mesh's radius × the length of the longest axis row.
  - **When the mesh also holds a second box** (a part of the mesh asset not named here), the result is merged with
    that box, transformed the same way: the union box, and a sphere radius that covers both.
  - **Then 1 is added** to each of the three extents and to the radius, and all four are multiplied by
    `BoundsScale`.
  - When the bounds changed by 0.0001 or more and the component is in the game world, the scene is told (a render
    update).
- **`SkeletalMeshComponent`** (read at the level of its branches only):
  - **Two flags make it return at once**, keeping the previous bounds. Neither flag is named here.
  - **With no mesh:** the base rule.
  - **Otherwise there are two sources:**
    - the **physics asset**: each body's box is put through that body's bone's **current transform** and merged;
    - otherwise the mesh's own stored bounds, transformed as for a static mesh.
  - The scale used includes `Scale`, `Scale3D` and the owner's draw scale.
  - Which source applies depends on flags of the component and of its mesh. That **source selector** (with the two
    early returns) was not read in detail.
  - Only the physics-asset branch uses the **current bone transforms** (the "space bases"). The mesh-bounds branch
    needs the mesh's own stored bounds instead, and the physics-asset branch also needs the asset's bodies. So the
    bone-transform layer answers only one of the two branches (section 7, B.7).

**The transform update** builds `LocalToWorld` from the component's `Translation`, `Rotation` and `Scale` ×
`Scale3D`, and the parent-to-world matrix (the owner's, for a component attached to an actor):
- `AbsoluteTranslation` drops the parent's translation;
- `AbsoluteRotation` and `AbsoluteScale` normalise the parent's axes;
- scale factors with magnitude below 0.0001 are raised to 0.0001;
- the rotation uses the engine's sine table, so a bit-exact port needs that table (UNVERIFIED).

For a component with zero `Translation`, no Absolute flag, and an owner whose transform is its `Location`, `Rotation`
and draw scale, the world position is the owner's `Location`. That is a reading of the matrix product, UNVERIFIED.

**The flag word.** The transform update tests bits 24 to 26 of the primitive flag word as the three Absolute flags,
and the attach tests bit 2 for its collision path. Both match `tools/ghidra/class_layout.py`'s names for that word
(`AbsoluteTranslation`, `AbsoluteRotation`, `AbsoluteScale`, `CollideActors`). This **supports**, but does not prove,
the same fit's name for bit 10. **Bit 10, `BulletListener`, by the layout tool's fit, is an UNVERIFIED candidate
only.** That is the skip flag the box routine tests, which NATIVE_GET_BOUNDING_CYLINDER.md left at main's `SKIP_FLAG`
refusal. The hub's ruling on #639 keeps that refusal: this note's answer does not read the bit (section 7).

## 4. What `GetBoundingCylinder` reads from them (question b)

From NATIVE_GET_BOUNDING_CYLINDER.md, unchanged by this lane. The actor default (a `WillowPickup`'s) takes the union
of the counted components' `Bounds` boxes, each box being `Origin` ± `BoxExtent`. A component counts when, in the
box routine's order:
1. it is not None and **passes a component virtual the routine asks first**, whose behaviour is still unread. The port
   treats every component as passing it (#595), an **UNVERIFIED** assumption;
2. it is a primitive;
3. it is attached;
4. its `CollideActors` is set;
5. its skip flag is clear.

The answer is:
- radius = √(half-x² + half-y²) of the union;
- height = its half z size.

`SphereRadius` is not read. So:
- **with one counted component the origin does not matter.** A lone `CylinderComponent` answers radius √2 × R and
  half height H. For links 6 and 7 that is radius 90.51 and height 20 (R 64, H 20; section 6);
- **with more than one, the origins matter**, because they place the boxes against each other.

## 5. What the port holds today, and why refusal L fires at `bAttached`

- The port computes no `LocalToWorld` and no `Bounds`: every component's `Bounds` is its zero default.
- **The port does not mark a primitive attached.** These attach paths leave `bAttached` as it was:
  - the spawn's component update (#605 and #610: for primitives it sets `Owner` and the two transform flags);
  - `Actor.AttachComponent` (#520).

  In the game, the attach sets it. That is why #595 refuses L on a live receiver at the first primitive entry,
  **before `Bounds`**: its `bAttached` does not mean what the game's does.
- So the bounds layer is **two missing pieces**: whether a live component is attached, and its `Bounds`.

## 6. The probe

The probe was a throwaway local branch on main df0970ec, never pushed and since deleted. Its measurement-only stub in
`GetBoundingCylinder`'s actor default printed each live entry. In place of refusal L, it answered with the first
colliding `CylinderComponent`'s box. Installed Off strict `--slice-gate`, under the machine lock:
4 ok / 10 NotYet / 0 errors.

**The pickup's entries at the call** (each live pickup of links 6 and 7, the same values):

| Entry | `CollideActors` | `bAttached` | fitted bit 10 (candidate `BulletListener`, UNVERIFIED) | Notes |
|---|---|---|---|---|
| `CylinderComponent` | set | clear | clear | radius 64, height 20, `Owner` set |
| `SkeletalMeshComponent` | **set** | clear | clear | `Owner` set |
| `ParticleSystemComponent` | clear | clear | clear | `Owner` set |

`CollideActors` on the mesh is now **set**, where #590's earlier probe saw it clear (before #520 and #609 landed). So
if both components were attached, **and both passed the unread component virtual and the unresolved skip check**
(section 4), the game's counted set would be **the cylinder and the skeletal mesh**, and the mesh's bounds would decide
the union. Neither condition is settled.

**The next stop past the stand-in.** Both links then stop, still in `touch`, at
`DroppedPickup.TouchPickupTrace` ("what blocks the touch trace inside the native"). That is the port's Off stub,
NATIVE_PICKUP.md section 3, which NATIVE_TOUCH_PICKUP_TRACE.md (#624, open) reads. It needs a collision service the
equip world does not install. So the stand-in moves both links to an already-owned stop. That is **probe evidence
only**: the proposed answer (section 7) stops at refusal S before it.

## 7. The proposed Off answer (for a separate implementer)

**The hub's ruling on the skip flag (review r1 of #639): refusal S stays.** The skip flag is not read on any path. "Bit
10, `BulletListener`, by the layout tool's fit" is recorded only as an **UNVERIFIED candidate** (section 3). So on a
live receiver, the first entry that reaches the skip check refuses S, and no live receiver's bounds are counted yet.
The bounds steps below (B.7) are future cases. They are written down so the implementer knows their shape, but S
blocks them today.

**A. Attachment, recorded, not written.**
- **The record.** An Off service in `ow-world`, the attach record: the components the port's own attach paths attached
  while the scene layer is present. Each entry holds the component, its owner actor and the world it was attached in.
- **Publish after success only.** The spawn's component update (#610) and `Actor.AttachComponent` (#520) each add
  their component **after** their own attach succeeded. An attach that refuses or fails adds nothing, and an earlier
  record of that component is left as it was.
- **Invalidate on the supported lifecycle paths.** The record of a component is removed when:
  - the port detaches it (`Actor.DetachComponent`, or the detach half of a reattach the port models);
  - the component or its owner is destroyed (the heap's destroy, `Actor.Destroy`);
  - its `Owner` changes to another actor;

  whichever the port supports. Any other path leaves a record that might be stale, so the lookup checks it again (next
  item).
- **The lookup is scoped.** A component counts as attached only when its record names the **current** owner (the
  receiver whose `Components` holds it, and its own `Owner`) and the **current** world, and the component is not
  destroyed. Anything else is "attachment unknown".
- **The write boundary.** The record is one more write of the attach paths, so it is published **after** their last
  existing write. Any check the record needs (the component resident, its `Owner`, the scene layer) belongs to those
  paths' **existing preflights**. Those preflights end before the first write, and they use resident, lookup-only
  reads (`ObjectSpace::prop`, never a reader that loads or interns), with no RNG draw. No new refusal comes after a
  write.
- **`bAttached` is not written.** The #605 ruling keeps it as it was, and the encroachment check and the deferred
  reattach read it. So the record changes no other rule.
- **A component the port did not attach through those paths is "attachment unknown".** That covers a component of a
  loaded or placed actor.

**B. `GetBoundingCylinder` on a live receiver** (the actor default, #595's step 5). For each `Components` entry, in
order, write-free, load-free and RNG-free:
1. None, and a non-primitive: skipped (as now);
2. a lazy handle: refusal N (as now);
3. **the component virtual the box routine asks first** (section 4): its behaviour is unread. The port treats every
   component as passing it, as #595 already does. That pass-through is **UNVERIFIED**, and a later reading of the
   virtual may turn it into a refusal at this step;
4. **`CollideActors` clear: skipped.** This is new on a live receiver. The game's test needs attached **and** colliding,
   so a non-colliding entry is never counted, attached or not. Asking `CollideActors` first changes no answer, only
   which refusal comes first;
5. **attachment:** not attached by the record's scoped lookup (A): refusal L, with #595's text. Attachment is unknown,
   as now;
6. **the skip flag: refusal S**, always. That is `NotYet { NotRead }` with main's `trace_filter::SKIP_FLAG` text and
   #595's record, raised whenever an entry reaches this step. The flag is not read (the hub's ruling);
7. **future cases, unreachable while step 6 refuses.** The entry's bounds, by its exact class:
   - `Engine.CylinderComponent`: the box of section 3, with `BoxExtent` (R, R, H). Its origin is the owner's
     `Location`, **only when** the component's `Translation` is zero, none of the three Absolute flags is set, and the
     owner is the receiver. Otherwise `NotYet { NotRead }`, "the component's world transform". `CollisionRadius` and
     `CollisionHeight` must be floats, else `VmError::Script`;
   - `Engine.SkeletalMeshComponent`, in this order:
     - the skeletal bounds' **source selector** (section 3: the two early returns, and the choice between the physics
       asset and the mesh's bounds) is unread: `NotYet { NotRead }`, "Actor.GetBoundingCylinder: a skeletal mesh
       component's bounds source";
     - once a later reading settles the branch: on the **physics-asset branch**, which uses the current bone
       transforms, `NotYet { LayerAbsent }` with the layer "a skeletal mesh component's bone transforms" (the layer
       `SetSkeletalMesh` names, so both go to one implementer);
     - on the **mesh-bounds branch**, which needs no bone transforms, `LayerAbsent` with the layer "a skeletal mesh's
       stored bounds";
     - missing physics-asset bodies are their own `LayerAbsent`, "a physics asset's bodies".

     The bone-transform layer solves only the first of these three;
   - `Engine.StaticMeshComponent`: `NotYet { LayerAbsent }`, "Actor.GetBoundingCylinder: a static mesh component's
     bounds", layer "a static mesh's stored bounds". The mesh asset's bounds are not a script property the port reads;
   - any other primitive class: `NotYet { NotRead }`, "the bounds update of <class>". Its override was not read, and
     the base rule's unbounded box would be wrong for any class that overrides it;
8. with every entry skipped (and, in the future, answered): the union through `placement::box_cylinder` (#595's). No
   entry gives 0 and 0.

Nothing is written in B. The two out arguments are written at the end, as now. Nothing is stored in `Bounds` either:
when B.7 is reachable, the box is computed at the query from stored fields. Storing it would need the transform update,
which the port does not have.

**C. Template receivers** (`Origin::Default`) keep #595's path unchanged: `bAttached` read from the entry, then
`CollideActors`, then refusal S. Link 9's spawn extent is untouched.

**What links 6 and 7 get.** The spawn's component update attaches the pickup's components into the record. Then, on
the live pickup:
- the `CylinderComponent` is the first entry: it passes step 3 (the UNVERIFIED pass-through), is colliding (step 4) and
  attached by the record (step 5);
- so it reaches the skip check and **refuses S** (`NotRead`, main's `SKIP_FLAG` text), marked "stopped after partial
  writes: " by the pickup driver (section 2).

Both links still pass `make` and stop in `touch` (1 of 3, and 1 of 4). The stop moves from refusal L to refusal S. The
skeletal mesh entry is not reached. The probe's later stop at `TouchPickupTrace` (section 6) came from a stand-in past
L and is **probe evidence only**: no proposed step answers past S.

**What stays refused:** the skip flag (S), every live entry's bounds (B.7's future cases), and with them the skeletal
mesh's source selector and layers and the static mesh's layer. The port keeps no `Bounds` storage, no transform update,
and no bounds update on attach.

**Reference** is unchanged and binds nothing.

**Tests for the implementer** (synthetic, built with the existing `package()` / `tag()` helpers; every value invented).
Every refusal case checks #518's counters (heap writes, draws, load transitions) and the name table's size unchanged.
1. A live actor whose one entry is a recorded, colliding cylinder: refusal S, `NotRead` with main's `SKIP_FLAG` text,
   whatever its stored `Bounds` and its fitted skip bit.
2. The same with the fitted bit (`BulletListener`) set and clear: refusal S both times (the flag is not read).
3. A colliding entry not in the attach record: refusal L, unchanged text and layer.
4. A non-colliding entry, recorded or not: skipped, with no refusal for it. A live actor with only non-colliding or
   non-primitive entries answers 0 and 0.
5. Order: a non-colliding recorded entry, then an unrecorded colliding one: L. A recorded colliding entry first: S,
   before any later entry is read.
6. The record's lifecycle:
   - a successful spawn component update and a successful `Actor.AttachComponent` add their component;
   - an attach that refuses (any of its existing preflight refusals) adds nothing and leaves #518's counters and the
     name table unchanged;
   - a supported detach, a destroyed component or owner, and an `Owner` changed to another actor each remove the
     record, so the next query answers L.
7. The scoped lookup: a record naming another owner or another world, or a destroyed component, is "attachment
   unknown": L.
8. `bAttached` is left as it was by every attach path.
9. A template receiver: #595's existing tests unchanged.
10. Reference: nothing bound.

The future cases of B.7 get their tests when S is answered.

## 8. Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On a dropped weapon pickup, read each component's `Bounds`, `bAttached`, `CollideActors`, `BulletListener`,
   `CollisionRadius` and `CollisionHeight`. Expect:
   - the cylinder's `Bounds` = its world position ± (R, R, H), with sphere radius √(2R² + H²);
   - `GetBoundingCylinder` = √(hx² + hy²) and hz of the union of the counted boxes.
2. Call `SetCylinderSize` on an unowned cylinder component. Expect its `Bounds` extent to follow at once.
3. On a static mesh actor, compare `Bounds` with the mesh's bounds transformed by `LocalToWorld`, plus 1, times
   `BoundsScale`.
4. On a skeletal mesh with a physics asset, compare `Bounds` with the union of its bodies' boxes at the current pose.

## 9. Amendment (2026-11-25): where a skeletal mesh component's bounds come from

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SKELMESH-BOUNDS-SOURCE (decision 2026-11-25-28),
analyst only: **another lane implements this section.** Raw output stays under ignored `local/analysis/skelbounds/`.
**Every rule here is UNVERIFIED in the running game.**

**Why.** The implementation of NATIVE_COMPONENT_SKIP_FLAG.md section 6 (#677) moved links 6 and 7. They now stop in
`touch` at "stopped after partial writes: Actor.GetBoundingCylinder: a skeletal mesh component's bounds source: not
read from native code or script yet" (section 7 B.7's skeletal case, record this note).

### 9.1 Evidence, in the order asked

- **bl-sdk live reads:** none. No session was granted, and no recorded capture holds a skeletal component's `Bounds`.
- **The script:** nothing. The bounds update is a C++ virtual with no script face, and no script function writes
  `Bounds`.
- **The native** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - the bounds update's slot was found by its base rule (section 3), and its override was read from
    `SkeletalMeshComponent`'s primary table (found as in NATIVE_COMPONENT_SKIP_FLAG.md section 6);
  - field names: a local copy of `tools/ghidra/class_layout.py` that gives `MapProperty` a stand-in size (the tracked
    tool stops at one in this class). The copy's offsets sit a fixed distance from the code's for this class's own
    fields. That distance is pinned by fields whose use the code shows unambiguously:
    - `SkeletalMesh`, whose target holds the mesh's `Bounds` first;
    - `SpaceBases`' count, compared with the mesh's `RefSkeleton` count;
    - `ParentAnimComponent`, whose target holds a mesh at the same place;
    - `RootBoneTranslation`, added to the mesh's origin.

    Each flag below was named by that rule, so the names are **by fit (medium-high)**, not read from the executable.

### 9.2 The answer: `GetBoundingCylinder` reads the stored `Bounds`

The box routine (sections 4 and NATIVE_COMPONENT_SKIP_FLAG.md section 4) reads each counted component's **stored**
`Bounds` field. It computes nothing at the query. So a skeletal component's box is whatever its **bounds update** last
wrote. That update runs at attach and on transform updates (section 3). The port has no bounds update, so `entry_box`
recomputes at the query what the update would have written, from stored fields (section 7 B). The rules below are that
update.

### 9.3 The skeletal bounds update, in order

1. **Early return: a sleeping rigid body.** When the component's `Owner` has `Physics` = `PHYS_RigidBody`, the
   **component's own** `FramesPhysicsAsleep` is above 5, and a further physics-state test on the owner passes, the
   update returns and keeps the previous `Bounds`. That test goes through an owner object the layout tool does not
   place, and it is **not named here**. The component's `bSkipAllUpdateWhenPhysicsAsleep` plays no part in the bounds
   update: the update does not test it (as read).
2. **Early return: skipped pose update.** With `bSkipUpdateSkelPose` set (by fit), it returns and keeps the previous
   `Bounds`.
3. **Visible or not.** In the game (not the editor), the component counts as visible when **both** of these hold:
   - its `DetailMode` is no higher than the engine's current detail mode (a global setting);
   - it is shown, or `bCastHiddenShadow` is set. "Shown" means `HiddenGame` is clear, and either there is no `Owner`,
     the owner's `bHidden` is clear, or `bIgnoreOwnerHidden` is set.

   The editor uses `HiddenEditor` and the owner's editor-hidden flags instead.
4. **Can it use the bones?** This is true when all of these hold:
   - the combined scale is uniform: `Scale` × `Scale3D`, times the owner's `DrawScale` × `DrawScale3D` when there is an
     owner, with the three axes within 0.0001 of each other;
   - `SkeletalMesh` is set;
   - either `SpaceBases` has as many entries as the mesh's reference skeleton, or a `ParentAnimComponent` is set and
     has a `PhysicsAsset`.

   It is used only by the two physics sources below. With `SpaceBases` empty and a mesh whose reference skeleton has
   at least a root bone, the count half is already decidable: it is false, and only the parent's `PhysicsAsset` can
   make the last condition true.
5. **The source**, the first that applies:
   - **a. Not visible, or `bComponentUseFixedSkelBounds` set:** the **mesh's stored bounds**. The mesh's own `Bounds`
     origin, plus `RootBoneTranslation`, goes through the component's `LocalToWorld`. The extent is, per axis, the
     largest offset over the box's eight transformed corners; that is the sum of the absolute matrix entries weighted
     by the mesh's extent. The sphere radius is the mesh's radius times the longest of the matrix's three axis rows.
     This source does not check that `SkeletalMesh` is set;
   - **b. A `ParentAnimComponent` with a `SkeletalMesh` and its own `bComponentUseFixedSkelBounds` set:** the
     **parent's mesh's stored bounds**, through this component's `LocalToWorld` as in a. Whether the root offset is
     added here was not checked;
   - **c. Its own `PhysicsAsset`, "can use the bones" (step 4), and `bHasValidBodies` set:** the **physics-asset
     bodies' box** under the current bone transforms (the routine section 3 names; not re-read in detail);
   - **d. A `ParentAnimComponent` with a `PhysicsAsset`, and "can use the bones":**
     - with `bUseBoundsFromParentAnimComponent` set, a **copy of the parent's stored `Bounds`**;
     - otherwise the physics-asset box of c, computed for this component;
   - **e. No `SkeletalMesh`:** the **base rule** (section 3: origin 0, extent 262144 on each axis, sphere radius
     √3 × 262144). The update then **returns at once**: no per-bone step, no `BoundsScale`;
   - **f. Otherwise:** the mesh's stored bounds, exactly as in a.
6. **Per-bone collision boxes.** After a source other than e: when the mesh has per-poly collision bones (its
   `PerPolyBoneKDOPs` and `PerPolyCollisionBones`) and `SpaceBases` is not empty, each such bone found in the skeleton
   widens `Bounds` by its collision box under that bone's current transform.
7. **`BoundsScale`:** the box extent and the sphere radius (not the origin) are multiplied by `BoundsScale`. Unlike the
   static mesh rule, **nothing is added**. Then the scene is told.

**Confidence:**
- high for the order and the branch structure (read);
- medium-high for the field names (by fit, 9.1);
- not read: the unnamed physics-state test of step 1, the physics-asset box routine of c and d, and the per-bone step
  in detail.

### 9.4 The "stopped after partial writes" prefix

It is expected. Section 2 still holds on main: nothing on the touch path (`Touch`, `ValidTouch`,
`GetTouchTraceLocation`) writes before `GetBoundingCylinder`. The prefix is the pickup driver's (`ow-cli`
`slice_pickup`, `PARTIAL_WRITES`): it marks every `NotYet` after `make`'s first write, the gun's creation. The bounding
cylinder itself writes nothing before it refuses.

### 9.5 What the slice's pickup component holds

The port's `new` ignores the template (NATIVE_ATTACH_MESH_TO_PICKUP.md section 1). So the pickup's
`SkeletalMeshComponent` holds the class defaults, read from `Engine.Default__SkeletalMeshComponent`:
- `HiddenGame`, `bIgnoreOwnerHidden`, `bCastHiddenShadow`, `bComponentUseFixedSkelBounds`,
  `bUseBoundsFromParentAnimComponent`, `bHasValidBodies`, `bSkipUpdateSkelPose` and `bSkipAllUpdateWhenPhysicsAsleep`
  are clear;
- `DetailMode` 0, `BoundsScale` 1, `Scale` 1, `Scale3D` (1, 1, 1), `RootBoneTranslation` zero;
- no `PhysicsAsset`, no `ParentAnimComponent`, and **no `SkeletalMesh`** (unless a later lane sets one).

**A prediction, not a run.** Such a component is visible (step 3) and has no fixed bounds, no parent and no physics
asset. With no mesh it takes **source e, the base rule**: an "unbounded" box of half extent 262144 around the world
origin. The bounding cylinder's union with the pickup cylinder's box then gives a radius near √2 × 262144 and a half
height near 262144. **That is the game's rule applied to the port's input, not the game's answer.** In the game the
component is made from the weapon's `ThirdPersonMesh` template and so has a mesh, and the source would be f.

### 9.6 The proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces `entry_box`'s `SKELETAL_SOURCE` refusal for exact class
`Engine.SkeletalMeshComponent`. Reads are loaded-only and by name, as `entry_box` already does: nothing is written,
loaded or drawn. A required object that is not resident is the bounding cylinder's existing refusal N. A field of the
wrong kind is a `VmError::Script` naming it. In order:
1. **Step 1.** `Owner` set, its `Physics` = `PHYS_RigidBody`, and the component's `FramesPhysicsAsleep` above 5: a
   `NotYet` of kind `NotRead`, "Actor.GetBoundingCylinder: a sleeping rigid body's kept bounds (the owner's physics test
   is not read)". Otherwise go on. `PHYS_RigidBody` is found by the `EPhysics` enum's order, by a lookup only: a
   missing enum or value name is a `VmError::Script` naming it, never a class-code load;
2. **Step 2.** `bSkipUpdateSkelPose` set: a `NotYet` of kind `NotRead`, "Actor.GetBoundingCylinder: a skeletal mesh
   component's kept bounds (bSkipUpdateSkelPose)". The port stores no previous `Bounds`;
3. **Step 3.** `DetailMode` above 0: a `NotYet` of kind `NotRead`, "Actor.GetBoundingCylinder: the engine's detail
   mode". The global is not in the port, and `DM_Low` (0) is never above it. Otherwise visible = shown or
   `bCastHiddenShadow`, with "shown" as in 9.3 step 3;
4. **Source a** (not visible, or `bComponentUseFixedSkelBounds`):
   - no `SkeletalMesh`: a `NotYet` of kind `NotRead`, "Actor.GetBoundingCylinder: a hidden or fixed-bounds skeletal
     component with no mesh (the game reads it unchecked)";
   - otherwise the mesh's stored bounds, through **the mesh-bounds accessor** below. While it answers none, a `NotYet`
     of kind `LayerAbsent`, "Actor.GetBoundingCylinder: a skeletal mesh component's bounds", with layer "a skeletal
     mesh's stored bounds" (the layer B.7 already names). Once it answers, the next stop is the world transform (step
     8's rule);
5. **Source b:** the parent's mesh's stored bounds, through the same accessor, with the same two outcomes;
6. **Sources c and d.** First decide "can use the bones" (9.3 step 4) from what the port holds:
   - uniform scale, from the component's and the owner's scale fields;
   - `SkeletalMesh` set;
   - the bone count: `SpaceBases`' length (a script array) against the mesh's reference-skeleton bone count from the
     accessor below. With `SpaceBases` empty and at least one bone, the count half is false. While the accessor
     answers none and `SpaceBases` is not empty, the count is undecidable: a `NotYet` of kind `LayerAbsent` with the
     stored-bounds layer above (the same reader holds both).

   Only when "can use the bones" holds:
   - **c**, own `PhysicsAsset` and `bHasValidBodies` set. It needs **two** absent layers: the physics asset's bodies
     ("a physics asset's bodies", section 7 B.7's text, its own constant) and the current bone transforms
     (`BONES_LAYER`). The bodies are reported first, because the transforms apply to them; the refusal names both
     layers in its text. A headless world has no physics bodies, so `bHasValidBodies` stays clear and c is skipped;
   - **d**, a parent with a `PhysicsAsset`: `bUseBoundsFromParentAnimComponent` set is `LayerAbsent` with the bounding
     cylinder's `BOUNDS_LAYER` (the parent's stored `Bounds`). Otherwise it is c's two layers, bodies first;
7. **Source e** (no `SkeletalMesh`): the box origin (0, 0, 0), extent (262144, 262144, 262144), as read. No
   `BoundsScale` is applied and no transform is needed, so this is answerable now;
8. **Source f:** the mesh's stored bounds through the accessor, as in step 4. While it answers none, step 4's
   `LayerAbsent`. Once it answers, the world transform is the next stop. It is answerable only in the simplest case:
   the cylinder rule's conditions (`Translation` zero, none of the three Absolute flags, `Owner` the receiver), plus a
   zero component `Rotation`, a zero owner `Rotation`, and a unit combined scale. `LocalToWorld` is then a pure
   translation to the owner's `Location`, and the box is (`Location` + mesh origin + `RootBoneTranslation`, mesh
   extent), with the extent times `BoundsScale` (no per-bone step while `SpaceBases` is empty). In every other case it
   is `WORLD_TRANSFORM`'s `NotRead`, as the cylinder case uses.
   - **The hub's ruling (lane SCALED-BOUNDS-TRANSFORM, decision 2026-11-26-21):** a **uniform, positive** combined
     scale `s` is answered too, the three axes exactly equal, finite and above 0. The box is then (`Location` + `s` ×
     mesh origin + `s` × `RootBoneTranslation`, mesh extent × `s` × `BoundsScale`), the standard UE3 bounds transform
     with no rotation. **UNVERIFIED**: the port's reading of the engine maths, not read from the native. A non-uniform,
     zero, negative or non-finite scale stays `WORLD_TRANSFORM`'s `NotRead`.

**The mesh-bounds accessor.** Main already reads the data. `ow-assets`' `skeletal_lods::read_lod_count` (#673,
landed) returns the mesh's stored bounds (`bounds_origin`, `bounds_extent`) and its bone count (`bones`), and #693
(open) exposes the reference skeleton. Neither is exposed through the world's `SkeletalMeshes` service. Proposed: a
read-only, load-free accessor on `SkeletonSource`, shaped like `lod_count_in`. It answers the mesh's bounds origin and
extent, and its bone count, for an export of the definition store's in-memory package. It answers `Ok(None)` without
a source, which gives the `LayerAbsent` above. It writes and loads nothing.

**A ruling the hub should make first (9.5).** Implementing step 7 makes links 6 and 7 pass this entry with the
unbounded box, because the port's component lacks its template's mesh. The rule is faithful, but the input is not the
game's, so a later `GetTouchTraceLocation` would raise the trace by about 262144. Two options:
- **A (rule-faithful):** step 7 as written. Label the slice result UNVERIFIED and record the template gap beside it;
- **B (honest about the input), recommended:** keep step 7 for components with no template. When the component was
  made from a template that holds a mesh, answer a `NotYet` of kind `LayerAbsent`, "the component's template (new
  ignores it)". This needs the port to record a `new` component's template. If it cannot, the real fix is
  AttachMeshToPickup's template copy (NATIVE_ATTACH_MESH_TO_PICKUP.md open question 1), before this step matters.

*Read 2026-11-26 in NATIVE_TOUCH_COMPONENT_TEMPLATE.md (decision 2026-11-26-02): the game's `new` takes the template
as a fifth operand, copies its values (transient fields from the class default) and loads nothing.*

### 9.7 Tests for the implementer

Synthetic, with `package()` / `tag()`, on a live, recorded, attached, colliding entry of exact class
`Engine.SkeletalMeshComponent` with `BulletListener` clear. Each refusal leaves #518's three counters and the name
table unchanged, causes no class-code load (where #685 has landed), and carries no prefix from the cylinder itself:
1. class defaults, no mesh: box origin 0, extent 262144 on each axis. With the pickup cylinder's box beside it, the
   union and the cylinder answer follow (`box_cylinder`). `BoundsScale` 2 changes nothing (source e returns first);
2. a toy `SkeletalMesh` object set: source f's `LayerAbsent` (stored bounds);
3. `HiddenGame` set (no owner): not visible → source a. With a mesh it is the `LayerAbsent`; without one, the
   "no mesh" `NotRead`. With `bCastHiddenShadow` also set it is visible again → e or f;
3a. `HiddenGame` set, no mesh, and a `ParentAnimComponent` with a mesh and its `bComponentUseFixedSkelBounds` set:
    still source a's "no mesh" `NotRead`, which pins a before b;
4. an owner with `bHidden` set: not visible, unless `bIgnoreOwnerHidden` is set;
5. `bComponentUseFixedSkelBounds` set while visible: source a;
6. `DetailMode` 1: the detail-mode `NotRead`, before the source;
7. owner `Physics` = `PHYS_RigidBody` with `FramesPhysicsAsleep` 6: the sleeping-body `NotRead`. With 5, or another
   `Physics`, it goes on;
8. `bSkipUpdateSkelPose` set: the kept-bounds `NotRead`;
9. a `PhysicsAsset` with `bHasValidBodies` clear: skipped to e or f. With `bHasValidBodies` set, uniform scale, a
   mesh and a matching bone count: the bodies layer first, naming the bones layer too. With `SpaceBases` empty and a
   one-bone mesh: c is skipped (the count is false);
9a. with a stub accessor answering a mesh's bounds: source f with the simplest transform gives `Location` + origin +
    `RootBoneTranslation` and the extent times `BoundsScale`; a nonzero owner `Rotation` gives `WORLD_TRANSFORM`'s
    `NotRead`;
10. a `ParentAnimComponent` with a mesh and its `bComponentUseFixedSkelBounds` set: source b's `LayerAbsent`. A parent
    with a `PhysicsAsset` and `bUseBoundsFromParentAnimComponent` set: `BOUNDS_LAYER`. A lazy parent: refusal N;
11. a field of the wrong kind (`DetailMode` not a byte, `bHidden` not a bool): `VmError::Script` naming it;
12. a `SkeletalMeshComponent` subclass (a toy): still "the bounds update of <class>" (`entry_box` stays exact-class);
13. Reference: unchanged.

### 9.8 Live checks that would confirm it (bl-sdk; none run, no session granted)

1. On a dropped weapon pickup, read the mesh component's `Bounds`, `SkeletalMesh`, `PhysicsAsset`, `bHasValidBodies`
   and `bComponentUseFixedSkelBounds`. Compare `Bounds` with the mesh's bounds through `LocalToWorld`, times
   `BoundsScale` and with nothing added.
2. Let the pickup come to rest, then move it with a script write to `Location`: while it sleeps, expect `Bounds` to
   keep its old value (step 1).
3. Set `HiddenGame` on a pawn's mesh and read `Bounds` after a transform update: expect the fixed mesh bounds (source
   a), not the physics bodies' box.

## Open questions

1. **Not read:**
   - ~~the component virtual the box routine asks first (section 4)~~: read in NATIVE_COMPONENT_SKIP_FLAG.md section 6;
   - the deferred reattach's scheduling for an owned component;
   - whether every transform update is followed by a bounds update;
   - ~~the two early-return flags and the source choice of the skeletal bounds~~: read on 2026-11-25 (section 9),
     except the owner's physics-state test of its step 1 and the physics-asset box routine;
   - the static mesh's second box;
   - which other primitive classes override the bounds update.
2. **UNVERIFIED:**
   - the skip flag's name (section 3: corroborated, not proven);
   - the world position of a component with zero `Translation` (the matrix product read, not run);
   - the sine table's bit parity.
