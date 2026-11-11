cmake_minimum_required(VERSION 3.20)
if(POLICY CMP0135)
  cmake_policy(SET CMP0135 NEW)
endif()
project(OpenWillow LANGUAGES C CXX)
option(OPENWILLOW_LZO "Enable the MIT-licensed LZO decoder" ON)
if(OPENWILLOW_LZO)
  add_library(ow-lzokay STATIC third_party/lzokay/lzokay.cpp)
  target_include_directories(ow-lzokay PUBLIC third_party/lzokay)
  target_compile_features(ow-lzokay PRIVATE cxx_std_14)
endif()
add_library(ow-core STATIC src/package.cpp src/assets.cpp src/container.cpp src/natives.cpp src/script.cpp src/vm.cpp src/interp.cpp src/natives_core.cpp src/vm_sweep.cpp src/inventory_navigation.cpp src/mover.cpp src/kismet.cpp src/mission.cpp src/dialog.cpp src/mission_script.cpp src/combat.cpp src/combat_hit.cpp src/combat_death.cpp src/combat_xp.cpp src/combat_down.cpp src/combat_loot.cpp src/combat_run.cpp src/progression.cpp src/behavior.cpp src/slice.cpp src/census.cpp src/weapon_gen.cpp src/weapon_firing.cpp)
target_include_directories(ow-core PUBLIC src)
add_executable(ow-package src/cli.cpp)
target_link_libraries(ow-package PRIVATE ow-core)
add_executable(ow-weapongen src/weapon_gen_cli.cpp)
target_link_libraries(ow-weapongen PRIVATE ow-core)
if(OPENWILLOW_LZO)
  target_link_libraries(ow-core PUBLIC ow-lzokay)
  target_compile_definitions(ow-core PUBLIC OW_LZO=1)
endif()

target_compile_features(ow-core PUBLIC cxx_std_20)
target_compile_features(ow-package PRIVATE cxx_std_20)
target_compile_features(ow-weapongen PRIVATE cxx_std_20)
if(MSVC)
  target_compile_options(ow-core PRIVATE /W4 /permissive-)
  target_compile_options(ow-package PRIVATE /W4 /permissive-)
  target_compile_options(ow-weapongen PRIVATE /W4 /permissive-)
else()
  target_compile_options(ow-core PRIVATE -Wall -Wextra -Wpedantic)
  target_compile_options(ow-package PRIVATE -Wall -Wextra -Wpedantic)
  target_compile_options(ow-weapongen PRIVATE -Wall -Wextra -Wpedantic)
endif()
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
#   numpy or Pillow (CI's Python has neither): ambient_transform, creature_zone, gfx_to_swf, lootable_tracks,
#     phaselock_target_attachments, prepare_map_screen, prepare_minimap, prepare_mission_log, prepare_mission_screen,
#     prepare_phaselock_target, sanctuary_pillar, slice_npc_assets, weapon_paint, weapon_part_census, weapon_reload_anims,
#     weapon_reload_library (slice_npc_assets skips its only test class without numpy, so it would pass on CI having run nothing)
#   too slow for every PR (30-145 s each on the zuhu desktop): cash_capture, check_hud_minimap, controls_capture,
#     provision_worktree, real_game_capture_safety, real_game_pending, realgame_snapshot, skills_live_probe
#   refuses while any Unreal editor runs on the PC (exit 2, "NOT RUN"): run_ue_flash_hud
#   tests/*_test.js need Node, which CI does not install.
set(OW_STANDALONE_PY_TESTS
  audio_census blcmm_dumps block_saves bsp_calibration bsp build_phaselock_fx_constants check_action_skill_hud check_hud_down
  check_hud_shots check_interactives check_mission_page check_overlay_data check_phaselock_kill check_shield_check
  check_slice_gate check_vendors check_weapon_firing collision crosscheck_blcmm_dumps crosscheck_bsp_polys
  crosscheck_umodel_assets crosscheck_umodel den_probe editor_inputs game_colours gen_confirm gestalt_sockets
  golden_card_compare gun_ads_capture import_phaselock_fx_source inspection_views inventory_gear loot_pools lootables_editor
  material_assignment material_audit material_resource mission_camera_probe mission_screen_capture particle_system
  phaselock_attachment_check phaselock_fx_inputs phaselock_integrity_probe prepare_action_skill prepare_hud_strings
  prepare_interactives prepare_inventory_movie prepare_vendors private_sync range_tools real_game_finalizer_recheck
  real_game_loot_capture real_game_menus_paths real_game_migration real_game_minimap_paths run_limit sanctuary_geometry
  sanctuary_host_flags sanctuary_visual_rules serve_paths skill_effects skill_stats sky_census slice_world
  start_shield terrain_geometry terrain_material terrain_probe terrain terrain_weightmap ui_trace_returns walking_routes
  weapon_assembly_oracle weapon_assembly_sample weapon_balance weapon_capture_items weapon_catalog weapon_library
  weapon_muzzle_fx weapon_parity2_probe weapon_parity_probe weapon_recipe weapon_stats weapon_view_model)
foreach(ow_test IN LISTS OW_STANDALONE_PY_TESTS)
  add_test(NAME py-${ow_test} COMMAND ${Python3_EXECUTABLE} ${CMAKE_SOURCE_DIR}/tests/${ow_test}_test.py
           WORKING_DIRECTORY ${CMAKE_SOURCE_DIR})
  set_tests_properties(py-${ow_test} PROPERTIES TIMEOUT 120)
endforeach()
