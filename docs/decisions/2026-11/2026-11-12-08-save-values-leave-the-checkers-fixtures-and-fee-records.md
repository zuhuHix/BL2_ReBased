## 2026-11-12: Save values leave the checkers, fixtures and fee records

AI-assisted (Claude Code). Rule 1 says nothing copied from a player's save is tracked, and an audit found the maintainer's
own save amounts in tracked files: the cash the level-8 Maya held and the respawn fees charged on it (lane L3, 2026-10-07),
and her experience total in the Fire mission trace (lane L2, 2026-10-07). This change takes them out of the two checkers,
three test fixtures and four records. The rules they illustrate stay as they were. The removed amounts, with their old
lines, are in the maintainer's private record, the only place they survive. History is not rewritten.

Edits: docs/decisions/2026-10/2026-10-07-07-fire-mission-golden-trace-recorded-in-the-real-game-and.md

That entry gave the experience total before and after the turn-in; it now gives only the change (+395 XP, level 8 kept),
which is the mission's reward and the same for any save.

- `tools/verify_combat_pawn.py`: the three `down.fee.k` rows now expect the invented cash 3,000, 2,410 and 1,620 (fees by
  the rule 210, 169 and 113; the rule-only sixth row 105 on 1,507). The self-test keeps its deliberately wrong rows wrong
  the same way: one fee off by one, one on the wrong cash. The invented amounts exercise both roundings (a fraction above
  a half and one below) and none sits on an exact half, as the live amounts did.
- `tools/verify_fire_event_order.py`: the step `player:8:<total>` takes an invented 21,000. Level 8 runs from 20,208 to
  28,126 by the curve in `NATIVE_PROGRESSION.md`, so 21,000 and 21,000 + 395 both stay at level 8 and the turn-in raises
  no level, as in the trace.
- `docs/verification/COMBAT_DOWN_RESPAWN.md`: the run's `cash:` steps take the checker's invented amounts. What was seen
  live keeps its fact without the amounts (0.07 of the cash rounded half up, 3 of 3 respawns in L3, and the fee charged
  first in the later capture). The VM column for the invented steps is the rule's value and was not rerun: `UNVERIFIED`.
- `COMBAT_GOLDEN_TRACE.md`, `NATIVE_DAMAGE_DEATH.md`, `FIRE_MISSION_GOLDEN_TRACE.md`: the fee and experience passages say
  the rule, the sample counts and the changes (+45, +7, +9, +395) instead of the totals.
- `tests/phaselock_integrity_probe_test.py`, `tests/vending_test.js`, `tests/vending_movie_test.js`: the fixtures that
  carried her own shield's level and capacity, or a health value of the save, take invented values and say so.

Verified: the checkers' self-tests and the Fire `--reader` run pass with the new values (counts in the PR). Not done here,
left for a second pass: `tests/realgame_snapshot_test.py` and `docs/verification/REALGAME_GROUND_TRUTH.md` (another open
change touches them), and the values that carry the profile's Badass Rank, her own shield's and gun's numbers and the
item names in the records.
