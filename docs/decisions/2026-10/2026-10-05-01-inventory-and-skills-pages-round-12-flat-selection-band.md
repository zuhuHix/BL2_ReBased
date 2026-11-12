## 2026-10-05: Inventory and Skills pages round 12: flat selection band, focus-view extents, compare highlight, Skills chrome

AI-assisted (Claude). Page code only (`inventory.js`, `skills.js`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 12).

- **Band overrun:** the movie's highlight symbol has a glow tail that ran about 20 px past the panel's right edge. The
  focus view now draws a plain filled band (colour sampled from a real capture) inside the panel frame.
- **Extents:** the focus panel is 15 px shorter, the list starts 7 px higher, the hint line is lifted 9 px and the
  Inventory tab group shrinks 7%. Compare from equipped keeps the backpack tile highlight (clipped at the tile) and
  the compare hint has no Sort entry. Skills tab group and footer re-placed so the hint clears the Siren plate. Card
  text is 16 with bold values; the Inspect card is narrower with a hint strip clipped to its text; the mini equipped
  column hides its clipped title.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED).
- **Visual review:** a blind A/B critic preferred round 12 over round 11 on 11 of 12 screens (one could not be judged),
  means 7.0 against 5.5 (absolute scores drift about a point between critics). Main gaps: the list clips about one row
  early, compare from equipped should still show the Sort hint, the selected tile in compare from backpack should be
  green, Skills hint about 38 px right of the game's, perspective tilt and glass not reproduced.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE (run with the gun lane's preview and paint changes in place).

