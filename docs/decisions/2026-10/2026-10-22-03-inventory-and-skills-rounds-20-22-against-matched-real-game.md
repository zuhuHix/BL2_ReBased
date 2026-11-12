## 2026-10-22: Inventory and Skills rounds 20-22 against matched real-game references; card content from the parts

AI-assisted (Claude, with Codex sub-agents). Lane D of the 2026-10-21 orchestration. `tools/hud_overlay/inventory.*`,
`skills.*`, new `skill_nav.js` and `glass_sheet.js` (both routed by `serve.py`), `tools/weapon_stats.py`, the weapon
`fun_stats_markup` forwarding in `OpenWillowInventory.*`, and the `-owinvshots` capture block in `OpenWillowMayaHUD.cpp`
(capture-only, approved by the hub).

- **Matched references.** New real-game frames (`local/realgame/d20/`, ignored) have the host's capture state: the six
  bounded recipes in the same slots (spawned by full definition), Maya at level 36 (set in memory; saves restored and
  hash-checked), two weapon slots, $0, keyboard glyphs, Phaselock 1/1 and 31 points. The host captures run with
  `-owlevel=36`, an empty gear manifest, `-owslots=2 -owmoney=0 -owerid=0`.
- **Cards (parsing of game data; `docs/verification/INVENTORY_CARD_STATS.md`).** `weapon_stats.py` now emits the Accuracy
  row where it was missing, the coloured text lines (flavour, weapon zoom, ammo per shot, bonus damage, elemental
  effectiveness), Burn Damage / sec. and Ignite Chance, the price, and the element from the resolved damage type when no
  elemental part sets it (the Torgue explosive badge). Numeric `stats.card` changes: Wild Rifle `fire_rate` 6.557 to 5.56
  (it also changes the host's firing interval; reported to lane E), Hellfire `fire_rate` 9.09 to 8.96 and `damage` 659.2
  to 621.6, Volcano `damage` 2625.6 to 2481.0, and accuracy, reload, magazine and spread corrections listed in the
  verification record. Real-card comparison on cooked data (what the host's recipes use; 69 recorded cards, 67
  supported): damage 63/67, accuracy, fire rate, projectiles and price 67/67, reload 62/67, magazine 55/67, status DPS
  16/16, status chance 15/16; of the six slice cards, the two shotguns still differ in reload and magazine (4/6). An
  earlier "all match" log used the `--live-data` overlay of values read from the running game (67/67 with it); the
  difference between the cooked and live values is open (DECISIONS 2026-10-02: not online hotfixes). **Not
  solved:** the zoom line is 0.1 high on two of six cards (raw values 3.264 and 2.471 show as 3.2 and 2.4; no rounding
  rule fits all six, UNVERIFIED); the Nukem price differs because the bounded recipe has a different name part.
- **Navigation (read from script where named; each case matches the matched real frames).** A backpack weapon is compared with the active slot's weapon
  (`StatusMenuEquippedPanelGFxObject.GetTypeCellSuffixBasedOnThing` -> `WillowInventoryManager.GetEquippedAnalogue`: the
  current weapon, then the active quick-select slot); Left from the Backpack returns to that analogue; each sort selects
  its first entry and an empty ITEMS sort selects an (EMPTY) cell. Skills: Up/Down move between tiers
  (`SkillTreeGFxObject.HandleInputKey` -> native `Navigate`); Down x4 from Phaselock reaches Harmony's Sustenance in the
  game and in the host. The column choice between tiers of different widths, ties and the edges are UNVERIFIED. The
  ALL list breaks equal-rarity ties by descending type key (fits ALL; the BRANDS frame orders the same pair the other
  way, so one of the two always differs; the sort is native, UNVERIFIED).
- **Fixes found on the way.** `serve.py` served only listed files, so `skill_nav.js` returned 404 (a test now checks every
  script a page loads is routed); rebuilding the Skills hit layer under a resting cursor selected the tile under it, so
  hover now selects only after real pointer movement; a stale overlay server from an earlier run kept serving old routes.
- **Stand-ins (UNVERIFIED):** the glass sheet is our own drawing (normal-alpha wash, scanlines, curved edges); the
  game's brightening blend cannot be reproduced by a page composited with alpha.
- **Critic** (a fresh independent agent per round, 14 matched pairs): round 0b 5.95, round 20 7.39, round 21 7.85,
  round 22 7.57. Round 22 and a fourth attempt regressed and were not kept; the pages are at round 21. 8/10 not reached.
- **Checks:** CTest 10/10, packages 9/9, inventory navigation 40/40, skill navigation 6/6, weapon stats 36/36 (all 46
  standalone Python/JS tests under `tests/` pass; the other 8 need the reader and run inside CTest); inventory suite
  49 PASS / 0 FAIL / 0 NOT_RUN / 0 KNOWN_DIVERGENCE with `-Items local/inventory-actions/bounded-recipes`. The suite first
  failed step 20 (and skipped the seven steps after it) because a transfer started on the analogue even after a number key had
  chosen slot 2; a number-key choice for the selected item now wins (host control, the game's behaviour is not observed).

