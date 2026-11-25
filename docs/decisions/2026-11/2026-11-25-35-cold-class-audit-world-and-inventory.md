## 2026-11-25: Cold-class audit of ow-world and ow-inventory

AI-assisted (Claude Code, Opus 5.5). Lane COLD-CLASS-AUDIT-WORLD-INVENTORY, the rest of the cold-class audit after
#690 (ow-skills). The file number is a placeholder until the landing run numbers it. **Tests only:** no rule and no
native changed. Synthetic fixtures only.

- **Method, as #690's:**
  - every class-code-loading lookup in the two crates' runtime code (`find_class`, `find_class_if_present`,
    `find_method`, `class_default`) was listed with its enclosing function. That gives 49 sites;
  - the ones on a preflight or a write-free path got a cold-class variant of an existing test. The variant's object or
    lookup class has its layout defined through the code index and its code not loaded, and the test asserts that
    `Vm::class_code_loads` (#685), and where relevant #518's counters, do not move;
  - where one moves, the test is `#[ignore]`d with the path named, and the finding is left for its own queue item.
- **Findings (ignored tests, each a code load on a write-free path):**
  1. `ow-inventory` `pickup` `get_inventory_pawn_on_a_cold_pawn_loads_no_code`: `GetInventoryPawn`'s
     `find_class("WillowGame.WillowPlayerPawn")` loads that class's cold code, on a read-only query (2 code loads);
  2. `ow-inventory` `pickup` `the_definition_query_on_a_cold_inventory_loads_no_code`:
     `GetPickupableInventoryDefinition`'s `find_method` loads the inventory class's cold code before its dispatch
     check (6 code loads);
  3. `ow-inventory` `globals::achievement` `no_match_with_a_cold_globals_definition_loads_no_code`:
     `UnlockAchievementIfConditionsMet`'s read-only "no match" answer loads `GlobalsDefinition`'s chain's cold code,
     by reading through `globals_definition`'s `find_class` (2 code loads; the fixture gains a `cold_definition` set-up
     flag for it);
  4. `ow-world` `datastore` `a_fill_that_cannot_be_planned_loads_no_class_code`: `GetDataStoreClient`'s preflight
     takes every store class with `find_class` before anything is made, and on a cold world that loads their code
     before its refusal (3 code loads);
  5. `ow-world` `class_route` `load_async_with_a_cold_context_loads_no_code`: `LoadPlayerPawnDataAsync`'s
     not-implementing refusal comes after two code loads, by reading its `find_class` of `IStreamingDataEvent`.
- **Passes, with a condition:** `ow-inventory` `pickup_targeting` `the_pawn_test_on_a_cold_pawn_class_loads_no_code`.
  The pawn's own class is cold, but the test passes only with `WillowPlayerPawn` warm (the fixture looks it up):
  `suspended`'s `find_class(PLAYER_PAWN)` is finding 1's pattern and would load that class's code were it cold.
  - **A side finding, not a code load:** `suspended` interns the name "Dead" on each pass, a name-table write in that
    preflight. The test does not compare the name table, and says so.
- **Not tested here, with the reason:**
  - **Post-commit or call dispatch:** the lookup sits on a path that already writes, or dispatches a call:
    - `anim_tree::notify_owner`, `equip::equip_controller`, `spawn::call_method`;
    - `actors::send_event`, `actors::spawn_from`'s class default;
    - `pools` (`ensure_manager`, `find_script`, `create_in`, `delete_pool_at_index`), `timers::update_pass`;
    - `mapload::player::run`, `mission_numbers::call_method`;
    - `online` (`call_method`, `setup_input_devices`, `server_change_name`).
  - **Set-up or registration:** `equip::card::setup`, `internal_slots::register`, `manager::register_natives`,
    `online::register_offline_engine`, `world::new`, `mapload::load_map`, `mapload::player::make_local_player`.
  - **Already handled:** `mapload::game_info::configured` (#692), `particle_effect::evaluate` (#686), and the card's
    runtime `globals_definition`, which `card::setup` warms in the equip world (#695's review).
  - **No cold fixture without new scaffolding:** `equip::value::rarity` (its fixture hand-defines its classes).
    By reading, its `find_class("WillowGame.MissionItemDefinition")` would load that class's cold code before the
    first write.
  - **Not reached in the time box:** `pickup_targeting::suspended` with `WillowPlayerPawn` itself cold (its
    `find_class(PLAYER_PAWN)`; the fixture warms that class, so a variant needs a second fixture); `spawn::resolve_objects` (it resolves a class about to be spawned, whose refusal
    is a data error, not a `NotYet`); `pickup_targeting::prepare` and `plan`'s `class_default` / `find_class` of
    `PICKUP` (the fixture warms `PICKUP`, so a cold variant needs a second fixture); and `manager::manager_of`.
- **Not verified:** nothing in the running game; whether these classes are cold in the real Off world is not
  measured.
- **Review r1 of #701:** the passing pawn test's claim is narrowed (it holds only with `WillowPlayerPawn` warm), and
  that case is listed as not reached. The cold `LoadPlayerPawnDataAsync` test now checks the refusal is the
  not-implementing one. The data-store test's doc says what the fill world does warm (`Core.Object`, through
  `find_function` `WillowPlayerController`, and `UIInteraction` through `register`'s bind), and the test checks the
  error is "class not found: Engine.UIDataProvider_Settings". The cold definition-query test checks its answer, the
  inherited query's `UnboundNative`, as its sibling test does.
