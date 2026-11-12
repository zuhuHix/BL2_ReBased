## 2026-11-08: The shop page runs BL2's own vending movie (vendor movie PR A)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/VENDORS.md` section 10.

- **Change:** with the converted movies present, `tools/hud_overlay/vending.html` draws the shop in `VendingMachineInterface` and the `TwoPanelInterface` library it imports (`tools/hud_overlay/vending_movie.js`). It shows the stock in the movie's own cells and item card, the item of the day and its countdown, the credits, the ammo and health panels by machine kind, and the backpack count. The page's rules, keys and requests are unchanged, except that Left and Right follow the movie's tab order. `tools/prepare_vending_movie.py` converts both movies.
- **Fallback:** without the files, or when Ruffle fails, the HTML page runs as before. The page logs which page ran, and `tools/check_vendors.py` reports it as `page_mode`. A load still pending after 8 s is cancelled through the guard's new `cancel()`, so a late answer does nothing. A switch between the movie and HTML keeps the selected item by id.
- **Why the library loads on its own:** Ruffle does not resolve the imports of an imported library, so the shop's placed copy had no panels. It was seen in a headless browser, and it is a Ruffle workaround.
- **Cells:** the game's cells hold only a run-time 3D thumbnail, with no text, and this host has none. They show the card's own type icon from the movie libraries, tinted by rarity. These are stand-ins, and nothing is invented for items without an icon.
- **Backgrounds:** the list panel and the item of the day get the frame label the panels' script wrappers derive from the shop type: `weapons`, `ammo`, `medicine` or `eridium`. So the Ammo Dump is green and Dr. Zed grey with his face, as in the real frames.
- **UNVERIFIED:** the layout against the game, the tab mapping, titles and section labels (host wording), and the row count.
- **Checked:** synthetic page tests, and the page in headless Edge with an invented state. The in-engine suite has not run it.

