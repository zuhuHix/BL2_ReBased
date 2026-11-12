## 2026-10-21: Inventory and Skills pages round 16: skill tiles, Phaselock plate, overview, Inspect card, card title lines

AI-assisted (Claude). Page code (`skills.js`, `inventory.js`, `inventory.html`) and the `-owinvshots` key sequence in
`OpenWillowMayaHUD.cpp`; no change to `src/`, `CMakeLists.txt` or package parsing. Details and the 2026-10-21 real-game session:
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` ("Rounds 16-19").

- Two movie states Ruffle never cleared are cleared by the page: the skill cell's `overclocked` outline (now only with a class-mod
  bonus, UNVERIFIED rule) and the Phaselock plate's pink "disabled" tint (icon in an untinted clip once trained).
- Ruffle lacks Scaleform's `TextField.getLineLength`, so the card's `SetTitle` counted every name as two lines; the page now passes
  the field's `bottomScroll` to `SetRequiredTitleLines`.
- Skills overview poses, tree height, plate size and position, description leading, Inspect zoom and the hint line are fits to
  captured frames (UNVERIFIED). Inspect uses the card background's first (blue) frame and hides the movie's `ring` layer.
- The capture sequence selects weapon slot 2 and backpack row 2 so the engine frames show the reference frames' states.
- **Visual review:** baseline critic 7.27/10, round-16 critic (another instance) 6.85/10 on the same 11 pairs; see the record for why.
- **Checks:** navigation 27/27, skill info 9/9; inventory suite 49 / 0 / 0 / 0 with the bounded item set, 24 / 3 / 22 / 0 with this
  machine's default item set (over capacity, local data).

