## 2026-11-07: The gameplay HUD hides behind the mission page and comes back on every close (mission screen)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/MISSION_SCREEN.md`.

- **Change:** while the mission page is on screen, `DrawHUD` draws no gameplay HUD: not the movie, not the canvas fallback, and it pushes no HUD state. The status pages already work this way. It reads `UOpenWillowQuest::IsScreenPageShown` each frame and stores nothing else, so no close path can leave the HUD hidden. The quest's on-screen debug text now shows only while the canvas fallback draws, never with the movie HUD ready or over a page; its log lines stay. This is real behaviour, not a test switch; runs without a mission page URL keep the page behaviour as it was, and the debug text as it was unless the movie HUD is ready.
- **Why:** a host frame showed the HUD behind the panel. The real game's mission screen captures (offer, turn-in, reward: one frame of each, level 8, with the SDK mods listed in the record loaded, and looked at again for this change) show no HUD, minimap or tracker.
- **UNVERIFIED:** when the real HUD hides and returns, and whether it fades; what hides it, which was not read; a stock run without the mods. The host hides and restores within a frame.
- **Checked:** the module builds and the synthetic checker test passes. Six new rows in `tools/test_mission_page.ps1` (27 in all) are not run yet. A fall and a lost page are not exercised by them. Passing rows will say what the host did, not that it matches the game.

