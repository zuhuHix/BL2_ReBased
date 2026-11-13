## 2026-11-13: Real-game tool tests in ctest, and a guard on the rest

AI-assisted (Claude Code). Lane C of the hub run of 2026-11-13. The scripts under `tools/real_game/` drive the installed game with
the maintainer's saves. Their safety (backup, restore, nothing sent once the driver is lost) is shown only by stubbed tests under
`tests/`, and six of those files ran in no ctest entry, so CI never ran them. The six were `cash_capture`, `controls_capture`,
`real_game_capture_safety`, `real_game_pending`, `realgame_snapshot` and `skills_live_probe`. `audio_events_capture` was said to be
outside ctest too, but it was already registered as `audio-events-capture-synthetic`.

- **Now in ctest:** `py-controls_capture`, `py-real_game_capture_safety` and `py-real_game_pending`, with a 300 s timeout. That is
  three of the six, 171 s together. On the zuhu desktop each was timed three times, started the way ctest starts it, and the runs
  were 43.0-43.2 s, 57.4-58.5 s and 70.2-70.5 s.
- **Left out, measured the same way:** `cash_capture` (235.8-237.0 s), `realgame_snapshot` (146.7-147.7 s) and `skills_live_probe`
  (59.8-60.7 s). The budget was about 90 s per file and about 3 minutes for all additions together, and the three above take that
  3 minutes. `cash_capture` could be split: its 74 cases outside `DenDownReplacementTest` (128 s) and `LoadWaitTest` (27 s) take
  about 85 s. That subset does not fit beside the three, and an entry naming classes would silently leave out any class added
  later. Raising the budget to take that subset is the maintainer's call. `realgame_snapshot` and `skills_live_probe` run fixed
  lists of functions with no way to pick a subset, and their slowest functions are their main restore and hook-stop cases.
- **Portability:** all six passed in a shell with no `OPENWILLOW_*` or `OW_*` variable, PowerShell 7's own `PSModulePath` (as a
  `pwsh` CI step passes it on) and a working directory outside the checkout. Each took about the same time there as in the timed
  runs, and `audio_events_capture` passed there too. `tests/realgame_test_env.py` and the test files are unchanged.
- **Guard:** the outside list is now a CMake list, `OW_OUTSIDE_CTEST` in `tests/CMakeLists.txt`, with a reason on each entry. It
  replaces a comment. The new `tests/realgame_registration_test.py` (`py-realgame_registration`) fails when a test that imports
  `realgame_test_env` or names a `tools/real_game/` path is in no ctest entry and not in that list. It also fails when an entry
  has no reason or no file, or is registered whole as well. Its rule cases use invented names and CMake text.
- **Verified:** ctest 174/174 on the zuhu desktop (170/170 before). Not seen yet: the three new entries on GitHub's runner. The
  tests show that the stubbed scripts behave; they say nothing about a run against the game.
