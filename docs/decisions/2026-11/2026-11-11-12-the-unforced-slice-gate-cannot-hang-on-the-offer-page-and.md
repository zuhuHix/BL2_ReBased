## 2026-11-11: The unforced slice gate cannot hang on the offer page, and its evidence is tighter (review round 2 of #218)

AI-assisted (Claude Opus 5.5). Follows the two entries above; test behaviour only. Record: `docs/verification/SLICE_GATE.md`, "The unforced variant".

- **Accept.** Escape is sent once after 8 s. The next tick falls back to the direct `Accept()` whatever the page did, and a page still up is closed by calling its key handler directly. Before this, a page that the keys did not reach could hold the step until the runner's time limit.
- **Start record.** The checker accepts only the forms the host prints: `range_command=0` or `1` once, and `unforced=1` as the last word. A malformed marker, a second start line, or start lines that disagree fail the log in every variant. Valid host logs grade as before.
- **Loot.** Range corpses are recorded from the pickup snapshot before the fight, so a gun dropped by a death during the fight or the recovery counts.
- **Down.** A down is put down to an enemy only when a range-enemy hit was seen at most 0.5 s before it, while Maya was Up. Before, any such hit since the fight counted, including one minutes old.
- **Verified (automated):** `tests/check_slice_gate_test.py` 63 of 63. 400 random older-variant logs grade the same with the checker on main.
- **Not verified:** still no compile and no engine run. The 0.5 s margin is a host choice, UNVERIFIED.

