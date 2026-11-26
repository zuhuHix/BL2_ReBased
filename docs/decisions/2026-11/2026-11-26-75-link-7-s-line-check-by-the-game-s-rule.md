## 2026-11-26: Link 7's line check by the game's rule

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-LINK7-LINE-CHECK, the implementer half of the clean room, from
`docs/verification/NATIVE_LINK7_LINE_CHECK.md` as landed (#814 with its review r1 fix, main c876a397; the hub's ruling
in decision 2026-11-26-71: section 5 replaces ruling 4's three-actor list). **Stacked on #816** (lane
IMPL-FLOOR-TRACE-RULINGS), whose `trace_filter::owner_chains` gives the `Owner` chain both ways that the note's
source exclusion needs; on main c876a397. **Every rule is UNVERIFIED in the running game.** Reference is unchanged.

- **What was decided (Off only, under `pickup_targeting::NoLevelLineCheck` and a collision service with no
  geometry):**
  - the check is a point line **from the view point** to 25 units short of the target, with **the HUD as the source**
    (the pass's receiver). `Host::line_blocker_for` now takes the source in place of the pawn, and the pass gives the
    view point as the start;
  - every live actor's primitives are walked (`CollisionComponent`, then `Components`; each one's owner resolved),
    each through `ow_world::trace_filter`'s point-line filter with the HUD as the source, the touch trace's. In the
    walkers' order a terrain actor's primitive is refused first; any other is passed over by the skip flag,
    `BlockZeroExtent`, `CollideActors`, the source's `Owner` chains both ways, the owner's `bCollideActors` or the
    owner's filter row. With no bits 1, 8, 16, 32
    or 64, every pawn, every `Actor`-row owner without `bWorldGeometry` (pickups included) and every plain volume is
    passed over. The three Willow pawn classes keep the `Pawn` row for this query (NATIVE_FLOOR_TRACE_FLAGS.md section
    3: their step only rejects, then the `Pawn` row);
  - a candidate left is the target or its `Base` (kept), or is passed over when its bounding-cylinder box is disjoint
    from the line's box, with finite, ordered, non-negative values only. Anything else is `NotRead`, naming the first
    unread rule on its path: the hash admission (or the primitive's line test with the admission stand-in), the
    source's volume test, "the trace filter of <class>", terrain, or an `Owner`-chain link. A class with an unread
    override is `NotRead`, not skipped; a pickup with `bWorldGeometry` set is tested;
  - malformed endpoints or bounds are `NotRead`; an object the store has not loaded that decides a candidate is a
    named `NotRead`; reads only, resident objects only;
  - a service holding geometry with the setting on keeps `LINE_GEOMETRY`'s `NotRead`; the setting off, or no service,
    the headless `LayerAbsent`;
  - each primitive's owner is `trace_filter::owner_of`'s (the component's own `Owner` when set, else the walked actor),
    as the note's step 1a says;
  - `ow_world::trace_filter::part_class` and `PartClass` are now public, for this walk.
- **Verified (host only):**
  - `ow-inventory` hud_harness, 5 new tests on invented fixtures: the settings and services; pickups (a cylinder and a
    skeletal mesh decoy, the target) and pawns (`Pawn`, a stationary one, a `WillowPlayerPawn`) never block; a
    world-geometry pickup, a mover (and one with disjoint bounds), a world-geometry `Base` (kept) and the same actor
    not the `Base`, the HUD's controller, a `bPawnsOnly` volume, and a pickup subclass with no read row; a primitive
    only in `Components`, a negative radius, a NaN endpoint and an unloaded part; the note's 4a cases (a mover whose
    only primitive is in `Components`, a NaN bounds centre, an unloaded `Owner` link, a mistyped `bWorldGeometry`, a
    terrain, a `Pawn` subclass); the HUD's controller passed over only as the HUD's `Owner`. Each call leaves #518's counters and
    the class-code loads unchanged. Dropping the Willow pawn row, or the target-or-`Base` step, fails a test (checked by
    hand);
  - `cargo test --workspace -j 8 --no-fail-fast` 4,089 passed / 0 failed / 59 ignored;
  - the installed Off strict `--slice-gate` under the machine lock, base #816's head 056260f1 against this change on
    it (9254be78; the rebase onto main c876a397 brought only #814's docs): **link 7 passes, 9 ok / 14, X = 9.0/14 =
    64.3%** (base: 8 ok, 60.7%, link 7 at the decoy `WillowPickup`'s skeletal mesh). Link 6 still passes; every other
    row is unchanged. The first version, on main f80b68e7 alone, gave the same rows.
- **The note's fix:** the lane built in the hub's points (a) to (g) first, then followed the note as landed: owners by
  `owner_of`, the malformed text, and section 6's 4a tests.
- **Review of #818 (Sol, high):** the box's limits must be finite and ordered (an overflowing limit is malformed, not a
  miss on another axis); the shared filter resolves the owner before the terrain check, so a primitive a terrain lists
  but another actor owns takes that owner's row (the floor-trace and touch tests updated with it); bounds the rule
  does not answer (a mover's static mesh) leave the candidate's own refusal (the hash admission), not a layer error;
  the exact Willow pawn rows pass over whatever rule the filter met first; `Components` must be declared an array of
  objects and both fields must exist, else a data error; a part's class is classified before its residency, so a
  slotless non-primitive is skipped; the view-point-first endpoints and the HUD as source apply only under the Off
  `NoLevelLineCheck` setting (Reference keeps the old call). New tests cover each, and the reversed endpoints, the
  missing 25-unit shortening, the pawn as source and the reverse `Owner` walk.
- **UNVERIFIED:** every rule; the primitives' own line tests are not ported.
