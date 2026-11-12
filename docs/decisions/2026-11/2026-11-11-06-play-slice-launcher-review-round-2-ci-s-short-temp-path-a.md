## 2026-11-11: Play-slice launcher, review round 2: CI's short temp path, a locked save, the test editor marked (play-slice lane)

AI-assisted (Claude Opus 5.5). PR #216 review round 2 and its CI failure; launcher and test only.

- **CI failure.** On the runner `%TEMP%` is an 8.3 short path (`RUNNER~1`). The test compared its own short-form save path with the launcher's, and the launcher passes every path in the long form. The CI log shows all of them as `runneradmin`, built from `$PSScriptRoot`. The test now resolves its temp folder to the long form once. A new test starts the launcher through the folder's short name and checks that the six paths reach the editor in the long form, and that the grading reads the same log path. This PC also gives the folder an 8.3 name, so the test ran here instead of skipping. The launcher was not changed for this.
- **Never launched.** A save that cannot be deleted (held open by another program) and an unresolvable game folder now exit 3 with `PLAY SLICE NOT STARTED: the save` (or `the game folder`). Before, they exited 1.
- **Test editor.** A run with `-EditorExe` prints `Note: test editor <path>` and tags its verdict `(test editor, not Unreal)`. A missing one is refused by its own name.
- **Page server id.** While the server it started runs, the launcher records the server's id in `local/playslice/page-server.pid`, and deletes the file once the server is stopped. If the test's timeout kills the launcher before its `finally`, the test's cleanup stops that id. It first checks that the command line holds `serve.py`, the port and the test folder, never the image name.
- **Checked.** `tests/play_slice_test.py` 23/23. The launcher has still not run in Unreal (UNVERIFIED).

