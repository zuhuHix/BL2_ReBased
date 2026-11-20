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
- **Not changed:** `Engine.NavigationPoint`, which #595 also answers. That is outside this lane's pickup scope, and
  its spawn extent still refuses `NotRead`. The two lists live in two files. Once #595 lands, one could be built from
  the other.
- **Tests** (`crates/ow-world/src/placement_pickup_tests.rs`, 3 new, on invented packages):
  - a pickup template takes `Actor`'s rule: (0, 0, 0), an unattached entry skipped, an attached colliding entry
    refused at the skip flag;
  - a cylinder `CollisionComponent` still comes first;
  - a `WillowPickup` subclass still refuses.
- **Slice:** the pickup driver spawns with `bNoCollisionFail`, so links 6 and 7 never ask for the extent. The
  installed gate on this branch (GATES) shows them at `InitializePickupForRBPhysics`, which comes before any extent
  question. No baseline run of main at c687c135 was made.
