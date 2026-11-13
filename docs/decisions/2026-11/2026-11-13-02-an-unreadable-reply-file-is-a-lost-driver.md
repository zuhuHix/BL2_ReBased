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
- What it does not cover: an empty or cut-off reply cannot be told from a complete one. The driver writes the reply file in place,
  with no length or end mark, so such a reply is still taken as read (the 50 ms wait after the file appears is the only guard).
  Telling them apart needs the driver to write through a rename, a change to the in-game side that no session has run.
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
compare and pending record come out as on a timeout. Each test fails on 725ec72. `UNVERIFIED`: nothing here has run in the game, and
whether reply-file read errors happen there at all is not known.
