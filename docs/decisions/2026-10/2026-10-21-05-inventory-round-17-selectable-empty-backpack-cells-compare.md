## 2026-10-21: Inventory round 17: selectable empty backpack cells, compare column anchor, ITEMS headers confirmed

AI-assisted (Claude). `inventory.js`, `tests/inventory_navigation_test.js`, the `-owinvshots` sequence in `OpenWillowMayaHUD.cpp`.

- The trailing "(EMPTY)" backpack cells can be selected, as observed in the original game on 2026-10-21: no card, item actions
  ignored, Sort hint kept. The VM's `MoveDelta` still receives item rows only; moves involving empty cells are host code (UNVERIFIED
  against the VM's own handling of empty entries).
- The compare view places the Equipped column by slot 1's cell instead of the panel's bounds (a transient child skewed those).
- Live category keys of a class mod, relic, grenade mod and shield (`comm`, `Artifact`, `mod`, `Shield`) and the ITEMS headers
  RELICS, CLASS MODS, GRENADE MODS, SHIELDS were read in the running game; the host's labels and order already matched.
- **Visual review:** critic 7.21/10 (11 pairs). **Checks:** navigation 29/29; inventory suite 49 / 0 / 0 / 0 (bounded item set).

