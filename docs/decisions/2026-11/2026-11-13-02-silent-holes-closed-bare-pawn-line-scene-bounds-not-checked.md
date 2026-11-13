## 2026-11-13: Silent holes closed: bare pawn line, scene_bounds not checked, automation launcher

AI-assisted (Claude Code). Leftovers of the 2026-11-12 sweep (a tool or check keeps or refuses, it never degrades in silence,
and a result line names what it ran on).

- The host's `AttachWornMeshes` (`OpenWillowCombatTarget.cpp`) returned without a word when a stock pawn's entry had no
  `attachments` field, so old or broken prep data gave a bare enemy. It now logs a warning,
  `OpenWillow target: no attachments list on the stock pawn entry ...`, naming the import's attach step and
  `tools/prepare_range.py`. An empty list is still a valid answer for a pawn that wears nothing and logs `0 of 0` as before.
  `tools/check_range.py` (test, survey and kill logs) and `tools/check_slice_gate.py` fail a run that holds the line. Checked:
  synthetic logs in `tests/range_tools_test.py` and `tests/check_slice_gate_test.py`, which also hold the marker text to the
  host source. Not yet seen in an engine run: whether a real run logs the line is `UNVERIFIED` until the hub's suite run.
- `tools/prepare_slice_world.py` left `scene_bounds` out of `world.json` when `local/sanctuary/scene.json` was missing. It now
  prints a line and writes `scene_bounds` as `checked: false` with the reason; a checked record carries `checked: true`. A run
  without the scene file refuses, before the reader work, to replace a `world.json` holding checked bounds (or one it cannot
  read), unless `--drop-scene-bounds` is passed. Refusing rather than writing follows `tools/prepare_ambient_world.py`'s guard:
  nothing reads the field today, so the refusal blocks no suite, and writing would have lost the record's check without a sign.
  Checked: synthetic cases in `tests/slice_world_test.py`. Not run against the installed game.
- The `ue_automation` suite runs `tools/test_automation.ps1` instead of an inline command. The test names and the count live
  in one place, its `-Tests` and `-Expected` defaults; `tests/automation_paths_test.py` reads them there (and the printed and
  `-eq` counts from the launcher's own lines, the pass line's count from `tools/suites.txt`) and refuses a row that does not
  run the launcher or overrides them. The launcher prints the `Path=` of every completed test. Checked: the guard's synthetic
  cases and its run on this checkout (8 names, 13 tests), and the launcher's `-DryRun` under Windows PowerShell and pwsh. The
  in-engine run (`success=13 failed=0 expected=13`) is `UNVERIFIED` here until the hub's suite run.
- The unit row's comment in `tools/suites.txt` no longer asks for a fixed `#pass` count; the pass line takes any count.
- `python tools/suite_map_lint.py` with no arguments now reads the tracked `tools/suites_switch_only.txt` with the tracked map,
  as CTest `suite-map-tracked` did by its arguments; before, the plain command reported the list's nine pairs (loads behind
  switches the suites never pass, read again in the launchers) and exited 1. No suite row is widened. `suite-map-tracked`
  runs the plain command, and `py-suite_map_lint` checks that every pair in the list is still reported without it.
