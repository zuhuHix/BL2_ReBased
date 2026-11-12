## 2026-10-03: host card rounding and integer truncation follow the native weapon rules (UNVERIFIED in game)

AI-assisted (Claude). Host and page only; no change to `src/`, `CMakeLists.txt`, package parsing or `tools/weapon_stats.py`.
This closes the host items the 2026-10-02 weapon-rules entry left open. The rules come from
`docs/verification/NATIVE_WEAPON_RULES.md` sections 1 and 2, which were read from native code and presentation data. The
only in-game evidence is the golden-card set in `REALGAME_GROUND_TRUTH.md`. Those cards show damage rounded up and
accuracy printed with one decimal and no `%`. They do not test this host code.

- **Truncation.** `UOpenWillowInventory::MagazineSize` and `ShotCostRounds` now truncate toward zero instead of rounding to
  nearest, because `ClipSize` and `ShotCost` are integer attributes. The slice recipes were evaluated before that rule
  and still carry fractional magazines (10.5 and 13.8 now give 10 and 13, not 11 and 14). The magazine's floor of one
  round is a host guard, not a game rule. A shot cost below 1 now truncates to 0, meaning no ammo cost; no local recipe
  has one.
- **Card numbers.** The host still sends raw stats. `inventory.js` `cardRound` rounds the stored single-precision value
  the way `present()` does: damage up, magazine down, and fire rate, reload and accuracy half up to one decimal. The
  accuracy line drops its `%`. Compare deltas are now differences of the printed numbers. The page does not print the
  recipes' `stats.card.display`, because the local slice's copy predates single precision (it holds 1.2 for a 1.25 fire
  rate that the game prints as 1.3).
- **Rarity colour.** Weapons now forward the card's `rarity_color` as `rarityColor` when the recipe has it. The page
  already prefers it, so E-tech gets its own colour. The local slice predates the field, so nothing changes until it is
  regenerated; that regeneration is still a separate decision.
- **Tests.** `tests/card_rounding_cases.json` holds invented cases. Both `tests/weapon_stats_test.py` (Python
  `display`) and `tests/inventory_navigation_test.js` (the page) check them, so the two implementations must agree.
  The navigation test was already failing at HEAD because its `document` stub lacked `addEventListener` (since
  `988b0b9`); the stub now has it. The inventory self-test checks truncation on invented values.
- **Changed card text in the suite:** the demo Infinity pistol with damage 753.22 now prints 754 (was 753), and the
  compare delta follows (+105 against 649, was +104). This was seen in the `backpack_transfer_changes_destination`
  frame. The 780.44 pistol would print 781 (was 780), but no captured frame shows it. Its other stats print as before.
  Suite frames from before and after are under ignored `local/card_rounding/`.
- **Checks (automated):** inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN
  rows, as before); inventory self-test passed; quest first run PASS 79 / 0 errors and resume PASS 11 / 0; CTest 10/10;
  `verify_packages.py` 9/9; navigation test 23/23; weapon_stats 26, weapon_recipe 14, weapon_balance 7 and weapon_paint
  29 tests OK. **In-game:** none. The host widget fallback (`OpenWillowInventoryWidget.cpp`) still formats unrounded
  values.

