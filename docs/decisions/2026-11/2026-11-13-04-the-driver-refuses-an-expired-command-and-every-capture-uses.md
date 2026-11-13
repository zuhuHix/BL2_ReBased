## 2026-11-13: The driver refuses an expired command and every capture uses one reply wrapper

AI-assisted (Claude Code). Before the approved capture session, a second-model review of the capture tools on main said NO-GO with two
blockers. Both were found again after six review rounds of the same transport, each finding one more case of two classes: a command
acting after its caller gave up, and one capture script not following the lost-driver rule the others follow. This change closes the
two classes where they start, not only the cases listed.

Edits: docs/decisions/2026-11/2026-11-13-02-an-unreadable-reply-file-is-a-lost-driver.md

That entry's last round says the deadline is checked "before each publication attempt" so that "no command is published after it".
That was not true: when writing the command's temporary file outlasted the deadline, the rename that publishes it still ran, and the
driver ran whatever was queued with no expiry, so a command its caller had given up on could run later. The file is left as it was;
what holds after this change is below.

**B1, the deadline is the receiver's to enforce.**
- Every command the sender publishes (`Invoke-GamePy`, `tools/real_game/realgame.ps1`) starts with the line
  `# ow-deadline-unix-ms: <ms>`: the call's own deadline in Unix milliseconds of this machine's wall clock, which the game's driver reads
  too (both run on the same PC).
- The driver (`tools/real_game/openwillow_realgame/__init__.py`, `run_pending`) checks that line once, when it takes a command and
  before it runs it. A command whose deadline has passed, or that names none or an unreadable one, is not run: it is renamed
  `<name>.py.expired` and its reply is the one line `OW-REFUSED: not run: <why>`. The folder shows it, a test can read it, and a
  sender still waiting turns it into "no reply to <name> (the driver refused it: ...)", which the lost-driver latch takes.
- The sender checks the deadline again after the command's file is written, immediately before the rename that publishes it; a write
  that ends after it is removed and not renamed. `Invoke-GamePyFile` starts the call's deadline before it reads the probe's source and
  hands that same deadline to `Invoke-GamePy`, so a slow read counts against the timeout.
- Not bounded, named plainly: a command the driver took before its deadline runs to its end, even past the deadline (the driver
  cannot stop Python code on the game thread); and a file operation under way when the deadline passes finishes, so a call can end
  later than its timeout by that one operation plus at most one pause, and a rename that began just before the last check can still
  land (the driver then refuses that command).
- A late reply is never taken for another command's: each command's name holds its own GUID, each call reads only its own name's
  reply, and a file under that name is removed before the command goes out. A test keeps this true.

**B2, one wrapper holds the lost-driver rule.**
- The condition and its helpers (`Get-DriverLostReason`, `Set-SliceWatchDriverLost`, `Assert-DriverAnswers`) moved from
  `slice_watch.ps1`, which only some captures load, to `realgame.ps1`, which all of them load. The variable keeps its name.
- `Invoke-GameReply` and `Invoke-GameReplyFile` (`realgame.ps1`) are the one way a capture sends a command: they refuse once the
  condition is set, set it on any transport failure, and refuse an empty or white-space reply unless the call passes `-AllowEmpty`.
  `-SilenceAllowed` is for the waits that expect the driver to be silent for a while (the boot wait, a travel's or a load's polls):
  their timeouts do not set the condition, as before, and they still refuse once it is set. `-NoTraceback` refuses a game-side
  traceback, as `cash_capture`'s `Py` did.
- Every capture script uses them: the five the sessions run (`cash_capture`, `phaselock_integrity_capture`, `audio_events_capture`,
  `mission_screen_capture`, `slice_watch.ps1`) and, the conversion being mechanical, the fourteen others with `walking_capture`
  among them. A script's `Py` or `PyJson` is now one line that hands the command on. Their probe loads and quits set the condition too
  now (before, only `cash_capture`, `mission_screen_capture` and the watch helpers did), and a quit's catch no longer says "quit sent"
  when the quit was refused to a lost driver.
- The input helpers of `realgame.ps1` refuse after the latch as well (`Assert-InputAllowed`): every key press, mouse press or move,
  wheel turn and focus attempt; releases always go out. So a capture that forgets its own `Assert-DriverAnswers` before an input, as
  `phaselock_integrity_capture` had none, still sends nothing.
- `tests/realgame_reply_guard_test.py` (ctest `py-realgame_reply_guard`) fails a capture script that calls `Invoke-GamePy` or
  `Invoke-GamePyFile`, calls `ConvertTo-ReplyText`, defines a piece of the wrapper or writes the condition. Its exception list, kept in
  the style of `OW_OUTSIDE_CTEST`, is empty; an entry that no longer breaks the rule fails it.

**Smaller items of the same review.** The watch's start (`Start-SliceWatch`) refuses an empty, white-space or nothing-parsing answer and
leaves the watch off, so its step fails; loading the recorder, which prints nothing, passes `-AllowEmpty` explicitly. `ConvertTo-ReplyText`
refuses a reply of only white space like an empty one. `walking_capture` no longer takes an empty boot reply for success or an empty
route status for the route's end.

Verified on stand-ins only (no game was started): the driver refuses a command taken a millisecond after its deadline, and one with no
or an unreadable deadline, and runs neither then or later, while one in time runs (`tests/block_saves_test.py`); the sender's deadline
line is the one the driver reads, and its refusal prefix the driver's; no rename after a write that outlasts the deadline (a virtual
clock); the probe source's read inside the deadline; a late reply not taken for the next command; white-space replies refused; the
wrapper and the input helpers refusing after a latch (`tests/real_game_migration_test.py`); the Phaselock capture, run whole on a
stand-in game whose watch start times out and which answers every later command, sends no further command and no key, and still
restores everything (`tests/real_game_capture_safety_test.py`); an empty or white-space watch start leaves the watch off and fails its
step (`tests/slice_watch_test.py`). Each of these fails on main 8c08bc6 except the late-reply test, which main already passed. The
existing real-game test files pass with the change. `UNVERIFIED`: none of this has run in the game; whether the game's wall clock and
the PowerShell side ever disagree on this PC is not known (both read the same system clock).
