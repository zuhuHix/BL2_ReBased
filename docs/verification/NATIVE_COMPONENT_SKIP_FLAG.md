# The component skip flag and the check before it (2026-11-20)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SKIP-FLAG (decision 2026-11-19-NNN, number pending), the analyst
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

  The declaration order holds at both ends of bit 10, and the bools of a native class's script word are the C++
  members in the same order. So bit 10 is the declared bool between `bBlockFootPlacement` and
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

**So bit 10 / `BulletListener` is confirmed as far as packages and code can confirm it, not rejected.** This agrees
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

**So for the cylinder family the check always passes**, and treating it as passing is no longer an assumption
there. For every other primitive class it is still unread.

## 4. The box routine, in order (for the record)

Per entry of the actor's `Components`, in order:
1. the entry is not None;
2. the per-component virtual answers true (section 3);
3. the entry is a `PrimitiveComponent`;
4. its `bAttached` is set. This is bit 0 of `ActorComponent`'s first bool word; that class's size matches its native
   registration, so the field is not shifted;
5. either the routine's argument says "non-colliding too", or `CollideActors` (bit 2) is set. `GetComponentsBoundingBox`
   asks for colliding components only. NATIVE_GET_BOUNDING_CYLINDER.md (#590) read the bounding cylinder's call as
   colliding only too, so `CollideActors` is required there (not re-read by this lane);
6. bit 10 (`BulletListener`) is clear;
7. then its `Bounds` box (`Origin` ± `BoxExtent`, read at that field's place) joins the union.

This agrees with NATIVE_COMPONENT_BOUNDS.md section 4's order. It adds that step 2 is a virtual of the component,
asked before the primitive cast, and that step 5 depends on the routine's argument.

## 5. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces #643's refusal S on the live path and #595's on the template path,
and changes nothing else (`ow-world` `bounding_cylinder`). Reference is unchanged. New refusals carry this note as
their record: `docs/verification/NATIVE_COMPONENT_SKIP_FLAG.md`.

**Whether to lift S is the hub's call** (its ruling on #639 kept S while the bit was a candidate). With this note:
1. **The skip check** (both paths, where S stands today): read `BulletListener` from the entry, a bool, else a
   `VmError::Script` naming it.
   - set: the entry is skipped, with no refusal;
   - clear: the entry goes on to the bounds step (NATIVE_COMPONENT_BOUNDS.md B.7 on a live receiver, #595's on a
     template).
2. **The per-component check:** for an entry whose exact class is `Engine.PrimitiveComponent`,
   `Engine.CylinderComponent` or `WillowGame.BulletListenerComponent`, it passes (read). For any other primitive class,
   two choices for the hub:
   - keep #595's UNVERIFIED pass-through, as today; or
   - refuse `NotYet { NotRead }`, "Actor.GetBoundingCylinder: the component check of <class> (not read)", before
     `CollideActors` is asked.

   Today's links reach it first with a `CylinderComponent`.
3. **Order:** unchanged from #643: the per-component check, `CollideActors`, the record (L), then the skip check.
4. **Trace filters:** `ow_world::trace_filter` already reads the bit as `BulletListener` (NATIVE_SPAWN_TRACE_FILTER.md).
   This note gives that reading the same higher confidence. No change is asked there.

**What links 6 and 7 would meet:** their first counted entry is the pickup's `CylinderComponent`. If its
`BulletListener` is clear, it passes the skip check and reaches the bounds step. That is the cylinder's box (B.7,
the owner's `Location`, R 64 and H 20, as NATIVE_COMPONENT_BOUNDS.md section 6 measured), unless the component's
world-transform conditions refuse. The next entry, the skeletal mesh, then meets B.7's skeletal source selector
(`NotRead`). This is a prediction from the recorded probe values, not a run (the probe saw the bit clear on all three
entries).

**Tests for the implementer** (synthetic, `package()` / `tag()`). Each refusal leaves #518's three counters and the
name table unchanged:
1. an entry with `BulletListener` set is skipped, on a live and on a template receiver, with no refusal;
2. an entry with it clear goes on to the bounds step;
3. `BulletListener` declared as another kind is a `VmError::Script` naming it;
4. the per-component check: a `CylinderComponent` and a `BulletListenerComponent` pass. Another primitive class
   (a toy one) follows the hub's choice in item 2;
5. order: an unrecorded colliding entry is still L before the skip check is read;
6. Reference: unchanged.

## Open questions

1. **Not read:**
   - the per-component virtual of `SkeletalMeshComponent`, `StaticMeshComponent`, `ParticleSystemComponent` and the
     other primitive classes;
   - who sets `BulletListener` on an instance (no script writes it, the class default does not, and no native writes
     the bit directly in the forms searched). Perhaps a projectile definition's component template, or a native
     setter through a register;
   - the bounding cylinder's own call argument, which this lane takes from #590.
2. **UNVERIFIED:** the bit's name (a structural fit plus fitting behaviour, no name in the executable); the
   folded-body readings of section 3.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On a live projectile, read its `BulletListenerComponent`'s `BulletListener`: expect it set.
2. On a dropped pickup, read each component's `BulletListener`: expect clear. Then call `GetBoundingCylinder` and
   compare with the cylinder's √2 × R and H.
3. Set `BulletListener` on a pickup's cylinder and call `GetBoundingCylinder` again: expect that component left out.
