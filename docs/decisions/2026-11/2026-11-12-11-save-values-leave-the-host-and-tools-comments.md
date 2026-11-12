## 2026-11-12: Save values leave the host and tools comments

AI-assisted (Claude Code). The follow-up that
`docs/decisions/2026-11/2026-11-12-10-save-values-leave-the-records-shield-badass-rank-gun-and.md` names. Rule 1 says nothing
copied from a player's save is tracked, and four comments under `host/` and `tools/` still held values of the maintainer's own
save: the second-wind health with the profile's Badass Rank, the level-up scale sums and her gun's two damages, and the name of
her grenade mod. Each comment keeps its rule and its pointer to the record: the second wind is "0.2500 of the maximum", the
level-up scale is "a +1.0 Scale joined the existing sum" (the amounts are in the maintainer's private record), and the throws
are those of "the save's own grenade mod, not the frag". History is not rewritten.

- Changed: comments in `OpenWillowMayaState.h`, `OpenWillowWalker.cpp`, `OpenWillowWalker.h` and `tools/verify_combat_pawn.py`.
  No code token changed: in each of the 7 changed lines the text before the comment marker is byte-identical.
- No behaviour changes. `tools/verify_combat_pawn.py --self-test` passes; the host build and suites are the hub's
  in-engine run, not run here.
