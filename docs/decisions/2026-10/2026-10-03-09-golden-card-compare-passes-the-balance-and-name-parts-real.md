## 2026-10-03: Golden card compare passes the balance and name parts (real-game lane)

- **What.** `tools/real_game/golden_card_compare.py` now gives the evaluator the record's balance and its prefix and title name
  parts, renders the level line from the evaluated `level_requirement` instead of the spawn stage, and blanks it when the model
  says there is no level line. Requested by the weapon lane, whose commit `13d9e87` made the rules need these inputs for sale
  value, the name and the level line.
- **Result (automated, golden-card evidence; the rules stay UNVERIFIED).** On the 69 golden weapons with exact parts: cooked data
  alone 53 matching the main four stats, 53 every printed stat, 52 every field; with the live-data overlay 69 / 69 / 68. The one
  miss is the host slice's own display name (`host_name`, 5 of 6). `tests/golden_card_compare_test.py` passes (12 tests).
- **Not done.** No new in-game capture; the rules behind the level line and rounding are still read from native code and
  confirmed only against the golden cards.
- **Checks.** Compare tool run twice as above; no sensitive files touched.

