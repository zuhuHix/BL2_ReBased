## 2026-11-06: A missing overlay movie no longer covers the game (inventory lane)

AI-assisted (Claude). Record: `docs/verification/PICKUP_CARD.md`. Found by the scripted playtest on pc-hummer.

- With its movie absent, Ruffle draws an opaque error page over its whole player and `load()` still resolves. The pickup card's player fills
  the HUD, so aiming at a drop covered the game until the end of the run.
- `tools/hud_overlay/movie_guard.js` checks a page's movie files and their imports with HEAD before Ruffle sees them, and removes a player that
  shows the panel at any time in its life. Only a 404 counts as missing; other failures are asked again a few times. The HUD, Inventory, Skills,
  Missions, Map and pickup card use it; vending already had its own check. Each page shows its existing message or nothing, and logs
  `OpenWillow movie missing`. A HUD movie lost, before or after ready, sends the host back to its canvas HUD for good (`UNVERIFIED` in engine: synthetic tests only). The card asks again later.
- `tools/check_overlay_data.py` lists missing movies; the run script and the inventory suite print it, and stop only for the movie they
  cannot do without. Tested with synthetic fixtures and with real Ruffle in headless Edge (movie absent and present). Not yet run in engine.

