## 2026-11-13: An unreadable reply file is a lost driver

AI-assisted (Claude Code). The session gate's review of the capture tools on main (round 3) found one gap in the driver-lost rule:
the condition latched only on an error that said "no reply", which `Invoke-GamePy` gave only for a reply that never came. A reply
file that was there but could not be read (a sharing, access or I/O error) failed with the file system's own text, so a helper that
swallows its errors (`Save-SliceWatchSnap 'smoke_unsettled'`, for example) did not latch, and the key after it still went out.

- `Invoke-GamePy` (`tools/real_game/realgame.ps1`) classifies the failure where it happens. A reply file that cannot be read is read
  at most four more times, 200 ms apart; then the call fails in the timeout's own form, `no reply to <name> (its reply file could not
  be read: <the error>)`. Every existing latch matches "no reply", so none of them changed. The retry reads only this call's own
  file: its name holds this call's GUID, and a file under that name was removed before the command went out. Nothing is taken from a
  failed read.
- The publish is classified the same way (found by CI on 2026-11-13, see below). `Invoke-GamePy` writes `<name>.py.tmp` and renames
  it onto `<name>.py`; a file scanner holding the new file can refuse the rename, which failed with the file system's text. The
  rename is now tried at most 5 times, 200 ms apart, and then fails as `no reply to <name> (the command could not be sent: ...)`.
  The command goes out at most once: the driver runs only `<name>.py`, and once the rename went through the source is gone.
- CI on PR #263's first head failed `test_an_unreadable_reply_to_a_probe_file_is_a_lost_driver` on both Windows jobs: after the
  loss came the finalizer's hook stop (`mission_screen_capture.ps1`, the `ms_stop` call in the `finally`), so the condition had not
  been set. The log did not show the run's own error. Locally, with a stand-in scanner that holds every new file in the command
  folder open for 300 ms, 46e0e6a's transport failed its publish in 18 of 20 calls with an error no latch matched, and the test's
  own stand-in failed its removal of the command file in 1 of 20. So the failure was a real transport path, or the stand-in's own
  race, or both; the log cannot tell which. Both are fixed: the publish as above, and the stand-in no longer touches any file the
  transport wrote. With the fix, the same scanner gives 20 of 20 calls that latch.
- The driver (`tools/real_game/openwillow_realgame/__init__.py`, `run_pending`) publishes each reply whole: it writes
  `<name>.out.tmp`, a name no reader looks for, then renames it onto `<name>.out` (atomic on one volume). Before, it wrote
  `<name>.out` in place, so a reader that saw the file appear could read it empty or cut off, which no retry can detect. Now a reply
  file is complete once it exists, an unreadable one is unexpected (the retry stays as a second line of defence), and an empty reply
  means the command printed nothing. If the write or the rename fails, the `.tmp` is removed and no reply is left: the reader's wait
  runs out and the run counts the driver as lost. The script was already renamed to `<name>.py.done` before it ran, so it never runs
  twice. A `.out.tmp` left by a game that died mid-write is harmless: nothing globs or counts it (the readers wait on `<name>.out`
  by name, the captures' clean-up and the driver remove only `*.py`, and the driver-folder check of `pending.ps1` looks at the
  driver's own folder, not the command folder), and every call has a new name. The events file is append-only and no reader waits
  on it, so it is left as it is.
- The waits that expect silence (the boot wait, a travel's map polls, `Wait-PlayerPawn`) still refuse once the condition is set and
  still do not set it, on a timeout or an unreadable reply alike: each is one missed read, and the next read asks again under its own
  bound. The retry adds at most 0.8 s to a failed read, and `Wait-PlayerPawn` clamps each read to 1 s less than its bound, so its
  bound still holds.
- Every tool that sources `realgame.ps1` gets the same change: an unreadable reply now fails after the retries with the "no reply"
  text, where it failed at once with the file system's text. What latches on "no reply" now latches on it too: `cash_capture` and
  `mission_screen_capture` (their `Py`, probe loads and quit), and the watch helpers (`slice_watch.ps1`), which `audio_events_capture`
  and `phaselock_integrity_capture` also call with a watch. Every other capture and `weapon_part_session.ps1` only sees a different
  error text after up to 0.8 s more; their quit handlers print it as before.

Docs: `docs/TOOLING.md`, the driver-lost paragraph; and two sentences put right without code changes: recorded inputs are cleared
entry by entry once each release went out (`Release-RecordedInputs`), and only `cash_capture` stamps its stages
(`mission_screen_capture`'s `Set-Stage` only records results).

Verified: the synthetic tests (stand-ins, no game), where a stand-in driver answers through the real transport with a reply file held
open with no sharing. A read failure at the `smoke_unsettled` snapshot, at a kill-loop `Py` and at the mission screen's probe-file
load each set the condition, and nothing is sent after it; the game is stopped by the run's own PID; and the cleanup, restore, saves
compare and pending record come out as on a timeout. Each test fails on 725ec72. In `tests/block_saves_test.py` (the driver's own
test file), `run_pending` over a stand-in command folder: the reply is written only under the `.tmp` name with no `.out` there yet,
the script is `.py.done` before it runs, and a refused rename leaves no reply and no `.tmp`, runs the next script, and runs neither
again; both fail on 46e0e6a's driver. `UNVERIFIED`: nothing here has run in the game, and
whether reply-file read errors happen there at all is not known.
