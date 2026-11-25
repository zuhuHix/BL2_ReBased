## 2026-11-25: The pickup classes' trace-filter row

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-PICKUP-TRACE-ROWS, on main 4464e814. It implements
docs/verification/NATIVE_TOUCH_PICKUP_TRACE.md section 3.1.1 (#711, lane F's amendment). Clean room: this lane did not
read the executable. No real-game session was run. **UNVERIFIED in the running game.** Reference is unchanged.

**What changes.** `ow-world` `trace_filter::ROWS` gains two exact-class `Row::Actor` rows, `Engine.DroppedPickup` and
`WillowGame.WillowPickup`; the note reads both classes as keeping `Actor`'s filter body. Nothing else changes. The
`Actor` row already reads `bWorldGeometry`, and only with bit 32 `bProjTarget` and `bBlockActors`, loaded-only. A
subclass of either class still has no row, so it stays "the trace filter of <class>".

**For link 7** (the note's prediction, not a run): a second pickup on the touch line, not the source, with the class
default's `bWorldGeometry` clear, is now not tested. The touch's flags hold neither bit 16 nor bit 1, so only the
world-geometry shortcut could test it. The source pickup is excluded before any row is read, as before.

**Tests**, section 3.1.2's list:
- `trace_filter/tests.rs`: the rows, exact-class (a subclass has none). Test 5: the spawn's box test (8902) does not
  test a pickup's primitive with `bWorldGeometry` clear, does test one with it set (the hash admission), and bit 16
  alone passes the `Actor` row;
- `ow-inventory` `pickup_touch_tests.rs`:
  - tests 1 and 3: a second `WillowPickup` or `DroppedPickup` is not tested;
  - test 2: one with `bWorldGeometry` set is the hash admission, and with the admission stand-in its primitive's line
    test;
  - test 4: a `WillowPickup` subclass still refuses;
  - test 6: the source pickup's own cylinder is excluded, even with `bWorldGeometry` set;
  - test 7: Reference, covered by the existing `reference_binds_nothing`.

**Test 0 is represented, not run.** Its "before the rows" state, a second `WillowPickup` refusing, is gone once the
rows exist. The subclass test checks the same refusal text, and main's slice gate recorded it for `WillowPickup`
before this change.

**The slice gate** (installed Off strict, release build, under the machine lock): 5 ok / 9 NotYet / 0 error, `links
ok 5/14`, `step share X = 5.783/14 = 41.3%`. Links 6 and 7 stop earlier than the trace filter, at `touch` on "Actor.GetBoundingCylinder: the
component's template (option B) is not read" (#707), so this change cannot show there yet. Their rows are 1 of 3 and
1 of 4.

**Gates.** `cargo test --workspace -j 8`: 3,582 passed / 0 failed / 57 ignored. `crate_edges_test.py`: 19 OK.
