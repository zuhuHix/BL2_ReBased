## 2026-10-04: Inventory and Skills pages round 11: list scroll origin, compare and Skills layout, Inspect strip

AI-assisted (Claude). Page code only (`inventory.js`, `skills.js`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 11).

- **Cause of the off-centre selection band:** round 10 gave the row list a scroll rectangle with a negative x origin.
  Ruffle shifts the content right by that amount instead of revealing it, so every row sat about 23 px right of the
  panel centre and a band shift constant hid it. The origin is zero now and the shift constant is gone; the band is
  sized from the panel frame.
- **Compare:** the Equipped panel is wider and the Backpack panel moved left; padlocks show on locked slots (checked
  with `-owslots=2`, the real session's count, only). Compare from equipped keeps the highlight on the chosen
  equipped slot (drawing only; behaviour unchanged).
- **Skills:** tab group and trees moved right, Phaselock card refitted after each tree tween, overview footer below
  the panels and locked tiers dimmed. **Inspect:** level strip and price chip restored, thinner hint strip.
- All placements are fits read off the captured frames (UNVERIFIED).
- **Visual review:** an independent critic scored round 11 7.3/10 (round 10: 6.6) and judged it closer than round 10
  on every pair, except that compare-from-equipped lost the highlight on the focused backpack tile. Main gaps: the
  perspective tilt and glass (not reproduced; Ruffle ignores the movie's 3D transforms), band overrun at the panel's
  right edge, card text about 15 px against 17 px, Skills plate colliding with the footer hint.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

