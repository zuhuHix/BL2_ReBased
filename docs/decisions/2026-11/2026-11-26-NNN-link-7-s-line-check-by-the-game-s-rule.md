## 2026-11-26: Link 7's line check by the game's rule

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-LINK7-LINE-CHECK, the implementer half of the clean room, from
`docs/verification/NATIVE_LINK7_LINE_CHECK.md` (#814 at 09dd27fe, with the hub's ruling in decision 2026-11-26-71:
section 5 replaces ruling 4's three-actor list) and the hub's points (a) to (g) for the note's review fix, on main
f80b68e7. **Every rule is UNVERIFIED in the running game.** Reference is unchanged.

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
  - `ow_world::trace_filter::part_class` and `PartClass` are now public, for this walk.
- **Verified (host only):**
  - `ow-inventory` hud_harness, 4 new tests on invented fixtures: the settings and services; pickups (a cylinder and a
    skeletal mesh decoy, the target) and pawns (`Pawn`, a stationary one, a `WillowPlayerPawn`) never block; a
    world-geometry pickup, a mover (and one with disjoint bounds), a world-geometry `Base` (kept) and the same actor
    not the `Base`, the HUD's controller, a `bPawnsOnly` volume, and a pickup subclass with no read row; a primitive
    only in `Components`, a negative radius, a NaN endpoint and an unloaded part. Each call leaves #518's counters and
    the class-code loads unchanged. Dropping the Willow pawn row, or the target-or-`Base` step, fails a test (checked by
    hand);
  - `cargo test --workspace -j 8 --no-fail-fast` 4,081 passed / 0 failed / 59 ignored;
  - the installed Off strict `--slice-gate` under the machine lock, base f80b68e7 against this head: **link 7 passes,
    9 ok / 14, X = 9.0/14 = 64.3%** (main: 8 ok, 60.7%, link 7 at the decoy `WillowPickup`'s skeletal mesh). Link 6
    still passes; every other row is unchanged.
- **The note's fix:** the hub's points (a) to (g) are built in. The lane re-reads the note when the hub says F's fix is
  in, and follows it where it differs.
- **UNVERIFIED:** every rule; the primitives' own line tests are not ported.
