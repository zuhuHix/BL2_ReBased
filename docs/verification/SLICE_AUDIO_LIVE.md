# Which sound events the running game posts (slice audio; AI-assisted, Claude; 2026-11-06)

**State: script written and tested with the game stubbed. It has not run in the game, so this file holds no result yet.**

`tools/real_game/audio_events_capture.ps1` logs the game's sound calls (`scripts/audio_events.py`) in labelled windows: `idle` (baseline), `phaselock`, `fire_away`
(shots at the sky), `fire_hit` (shots at a target) and `pickup`. A window's `new` list holds events its baselines did not see; `tied` is the part of `new` that followed
enough of the action's moments within a short span (the game's own call when it fires often enough, else the key press; both are logged). A stage passes only on `tied`,
so an ambient sound that merely falls in the window does not count. The result feeds the role map of `tools/prepare_slice_audio.py` (feature/slice-audio).

- **Logging only.** The hooks are pre-call and record; they do not block or change a call. Output is game data under ignored `local/realgame/audio_events/`.
- **What it cannot see.** A function called from native code to native code passes no hook, so an absent event means "not seen here", not "not played". The run then ends `incomplete` for that stage.
- **What a result would mean.** One session, one map, one target and the gun the save holds (logged as `weapon`), which need not be the lent pistol. A `tied` event followed the action in time; that does not prove it belongs to the action or is "the" sound of it: a hit can also set off the enemy's own pain line, and one cast is a single sample. `event_overlap` names other windows that saw the same event. It stays UNVERIFIED until compared in play.
- **No pawn held.** It does not use the Phaselock recorder. Whether a cast locked comes from the skill's `LockTarget` call, and the target is looked up by name for each command.
- **Data protection.** The same backup, restore and exit rules as `phaselock_integrity_capture.ps1`. The cleanup tail is identical but for which hooks it stops, and a test checks that. All four roots are compared: SaveData, Config, `sdk_mods/settings`, SanitySaver. The run stops before any input unless `block_saves()` answers that it hooked the save functions, and key and mouse input refuse to go out unless the game is the foreground window.

Tests: `python tests/audio_events_capture_test.py` (synthetic; game stubbed; runs the capture's cleanup through the harness of `tests/realgame_snapshot_test.py`).
