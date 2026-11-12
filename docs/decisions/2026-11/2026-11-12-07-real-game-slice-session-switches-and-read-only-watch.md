## 2026-11-12: Real-game slice-session switches and read-only watch

AI-assisted (Claude Code). The capture tools the two slice capture sessions used, published as they ran.

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
- `tests/slice_watch_test.py` joins ctest as `py-slice_watch`: 55 cases, about 7 s, passing in a clean export under
  `python -S`. Documentation: `docs/TOOLING.md`, "Slice-session switches"; `docs/verification/REALGAME_GROUND_TRUTH.md`,
  "Slice capture sessions (2026-11-12)".

Verified: the synthetic tests (invented objects, a stand-in SDK; no game). In the game, at the same tool files, two
unattended sessions on 2026-11-12 ran 9 item runs including the smoke item with 0 game crashes, and after every item the
saves, config, SDK settings and mods list were identical to the state before.
`UNVERIFIED`: what the captures were meant to measure (the cash readings failed in every cash run; three watch checks said
not usable); every hook that never fired in those sessions; the load wait's longer bounds (every load ended on its first
read); the den-down re-placement in the game (0 re-placements were needed); Steam Cloud, which the tools neither save nor
restore.
