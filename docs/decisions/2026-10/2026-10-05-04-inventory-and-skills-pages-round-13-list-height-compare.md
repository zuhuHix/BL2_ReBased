## 2026-10-05: Inventory and Skills pages round 13: list height, compare hints, focus spacing, Skills footer

AI-assisted (Claude). Page code only (`inventory.js`, `skills.js`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 13).

- The focus view shows 7.6 rows, ending near y 625 like the game (was 6.5); the panel title sits 8 px higher and the
  sub-header-to-tile gap is about 12 px. The Sort hint shows in compare from equipped and not in compare from backpack,
  as observed. Skills footer moved 38 px left, tab group 5 px down, description text one size smaller.
- The moved tile in compare from backpack gets a host-drawn green frame (colour chosen by eye); the critic did not see
  it in the frames, so it is an open item.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED).
- **Visual review:** a blind A/B critic preferred round 13 over round 12 on 9 of 12 screens (3 ties), means 6.8 against
  5.8. Main gaps: compare-from-backpack tile frame and list position, Skills card height and line wrap, Inspect card
  height (no part or flavour lines), perspective tilt and glass.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

