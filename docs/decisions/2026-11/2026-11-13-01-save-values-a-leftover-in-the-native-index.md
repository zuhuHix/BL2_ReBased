## 2026-11-13: Save values: a leftover in the native index

AI-assisted (Claude Code). A check after
`docs/decisions/2026-11/2026-11-12-10-save-values-leave-the-records-shield-badass-rank-gun-and.md` and
`docs/decisions/2026-11/2026-11-12-11-save-values-leave-the-host-and-tools-comments.md` landed found one value of the
maintainer's save that those passes did not reach. It was the amount of the capping hit in the Fire dummy's health trail,
in one row of `docs/verification/NATIVE_INDEX.md`. That amount follows from her controller's grenade modifier and the trail,
both removed from `COMBAT_HIT_CHAIN.md` on 2026-11-12. The row now says the capped hit reproduces the live L5 row, with
the amount in the maintainer's private record. `NATIVE_INDEX.md` is written by hand (`tools/native_index_find.py` only reads
it), so the file itself changed.

Checked: every numeric token of four or more digits on the old lines in the maintainer's private records, and the rounded
forms used before, searched whole-word in the tree. Of 183 tokens, 67 are present: this one leftover, and 66 that are game
data, behaviour, counts and timings, invented fixture values, or unrelated numbers. Two were looked at more closely, and
neither gives a save value back. The split of a grenade hit across shield and health gives the shield's strength at one
moment of the fight, from which the shield's capacity cannot be recovered. The three equal steps of a target's health name
no endpoint, so the gun's damage per shot cannot be recovered either. No other decision file changes.
