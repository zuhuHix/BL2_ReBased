## 2026-11-12: Capture reads after the slice sessions

AI-assisted (Claude Code). The capture defects and missing reads that the analyses of the two slice sessions of 2026-11-12 named,
fixed in reads and reporting only: no change here alters what a run does to the game or sends to it as input.

- `cash_capture` and `scripts/cash_probe.py`: `down_state` tests the injured flag before the health (in the four downs of session 2
  the health already read 0 at the poll that saw the down, so every down was labelled `no_health`); the time after the down is
  logged as `den_down_health_back_s`, the health back in the holding cell, not standing. The readings loop skips a pickup that gave
  no reading and stops after six distinct failures in a row; each try records whether `TouchedPickupable` fired (an observer hook that
  counts calls and reads nothing) and the pickup's location and physics fields; deaths are counted apart from kill attempts (fields
  only). A labelled `drop` snapshot follows each kill; the watch's pickups part names each item's balance; the cash rows hold the
  items' part definitions.
- `mission_screen_capture` and `scripts/mission_screen_probe.py`: the camera is read with `GetPlayerViewPoint` and `GetFOVAngle` on
  the player's own controller (it has no `PlayerCamera`, so every camera row of session 1 was an error); a `camera` stage fails when
  the trace read nothing; labelled snapshots follow the accept and the reward.
- `scripts/slice_watch.py`: the `frame_controller` switch puts the player's own controller's `bCinematicMode`, `IgnoreMoveInput`
  and `IgnoreLookInput` in each frame row, as fields; the AI pawn read carries `Velocity`. `slice_watch_check.py`: the `pawns` rule
  takes a `name`, so a named NPC's own clean reads are counted.

The watch's properties stay as they were: nothing is called on a scanned object, hooked functions are read by their parameters only,
pools fail closed, nothing is scanned before the map settles, and recordings are streamed and capped. The calls added are on the
player's own controller only (`GetPlayerViewPoint`, `GetFOVAngle`) and in the cash probe's existing scans (`cp_died` reads fields of
the pawn it finds by name).

Verified: the synthetic tests (stand-ins, no game), each new one failing on the code before its fix. `UNVERIFIED`: every one of these
reads in the game, until a session runs them; whether `TouchedPickupable` fires for money pickups; whether the controller's fields
hold the cinematic and input-ignore state as named. Left out: a part with Maya's active skills (it needs a live read to find where
the game keeps them), any new input sequence, a kill while down, a fixed frame cap.
