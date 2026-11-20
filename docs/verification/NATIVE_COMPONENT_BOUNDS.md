# A primitive component's `Bounds`: how they are computed, and what links 6 and 7 need

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-COMPONENT-BOUNDS (decision 2026-11-19-NNN), the analyst half of the
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
  - Which source applies depends on flags of the component and of its mesh.
  - Both sources need data the port does not hold: the physics asset's bodies, the bone transforms (the "space
    bases"), or the mesh's own stored bounds.

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
the same fit's name for bit 10, `BulletListener`. That is the skip flag the box routine tests, which
NATIVE_GET_BOUNDING_CYLINDER.md left at main's `SKIP_FLAG` refusal. `trace_filter::SKIP_FLAG_FIELD` already reads the
bit as `BulletListener`.

## 4. What `GetBoundingCylinder` reads from them (question b)

From NATIVE_GET_BOUNDING_CYLINDER.md, unchanged by this lane. The actor default (a `WillowPickup`'s) takes the union
of the counted components' `Bounds` boxes, each box being `Origin` ± `BoxExtent`. A component counts when it is a
primitive, attached, `CollideActors`, and without the skip flag. The answer is:
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

| Entry | `CollideActors` | `bAttached` | skip flag (`BulletListener`) | Notes |
|---|---|---|---|---|
| `CylinderComponent` | set | clear | clear | radius 64, height 20, `Owner` set |
| `SkeletalMeshComponent` | **set** | clear | clear | `Owner` set |
| `ParticleSystemComponent` | clear | clear | clear | `Owner` set |

`CollideActors` on the mesh is now **set**, where #590's earlier probe saw it clear (before #520 and #609 landed). So
if both components were attached, the game's counted set would be **the cylinder and the skeletal mesh**, and the
mesh's bounds would decide the union.

**The next stop past the stand-in.** Both links then stop, still in `touch`, at
`DroppedPickup.TouchPickupTrace` ("what blocks the touch trace inside the native"). That is the port's Off stub,
NATIVE_PICKUP.md section 3, which NATIVE_TOUCH_PICKUP_TRACE.md (#624, open) reads. It needs a collision service the
equip world does not install. So **the stand-in moves both links to an already-owned stop.**

## 7. The proposed Off answer (for a separate implementer)

**What the minimal computed case buys.** It is the `CylinderComponent`'s box, and it is not enough on its own for links
6 and 7. Their skeletal mesh is a counted entry once attachment is known, and its bounds need the bone transforms or the
physics asset: a layer, the same bone-transform layer #599's `SetSkeletalMesh` refuses on for link 8. The steps below
still help: they replace "attachment unknown" with an exact refusal on the entry that needs the missing layer.

**A. Attachment, recorded, not written.**
- **The record.** Add an Off service in `ow-world`, an attach record: the set of components the port's own attach
  paths attached, with the scene layer present. The spawn's component update (#610) and `Actor.AttachComponent`
  (#520) add each component they attach.
- **`bAttached` is not written.** The #605 ruling keeps it as it was, and the encroachment check and the deferred
  reattach read it. So the record changes no other rule.
- **A component the port did not attach through those paths is "attachment unknown".** That covers a component of a
  loaded or placed actor.

**B. `GetBoundingCylinder` on a live receiver** (the actor default, #595's step 5). For each `Components` entry, in
order, write-free, load-free and RNG-free:
1. None, and a non-primitive: skipped (as now);
2. a lazy handle: refusal N (as now);
3. **`CollideActors` clear: skipped.** This is new on a live receiver. The game's test needs attached **and**
   colliding, so a non-colliding entry is never counted, attached or not. Asking `CollideActors` first changes no
   answer, only which refusal comes first;
4. **not in the attach record:** refusal L, with #595's text. Attachment is unknown, as now;
5. **the skip flag:** read as `BulletListener`, set means skipped. The flag-word fit in section 3 supports it, and the
   trace filter already reads the bit that way (UNVERIFIED). **For the hub:** this replaces refusal S on this path.
   Keeping refusal S here is the other option, and then nothing is counted on a live receiver;
6. **the entry's bounds, by its exact class:**
   - `Engine.CylinderComponent`: the box of section 3, with `BoxExtent` (R, R, H). Its origin is the owner's
     `Location`, **only when** the component's `Translation` is zero, none of the three Absolute flags is set, and the
     owner is the receiver. Otherwise `NotYet { NotRead }`, "the component's world transform". `CollisionRadius` and
     `CollisionHeight` must be floats, else `VmError::Script`;
   - `Engine.SkeletalMeshComponent`: `NotYet { LayerAbsent }`, "Actor.GetBoundingCylinder: a skeletal mesh
     component's bounds", layer "a skeletal mesh component's bone transforms". The layer name is `SetSkeletalMesh`'s,
     so both go to one implementer;
   - `Engine.StaticMeshComponent`: `NotYet { LayerAbsent }`, "Actor.GetBoundingCylinder: a static mesh component's
     bounds", layer "a static mesh's stored bounds". The mesh asset's bounds are not a script property the port reads;
   - any other primitive class: `NotYet { NotRead }`, "the bounds update of <class>". Its override was not read, and
     the base rule's unbounded box would be wrong for any class that overrides it.
7. With every entry skipped or answered: the union through `placement::box_cylinder` (#595's), and no entry gives
   0 and 0.

Nothing is written in B. The two out arguments are written at the end, as now. Nothing is stored in `Bounds` either:
the box is computed at the query from stored fields. Storing it would need the transform update, which the port does
not have.

**C. Template receivers** (`Origin::Default`) keep #595's path unchanged: `bAttached` read from the entry, then
`CollideActors`, then refusal S. Link 9's spawn extent is untouched.

**What links 6 and 7 get.** The spawn's component update attaches the pickup's components into the record, so:
- the cylinder entry passes step 6 with a box (R 64, H 20, the owner's `Location`);
- the skeletal mesh entry is the next counted entry. It refuses `LayerAbsent` ("a skeletal mesh component's bone
  transforms"), marked "stopped after partial writes: " by the driver;
- the particle component is skipped at step 3 and never reached.

So the links' stop moves from "attachment unknown" to the bone-transform layer. That is a move in kind, not in steps:
both still pass `make` and stop in `touch`. Only when that layer exists does the touch reach `TouchPickupTrace`
(section 6).

**What stays LayerAbsent:** skeletal mesh bounds and static mesh bounds. The port keeps no `Bounds` storage, no
transform update, and no bounds update on attach.

**Reference** is unchanged and binds nothing.

**Tests for the implementer** (synthetic, built with the existing `package()` / `tag()` helpers; every value invented).
Every refusal case checks #518's counters (heap writes, draws, load transitions) and the name table's size unchanged.
1. A live actor whose one entry is a recorded, colliding cylinder: radius √2 × R and half height H, whatever its stored
   `Bounds`.
2. Two recorded colliding cylinders at the owner's location: the union. For two equal cylinders that is the same as
   one.
3. A recorded cylinder with a non-zero `Translation`, or with an Absolute flag set: `NotRead` "the component's world
   transform".
4. A colliding entry not in the attach record: refusal L, unchanged text and layer.
5. A non-colliding entry, recorded or not: skipped, and no refusal for it.
6. The skip flag set on a recorded colliding cylinder: skipped. Or refusal S, if the hub keeps it.
7. A recorded colliding skeletal mesh entry: `LayerAbsent` with the bone-transform layer. A static mesh entry:
   `LayerAbsent` with the static-mesh layer. Any other primitive class: `NotRead` naming the class.
8. Order: a recorded cylinder before an unrecorded colliding mesh answers L (the mesh), after the cylinder passed.
9. The spawn's component update and `Actor.AttachComponent` add their components to the record, and `bAttached` is
   left as it was.
10. A template receiver: #595's existing tests unchanged.
11. Reference: nothing bound.

## 8. Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On a dropped weapon pickup, read each component's `Bounds`, `bAttached`, `CollideActors`, `BulletListener`,
   `CollisionRadius` and `CollisionHeight`. Expect:
   - the cylinder's `Bounds` = its world position ± (R, R, H), with sphere radius √(2R² + H²);
   - `GetBoundingCylinder` = √(hx² + hy²) and hz of the union of the counted boxes.
2. Call `SetCylinderSize` on an unowned cylinder component. Expect its `Bounds` extent to follow at once.
3. On a static mesh actor, compare `Bounds` with the mesh's bounds transformed by `LocalToWorld`, plus 1, times
   `BoundsScale`.
4. On a skeletal mesh with a physics asset, compare `Bounds` with the union of its bodies' boxes at the current pose.

## Open questions

1. **Not read:**
   - the deferred reattach's scheduling for an owned component;
   - whether every transform update is followed by a bounds update;
   - the two early-return flags and the source choice of the skeletal bounds;
   - the static mesh's second box;
   - which other primitive classes override the bounds update.
2. **UNVERIFIED:**
   - the skip flag's name (section 3: corroborated, not proven);
   - the world position of a component with zero `Translation` (the matrix product read, not run);
   - the sine table's bit parity.
