## 2026-11-25: A class code load marks a refusal "stopped after partial writes"

AI-assisted (Claude Code, Opus 5.5). Lane RUNTIME-MARK-CODE-LOADS, stacked on #692 (decision 2026-11-25-26). The file
number is a placeholder until the landing run numbers it. A rule decision, the hub's ruling 0b, **which the
maintainer may overturn**. Synthetic fixtures only.

- **The rule:** a load is a write, so a class code load (`Vm::class_code_loads`, decision 2026-11-25-20) now counts
  in the runtime partial-write marking, not only in tests. Decision 2026-11-25-20 left the runtime on #518's three
  counters and named this as a separate ruling; this is that ruling. A `NotYet` returned after a code load is marked
  "stopped after partial writes", its kind and record kept, everywhere the port marks.
- **What changes (runtime):** every runtime write mark now compares the code loads too:
  - `ow-world`: `pools::Counters::moved` (so `after_counted_writes`), `body_composition::after_writes`,
    `mapload::game_info::WriteMark::moved` (its `still()` is now just its negation), and `set_base_value`'s counters;
  - `ow-skills`: `flag_expression::WriteCounters::moved`;
  - `ow-inventory`: `equip::card::marked_after_loads` (it now marks on `load_and_code_mark`), `equip::internal_slots`,
    `attach_mesh::pickup_rb`, `globals::achievement`, `globals::class_achievement` and `equip::particle_effect`'s
    counters;
  - `ow-mission`: `slice`'s `WriteCounters`.
- **Not changed:** `ow-attr`'s evaluation tracker (`eval.rs` `counters`), which has no VM and so no code-load count;
  and `ow-cli`'s set-up line that reports whether a preload loaded (a harness report, not a mark).
- **Tests whose outcome changed** (cargo test --workspace on #692 bba377f), each looked at, not blindly flipped:
  1. `ow-cli` `combat::harness_tests::a_class_select_refusal_is_marked_only_after_a_write`, in the no-preload case.
     The only code load in the step was the test's own toy class-select native probing
     `find_class("WillowGame.Thing")`. That is a fixture artifact, not the step's write, so the fixture now warms that
     class and the no-preload refusal is unmarked again. The preload case stays marked.
  2. `ow-inventory` `equip::card::tests::an_unloaded_weapon_type_is_not_read_and_not_loaded`. The native's lookups
     loaded `Core.Object`'s code, which this fixture's separate Core package leaves cold; the game always has the root
     class's code loaded. The fixture now warms it, and the refusal is unmarked again.
  3. `ow-world` `mapload::game_info::tests::a_refused_spawn_after_a_class_code_load_is_marked`. It asserted
     `!mark.moved` to mean "#518's three did not move". `moved` now includes code loads, so it checks the three
     counters explicitly; the outcome (marked) is the same.
  4. `ow-inventory` `equip::particle_effect::tests::an_evaluator_refusal_with_no_write_passes_through_unchanged`
     **fails on this branch alone, and is not changed.** On #692's base the particle-effect evaluator dispatch still
     loads the evaluator's class code (#685's first finding), so the refusal is now correctly marked. #686 fixes that
     dispatch. With #686 merged in locally (a throwaway merge, not pushed) all 24 particle-effect tests pass,
     including this one. So this PR needs #686 below it, or a rebase once #686 lands.
- **Not verified:** nothing in the running game (**UNVERIFIED**). Which refusals in the slice's real runs now gain
  the prefix depends on which classes are cold there, which is not measured. The installed slice gate was not run.
