## 2026-10-05: Inventory and Skills pages round 15: equipped header, compare rows, Skills plate and wrap, Inspect hint

AI-assisted (Claude). Page code only (`inventory.js`, `inventory.html`, `skills.js`); no change to `src/`,
`CMakeLists.txt` or package parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 15).

- Equipped view: the panel title sits 13 px higher, so it no longer collides with the first sub-header. Compare
  views show 7.6 rows like the focus view; the moved tile has a translucent lime fill (solid would hide the gun art).
- Skills: trees 7 px higher, the SIREN plate 22 px higher with the card kept in place, description text one size
  larger so it wraps to six lines like the game, MOTION name label hidden as in the game. Inspect: hint strip moved
  right and the right-edge fade is off while inspecting.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED). This lane stops here.
- **Visual review:** a blind A/B critic preferred round 15 over round 14 on 5 of 12 screens and tied the rest, means
  7.4 against 6.9. Open: perspective tilt and glass (Ruffle ignores the movie's 3D transforms), Skills action plate
  about 25 px low and line pitch 25 against 22 px, Inspect card height (no part or flavour lines) and watermark,
  black gun parts keyed out as holes in Inspect, selectable empty cells, gear compare not observed in the real game,
  the Phaselock HUD sigil.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

