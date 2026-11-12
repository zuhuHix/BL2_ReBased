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
# The input stamp that tools/preflight.ps1 -CheckBuild reads: build/Release/ow-core.lib.inputs (AI-assisted, Claude Opus
# 5.5, 2026-11-12). Target ow-build-stamp has no output, so it runs on every build, after ow-core, also when nothing was
# rebuilt: the stamp means "a build finished on these inputs", and a change the build rightly ignored (a CMakeLists.txt
# edit that changes no flag of ow-core) does not make the library look stale. The script between the brackets is written
# into the build folder at configure time and run there with cmake -P; tests/preflight_test.py runs it the same way.
# The stamp is text: a version line, the paths, the commit, how many `git status --porcelain` lines the paths had (not 0
# means the build saw uncommitted inputs, which the preflight reports as unknown), then their `git ls-files -s` lines.
# Only git runs, with --no-optional-locks. When git fails, no stamp is left, which the preflight reads as unknown.
# Keep PATHS equal to $stampPaths in tools/preflight.ps1.
file(WRITE "${CMAKE_BINARY_DIR}/ow_build_stamp.cmake" [==[
set(PATHS src third_party/lzokay CMakeLists.txt tests/CMakeLists.txt)
execute_process(COMMAND "${GIT}" --no-optional-locks -C "${REPO}" rev-parse HEAD
                OUTPUT_VARIABLE commit RESULT_VARIABLE rc_commit OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
execute_process(COMMAND "${GIT}" --no-optional-locks -C "${REPO}" status --porcelain -- ${PATHS}
                OUTPUT_VARIABLE status RESULT_VARIABLE rc_status ERROR_QUIET)
execute_process(COMMAND "${GIT}" --no-optional-locks -C "${REPO}" ls-files -s -- ${PATHS}
                OUTPUT_VARIABLE entries RESULT_VARIABLE rc_entries ERROR_QUIET)
if(rc_commit OR rc_status OR rc_entries)
  file(REMOVE "${OUT}")
  message(STATUS "build stamp: git failed in ${REPO}, no stamp written")
  return()
endif()
string(REGEX MATCHALL "[^\n]+" status_lines "${status}")
list(LENGTH status_lines dirty)
string(JOIN " " joined ${PATHS})
file(WRITE "${OUT}.tmp" "ow-build-stamp 1\npaths ${joined}\ncommit ${commit}\ndirty ${dirty}\n${entries}")
file(RENAME "${OUT}.tmp" "${OUT}")
]==])
find_package(Git QUIET)
if(GIT_FOUND)
  add_custom_target(ow-build-stamp ALL
    COMMAND "${CMAKE_COMMAND}" "-DGIT=${GIT_EXECUTABLE}" "-DREPO=${CMAKE_SOURCE_DIR}" "-DOUT=$<TARGET_FILE:ow-core>.inputs"
            -P "${CMAKE_BINARY_DIR}/ow_build_stamp.cmake"
    VERBATIM)
  add_dependencies(ow-build-stamp ow-core)
endif()
include(tests/CMakeLists.txt)
