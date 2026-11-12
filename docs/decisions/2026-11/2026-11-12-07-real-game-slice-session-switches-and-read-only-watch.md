## 2026-11-12: Real-game slice-session switches and read-only watch

AI-assisted (Claude Code). The capture tools of the two slice capture sessions. The sessions ran commit e7b5fa3 (run
locally as a627b82, the same files). The review fixes of #245 that follow it in this change came after the sessions.

- Four captures (`cash_capture`, `mission_screen_capture`, `phaselock_integrity_capture`, `audio_events_capture`) take
  `-WatchSpec` (the read-only recorder `tools/real_game/scripts/slice_watch.py` and its usability check
  `tools/real_game/slice_watch_check.py`) and `-SdkModsOff` / `-SdkModsExpected`. `cash_capture` adds `-DenDown` (the pack
  downs Maya with the run's immunity hook removed, with a bounded re-placement) and `-WatchSmoke` (the smoke item). The
  load step waits out the menu's online session within named bounds. All switches are off by default.
- The watch calls no native function on any object a scan returns, reads hooked functions by their parameters only, and is
  switched off per read kind after a first failure. This follows two game crashes on 2026-11-11, each soon after the
  watch's first pawn read; the cause among the candidates is not proven.
- `tools/real_game/pending.ps1` (a shared safety file) sends recorded input releases with or without a game window and clears
  an entry only after its release went out.
- Review fixes of #245 (two rounds), after the sessions: a pending record is replaced in one step, and after a failed rewrite
  either the old record or its `.tmp` is there, a leftover `.tmp` stopping the next run; an earlier run's record is completed,
  and its inputs released, only under the run lock, and a recovery checks the record as it is under its lock; every reply
  timeout, a shortened den-down poll's included, latches the driver-lost condition, after which nothing more is sent, not even
  the quit; each reply wait and pause is clamped to the time left in its bound, no den-down poll starts in a window's last 6 s,
  and no reply, placement or success is taken after its bound; the focus helper's Alt tap is journalled; three captures wait on
  their own game's exit; the checker counts every dropped labelled line, refuses an `edges` rule without values, and prints
  file names, not paths.
- `tests/slice_watch_test.py` joins ctest as `py-slice_watch`: 59 cases, about 7 s, passing in a clean export under
  `python -S`. Documentation: `docs/TOOLING.md`, "Slice-session switches"; `docs/verification/REALGAME_GROUND_TRUTH.md`,
  "Slice capture sessions (2026-11-12)".

Verified: the synthetic tests (invented objects, a stand-in SDK; no game). In the game, at commit e7b5fa3, two
unattended sessions on 2026-11-12 ran 9 item runs including the smoke item with 0 game crashes, and after every item the
saves, config, SDK settings and mods list were identical to the state before.
`UNVERIFIED`: what the captures were meant to measure (the cash readings failed in every cash run; three watch checks said
not usable); every hook that never fired in those sessions; the load wait's longer bounds (every load ended on its first
read); the den-down re-placement in the game: in the first S2.1 run one re-placement ran and placed Maya, and no down followed
within 150 s, while the runs that saw a down needed none, so whether a re-placement brings a down is not shown; Steam Cloud,
which the tools neither save nor restore. The review fixes of #245 came after the sessions and no session has run them, so their
behaviour in the game is `UNVERIFIED`: the smoke item runs first at the new commit.
