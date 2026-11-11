# Gates and the current baseline

Update this file in the commit that changes a number. Paste real output in reports; do not copy these numbers.

**Baseline (2026-10-08, commit c33ed0c, second machine):** ctest 15/15; verify_packages 9 packages, 3,339/3,339 class
bodies exact; Fire event order self-test and 32 of 32 rules; test_quest 117/117 plus resume 11/11; test_mover 16/16;
test_inventory_actions 49 pass / 0 fail; verify_combat_pawn `--vm` 25 pass / 0 fail / 4 missing (the Defense Boost rows).

**After lane Sf (2026-10-09, S-f down state and respawn):** ctest 15/15 (combat-synthetic extended); verify_combat_pawn `--vm` 55 pass / 0 fail / 4 missing (the 25 earlier passes plus 30 down rows).

**After lane Sg (2026-10-09, S-g loot on death):** ctest 15/15 (combat-synthetic extended); verify_combat_pawn `--vm` on the S-a to S-f run still 55 pass / 0 fail / 4 missing (of 59); with the loot run's JSON added (`--vm all_rows.json loot.json`, [COMBAT_LOOT.md](COMBAT_LOOT.md)) 97 pass / 0 fail / 4 missing / 1 open (the live 3-item money drop) of 102.

**After the Weapons momentum/sweep change (2026-11-03, pc-hummer, on main 95e48fb):** ctest 18/18 (one test fewer:
`weapon_refresh_firing` retired); verify_packages 9 packages, all class bodies exact; `tools/test_weapon_firing.ps1` 14 PASS /
0 FAIL with the new push, wide and flight-timing checks (WEAPON_PROJECTILES.md section 5).

**After the HUD lane (2026-11-03, pc-hummer, main 95e48fb + feature/hud-integrity, Fight For Your Life in the UE host):** ctest 19/19;
verify_packages 9 packages, all class bodies exact. `test_quest` now has 112 checks; the new `player_death_goes_down_first` and the respawn
after the 20 s bleed-out pass. `run_hud_shots.ps1 -Down` 10/10, `-Tracker` 4/4, default 16/16; `test_action_skill_hud` 19/19;
`test_hud_minimap` 15/15. Rebased on main 490661e (PR #61) with a regenerated audio manifest: ctest 21/21, `test_quest` first run
112/112 and resume 11/11. On main 542d0c6 (PR #62): ctest 20/20, `test_quest` 112/112 + resume 11/11, `-Down` 10/10, `-Tracker` 4/4,
`test_weapon_firing` 14/14 (with the local slice recipes' `firing` block regenerated).

**Slice gate (2026-11-04, pc-hummer, main be94f4a + feature/slice-gate):** `tools/test_slice_gate.ps1` 9 PASS / 0 FAIL /
4 NOT_AVAILABLE of 13 links (the gaps are pieces other lanes own; [SLICE_GATE.md](SLICE_GATE.md)); ctest 21/21.
On main 8bb74a0 (after #88): 10 PASS / 0 FAIL / 3 NOT_AVAILABLE (the pickup link now checks the pickup card); ctest 22/22.
On main 195a016: 10 PASS / 0 FAIL / 4 NOT_AVAILABLE of 14 (new decoy-pickup link); ctest 22/22.
On main 7eee9ee (2026-11-07, zuhu, Z23): 14 PASS / 0 FAIL / 0 NOT_AVAILABLE of 14 in four runs (no switch twice, `-Shield`, `-DownAfterFight`); ctest 28/28; verify_packages exact; range 36 / 0; firing 14 of 14; interactives 11 / 0; mission page 21 of 21 on its rerun (the first run failed 4 rows with the movie unavailable). `-Tracker` 3 of 4 (`tracker_page` saw 6 of 7 states) on this PC.

**Ungated tests registered (2026-11-11, zuhu, main c66f24e + fix/ungated-tests):** ctest 119/119 (34 before; the 85 new `py-*` entries took 76 s of the 211 s run); after review r1, 118 (84 `py-*`, `slice_npc_assets` moved out); verify_packages 9 packages, all class bodies exact. The tests left out are in "Tests outside ctest" below.

## Every change
- Build (Windows; CMake ships with VS Build Tools):
  `export PATH="/c/Program Files (x86)/Microsoft Visual Studio/18/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"`,
  then `cmake --build build --config Release --parallel`. A fresh configure uses the toolset Unreal uses:
  `cmake -S . -B build -T version=14.50`.
- `ctest --test-dir build -C Release --output-on-failure`
- `python tools/verify_packages.py --reader build/Release/ow-package.exe`

## When the VM, missions or the slice change
- `python tools/verify_fire_event_order.py --self-test` and `python tools/verify_fire_event_order.py --reader build/Release/ow-package.exe`
- Compare `--slice-run` output before and after against a copy of the previous build, and explain every difference.

## When combat changes
- `python tools/verify_combat_pawn.py --self-test`, and `--vm <json>` on a fresh `--combat-run`.
- `python tools/verify_combat_pawn.py --live` checks the formulas against ignored live dumps when they exist.

## When the host or anything it reads changes (UE; one process at a time)
- UE module build: `powershell -File local/p2/locked.ps1 -Owner <lane> -Command build` (or the build step of the suite scripts).
- `tools/test_quest.ps1`, `tools/test_mover.ps1`, `tools/test_inventory_actions.ps1`. Rerun the inventory suite after any change to
  `tools/hud_overlay/inventory.js` or the HUD page report. Report pass and fail counts honestly, including steps that fail only
  because local data is missing. Run the inventory suite with `-Items local/inventory-actions/bounded-recipes` (the six-weapon set the
  baseline uses): a larger local set can fill the backpack, and the suite then refuses to start (fewer than 2 free rows after its fixture).
  Since 2026-11-04 (inventory round 24) the suite has 50 steps (number keys no longer pick a transfer slot; `locked_slot_transfer_cancel`
  added); baseline 50 pass / 0 fail.

## Records
- `python tools/decisions_index.py --check` after moving decisions between files.

## Tests outside ctest
Since 2026-11-11 every `tests/*_test.py` that passes in a clean export (no `local/`, no build, no installed game, no network) under
a Python with no third-party packages, with none of its cases skipped, runs in ctest as `py-<name>` (the list at the end of `CMakeLists.txt`). CI also runs
`level_test.py` and `viewer_test.py` directly. The rest run only by hand (`python tests/<name>` or `node tests/<name>`); run those
that cover what you touched. Times are serial on the zuhu desktop, 2026-11-11.

| Tests | Why not in ctest | Status on 2026-11-11 |
|---|---|---|
| `ambient_transform`, `creature_zone`, `lootable_tracks`, `phaselock_target_attachments`, `prepare_phaselock_target`, `slice_npc_assets`, `weapon_reload_anims`, `weapon_reload_library` | need numpy; CI's Python has none (adding it is a maintainer decision). `slice_npc_assets` skips its whole class without numpy and exits 0, so in ctest it would pass on CI having run nothing | pass with numpy 2.5 |
| `gfx_to_swf`, `prepare_map_screen`, `prepare_minimap`, `prepare_mission_log`, `prepare_mission_screen`, `sanctuary_pillar`, `weapon_paint`, `weapon_part_census` | need Pillow; CI's Python has none | pass with Pillow (`prepare_mission_log` skips one case without the symlink privilege) |
| `realgame_snapshot` (144 s), `real_game_pending` (74 s), `real_game_capture_safety` (70 s), `skills_live_probe` (65 s), `controls_capture` (52 s), `cash_capture` (49 s), `provision_worktree` (36 s), `check_hud_minimap` (30 s) | too slow for every PR: about 520 s together | pass |
| `run_ue_flash_hud` | refuses (exit 2, `NOT RUN`) while any Unreal editor runs on the PC, so a shared PC fails it | not run here: an editor was up |
| all 18 `*_test.js` (`hud_*`, `inventory_*`, `map_screen`, `mission_log`, `mission_screen`, `pickup_card`, `skill_*`, `status_page_keys`, `vending*`) | need Node; CI does not install it (a maintainer decision) | 18/18 pass with Node 26.4 |

Notes: a test that skips cases there, or skips all of them, does not count as passing. Four registered tests skip cases off Windows
(`check_overlay_data`, `lootables_editor`, `private_sync`, `run_limit`: junctions, PowerShell launchers); CI runs
on Windows, so they run there. `serve_paths` skips nothing: it makes a directory symlink, and on Windows falls back to a junction when that is refused. `private_sync` also skips its UNC alias case without an administrative share. `game_colours` and
`prepare_hud_strings` create `local/` in a clean checkout (the second leaves it empty); it is git-ignored and harmless. The slowest registered entry
that starts processes, `run_limit` (about 40 PowerShell runs), took 11.2 s on CI (PR #219, run 43062806521) and 15.4 s here, well under
its 120 s timeout. On that CI run (Python 3.12) the new entries took 66 s of 172 s with LZO on.
