## 2026-11-07: A respawned combat target starts at rest

AI-assisted (Claude). A real host bug, in `AOpenWillowCombatTarget`, found by a new check, not by a report. The swing from hits only ticks while the target is alive, so a death froze it, and neither respawn path cleared it, though the comment said a respawned target starts at rest. A player would have seen a respawned target tilting, and could have missed it. The check kills the dummy with a real hit and reads its swing on the first live tick; before the fix a host run read 24.4 degrees and 193 deg/s (`b6a0c9b`, pc-hummer, H19). `ClearSwing` now runs in both respawn paths and `ResetForTest`. Verified: the code path and that failing read. Whether the check passes with the fix is pending its host run at this head. A stock pawn's respawn path has the same call; the check only runs on the shapes dummy, so that path is `UNVERIFIED`.

Separately: the rolled gun rows at 650 uu are marginal with a wide-spread gun on the slim shapes dummy (one shot's direction passes about 18 uu from the centre, half-width 20 to 27.5 uu, calculated from one log). A row failing there is the row being strict, not necessarily a regression.

