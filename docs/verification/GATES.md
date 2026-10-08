# Gates and the current baseline

Update this file in the commit that changes a number. Paste real output in reports; do not copy these numbers.

**Baseline (2026-10-08, commit c33ed0c, second machine):** ctest 15/15; verify_packages 9 packages, 3,339/3,339 class
bodies exact; Fire event order self-test and 32 of 32 rules; test_quest 117/117 plus resume 11/11; test_mover 16/16;
test_inventory_actions 49 pass / 0 fail; verify_combat_pawn `--vm` 25 pass / 0 fail / 4 missing (the Defense Boost rows).

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
  because local data is missing.

## Records
- `python tools/decisions_index.py --check` after moving decisions between files.
