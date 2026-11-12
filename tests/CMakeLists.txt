# Test registration for ctest. The top-level CMakeLists.txt pulls this file in with include(), so it runs in the top directory's
# scope: ${CMAKE_SOURCE_DIR} and relative source paths resolve as they did there, and ow-weapon-firing-test is still built to
# build/<config>/. Adding, removing or relabelling a test edits this file only, not the top CMakeLists.txt (which every in-engine
# suite counts as covered, because the Unreal module links the core it builds). Never add it with add_subdirectory(tests): the
# relative source path tests/weapon_firing_math_test.cpp would then resolve under tests/tests/ and stop the configure, and
# ow-weapon-firing-test would move to build/tests/<config>/.
# The core targets (ow-core, ow-package, ow-weapongen) and their sources and flags stay in the top CMakeLists.txt; this file holds
# test registration only. The in-engine suites do not count it as covered, so a core change made here would not make them due.
enable_testing()
find_package(Python3 COMPONENTS Interpreter REQUIRED)
add_test(NAME package-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/package_test.py $<TARGET_FILE:ow-package>)
add_test(NAME properties-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/properties_test.py $<TARGET_FILE:ow-package>)
add_test(NAME runtime-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/runtime_test.py $<TARGET_FILE:ow-package>)
add_test(NAME global-resolve-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/global_resolve_test.py $<TARGET_FILE:ow-package>)
add_test(NAME natives-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/natives_test.py $<TARGET_FILE:ow-package>)
add_test(NAME vm-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/vm_test.py $<TARGET_FILE:ow-package>)
add_test(NAME script-disasm-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/script_disasm_test.py)
add_test(NAME controls-keys COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/controls_keys_test.py)
add_test(NAME controls-check COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/check_controls_test.py)
add_test(NAME kismet-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/kismet_test.py $<TARGET_FILE:ow-package>)
add_test(NAME weapon-gen-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/weapon_gen_test.py $<TARGET_FILE:ow-weapongen>)
add_test(NAME weapon-gen-firing-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/weapon_gen_firing_test.py)
add_test(NAME weapon-gen-parity-logic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/weapon_gen_parity_test.py)
add_test(NAME behavior-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/behavior_test.py $<TARGET_FILE:ow-package>)
add_test(NAME mission-script-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/mission_script_test.py $<TARGET_FILE:ow-package>)
add_test(NAME combat-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/combat_test.py $<TARGET_FILE:ow-package>)
add_test(NAME decisions-index-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/decisions_index_test.py)
add_test(NAME native-index-find-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/native_index_find_test.py)
add_test(NAME tool-schema-lines COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/schema_lines_test.py)
add_test(NAME audio-slice-chain-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/audio_slice_chain_test.py)
add_test(NAME prepare-slice-audio-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/prepare_slice_audio_test.py)
add_test(NAME decode-slice-audio-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/decode_slice_audio_test.py)
add_test(NAME check-audio-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/check_audio_test.py)
add_test(NAME listen-report-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/listen_report_test.py)
add_test(NAME ue-lock-machine-wide COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/ue_lock_test.py)
add_test(NAME movie-hud-switch COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/movie_hud_test.py)
add_test(NAME play-slice-launcher COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/play_slice_test.py)
add_test(NAME audio-events-capture-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/audio_events_capture_test.py)
add_executable(ow-weapon-firing-test tests/weapon_firing_math_test.cpp)
target_link_libraries(ow-weapon-firing-test PRIVATE ow-core)
target_compile_features(ow-weapon-firing-test PRIVATE cxx_std_20)
add_test(NAME weapon-firing-math COMMAND ow-weapon-firing-test)
add_test(NAME weapon-firing-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/weapon_firing_test.py)
add_test(NAME weapon-projectile-fx-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/weapon_projectile_fx_test.py)
add_test(NAME grenade-mod-synthetic COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/grenade_mod_test.py)
add_test(NAME anim-import-frames COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/anim_import_frames_test.py)
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
#   too slow for every PR (30-145 s each on the zuhu desktop): cash_capture, check_hud_minimap, controls_capture,
#     provision_worktree, real_game_capture_safety, real_game_pending, realgame_snapshot, skills_live_probe
#   refuses while any Unreal editor runs on the PC (exit 2, "NOT RUN"): run_ue_flash_hud
#   tests/*_test.js: in OW_NODE_TESTS below, registered only where Node is found
set(OW_STANDALONE_PY_TESTS
  audio_census blcmm_dumps block_saves bsp_calibration bsp build_phaselock_fx_constants check_action_skill_hud check_hud_down
  check_hud_shots check_interactives check_mission_page check_overlay_data check_phaselock_kill check_shield_check
  check_slice_gate check_vendors check_weapon_firing collision crosscheck_blcmm_dumps crosscheck_bsp_polys
  crosscheck_umodel_assets crosscheck_umodel den_probe editor_inputs engine_default game_colours gen_confirm gestalt_sockets
  golden_card_compare gun_ads_capture import_phaselock_fx_source inspection_views inventory_gear land_batch
  leak_gate leak_gate_hook leak_gate_policy leak_gate_push leak_gate_repo loot_pools
  lootables_editor
  material_assignment material_audit material_resource mission_camera_probe mission_screen_capture particle_system
  phaselock_attachment_check phaselock_fx_inputs phaselock_integrity_probe preflight prepare_action_skill prepare_hud_strings
  prepare_interactives prepare_inventory_movie prepare_vendors private_sync range_kind_melee range_tools real_game_finalizer_recheck
  real_game_loot_capture real_game_menus_paths real_game_migration real_game_minimap_paths research_corpus run_limit sanctuary_geometry
  sanctuary_host_flags sanctuary_visual_rules serve_paths skill_effects skill_stats sky_census slice_world
  start_shield suite_map_lint suites_due terrain_geometry terrain_material terrain_probe terrain terrain_weightmap ui_trace_returns walking_routes
  weapon_assembly_oracle weapon_assembly_sample weapon_balance weapon_capture_items weapon_catalog weapon_library
  weapon_muzzle_fx weapon_parity2_probe weapon_parity_probe weapon_recipe weapon_stats weapon_view_model)
foreach(ow_test IN LISTS OW_STANDALONE_PY_TESTS)
  add_test(NAME py-${ow_test} COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/${ow_test}_test.py
           WORKING_DIRECTORY ${CMAKE_SOURCE_DIR})
  set_tests_properties(py-${ow_test} PROPERTIES TIMEOUT 120)
endforeach()
# preflight runs every case under Windows PowerShell 5.1 and under pwsh (about 50 s on the zuhu desktop).
set_tests_properties(py-preflight PROPERTIES TIMEOUT 300)
# land_batch runs the landing script in eleven throwaway repositories (about 100 s on the zuhu desktop).
set_tests_properties(py-land_batch PROPERTIES TIMEOUT 300)
# suites_due starts the batch driver under Windows PowerShell three times (the file takes about 40 s on the main desktop).
set_tests_properties(py-suites_due PROPERTIES TIMEOUT 300)
# The tracked suite map against this checkout: a PR that adds a load to a suite script without updating tools/suites.txt fails.
add_test(NAME suite-map-tracked COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tools/suite_map_lint.py
         --map ${CMAKE_SOURCE_DIR}/tools/suites.txt --switch-only ${CMAKE_SOURCE_DIR}/tools/suites_switch_only.txt
         WORKING_DIRECTORY ${CMAKE_SOURCE_DIR})

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
