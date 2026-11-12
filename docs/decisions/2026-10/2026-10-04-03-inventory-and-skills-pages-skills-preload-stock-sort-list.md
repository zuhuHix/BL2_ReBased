## 2026-10-04: inventory and Skills pages: Skills preload, stock sort list, backpack focus layout, compare frames from real-game captures

AI-assisted (Claude). Host page, host forwarding and the suite only; no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` ("Skills preload, real-game comparison and the stock
sort list: 2026-10-04") and `docs/verification/INVENTORY_CARD_STATS.md` ("What the page prints on a weapon card").

- **Skills page:** in the baseline frame it still said "Loading Maya's skill tree...". The page was created on the key
  press and took about 3.4 s. It is now loaded hidden at level start, like the inventory page:
  - key to a populated page: 49 ms (page log lines);
  - cost: the inventory page's own boot grows from 4.4 s to 6.1 s;
  - real game: page visible about 0.28 s after K, settled by 0.65-0.85 s. No timing parity is claimed.
- **Real-game session (Maya L8):**
  - Saves were backed up and compared byte-for-byte afterwards, and the driver was removed.
  - The install runs community mods: part-name lines on cards and instant gear equip are theirs, so gear compare was
    not observed.
  - Confirmed in these captures:
    - compare frames: the moved item is green, the other yellow (the host had them inverted);
    - compare rows show arrows only;
    - red gear cells appear in a weapon compare;
    - backpack focus layout: enlarged centred panel, receded equipped panel, "BACKPACK used/capacity" plate;
    - the lists of all five sort modes;
    - the full-screen Inspect.
- **Stock sort list:**
  - ALL/TYPES/BRANDS/ITEMS/VALUE with sub-headers replace the host's own modes and category filter, following the
    comparators of `NATIVE_INVENTORY_SORT.md`. Those rules stay UNVERIFIED; every point the capture could test agreed.
  - PageDown and PageUp step through the modes, and the first item is selected on each change.
  - Ties keep pickup order. This is a host choice: the game's sort is unstable.
- **Cards:**
  - They now show the projectile count, the status rows for elemental guns, and values in the label colour.
  - For the six slice recipes the page text matches the golden cards, except the two shotgun recipes (reload and
    magazine one step off). That is a recipe/evaluator question, not a page one.
- **Inspect:** now full screen. The picture is the 3D preview keyed against black, so black gun parts can show as holes.
- **Slot art:** the slots now show item art, from previews rendered locally for the slice guns (local data only).
- **Visual review:** an independent critic agent scored 11 matched host/real-game pairs at 5.6/10 overall. Main gaps:
  - flat panels against the movie's tilted glass (Ruffle ignores the movie's 3D transforms);
  - card text about 20% smaller;
  - sort sub-headers overlapping cards;
  - the inset selection fill;
  - the compare layout;
  - the Skills tiles.
- **Open:** selectable empty backpack cells, gear compare in the real game, white flavour lines on cards, Q on the
  Skills page, and the Phaselock eye sigil on the HUD.
- **Checks:**
  - `tools/test_inventory_actions.ps1`: 49 PASS / 0 FAIL / 0 NOT_RUN / 0 KNOWN_DIVERGENCE (baseline 45/0/2/2).
    - Steps 15/16 now assert the stock sort.
    - The two wheel steps run because the test adds four synthetic filler weapons.
    - Slate keys only; the suite does not judge visuals.
  - `node tests/inventory_navigation_test.js`: 27/27.
  - CTest 10/10 and `verify_packages` 9/9 on the same tree.

