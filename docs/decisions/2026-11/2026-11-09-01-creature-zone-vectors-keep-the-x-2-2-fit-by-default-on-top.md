## 2026-11-09: Creature zone vectors keep the x^2.2 fit by default, on top of the shader's light/dark scales (showcase follow-up)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/PHASELOCK_STOCK_DATA.md`, "Master_Creature's colour model".

- **Change:** `tools/creature_zone.py` applies round 23's x^2.2 to the zone vectors by default (`GAMMA_FIT_DEFAULT`); `OPENWILLOW_ZONE_GAMMA=0`
  uses them as stored. The x2 light/dark scales from the shader reading stay.
- **Why:** an A/B in identical light (runner-z36) scored by a Luna critic, colour only: 6.4/10 with the fit, 5.8 without.
- **Not verified:** a host fit on top of a shader reading, judged by eye in UE5's light; the game's own rendering of the colours was not matched.

