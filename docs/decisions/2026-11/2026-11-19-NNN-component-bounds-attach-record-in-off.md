## 2026-11-19: An attach record in Off, and GetBoundingCylinder's live path up to refusal S

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-COMPONENT-BOUNDS, coded from `docs/verification/NATIVE_COMPONENT_BOUNDS.md`
alone, as of review r1 of #639 (7916634c on this stack; analyst lane ANALYST-COMPONENT-BOUNDS. Clean room: this lane
did not read the natives). The decision number is pending from the hub (three digits); NNN is a placeholder.

- **The attach record** (`ow-world`'s `SceneLayer`, the world's scene layer):
  - it maps each component the port's own attach paths attached to its owner;
  - the spawn's component update (`component_attach::apply`) and `Actor.AttachComponent` (`ow_inventory::attach_mesh`)
    publish an entry only after all their writes succeeded. A refused or failed attach adds nothing, and the
    existing preflights are unchanged;
  - `bAttached` is not written (the #605 ruling);
  - the lookup (`actors::attached_to`) reads only. An entry counts only while it names the asking owner, the
    component's `Owner` still names that owner, and neither object is destroyed. The record lives in this VM's scene
    layer, so it is scoped to this world;
  - lifecycle: the port has no detach path. `actors::destroy` drops the destroyed actor's entries, and an `Owner`
    change made by any other path is caught at the lookup.
- **`Actor.GetBoundingCylinder` on a live receiver** (the actor default), per entry:
  - None and non-primitives are skipped;
  - a lazy handle is refusal N;
  - the component virtual the box routine asks first is taken as passing: **UNVERIFIED**, as #595;
  - `CollideActors` clear is skipped (new: before attachment is asked about);
  - not attached by the record: refusal L;
  - then **refusal S, unconditionally**. This is the hub's ruling on #639: the skip bit is not read, and its fitted
    name stays an UNVERIFIED candidate.
- **Template receivers** keep #595's path unchanged.
- **The bounds cases past S** (note section 7 B.7) are coded in `bounding_cylinder::entry_box`, but unreachable while S
  refuses, and untested, as the note allows:
  - the cylinder's (R, R, H) box around the owner's `Location`, when its `Translation` is zero, no Absolute flag is
    set and its `Owner` is the receiver, else `NotRead` "the component's world transform";
  - a skeletal mesh is `NotRead` at the unread source selector. The bone-transform layer (#599's name) belongs only to
    the physics-asset branch, once a reading picks the branch;
  - a static mesh is `LayerAbsent` "a static mesh's stored bounds";
  - any other primitive class is `NotRead`.
- **Reference** is unchanged. No dependency was added and no bounds check was loosened.
- **Tests:**
  - the note's r1 list: S on a recorded colliding cylinder, with the fitted bit set or clear; L for an unrecorded
    colliding entry; non-colliding entries skipped, giving 0 and 0; the first reached entry decides, with a later lazy
    entry never read; stale or foreign records (another owner, an `Owner` change, a destroyed component, the owner's
    entries dropped, no scene layer) counting as L; `bAttached` left as it was;
  - the attach paths: a successful attach records, and a refused or failed one records nothing;
  - #595's tests, one adjusted: a live non-colliding cylinder is now skipped rather than L;
  - every refusal checks #518's three counters and the name table's length.
- **Probe:** installed Off `--slice-gate` strict. Links 6 and 7 pass `make` (1 of 3, and 1 of 4) and stop in `touch` at
  refusal S, marked by the pickup driver. Before, they stopped at L.
- **Verified:** nothing in the running game. Every rule is UNVERIFIED.
- **Review r1 of #643 (Sol):**
  1. **A stale record could revive.** Change `Owner` from A to B and back to A, with no new attach, and the old
     entry still matched. The heap now counts, per object, the writes to a watched property (`Heap::watch` and
     `Heap::watched_writes` in `ow-object`; opt-in; not a write; constant `Debug`). Publishing an entry starts
     watching `Owner` and stores the component's count. The lookup requires the count to be unchanged, so any later
     `Owner` write, even one that restores the owner, makes the entry stale. Script cannot assign the const `Owner`,
     so in the port only natives write it; a test covers A to B to A.
  2. **`SpawnPickupParticles` did not publish.** Its part-1 attach now publishes the runtime copy, after every
     pickup-particle write. `ow-inventory` already depends on `ow-world`, so no new crate edge.
  3. **A borrow could fail after writes.** `record_ready` (the scene layer not borrowed) is now in the preflights:
     the spawn's plan when a component is to be attached, `Actor.AttachComponent`'s plan (and with it the particle
     attach) and `Actor.Destroy`'s. An empty spawn plan publishes nothing, and the lookup uses `try_borrow`.
  4. **Tests:**
     - `Actor.Destroy` drops the actor's entries as owner and as component, through the real destroy path;
     - the refusals of both attach paths keep the whole record (its `Debug` text), #518's three counters and the
       name table, with an earlier record present;
     - the particle copy is recorded after a successful call;
     - a heap test of the watch counts.
- **Probe after r1:** the slice gate is unchanged (4 ok / 9 NotYet / 1 error; links 6 and 7 at S).
