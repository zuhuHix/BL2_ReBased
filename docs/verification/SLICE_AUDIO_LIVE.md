# Which sound events the running game posts (slice audio; AI-assisted, Claude; 2026-11-06)

**State: two runs, 2026-11-07. Candidates for two roles, one with a real limit; gun fire not seen through hooks; pickup not exercised. All of it UNVERIFIED by play, and nothing has been heard.**

## Results

**Conditions.** One session in Ice_P, one target, the save's shotgun (`SG_Jakobs_2_Uncommon`), not the lent pistol. **This is a modded install, not stock.** Other SDK mods were enabled, among them RTSE (its stat hooks sit on weapon
functions) and Dialog Skipper, with `Autoskip Dialog` on (per the mission lane's read), which blocks dialog events from friendly NPCs. So heard lines and any timing around lines are not stock. No role below comes from a dialog event, but a spoken reaction at the cast could be missing from these rows, and an absence here is weak evidence. A rule that rests on a live session should say so; this file does.

**Run 2 (at 5b338ef; exit 1, `incomplete: fire_away, pickup`; saves compared identical after the restore).** Times are seconds from the window's first row. Event names only, no audio.

| Window | Observed | Reading |
|---|---|---|
| `idle`, 3 s | no calls | Nothing ambient through these hooks in that span, so the baseline excluded nothing. Longer ambient sounds are not ruled out |
| `phaselock`, locked first try | 0.000 `PhaseLockTarget`, same millisecond on the player pawn `Ak_Play_FX_Player_Phaselock_Activate`. 0.694 `LockTarget`, next millisecond on the AI pawn `Ak_Play_FX_Player_Phaselock_Loop`. 4.595 on the AI pawn `..._Phaselock_Collapse`. 6.695 on the AI pawn `Ak_Play_FX_Maya_Phaselock_False_Start` | **`phaselock_cast` candidate: `..._Activate`**, which also matches PHASELOCK_STOCK_DATA.md. One cast is one sample. `False_Start` fits neither its name nor its moment: unexplained |
| `fire_away`, 3 clicks | 2 `PlayFiringSound` calls, no sound event | A true "not seen here": `PlayFiringSound` has no event argument and nothing else fired, so the shot's event takes a path these hooks do not see (presumably native; UNVERIFIED). **Says nothing about which fire sound plays.** The `fire` role's evidence stays the pistol's `FireSounds` data (SLICE_AUDIO_CHAIN.md) |
| `fire_hit`, 5 clicks | 3 `PlayFiringSound`; health 706.5 -> 416.7, target not gone. `Ak_Play_Imp_Bullets` 33 to 34 ms after 2 of the 3 shots, none after the first. `Ak_Play_Imp_Bullet_Casings` about 0.57 to 0.6 s after shots: not tied at the 0.5 s span | **`hit` candidate: `Ak_Play_Imp_Bullets`, with the limit below.** Absent for shots at the sky. The casings look like a casing landing (presumed) |
| `pickup` | Threw a gun, faced that item's pickup, pressed use twice: not taken, no hook calls | **Not exercised.** Why is unknown: the aim, the use path or the throw. The role stays unresolved; `Ake_Obj_Pickup.Obj_Pickup.*` are candidates by name only |

**The limit on `hit`.** The event comes from `WillowPhysicalMaterialProperty:PlayImpactEffectSound`, so it belongs to a surface and is not shown to be a hit on an enemy: shotgun pellets also strike the ground. That function takes a `Switch` argument the probe did not log, and in the banks this event is a switch container with **96 distinct sources**. Which one plays for an enemy is unknown, so a random pick could play any surface's sound. Reading the `Switch` in a later run would settle it. The `Activate` event is also a switch container, with one distinct source across 4 banks.

**Run 1 (at 8e0e8bf; no result).** A hook's `func` is a bound function, so reading `func.Name` threw on all 42 calls during the casts and shots, and the error was only counted. Its "not locked" and "no event" lines are empty for that reason. Fixed: a window with hook errors now fails its stage with the first error's text.

**The data protection held both times:** saves, config and the SDK's folders compared identical after the restore (`python-sdk.json` and Sanity Saver's `Save0008` changed and were restored both times), and no recovery was needed.

## The capture

`tools/real_game/audio_events_capture.ps1` logs the game's sound calls (`scripts/audio_events.py`) in labelled windows: `idle` (baseline), `phaselock`, `fire_away`
(shots at the sky), `fire_hit` (shots at a target) and `pickup`. A window's `new` list holds events its baselines did not see; `tied` is the part of `new` that followed
enough of the action's moments within a short span (the game's own call when it fires often enough, else the key press; both are logged). A stage passes only on `tied`,
so an ambient sound that merely falls in the window does not count. The result feeds the role map of `tools/prepare_slice_audio.py` (feature/slice-audio).

- **Logging only.** The hooks are pre-call and record; they do not block or change a call. Output is game data under ignored `local/realgame/audio_events/`.
- **What it cannot see.** A function called from native code to native code passes no hook, so an absent event means "not seen here", not "not played". The run then ends `incomplete` for that stage.
- **What a result would mean.** One session, one map, one target and the gun the save holds (logged as `weapon`), which need not be the lent pistol. A `tied` event followed the action in time; that does not prove it belongs to the action or is "the" sound of it: a hit can also set off the enemy's own pain line, and one cast is a single sample. `event_overlap` names other windows that saw the same event. It stays UNVERIFIED until compared in play.
- **No pawn held.** It does not use the Phaselock recorder. Whether a cast locked comes from the skill's `LockTarget` call, and the target is looked up by name for each command.
- **Data protection.** The same backup, restore and exit rules as `phaselock_integrity_capture.ps1`. The cleanup tail is #138's, identical but for which hooks it stops, and a test checks that and runs it on stubs. All four roots are compared: SaveData, Config, `sdk_mods/settings`, SanitySaver. The run stops before any input unless `block_saves()` answers that it hooked the save functions, and key and mouse input refuse to go out unless this run's own game is the foreground window. A game this run did not start stops the launch or the restore and holds the locks.

Tests: `python tests/audio_events_capture_test.py` (synthetic; game stubbed; runs the capture's cleanup through the harness of `tests/realgame_snapshot_test.py`).
