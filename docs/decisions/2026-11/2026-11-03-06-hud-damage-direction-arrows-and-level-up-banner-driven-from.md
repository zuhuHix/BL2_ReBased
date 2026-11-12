## 2026-11-03: HUD damage-direction arrows and level-up banner driven from host state (HUD lane 2, after PR #63)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/HUD_PARITY.md` ("Hit arrows and the level-up banner").

- **What.** `AOpenWillowWalker::TakeDamage` counts hits that took health and have a known source (a point hit's shot direction,
  else the causer or instigator). The HUD sends the last one's side of the view (90 degree quadrants) and a level-up count. The page
  plays `p1.dmg_<side>` from frame 2 on its `Health` frame, and `p1.levelup` from `go`. Both animations, and the banner text, are
  the movie's own frame scripts.
- **Rules after review round 1 (UNVERIFIED, to confirm live with bl-sdk).**
  - The arrow side is computed in view space, pitch included. A source near the view's up or down axis shows the top
    (above) or bottom (below) arrow.
  - One banner per level change: a multi-level jump plays one banner, since the HUD script keeps a single
    `QueuedLevelUpLevel`.
  - A zero point-hit direction falls back to the causer, then the instigator.
  - `-owhudfxshots` is editor-only.
  - Checks: `-Feedback` 7/7 with `fx_arrows_clear`; `fx_levelup` compares the page's banner count with the host's level
    rises.
- **Not done.** The reload prompt (its state and text are native), crosshair widening (the host has no bloom) and the grenade icon
  type (no delivery type on the item). The HUD_PARITY inventory says why for each.
- **Checks.** `run_hud_shots.ps1 -Feedback` 6/6 in UE; 4 synthetic checker cases; `hud_fields_test.js` extended.
- **Not verified.** No real-game frame of either element exists on this PC. The side split and the inner label the game uses are
  UNVERIFIED. Nothing in the UE host attacks Maya yet, so in play the arrows show only for the test hits.

