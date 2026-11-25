## 2026-11-25: Write-free checks also count class code loads

AI-assisted (Claude Code, Opus 5.5). Lane COUNTERS-MISS-CLASS-CODE-LOADS, a verification-rule change.
No game data and no native reading; synthetic fixtures
only.

- **The gap (found on #682):** loading a cold class's code registers functions and can intern names, so in Off it is
  a write. But it moves none of #518's three counters (heap writes committed, random draws, store load transitions),
  and usually not the name table's length either. A refusal test that asserts "write-free" with those alone does not
  see it.
- **What changes:**
  - `ow-vm`: `Vm::class_code_loads()`, a count that only grows. It goes up once for each class whose functions and
    states are loaded, once for each package function declared (counted once declared, so a failed attempt and its
    retry are not counted), and once for each state's code declared on first use (`declare_state_code`). Review r1 of
    #685 added the last and moved the function count after the declaration.
  - `ow-world`: `pools::Counters` and `body_composition::Counters` hold it as a fourth field, so tests that compare
    two readings catch a code load. Their runtime use is unchanged: `Counters::moved` and `body_composition::after_writes`
    still read only #518's three, so no runtime refusal gains a "stopped after partial writes" prefix from a code
    load. Whether it should is a separate ruling, left to the maintainer.
  - **Tests, "nothing moved" checks:**
    - 59 test-side counter readings on a VM's store read `defs.load_transitions() + class_code_loads()`;
    - the production write marks used by tests got a sibling check: `game_info::WriteMark::still` and
      `flag_expression::WriteCounters::still` compare the code loads too. 19 test sites that asserted `!x.moved(..)`
      now assert `x.still(..)` (`mapload/game_info/tests.rs`, `mapload/player/tests.rs`, `bridge/adopt_tests.rs`,
      `flag_expression_tests.rs`). One new assertion, the pinned `a_refused_spawn_loads_no_class_code`, compares
      `class_code_loads()` directly;
    - `equip::card::load_and_code_mark` is `load_mark` plus the code loads, used at the 8 "nothing moved" sites of
      `gestalt_tests.rs` and `gestalt_data_tests.rs`.

    The runtime `moved()` / `load_mark` and their partial-write marking are unchanged.
  - **Tests, "a load did happen" checks** (review r1 of #685): these stay on `load_transitions()` alone, because a code
    load alone must not satisfy them. That is `slice_gate_driver_tests.rs` "a load happened", `body_composition_tests.rs`
    "the load is counted", `set_base_value_tests.rs` "the resolver was loaded", and `internal_slots_store_tests.rs`'s
    two `n + 1` counts. One exact count, `gestalt_data_tests.rs`'s "one write, no draw, no load", stays on `load_mark`.
  - **Not covered:** ow-attr's tests (no VM); the private three-counter marks inside `ow-inventory`'s
    `internal_slots.rs` and `attach_mesh/pickup_rb.rs` and `ow-mission`'s `slice.rs` (runtime-only, no test reads
    them directly); and any test that asserts "write-free" another way than the readings above. The other places
    those writes come from are under "Its limit" below.
- **What it caught:** two existing tests. Each keeps #518's three, and an ignored test pins the finding for the hub
  to route; neither is fixed here.
  - **Second (review r1 of #685):** `ow-world` `mapload::game_info`
    `a_missing_or_unfit_class_and_a_refused_spawn_leave_the_field_empty`. `spawn_game_info` for a class whose spawn
    is refused for collision has already loaded that class's code (its class lookup) when it refuses. Pinned by
    `a_refused_spawn_loads_no_class_code`.
  - **First:** `ow-inventory` `equip::particle_effect`
    `the_effect_handle_is_returned_unloaded_and_nothing_is_written_or_loaded`. On its fixture, `GetParticleEffect`'s
    evaluator dispatch loads `GearboxFramework.ExpressionEvaluator`'s and `GearboxFramework.Object`'s code (three code
    loads). Pinned by `the_effect_lookup_loads_no_class_code` (#686 fixes it).
- **Its limit:** two kinds of write are not counted.
  - **The child-declaration cache** (`decl_at`, which interns names and may define layouts). It fills on the first
    access to a property declaration not yet cached, which a script function's first run usually makes, so counting
    it flags the interpreter's own laziness, not a native's preflight. Counting it
    tripped one test, `bounding_cylinder`'s `a_bad_height_destination_leaves_the_radius_unwritten`, where only the
    test's own toy caller script ran for the first time.
  - **Layout definitions** (`layout_of` / `class_at` reaching `define_layout`). These define classes in the table and
    could be a separate counter. And the check only sees loads a fixture leaves cold. Most
  fixtures look their classes up with `find_class`, which loads their code first, so a native that would load cold code in the game's Off world can
  still pass. #682's queue test needed a fixture that never warms the class. A test that means "loads no code" should
  use such a fixture.
- **Verified (host only):** a new `ow-vm` test shows a cold class's load moving the counter while #518's three stay
  put, and a read-only lookup or a second lookup not moving it. The workspace suite passes, apart from the 4
  expected Linux N1 tests and the two ignored findings. Nothing here touches the running game.
