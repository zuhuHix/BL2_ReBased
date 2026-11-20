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
