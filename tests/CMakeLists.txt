# Test registration for ctest. The top-level CMakeLists.txt pulls this file in with include(), so it runs in the top directory's
# scope: ${CMAKE_SOURCE_DIR} and relative source paths resolve as they did there, and ow-weapon-firing-test is still built to
# build/<config>/. Adding, removing or relabelling a test edits this file only, not the top CMakeLists.txt. Never add it with
# add_subdirectory(tests): the relative source path tests/weapon_firing_math_test.cpp would then resolve under tests/tests/ and
# stop the configure, and ow-weapon-firing-test would move to build/tests/<config>/.
# The core targets (ow-core, ow-package, ow-weapongen) and their sources and flags stay in the top CMakeLists.txt; this file holds
# test registration only.
enable_testing()
find_package(Python3 COMPONENTS Interpreter REQUIRED)
add_test(NAME package-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/package_test.py $<TARGET_FILE:ow-package>)
add_test(NAME properties-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/properties_test.py $<TARGET_FILE:ow-package>)
add_test(NAME runtime-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/runtime_test.py $<TARGET_FILE:ow-package>)
add_test(NAME global-resolve-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/global_resolve_test.py $<TARGET_FILE:ow-package>)
add_test(NAME object-dump-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/object_dump_test.py $<TARGET_FILE:ow-package> --reference)
add_test(NAME item-registry-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/item_registry_test.py $<TARGET_FILE:ow-package>)
add_test(NAME item-roll-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/item_roll_test.py)
add_test(NAME natives-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/natives_test.py $<TARGET_FILE:ow-package>)
add_test(NAME vm-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/vm_test.py $<TARGET_FILE:ow-package>)
add_test(NAME vm-states-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/vm_states_test.py $<TARGET_FILE:ow-package>)
add_test(NAME script-disasm-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/script_disasm_test.py)
add_test(NAME script-cli-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/script_cli_test.py $<TARGET_FILE:ow-package>)
add_test(NAME controls-check COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/check_controls_test.py)
add_test(NAME kismet-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/kismet_test.py $<TARGET_FILE:ow-package>)
add_test(NAME weapon-gen-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/weapon_gen_test.py $<TARGET_FILE:ow-weapongen>)
add_test(NAME weapon-gen-firing-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/weapon_gen_firing_test.py)
add_test(NAME weapon-gen-parity-logic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/weapon_gen_parity_test.py)
add_test(NAME behavior-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/behavior_test.py $<TARGET_FILE:ow-package>)
add_test(NAME mission-script-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/mission_script_test.py $<TARGET_FILE:ow-package>)
add_test(NAME combat-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/combat_test.py $<TARGET_FILE:ow-package>)
add_test(NAME combat-loot-standin-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/combat_loot_standin_test.py $<TARGET_FILE:ow-package>)
add_test(NAME decisions-index-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/decisions_index_test.py)
add_test(NAME native-index-find-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/native_index_find_test.py)
add_test(NAME tool-schema-lines COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/schema_lines_test.py)
add_test(NAME audio-slice-chain-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/audio_slice_chain_test.py)
add_test(NAME prepare-slice-audio-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/prepare_slice_audio_test.py)
add_test(NAME decode-slice-audio-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/decode_slice_audio_test.py)
add_test(NAME check-audio-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/check_audio_test.py)
add_test(NAME listen-report-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/listen_report_test.py)
add_test(NAME ue-lock-machine-wide COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/ue_lock_test.py)
add_test(NAME audio-events-capture-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/audio_events_capture_test.py)
add_executable(ow-weapon-firing-test tests/weapon_firing_math_test.cpp)
target_link_libraries(ow-weapon-firing-test PRIVATE ow-core)
target_compile_features(ow-weapon-firing-test PRIVATE cxx_std_20)
add_test(NAME weapon-firing-math COMMAND ow-weapon-firing-test)
add_test(NAME weapon-firing-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/weapon_firing_test.py)
add_test(NAME weapon-projectile-fx-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/weapon_projectile_fx_test.py)
add_test(NAME grenade-mod-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/grenade_mod_test.py)
if(OPENWILLOW_LZO)
  add_test(NAME container-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/package_test.py $<TARGET_FILE:ow-package> --compressed)
  add_test(NAME assets-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/assets_test.py $<TARGET_FILE:ow-package>)
endif()
# Standalone Python tests (lane-ungated-tests, 2026-11-11). Each passed in a clean export of the repository (no local/, no build,
# no installed game, no network) under a Python with no third-party packages, as on CI. Registered as py-<name>, run from the
# source tree. A test is added here only after it passes that way without skipping its cases; docs/verification/GATES.md "Tests outside ctest" lists the
# ones left out and why:
#   numpy: in OW_NUMPY_PY_TESTS below, registered only where numpy imports
#   Pillow: in OW_PILLOW_PY_TESTS below, registered only where Pillow and numpy import
#   too slow for every PR, or refuses on a shared PC: in OW_OUTSIDE_CTEST below, each with its reason
#   tests/*_test.js: in OW_NODE_TESTS below, registered only where Node is found
set(OW_STANDALONE_PY_TESTS
  audio_census blcmm_dumps block_saves bsp_calibration bsp check_action_skill_hud check_hud_down
  check_hud_shots check_interactives check_mission_page check_overlay_data check_phaselock_kill check_shield_check
  controls_capture
  check_slice_gate check_vendors check_weapon_firing collision crate_edges crosscheck_blcmm_dumps crosscheck_bsp_polys
  crosscheck_umodel_assets crosscheck_umodel den_probe editor_inputs game_colours gen_confirm gestalt_sockets
  golden_card_compare gun_ads_capture inspection_views inventory_gear land_batch
  leak_gate leak_gate_hook leak_gate_policy leak_gate_push leak_gate_repo loot_pools
  material_assignment material_audit material_resource mission_camera_probe mission_screen_capture particle_system
  phaselock_attachment_check phaselock_integrity_probe prepare_action_skill prepare_hud_strings
  prepare_interactives prepare_inventory_movie prepare_stock_shield prepare_vendors private_sync range_kind_melee range_tools real_game_capture_safety real_game_finalizer_recheck
  real_game_loot_capture real_game_menus_paths real_game_migration real_game_minimap_paths real_game_pending realgame_registration realgame_reply_guard
  research_corpus sanctuary_geometry
  sanctuary_visual_rules serve_paths skill_effects skill_stats sky_census slice_watch slice_world
  terrain_geometry terrain_material terrain_probe terrain terrain_weightmap ui_trace_returns walking_routes
  weapon_assembly_oracle weapon_assembly_sample weapon_balance weapon_capture_items weapon_catalog weapon_library
  weapon_muzzle_fx weapon_parity2_probe weapon_parity_probe weapon_recipe weapon_stats weapon_view_model)
foreach(ow_test IN LISTS OW_STANDALONE_PY_TESTS)
  add_test(NAME py-${ow_test} COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/${ow_test}_test.py
           WORKING_DIRECTORY ${CMAKE_SOURCE_DIR})
  set_tests_properties(py-${ow_test} PROPERTIES TIMEOUT 120)
endforeach()
# land_batch runs the landing script in eleven throwaway repositories (about 100 s on the zuhu desktop).
set_tests_properties(py-land_batch PROPERTIES TIMEOUT 300)
# Three tests of the real-game tools (lane C, 2026-11-13): each starts Windows PowerShell for nearly every case, 43-72 s on the zuhu
# desktop (three runs each, 171 s together); the timeout leaves room for a slower runner.
set_tests_properties(py-controls_capture py-real_game_capture_safety py-real_game_pending PROPERTIES TIMEOUT 300)
# Tests left out of ctest, each with its reason on its line (seconds: serial, zuhu desktop, 2026-11-13 unless dated otherwise).
# Run them by hand when you touch what they cover. tests/realgame_registration_test.py fails when a test of the real-game tools
# (one that imports realgame_test_env or names a tools/real_game/ path) is neither registered above nor named here with a reason.
# A test added to a list above is taken out of this one; GATES.md "Tests outside ctest" mirrors it. Not a *_TESTS list on purpose:
# ow_require_test_groups reads every OW_*_TESTS list as one that must be registered.
set(OW_OUTSIDE_CTEST
  cash_capture        # too slow: 236-237 s, 100 cases; DenDownReplacementTest (128 s) and LoadWaitTest (27 s) spin through real time windows
  realgame_snapshot   # too slow: 147-148 s; its slowest functions (55, 33, 15, 12, 12, 11 s) are its main restore and hook-stop cases
  skills_live_probe   # too slow for the budget beside the three above: 60-61 s (two functions take 51 s of it)
  check_hud_minimap   # left out 2026-11-11 as too slow (about 30 s then); not re-measured by lane C
  provision_worktree  # left out 2026-11-11 as too slow (about 36 s then); not re-measured by lane C
  )

# Tests that need Node or numpy (lane ci-test-deps, 2026-11-11). CI installs both (.github/workflows/ci.yml). Elsewhere a group
# is registered only when configure finds its tool, and configure says so when it does not, so a machine without Node or numpy
# still runs everything else. Checking at configure time also keeps slice_npc_assets (which skips its only class without numpy)
# from passing having run nothing. Labels needs-node / needs-numpy; CI prints each entry's last lines to compare case counts.
function(ow_add_tests prefix label runner ext)
  foreach(ow_test IN LISTS ARGN)
    add_test(NAME ${prefix}-${ow_test} COMMAND ${runner} ${CMAKE_SOURCE_DIR}/tests/${ow_test}_test.${ext}
             WORKING_DIRECTORY ${CMAKE_SOURCE_DIR})
    set_tests_properties(${prefix}-${ow_test} PROPERTIES TIMEOUT 120 LABELS ${label})
  endforeach()
endfunction()
# CI configures with OW_REQUIRE_TEST_TOOLS=ON, so a group left out (Node not found, a Python that lacks the module) fails the job
# instead of passing with fewer tests. The check runs at the end of configure and covers every OW_<group>_TESTS list defined by
# then, so it needs no count and no change when a group is added or removed.
option(OW_REQUIRE_TEST_TOOLS "Fail configure when a group of tests that needs Node, numpy or Pillow cannot be registered" OFF)
function(ow_require_test_groups)
  get_cmake_property(ow_vars VARIABLES)
  foreach(ow_var IN LISTS ow_vars)
    if(ow_var MATCHES "^OW_.+_TESTS$" AND NOT ow_var STREQUAL "OW_STANDALONE_PY_TESTS")
      foreach(ow_test IN LISTS ${ow_var})
        if(NOT TEST js-${ow_test} AND NOT TEST py-${ow_test})
          message(FATAL_ERROR "OW_REQUIRE_TEST_TOOLS is ON and ${ow_test} (${ow_var}) is not registered: its tool is missing (message above)")
        endif()
      endforeach()
    endif()
  endforeach()
endfunction()
if(OW_REQUIRE_TEST_TOOLS)
  cmake_language(DEFER CALL ow_require_test_groups)
endif()
set(OW_NODE_TESTS
  hud_action_skill hud_fields hud_minimap hud_movie_loss hud_use_prompt_page hud_warp inventory_navigation inventory_preview_cache
  map_screen mission_log mission_screen pickup_card skill_info skill_navigation skill_rules status_page_keys vending_movie vending)
find_program(OW_NODE node)
if(OW_NODE)
  ow_add_tests(js needs-node ${OW_NODE} js ${OW_NODE_TESTS})
else()
  list(LENGTH OW_NODE_TESTS ow_count)
  message(STATUS "Node not found: the ${ow_count} tests in OW_NODE_TESTS are not registered (docs/verification/GATES.md)")
endif()
set(OW_NUMPY_PY_TESTS
  ambient_transform creature_zone lootable_tracks phaselock_target_attachments prepare_phaselock_target slice_npc_assets
  weapon_reload_anims weapon_reload_library listen_report_fit)
execute_process(COMMAND ${Python3_EXECUTABLE} -c "import numpy" RESULT_VARIABLE ow_numpy_rc OUTPUT_QUIET ERROR_QUIET)
if(ow_numpy_rc EQUAL 0)
  ow_add_tests(py needs-numpy ${Python3_EXECUTABLE} py ${OW_NUMPY_PY_TESTS})
else()
  list(LENGTH OW_NUMPY_PY_TESTS ow_count)
  message(STATUS "numpy does not import in ${Python3_EXECUTABLE}: the ${ow_count} tests in OW_NUMPY_PY_TESTS are not registered (docs/verification/GATES.md)")
endif()

# Tests that need Pillow (lane ci-test-deps commit 2, 2026-11-11; lands only with the maintainer's yes). sanctuary_pillar also
# needs numpy, so the group needs both. Label needs-pillow.
set(OW_PILLOW_PY_TESTS
  gfx_to_swf prepare_map_screen prepare_minimap prepare_mission_log prepare_mission_screen sanctuary_pillar weapon_paint
  weapon_part_census)
execute_process(COMMAND ${Python3_EXECUTABLE} -c "import numpy, PIL" RESULT_VARIABLE ow_pillow_rc OUTPUT_QUIET ERROR_QUIET)
if(ow_pillow_rc EQUAL 0)
  ow_add_tests(py needs-pillow ${Python3_EXECUTABLE} py ${OW_PILLOW_PY_TESTS})
else()
  list(LENGTH OW_PILLOW_PY_TESTS ow_count)
  message(STATUS "Pillow and numpy do not both import in ${Python3_EXECUTABLE}: the ${ow_count} tests in OW_PILLOW_PY_TESTS are not registered (docs/verification/GATES.md)")
endif()

# The Rust parity runner (lane W1-HARNESS, 2026-11-15; maintainer decision D15). py-rust_parity is the runner's self-test
# (synthetic manifests and stand-in binaries, stdlib only). rust-parity runs the real manifest, tests/rust_parity.d/*.txt, on
# the Rust release binary, with the C++ ow-package for its diff lines. It is registered only when the Rust binary exists at
# configure time (cargo build --release -p ow-cli --locked, then configure again); CI builds it before configuring, and with
# OW_REQUIRE_TEST_TOOLS=ON a missing binary fails the configure instead of leaving the entry out.
add_test(NAME py-rust_parity COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/rust_parity_test.py
         WORKING_DIRECTORY ${CMAKE_SOURCE_DIR})
set_tests_properties(py-rust_parity PROPERTIES TIMEOUT 120)
set(OW_RUST_PACKAGE ${CMAKE_SOURCE_DIR}/target/release/ow-package${CMAKE_EXECUTABLE_SUFFIX})
if(EXISTS ${OW_RUST_PACKAGE})
  add_test(NAME rust-parity COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tools/rust_parity.py --bin ${OW_RUST_PACKAGE}
           --cxx $<TARGET_FILE:ow-package> WORKING_DIRECTORY ${CMAKE_SOURCE_DIR})
  set_tests_properties(rust-parity PROPERTIES TIMEOUT 1800 LABELS rust)
  # Lane W4-REGISTRY: tools/item_registry.py on the C++ binary against the Rust reader and the Rust --item-registry,
  # byte for byte (the Rust registry comparison is reported skipped while DefStore::census() answers NotYet).
  add_test(NAME item-registry-rust COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/item_registry_rust_test.py
           $<TARGET_FILE:ow-package> ${OW_RUST_PACKAGE})
  set_tests_properties(item-registry-rust PROPERTIES TIMEOUT 300 LABELS rust)
  # Lane W4-ROLL: the Rust --item-roll against tools/item_roll.py (check and roll) and tools/weapon_recipe.py on the C++
  # reader. It pins the reference stand-ins, so it runs with OW_PARITY=reference, as its mode=reference manifest line
  # (tests/rust_parity.d/roll.txt) does under tools/rust_parity.py.
  add_test(NAME item-roll-rust COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/item_roll_rust_test.py
           $<TARGET_FILE:ow-package> ${OW_RUST_PACKAGE})
  set_tests_properties(item-roll-rust PROPERTIES TIMEOUT 300 LABELS rust ENVIRONMENT "OW_PARITY=reference")
  # Lane W4-STATS: the Rust --item-card against tools/weapon_gen_parity.python_gun and tools/prepare_stock_shield.stats on
  # the C++ reader. It pins the reference stand-ins, so it runs with OW_PARITY=reference, as its mode=reference manifest
  # line (tests/rust_parity.d/stats.txt) does under tools/rust_parity.py.
  add_test(NAME item-stats-rust COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/item_stats_rust_test.py
           $<TARGET_FILE:ow-package> ${OW_RUST_PACKAGE})
  set_tests_properties(item-stats-rust PROPERTIES TIMEOUT 300 LABELS rust ENVIRONMENT "OW_PARITY=reference")
  # Lane W4-POOLS: the Rust --item-pool against tools/loot_pools.py's CrtRand path on the C++ reader. The oracle's rules
  # are reproduced only in Reference mode, so it runs with OW_PARITY=reference, as its mode=reference manifest line
  # (tests/rust_parity.d/pools.txt) does under tools/rust_parity.py.
  add_test(NAME item-pools-rust COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/item_pools_rust_test.py
           $<TARGET_FILE:ow-package> ${OW_RUST_PACKAGE})
  set_tests_properties(item-pools-rust PROPERTIES TIMEOUT 300 LABELS rust ENVIRONMENT "OW_PARITY=reference")
  # Lane W5-DIALOG: the Rust --mover-probe and --mover-event against the C++ binary on an invented toy mover and Kismet
  # sequence (both binaries, byte for byte, with two differences checked on purpose; decision 2026-11-15-32).
  add_test(NAME mover-rust COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/mover_parity_test.py
           $<TARGET_FILE:ow-package> ${OW_RUST_PACKAGE})
  set_tests_properties(mover-rust PROPERTIES TIMEOUT 300 LABELS rust)
  # Lane W3-DEFS: --def-dump is Rust-only (the C++ binary has no such command), so this runs the Rust binary alone.
  add_test(NAME def-dump-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/def_dump_test.py ${OW_RUST_PACKAGE})
  set_tests_properties(def-dump-synthetic PROPERTIES TIMEOUT 120 LABELS rust)
elseif(OW_REQUIRE_TEST_TOOLS)
  message(FATAL_ERROR "OW_REQUIRE_TEST_TOOLS is ON and there is no Rust binary at ${OW_RUST_PACKAGE}: rust-parity is not registered (cargo build --release -p ow-cli --locked)")
else()
  message(STATUS "No Rust binary at ${OW_RUST_PACKAGE}: rust-parity is not registered (cargo build --release -p ow-cli --locked, then configure again)")
endif()
