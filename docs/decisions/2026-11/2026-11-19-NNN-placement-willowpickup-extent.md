## 2026-11-19: The spawn extent takes Actor's rule for a WillowPickup, as GetBoundingCylinder does

AI-assisted (Claude Code)

Lane PLACEMENT-WILLOWPICKUP-EXTENT, with Claude Opus 5.5. The file number is a placeholder until the maintainer gives
one. No dependency, no bounds check changed, Reference unchanged. **UNVERIFIED in the running game.**

- **The check.** On main plus #595 (`fix/rust-get-bounding-cylinder` @ 9de51c43), a pickup's spawn extent and its
  bounding cylinder agree when the template's `CollisionComponent` is a cylinder. The spawn takes that cylinder first
  (NATIVE_SPAWN_EXTENT.md, the spawn's own step), and `GetBoundingCylinder` is never asked. They disagreed for a
  `WillowPickup` template whose `CollisionComponent` is not a cylinder:
  - `placement::rule` gave `Actor`'s rule only to `Engine.Actor`, so the pickup refused `NotYet { NotRead }`, "the
    bounding-cylinder rule of WillowGame.WillowPickup";
  - #595 answers it with the actor default, because NATIVE_GET_BOUNDING_CYLINDER.md section 4, step 1 read that
    `WillowPickup`'s vtable holds that body.
- **The change** (`crates/ow-world/src/placement.rs`): `ACTOR_RULE_CLASS` becomes `ACTOR_RULE_CLASSES`, with
  `Engine.Actor` and `WillowGame.WillowPickup`. The match is still by exact class, so a subclass still refuses. The
  rule's reads, loads and order are unchanged.
- **Depends on #590** (accepted, landing before this): it carries NATIVE_GET_BOUNDING_CYLINDER.md, whose section 4,
  step 1 is the evidence. NATIVE_SPAWN_EXTENT.md section 5 gets a "Superseded in part" line naming it, and its step 4
  now says which bodies were read, so `EXTENT_RECORD`'s text stays true.
- **Changed for the second caller:** `placement_encroachment.rs`'s check calls `extent` on the live actor (step 3.4).
  A live `WillowPickup` with no colliding primitive now gets (0, 0, 0) there, where it used to get `NotRead`.
  - **Why that is right:** step 3.2 has already refused any primitive with `CollideActors` set, so `Actor`'s rule
    meets only entries it skips. The game's answer for those is the same zero box.
  - **No conflict with #595's refusal L:** that refusal covers a live receiver's colliding entries, whose attachment
    the port does not track. None reaches `extent` here.
- **Not changed:** `Engine.NavigationPoint`, which #595 also answers. That is outside this lane's pickup scope, and
  its spawn extent still refuses `NotRead`. The two lists live in two files. Once #595 lands, one could be built from
  the other.
- **Tests** (`crates/ow-world/src/placement_pickup_tests.rs`, 5 new, on invented packages; review r1 of #606 added
  the last two):
  - a pickup template takes `Actor`'s rule: (0, 0, 0), an unattached entry skipped, an attached colliding entry
    refused at the skip flag;
  - a cylinder `CollisionComponent` still comes first;
  - a `WillowPickup` subclass still refuses;
  - a live pickup whose primitives all have `CollideActors` clear gets (0, 0, 0);
  - a pickup archetype with a lazy primitive entry: `load_extent_components` loads it and reports the write.
- **Slice:** the pickup driver spawns with `bNoCollisionFail`, so links 6 and 7 never ask for the extent. The
  installed gate on this branch (GATES) shows them at `InitializePickupForRBPhysics`, which comes before any extent
  question. No baseline run of main at c687c135 was made.
