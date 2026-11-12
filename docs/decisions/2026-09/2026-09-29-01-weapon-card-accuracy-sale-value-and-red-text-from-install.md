## 2026-09-29: Weapon card accuracy, sale value and red text from install data

The inventory card lacked the Accuracy row, the price and the red flavour line.
`tools/weapon_stats.py` now derives them from the installed data, and the host
forwards them (`accuracy`, `accuracyKnown`, `value`, `valueKnown`, `funStats`).
No package parser changes; the properties are read with the existing
`ow-package --properties`.

- **Accuracy**: `AttrPresent_WeaponSpread` remaps spread 0..15 to 100..0, so
  accuracy is `100 * (1 - spread / 15)` (orientation inferred, clamp
  UNVERIFIED). The spread input is the existing UNVERIFIED model, which does not
  reproduce the spread of any of seven real cards (Conference Call: model 3.90,
  real 4.44; Striker: 1.02 vs 1.995). Emitted with `accuracy_known = false`.
- **Sale value**: the type's `MonetaryValue` price calculator with the product of
  the parts' `MonetaryValueMod` and the level, rounded down. Integer-exact for
  5 of 7 real cards (shotgun, AR, SMG); the two launchers do not reproduce.
  `sale_value_known` is true only for the shotgun, assault rifle and SMG
  calculators; others carry the number flagged false.
- **Red text**: the title part's `CustomPresentations` line with
  a red `TextColor`, overridden by the installed `.int` when present. All
  nine red lines on real cards exist verbatim in the data. White stat lines are
  not derived.
- Host: items with no recipe `type` (the Infinity recipes) get a type label from
  the resolved ammo type.

Automated: `tests/weapon_stats_test.py` 11/11 (8 new, synthetic), CTest 6/6,
`verify_packages.py` counts match, UE module build succeeded. No in-game check
yet. Evidence and limits: `docs/verification/INVENTORY_CARD_STATS.md`.
AI-assisted; the real-card observations are local ignored traces.

