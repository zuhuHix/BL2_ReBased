## 2026-11-12: Capture reads after the slice sessions

AI-assisted (Claude Code). The capture defects and missing reads that the analyses of the two slice sessions of 2026-11-12 named,
fixed in reads and reporting. One exception changes what a run does in the game: the readings loop's skip list. Before, a pickup
that gave no reading was walked onto again, up to six times in a row, and then the loop stopped. Now it is walked onto once and
skipped: the next pass walks to another pickup or, with none left, starts another kill (the placement beside the enemy, its health
set to 1, three clicks). The loop goes on until six failures in a row, -MaxKills, -WantItems or -MaxMinutes. The kinds of action and
the limits are the same as before, but a run's kill count and length can differ from the sessions'.

- `cash_capture` and `scripts/cash_probe.py`: `down_state` tests the injured flag before the health (in the four downs of session 2
  the health already read 0 at the poll that saw the down, so every down was labelled `no_health`); the time after the down is
  logged as `den_down_health_back_s`, the health back in the holding cell, not standing. The readings loop skips a pickup that gave
  no reading (recorded when the try starts) and stops after six distinct failures in a row; each try records whether
  `TouchedPickupable` fired (an observer hook that counts calls and reads nothing, installed in every cash run, with or without a
  watch, outside the watch's hook groups and markers) and the pickup's location and physics fields; deaths are counted apart from kill attempts (fields
  only). A labelled `drop` snapshot follows each kill; the watch's pickups part names each item's balance; the cash rows hold the
  items' part definitions. A `drop` snapshot is asked for only when one read says the map has settled (an unsettled drop is
  counted), and `-MaxKills` is at most 150 so the drops leave room under the 200 labelled reads.
- `mission_screen_capture` and `scripts/mission_screen_probe.py`: the camera is read with `GetPlayerViewPoint` and `GetFOVAngle` on
  the player's own controller (it has no `PlayerCamera`, so every camera row of session 1 was an error); a `camera` stage fails when
  the trace read nothing or any of its rows is an error; labelled snapshots follow the accept and the reward.
- `scripts/slice_watch.py`: the `frame_controller` switch puts the player's own controller's `bCinematicMode`, `IgnoreMoveInput`
  and `IgnoreLookInput` in each frame row, as fields; the AI pawn read carries `Velocity`. `slice_watch_check.py`: the `pawns` rule
  takes a `name`, so a named NPC's own clean reads are counted (with a name, at least one read and one pawn are required).

The watch's properties stay as they were: nothing is called on a scanned object, hooked functions are read by their parameters only,
pools fail closed, nothing is scanned before the map settles, and recordings are streamed and capped. That fields-only rule binds the
watch and the den down's re-placement only. Three older probe paths, unchanged since e7b5fa3, still call native functions on objects
found by a scan or by name: `cb_enemies` (`_pawn_state`'s four health and shield getters on every live `WillowAIPawn_<n>` in its
radius, reached by `cp_target` and `dp_begin`), `cp_ready_kill` (`SetHealth(1.0)` on the kill target) and `_values` (the value
functions of a money pickup's inventory item). They ran in each of the six `cash_capture` items of 2026-11-12 with 0 game crashes,
which is a record of runs, not proof against a crash. The driver-lost condition now also binds the capture's own commands: once it
is set, `Py` refuses every command, and a reply that never comes sets it. The calls added are on the
player's own controller only (`GetPlayerViewPoint`, `GetFOVAngle`) and in the cash probe's existing scans (`cp_died` reads fields of
the pawn it finds by name).

Verified: the synthetic tests (stand-ins, no game), each new one failing on the code before its fix. `UNVERIFIED`: every one of these
reads in the game, until a session runs them; whether `TouchedPickupable` fires for money pickups; whether the controller's fields
hold the cinematic and input-ignore state as named. Left out: a part with Maya's active skills (it needs a live read to find where
the game keeps them), any new input sequence, a kill while down, a fixed frame cap.
