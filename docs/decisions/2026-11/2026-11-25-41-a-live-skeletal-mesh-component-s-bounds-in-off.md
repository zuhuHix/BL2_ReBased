## 2026-11-25: A live skeletal mesh component's bounds in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SKELMESH-BOUNDS-SOURCE, on integration/2026-11-25-c15 (25425bb9). Coded
from NATIVE_COMPONENT_BOUNDS.md section 9 alone,
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
  5. source b (a parent's fixed-bounds mesh): the parent's flag is read first, so its mesh must be resident only
     when the source applies. The stored-bounds `LayerAbsent` while the accessor answers none. Once it answers,
     `WORLD_TRANSFORM`'s `NotRead` unless the transform is the simplest one (below), and then a `NotRead`, because
     the note did not check whether the root offset is added on this source. That last refusal is this lane's reading
     of an open point, not a rule the note states (review r1 of #707);
  6. sources c and d (physics assets), asked only when "can use the bones" holds (a uniform scale within 0.0001, a
     mesh, and the bone count or a parent's asset). Each is a `LayerAbsent`: the bodies ("a physics asset's bodies",
     its text naming the bone-transform layer), or the parent's stored `Bounds` (the bounding cylinder's
     `BOUNDS_LAYER`);
  7. source e (no mesh): **not answered**, a `NotRead`, "the component's template (option B) is not read" (the
     hub's revised ruling, below);
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
- **The template gap (note section 9.5): the hub's revised ruling (review r1 of #707).** The port's `new` ignores a
  component's template and records none, so option B (refuse a component made from a template that holds a mesh)
  cannot be built. A port component with no mesh always holds class defaults only, an input the game never has. So
  source e is not answered: answering the game's base rule on that input would be a guess. The first version of this
  change answered it (option A); the hub revised the ruling.
- **The slice gate** (installed Off strict, under the machine lock, on this change's head after review r1 of #707,
  on main a3b548d): 5 ok / 8 NotYet / 1 error.
  - **Links 6 and 7** pass `make` and stop in `touch` at "stopped after partial writes: Actor.GetBoundingCylinder:
    the component's template (option B) is not read". That is the honest stop: the pickup's skeletal component holds
    class defaults and no mesh.
  - Every other row equals the base's (25425bb9, measured for the first version) row for row. Link 1 stops at "no
    reference skeleton" in both, since neither holds #693.
- **What the first version showed (option A), kept as evidence.** Answering source e moved link 7 to the touch trace
  filter and made link 6 run `make`, `touch` and `key`. Link 6 then failed its driver's evidence check ("the weapon
  is the pawn's own afterwards"). The cause, traced in lane LINK-6-OWNER-CHECK:
  - the pickup world's `WorldInfo.Game` is None;
  - so `DroppedPickup.Pickup.ValidTouch`'s last condition, `WorldInfo.Game.PickupQuery(Other, self)`, reads through
    None and answers false silently;
  - so the touch never registers, `CurrentTouchedPickupable` stays None, and the key takes nothing.

  The box played no part: `TouchPickupTrace` answered true, and a 1-unit box gave the same result. The combat run
  installs a game-info stand-in for this; the pickup world does not (lane LINK-6-PICKUP-GAMEINFO).
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged.
- **Verified:** nothing in the running game.
