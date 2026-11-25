## 2026-11-25: A class code load marks a refusal "stopped after partial writes"

AI-assisted (Claude Code, Opus 5.5). Lane RUNTIME-MARK-CODE-LOADS, stacked on #692 (decision 2026-11-25-26).
A rule decision, the hub's ruling 0b, **which the
maintainer may overturn**. Synthetic fixtures only.

- **The rule:** a load is a write, so a class code load (`Vm::class_code_loads`, decision 2026-11-25-20) now counts
  in the runtime partial-write marking, not only in tests. Decision 2026-11-25-20 left the runtime on #518's three
  counters and named this as a separate ruling; this is that ruling. A `NotYet` returned after a code load is marked
  "stopped after partial writes", its kind and record kept, at every counter-based mark (#518's tracker). A code load
  here is each class whose code is loaded, each package function declared, and each state's code declared on first
  use (decision 2026-11-25-20).
- **What changes (runtime):** every counter-based write mark now compares the code loads too:
  - `ow-world`: `pools::Counters::moved` (so `after_counted_writes`), `body_composition::after_writes`,
    `mapload::game_info::WriteMark::moved` (its `still()` is now just its negation), and `set_base_value`'s counters;
  - `ow-skills`: `flag_expression::WriteCounters::moved`;
  - `ow-inventory`: `equip::card::marked_after_loads` (it now marks on `load_and_code_mark`), `equip::internal_slots`,
    `attach_mesh::pickup_rb`, `globals::achievement`, `globals::class_achievement` and `equip::particle_effect`'s
    counters;
  - `ow-cli`: `inventory::equip_run::equip_world` (review r1 of #695; it marked on `card::load_mark`'s three);
  - `ow-mission`: `slice`'s `WriteCounters`.
- **Not changed:**
  - `ow-attr`'s evaluation tracker (`eval.rs` `counters`, read through `EvalCx::wrote()`). It has no VM and so no
    code-load count. The gap is partly covered: its VM-layer callers mark on their own counters, which now include
    code loads;
  - `ow-cli`'s set-up line that reports whether a preload loaded (a harness report, not a mark);
  - **the flag-based marks** (review r1 of #695), which mark on a `wrote` flag their own code sets rather than on
    counters. They do not see a code load: `ow-cli` `slice_gate.rs` (~1005, ~1030, ~1418) and `slice_pickup.rs`
    (~301, ~337); `ow-world` `world/skeletal_mesh.rs` (~184), `world/datastore.rs` (~233),
    `world/mission_numbers.rs` (~246) and `placement.rs` (~285); `ow-combat` `holds.rs` (~237) and `hit.rs` (~1055);
    `ow-inventory` `pickup_targeting.rs` (~578).
- **The control-flow changes the maintainer accepts or overturns** (review r1 of #695). A marked `NotYet` is not only
  different text; three callers treat it as terminal, so a refusal after a code load now stops more:
  - `ow-combat` `run.rs` (~1216-1224): in Off, a step's marked `NotYet` latches `class_switch_stop`, so the whole
    combat run stops there (no later step, no snapshot after it);
  - `ow-mission` `bridge.rs` (~1097): `is_partial_stop` records the error as `s.partial`, so a mission script's stop
    becomes terminal;
  - `ow-mission` `slice.rs` (~603): `settle_conditions` returns a terminal (`is_terminal`) error at once, so the tracker
    stops before its next event instead of keeping the error for the step to report.
- **Tests whose outcome changed** (cargo test --workspace on #692 bba377f), each looked at, not blindly flipped:
  1. `ow-cli` `combat::harness_tests::a_class_select_refusal_is_marked_only_after_a_write`, in the no-preload case.
     The only code load in the step was the test's own toy class-select native probing
     `find_class("WillowGame.Thing")`. That is a fixture artifact, not the step's write. The toy now reads the class
     table (`by_path`), so it loads nothing, and the no-preload refusal is unmarked again; the preload case stays
     marked.
  2. `ow-inventory` `equip::card::tests::an_unloaded_weapon_type_is_not_read_and_not_loaded`. The load is on the
     card's runtime path: `card::compute` -> `globals::globals_definition` -> `Vm::find_class`, which loaded
     `Core.Object`'s code. In the equip world, `card::setup`'s ROOTS (both globals classes among them) load that chain
     at set-up, so at run time the lookup loads nothing and #521's set-up-only loading ruling holds. This fixture
     hand-builds its classes and skips `setup`, and with `Core.Object` from the store its code stayed cold. The
     fixture's constructor now loads it, standing for `setup`, rather than the one test.
  3. `ow-world` `mapload::game_info::tests::a_refused_spawn_after_a_class_code_load_is_marked`. It asserted
     `!mark.moved` to mean "#518's three did not move". `moved` now includes code loads, so it checks the three
     counters explicitly; the outcome (marked) is the same.
  4. `ow-inventory` `equip::particle_effect::tests::an_evaluator_refusal_with_no_write_passes_through_unchanged`,
     not changed. On #692's base alone the particle-effect evaluator dispatch still loaded the evaluator's class code
     (#685's first finding), so the refusal came back marked. #686 fixes that dispatch. Review r1 of #695 rebased this
     change onto #686, with #692's commits cherry-picked on top, and there it passes unchanged.
- **Not verified:** nothing in the running game (**UNVERIFIED**). Which refusals in the slice's real runs now gain
  the prefix depends on which classes are cold there, which is not measured. The installed slice gate was not run.
