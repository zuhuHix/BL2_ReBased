## 2026-11-03: Health 0 puts Maya down in the UE host; the HUD movie shows Fight For Your Life, the second wind and the mission tracker (HUD lane)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/HUD_PARITY.md` (element inventory; "Fight For Your Life, second wind and
respawn"); the down-state rules and their live numbers come from `docs/verification/COMBAT_DOWN_RESPAWN.md`.

- **What.** `AOpenWillowWalker` no longer respawns at health 0. It goes down (Injured): the bleed-out starts 2.28 s later and runs
  12 s, cut by 4 s per successive injury (30 s window, 1 s floor). Bleeding out is death, and the respawn follows 5.76 s later. A kill
  while down is a second wind (25 % of the maximum health). Damage while down is ignored. A health refill while Injured (a level-up)
  ends the down state, so down always means health 0. The game is reported to revive a player who levels up in Fight For Your Life,
  but that is not observed here (UNVERIFIED).
- **HUD.** `AOpenWillowMayaHUD` pushes `down`, `secondWind` and `tracker` to the HUD page, which drives the movie's `p1.dying1`,
  `p1.secondwind` and `p1.MissionNotifications.Module1-6`. `MissionSystem::trackerLines()` gives the active objectives with their
  package `ProgressMessage`, and `--mission-run` output now ends with a `"tracker"` array.
- **Weapon while down (UNVERIFIED, to confirm live with bl-sdk).** Going down puts the weapon away, as the injured special move does:
  it releases the trigger, drops owed burst shots and cancels a running reload. Fire and reload presses do nothing for the 2.28 s
  until `SwitchToSidearm`; after that the held weapon fires and reloads as usual (no sidearm switch). Checked in `-Down` (`down_weapon`).
- **Why.** Die and respawn is a Phase 4 gate piece, and BL2's loop goes through Fight For Your Life, not an instant respawn.
- **Checks.** `run_hud_shots.ps1 -Down` 10/10 and `-Tracker` 4/4 in UE, with `tests/check_hud_down_test.py` (20 synthetic cases)
  for their checker. The down check compares the title with the generated strings file and checks the bleed-out bar's pixels. The
  tracker check covers optional objectives, and `tests/mission_script_test.py` scenario T covers `trackerLines()` on a toy mission.
  `test_quest.ps1` waits out the bleed-out before its respawn checks and adds `player_death_goes_down_first`. On main 490661e it gives
  112/112 plus resume 11/11.
- **Not verified.** No real-game frame of these elements exists on this PC: bl-sdk and a Maya save are missing, so no critic score.
  Which `dying1` and `secondwind` fields the native fills is UNVERIFIED. Nothing in the UE host attacks Maya yet: the enemy attacks
  of PR #59 run in the C++ VM only.

