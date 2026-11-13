## 2026-11-13: The driver refuses an expired command and every capture uses one reply wrapper

AI-assisted (Claude Code). Before the approved capture session, a second-model review of the capture tools on main said NO-GO with two
blockers. Both were found again after six review rounds of the same transport, each finding one more case of two classes: a command
acting after its caller gave up, and one capture script not following the lost-driver rule the others follow. This change closes the
two classes where they start, not only the cases listed. A second review round of the same day (three blockers, one in the tools and
two in the private session wrapper, plus smaller items) is folded into this file, which is not on main yet.

Edits: docs/decisions/2026-11/2026-11-13-02-an-unreadable-reply-file-is-a-lost-driver.md

That entry's last round says the deadline is checked "before each publication attempt" so that "no command is published after it".
That was not true: when writing the command's temporary file outlasted the deadline, the rename that publishes it still ran, and the
driver ran whatever was queued with no expiry, so a command its caller had given up on could run later. The file is left as it was;
what holds after this change is below.

**B1, the deadline is the receiver's to enforce.**
- One clock for every deadline, on both sides: the command clock, the performance counter (`QueryPerformanceCounter`) in
  microseconds. The sender reads it through .NET's `Stopwatch` (`Get-CommandClockUs`, `realgame.ps1`), the driver through the Windows
  API with `ctypes`, which the SDK's Python 3.14 ships (`_ctypes.pyd` and `ctypes` in the installed SDK, listed read-only); without
  `ctypes` it would read `time.perf_counter`, which CPython takes from the same counter (here the two readings differed by 0.9 us, the
  gap between them). The resolution is the counter's frequency, 10 MHz on the zuhu desktop; the deadline is kept in whole microseconds.
  The counter is monotonic and shared by every process on the PC. The first round used the wall clock on both sides, and a clock
  stepped back after the sender gave up put the queued command inside its deadline again (the second review reproduced it with the
  synthetic driver). A tick-count fallback, which that review offered, is not used: the sender would have to read the same tick count,
  and the counter is there on both sides. The den down's re-placement window (`dp_replace`'s `until`) moved to the same clock. Since the third
  review round, both den-down windows (the down's and the respawn's) take their end once, on that clock, when they start, and the loop,
  each poll's wait, the re-placement's `until` and the pauses all count down to it; before, each re-placement converted the wall-clock
  time left, and the loop counted wall-clock time, so a clock stepped back lengthened the window. The load step `Wait-PlayerPawn` counts its
  menu, load and silence bounds on the same clock (its one wall-clock read places the caller's Enter, taken just before the call).
- Every command the sender publishes (`Invoke-GamePy`) starts with the line `# ow-deadline-qpc-us: <us>`, the call's deadline on the
  command clock.
- The driver (`tools/real_game/openwillow_realgame/__init__.py`, `run_pending`) checks that line once, when it takes a command and
  before it runs it. A command taken at or after its deadline (equality included; the first round compared floored milliseconds with
  `>`), or that names none, an unreadable one or one in the old wall-clock form, is not run: it is renamed `<name>.py.expired` and its
  reply is the one line `OW-REFUSED: not run: <why>`. The folder shows it, a test can read it, and a sender still waiting turns it into
  "no reply to <name> (the driver refused it: ...)", which the lost-driver latch takes.
- A call that gives up after its command went out (its deadline, or any transport error while waiting) withdraws the command by
  renaming it `<name>.py.withdrawn`, which the driver never lists; if the withdrawal lands between the driver's listing and its own
  rename, the driver finds no file and skips it. So no command starts after its deadline, and a withdrawn command the driver had not
  yet taken never runs.
- The sender checks the deadline again after the command's file is written, immediately before the rename that publishes it; a write
  that ends after it is removed and not renamed. `Invoke-GamePyFile` starts the call's deadline before it reads the probe's source and
  hands that same deadline to `Invoke-GamePy`, so a slow read counts against the timeout.
- Not bounded by these tools, named here and in `docs/TOOLING.md` ("Driver lost") only: a command the driver took before its deadline
  runs to its end, even long past the deadline (the driver cannot stop Python code on the game thread); a withdrawal can lose the race
  with the driver (its rename to `.py.done` lands first, and the command runs within its deadline although its caller stopped waiting);
  a withdrawal that fails for any other reason cancels nothing and is not reported, so the command stays queued and runs if the driver
  takes it before its deadline; a file operation under way when the deadline passes finishes, so a call can end later than its timeout
  by that operation plus at most one pause; and a rename that began just before the sender's last check can still publish a command
  (the driver then refuses it at its deadline, or a withdrawal removes it first). Nothing tracked here stops a command already running
  in the game; that, and stopping a capture that outlives its item, is the session runner's, which is not in the tree.
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
- `tests/realgame_reply_guard_test.py` (ctest `py-realgame_reply_guard`) fails a script (every `.ps1` and `.psm1` under
  `tools/real_game/` but `realgame.ps1` and `pending.ps1`, in any letter case) that calls `Invoke-GamePy` or `Invoke-GamePyFile`, calls
  `ConvertTo-ReplyText`, defines a piece of the wrapper, or writes the condition or the transport's entry token, by assignment or a
  `*-Variable` cmdlet. Its exception list, kept in the style of `OW_OUTSIDE_CTEST`, is empty; an entry that no longer breaks the rule
  fails it. A text check cannot see a name built at run time, so the run time closes the class: the raw transport refuses (nothing
  written) unless the wrapper entered it, through a script-scope token the wrapper sets for its call, and a second dot-source of
  `realgame.ps1` keeps a condition already set. `Wait-PlayerPawn` goes through the wrapper too, as a silence wait that takes an empty
  reply, as before.

**Smaller items of the same review.** The watch's start (`Start-SliceWatch`) refuses an empty, white-space or nothing-parsing answer and
leaves the watch off, so its step fails; loading the recorder, which prints nothing, passes `-AllowEmpty` explicitly. `ConvertTo-ReplyText`
refuses a reply of only white space like an empty one. `walking_capture` no longer takes an empty boot reply for success or an empty
route status for the route's end. From the second round: `hud_layout_capture`'s quit at the end of each resolution allows silence
(every resolution has a game of its own, stopped by its PID before the next starts), so a quit the game did not answer no longer keeps
the next resolution from talking to its game; `weapon_part_session` loads its probe through the file call (with a `-Prelude` for its
recipe list), so the source's read is inside the deadline and the boundary.

Verified on stand-ins only (no game was started). `tests/block_saves_test.py`: the driver refuses a command taken a microsecond after
its deadline and one taken at it, and one with no, an unreadable or a wall-clock deadline, and runs none of them then or later, while one
in time runs; a command the real sender published with a 1 s timeout, taken 10 s later on the command clock while the driver's wall
clock reads an hour earlier, is refused (this case fails on the first round's head 0b78a6f, where it ran); a command withdrawn between
the listing and the rename is skipped and the next one runs; the sender's deadline line is the one the driver reads.
`tests/real_game_migration_test.py`: no rename after a write that outlasts the deadline (a virtual clock); the probe source's read inside
the deadline; a late reply not taken for the next command; white-space replies refused; the wrapper and the input helpers refusing after
a latch; a raw call by a name built at run time refused with nothing written; a second dot-source keeping the condition; a command
withdrawn after an early transport error, and one the driver took left alone. `tests/real_game_capture_safety_test.py`: the Phaselock
capture, run whole on a stand-in game whose watch start times out and which answers every later command, sends no further command and no
key, and still restores everything. `tests/slice_watch_test.py`: an empty or white-space watch start leaves the watch off and fails its
step. The first round's cases fail on main 8c08bc6 except the late-reply test, which main already passed. The existing real-game test
files pass with the change. `UNVERIFIED`: none of this has run in the game, and the counter's agreement between the game's process and
PowerShell is shown on this PC with a test Python, not with the game's own.
