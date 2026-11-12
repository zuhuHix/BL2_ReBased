## 2026-11-03: inventory round 23: VALUE thumbnails, Inspect turn, card and plate fits (Inventory parity)

AI-assisted (Claude; Luna (Codex) critic). Record: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` "Round 23". Page behaviour
changes, checked in the UE host against the matched D20 real frames; no new real-game frames.

- **Thumbnails survive re-renders.** The movie's turn sways after some sorts and each step rebuilt every cell with a new image,
  which was still decoding when the VALUE frame was taken. Decoded thumbnails are now reused across renders. Confirmed on the host
  capture (D2e shows all four), and by a page log of the renders.
- **Single-card occlusion.** The single card hides equipped and gear thumbnails but not backpack ones (round 22 hid neither,
  which drew equipped thumbnails over the card text).
- **Inspect keeps the menu's 3D turn** (the movie sets 5 degrees there); the hint is larger and on one line. UNVERIFIED fit to D5.
- **Fits** (round-22 geometry without its sort tie change; card, compare card, focus panel, count and ammo plates, glass-sheet top
  edge) are measured on D20 only: UNVERIFIED elsewhere.
- **Not settled:** the number keys in the real backpack (the game would not start: Steam), the native BRANDS tie order, the zoom
  rounding in `src/weapon_gen.cpp`, the white Inspect weapon paint. One blind A/B critic round: final 7.99 against 7.84 for round 21.

