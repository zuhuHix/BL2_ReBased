## 2026-10-05: Inventory and Skills pages round 14: compare tile frame, list positions, Skills tree height

AI-assisted (Claude). Page code only (`inventory.js`, `inventory.html`, `skills.js`); no change to `src/`,
`CMakeLists.txt` or package parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 14).

- The stray yellow rectangle in compare view was the HTML hit box's browser focus ring, drawn around the larger hit
  box; it is off (the movie draws the selection). The moved tile's green frame is thicker with a light tint.
- Compare and equipped lists start about 30-34 px higher ("WEAPONS" near y 170 as in the game); Skills trees, action
  bar and HARMONY label 20 px lower, the card 2.5% taller; focus hint and backpack plate nudged.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED).
- **Visual review:** a blind A/B critic preferred round 14 over round 13 on 10 of 11 scored screens (1 tie), means 7.2
  against 7.0. Main gaps: Skills vertical layout (icon pitch 67 against 72 px, plate low), Skills card wraps to five
  lines against six, equipped-view header collision, compare lists one row short, the game's solid lime fill on the
  moved tile, perspective tilt and glass.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

