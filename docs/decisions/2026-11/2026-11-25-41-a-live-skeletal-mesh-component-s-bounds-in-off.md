## 2026-11-25: A live skeletal mesh component's bounds in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SKELMESH-BOUNDS-SOURCE, on integration/2026-11-25-c15 (25425bb9). The
decision number is pending from the hub; NNN is a placeholder. Coded from NATIVE_COMPONENT_BOUNDS.md section 9 alone,
as landed on main 6dad61f (#694, decision 2026-11-25-28; clean room: this lane is not its analyst, and no native code
or analysis output was opened).

- **What was decided:**
  - `Actor.GetBoundingCylinder`'s box for a live, recorded, colliding entry of exact class
    `Engine.SkeletalMeshComponent` follows the note's section 9.6 rule. It is in `ow-world`
    `world::skeletal_bounds::skeletal_box`, which `bounding_cylinder::entry_box` calls in place of its
    `SKELETAL_SOURCE` refusal.
  - The skeletal mesh data layer gets the note's mesh-bounds accessor: a defaulted `SkeletonSource::mesh_bounds_in`
    and `skeletal::mesh_bounds`, asked by the mesh's export origin. `ow-cli`'s source answers it from
    `ow_assets::skeletal_lods`' full read: the stored bounds origin and extent, and the bone count.
- **The rule, in the note's order** (each part UNVERIFIED in game; the field names are the note's by-fit names):
  1. a sleeping rigid-body owner (`PHYS_RigidBody` by enum name, `FramesPhysicsAsleep` above 5): `NotRead`;
  2. `bSkipUpdateSkelPose`: `NotRead`;
  3. `DetailMode` above 0: `NotRead`. Otherwise visibility comes from `HiddenGame`, the owner's `bHidden`,
     `bIgnoreOwnerHidden` and `bCastHiddenShadow`;
  4. source a (hidden or fixed bounds): with no mesh, `NotRead`; with one, the stored box;
  5. source b (a parent's fixed-bounds mesh): the stored-bounds `LayerAbsent` while the accessor answers none. Once
     it answers, a `NotRead`, because the note did not check whether the root offset is added on this source. That
     refusal is this lane's reading of an open point, not a rule the note states;
  6. sources c and d (physics assets), asked only when "can use the bones" holds (a uniform scale within 0.0001, a
     mesh, and the bone count or a parent's asset). Each is a `LayerAbsent`: the bodies ("a physics asset's bodies",
     its text naming the bone-transform layer), or the parent's stored `Bounds` (the bounding cylinder's
     `BOUNDS_LAYER`);
  7. source e (no mesh): the base rule, origin 0 and half extent 262144, answered;
  8. source f: the stored box.

  The stored box is the stored-bounds `LayerAbsent` while the accessor answers none. Once it answers, only the
  simplest world transform is answered: zero `Translation`, no Absolute flag, the receiver as `Owner`, zero component
  and owner `Rotation`, and a combined scale of exactly 1. The box is then `Location` + the mesh origin +
  `RootBoneTranslation`, with the extent × `BoundsScale`. Every other case is `WORLD_TRANSFORM`'s `NotRead`. A
  non-empty `SpaceBases` on a mesh that lists `PerPolyCollisionBones` is a `NotRead`, because the per-bone step is not
  read.
- **Write-free and load-free.** Reads are resident and by name. A non-resident object is refusal N, and a field of
  the wrong kind is a `VmError::Script` naming it. Every refusal test checks the heap, the RNG, #518's counters (class
  code loads included) and the name table.
- **The template gap (note section 9.5): option A coded.** The port's `new` ignores a component's template, and the
  port records no template, so option B (refuse a component made from a template that holds a mesh) cannot be built
  in this lane. Source e therefore answers for the slice's pickup component, which holds class defaults and no mesh.
  That answer is the game's rule on a non-game input, UNVERIFIED. **This is the hub's ruling to make (decision
  2026-11-25-28 asks for it); this lane did not take it.**
- **The slice gate** (installed Off strict, under the machine lock, this head against its base 25425bb9 run the same
  way):
  - base: 5 ok / 8 NotYet / 1 error. Links 6 and 7 stop in `touch` at "a skeletal mesh component's bounds source";
  - head: 5 ok / 7 NotYet / 2 errors. Link 7 moves to "the pickup touch trace: the trace filter of
    `WillowGame.WillowPickup`", an existing stop;
  - **link 6 runs `make`, `touch` and `key` and ends as an error.** The cause is the driver's own evidence check:
    "the weapon is the pawn's own afterwards (its Owner), taken from the pickup";
  - a throwaway local run with source e's half extent set to 1 instead of 262144 gives the same link 6 error. So the
    error does not come from the base rule's size: it is a later gap that link 6 had not reached before;
  - the other rows are unchanged.
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged.
- **Verified:** nothing in the running game.
