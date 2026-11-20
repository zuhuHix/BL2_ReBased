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

## 5. The Off answer (for a separate implementer)

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
