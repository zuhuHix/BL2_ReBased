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
