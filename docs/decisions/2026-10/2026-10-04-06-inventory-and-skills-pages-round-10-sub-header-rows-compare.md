## 2026-10-04: Inventory and Skills pages round 10: sub-header rows, compare layout, selection band, card fit, Inspect clip, Q overview

AI-assisted (Claude). Page and HUD capture sequence only; no change to `src/`, `CMakeLists.txt` or package parsing.
Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` ("Round 10").

- **Sort sub-headers:** each sits on its own row between groups. They were centred on the header clip's bounds, which
  are taller than the text, so every header sat about half a row high over the previous card.
- **Compare view:** the Equipped panel is narrowed into the gap between the cards with slots 1-4 drawn, the Backpack
  panel moves right so "(COMPARE)" stays readable, and both cards carry the price chip and Accuracy row (the synthetic
  test variants lacked those fields).
- **Selection:** the movie's own highlight symbol is stretched to the panel width behind the selected row (width and
  shift set by eye).
- **Card size:** cards are rescaled to visible widths measured on the real captures. A fit, UNVERIFIED.
- **Inspect:** the movie is clipped to the card frame and hint strip, so the backdrop box is gone.
- **Skills:** Q toggles an overview of the three trees. The layout numbers start from the installed `Gfx_SkillTree`
  defaults and are enlarged by eye; how the game combines them was not read, so UNVERIFIED.
- **Visual review:** an independent critic scored round 10 6.6/10 against real-game frames (round 9: 5.6) and judged
  round 10 closer than round 9 on all ten inventory pairs. Main gaps: the perspective tilt and curved glass (3D
  transforms, which Ruffle ignores), backpack-focus list geometry, compare panel positions and padlocks, Inspect card
  level strip, Skills page layout.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE, rerun after the weapon lane's preview and stat changes with the same result.

