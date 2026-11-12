## 2026-11-11: Play-slice launcher, review round 1: exit 3 for a run that never launched, `-NoRange`, the save deleted under the lock (play-slice lane)

AI-assisted (Claude Opus 5.5). Follows the launcher entry above; PR #216 review round 1.

- **Exit codes.** A lock timeout, a refused or silent page-server port, or an editor that will not start now exits 3 with one `PLAY SLICE NOT STARTED` line that names which. Before, those exited 1, the code of a failed smoke item, so a runner could not tell "never launched" from "launched and failed".
- **Refusals.** The game check now needs `Binaries/Win32/Borderlands2.exe`, which the quest requires (`OpenWillowQuest.cpp`), and not just the folder. The two Python checks run with stderr allowed, so a warning from them no longer stops the script without a refusal line.
- **Save.** `-Fresh` and the smoke delete the save after the run lock is taken, so a second run waiting on the lock cannot delete the save of the session that holds it.
- **`-NoRange`.** The range's start switch moves Maya to the range on every launch. The host cannot build it in place (`ow.Range` moves her as well), and the quest save stores no position (no position field is written in `OpenWillowQuest.cpp`). `-NoRange` leaves the range out: there are no enemies, the range manifest is not needed, and the smoke skips its range item. No host change.
- **A stalled editor.** When it cannot be stopped, the lock is still released, as after Ctrl+C in hand play. The next `Enter-OwUeLock` waits on any running editor. Children it leaves are listed with `Write-OwLeftChildren` and are not stopped.
- **Record.** `SANCTUARY_RPG_MISSION.md` now says the launcher has not run and its smoke is pending. The loot rate on the route is marked UNVERIFIED in the host, and the sentence says the stock 0.085 is the guns-and-gear pool entry, not guns alone.
- **Checked.** `tests/play_slice_test.py` 20/20. The test folder's path has a space, and four runs go through lock, page server, start, timed stop and grading with a stub editor (pythonw through `-EditorExe`) and a stand-in for `ue_lock.ps1`. The launcher has still not run in Unreal (UNVERIFIED).

