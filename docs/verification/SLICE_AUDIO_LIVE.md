# Which sound events the running game posts (slice audio; AI-assisted, Claude; 2026-11-06)

**State: script written and tested with the game stubbed. It has not run in the game, so this file holds no result yet.**

`tools/real_game/audio_events_capture.ps1` logs the game's sound calls (`scripts/audio_events.py`) in labelled windows: `idle` (baseline), `phaselock`, `fire_away`
(shots at the sky), `fire_hit` (shots at a target) and `pickup`. A window's `new` list holds events its baselines did not see. The result feeds the role map of
`tools/prepare_slice_audio.py` (feature/slice-audio).

- **Logging only.** The hooks are pre-call and record; they do not block or change a call. Output is game data under ignored `local/realgame/audio_events/`.
- **What it cannot see.** A function called from native code to native code passes no hook, so an absent event means "not seen here", not "not played". The run then ends `incomplete` for that stage.
- **What a result would mean.** One session, one map, one target and the gun the save holds (logged as `weapon`), which need not be the lent pistol. A `new` event occurred in that window; that does not prove it belongs to the action. It stays UNVERIFIED until compared in play.
- **Data protection.** The same backup, restore and exit rules as `phaselock_integrity_capture.ps1`. The cleanup tail is byte-identical, and a test checks that. All four roots are compared: SaveData, Config, `sdk_mods/settings`, SanitySaver.

Tests: `python tests/audio_events_capture_test.py` (synthetic; game stubbed; runs the capture's cleanup through the harness of `tests/realgame_snapshot_test.py`).
