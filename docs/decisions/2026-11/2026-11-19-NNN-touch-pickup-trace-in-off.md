# DroppedPickup.TouchPickupTrace in Off, from the analyst note

AI-assisted (Claude Code, Claude Opus 5.5), lane IMPL-TOUCH-PICKUP-TRACE, on main 4c4522d. This is the implementer
half of the clean room. The code follows only `docs/verification/NATIVE_TOUCH_PICKUP_TRACE.md`: #624, lane
ANALYST-LINKS-6-7-PAST-TOUCH, accepted at 6dbdea1e and copied into this branch unchanged. This lane did not analyse the
executable. **Every rule stays UNVERIFIED** in the running game. The note's section 7 gives the live check.

**What changed.**
- `DroppedPickup.TouchPickupTrace` replaces its `NotYet { NotRead }` stub in `ParityMode::Off`. Reference binds
  nothing, as before.
- `ow_inventory::pickup` does steps 1 to 4:
  - the receiver;
  - its exact class, `Engine.DroppedPickup` or `WillowGame.WillowPickup`, through `ClassTable::by_path` only;
  - two `Vector` arguments (a struct named `Vector` with float `X`, `Y`, `Z`);
  - an unloaded receiver.
- `ow_world::touch_trace::touch_line` does steps 5 to 9:
  - no collision service is `LayerAbsent`, "the pickup touch trace";
  - a zero-length segment is `NotRead`;
  - the level part is `point_sweep` from `EndLoc` to `StartLoc`. A hit is false, and `Unsupported` is `NotRead`;
  - then the actor walk;
  - otherwise true.
- **The walk** is the spawn placement's, moved into `placement::actor_walk` so both share it, with the same refusal
  texts. It runs under a new query kind, `trace_filter::Query::PointLine`:
  - its flags are 134226566 plus 1024, with `BlockZeroExtent` and the skip flag;
  - it has a source actor (`trace_filter::Source`);
  - after the per-component test, a primitive whose owner is the pickup or in its `Base` chain is not tested. The
    chain is read only for a candidate that reaches that step, from resident objects through `ObjectSpace::prop`. An
    unloaded link is `NotRead`, "a Base-chain link the store has not loaded"; a cycle or None ends it; a class with
    no `Base` is a data error;
  - a `bPawnsOnly` volume is `NotRead`, "the source actor's volume test".
- **Hash admission still comes first** for a primitive every read test accepts. `touch_trace::AdmissionStandIn` is a
  VM service that only tests install. With it, that primitive is refused naming its own line test instead (note test
  10a). No harness or host installs it.
- **Harness input** (the hub's ruling, UNVERIFIED stand-in): the slice gate's pickup run (links 6 and 7) installs link
  9's `NoLevel` collision service in its world, in `slice_gate.rs`. Without it, step 5 refuses there.
- **Documents:**
  - NATIVE_INDEX gains the note's row;
  - NATIVE_PICKUP.md section 3's "direction resolved" now points to the note's correction.

**Review r1 of #638:**
- **The Base chain** is now decided after the primitive's own flags and before any flag of its owner. An owner the
  chain names (an unloaded `Base` handle included) is not tested without its flags being read. An unloaded link
  leaves the chain undecided, and the remaining read filters still run. Only a primitive they leave as a possible
  blocker is "a Base-chain link the store has not loaded", so a pawn's primitive is still rejected by its row.
- **The vectors.** Each argument's struct id must be the declared parameter's: another struct named `Vector` is a
  data error. A script `None` no longer becomes a zero vector for this native in Off. The new
  `Vm::keep_struct_none(key)`, Off only, exempts a native key from the interpreter's conversion (decision
  2026-11-15-28, cause C), and the native refuses `None` as a data error. Reference keeps the conversion. The
  change in `crates/ow-vm/src/interp/eval.rs` is that one check.
- **The harness line.** The pickup run records the `NoLevel` service in its `harness` lines, as an UNVERIFIED
  stand-in.

**Tests** (`crates/ow-inventory/src/pickup_touch_tests.rs`): the note's 13 cases, with 10a and 10b, on invented
packages, and r1's two cases:
- an unloaded `Base` with only a pawn, then with a primitive whose own `Owner` is that handle;
- another `Vector` struct, then `None` passed from bytecode. Every call goes through the VM and asserts #518's three counters (`Heap::committed_writes`,
`GameRng::draws`, `DefStore::load_transitions`) and the name table's length unchanged. The old stub test in
`pickup_tests.rs` now expects `LayerAbsent` with no service. Its fixture's `Vector` gains `X`, `Y` and `Z`.

**Not verified:**
- anything in the running game;
- the direction of the line (the note's section 7 says how to settle it);
- the game classes' filter overrides. Every class with no read row refuses naming its filter.

The test counts and the slice-gate reading are in the GATES row.

**Review round 2** (Sol, the last round):
- **A primitive with its own `CollideActors` clear never refuses on the Base chain.** The filter tests that flag
  after the skip and blocking flags and before the Base walk. The owner's `bCollideActors` stays after the source and
  Base identity checks. A regression case covers an unloaded owner with the Base on a different unloaded handle,
  then the same primitive with `CollideActors` set, which refuses on the Base chain.
- **The `None` exemption is checked against the current parity.** `Vm::keeps_struct_none` also requires Off, so a
  VM switched to Reference after the key was kept converts again. The ow-vm test of the conversion now covers the
  kept key in Off and the switch to Reference, through an element read past the end of an empty array, which is
  the path that yields `None`.
- **Correction to r1.** In the r1 bytecode test, `NO_OBJECT` yields a null object reference, not `None`, so that
  test checks only that a null reference is refused. It never reached the conversion.
