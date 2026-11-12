## 2026-10-08: token budget for agents: one-page lane brief, monthly decisions archive, lookup tools

AI-assisted (Claude), at the maintainer's request. Subagent lanes were re-reading about 15 to 20k tokens of instructions each (AGENTS.md, CLAUDE.md,
docs/LEGAL.md, docs/NATIVE_ANALYSIS.md and an ignored local rules file that does not travel between worktrees), and some opened DECISIONS.md (436 KB)
or NATIVE_INDEX.md (102 KB) whole. Now: `docs/LANE_BRIEF.md` condenses the binding rules on one page (about 1.2k tokens; the long documents stay
authoritative and unchanged, and a pointer at the top of AGENTS.md and CLAUDE.md sends subagents to it); `docs/verification/GATES.md` holds the gate
commands and the current baseline; the 88 entries of 2026-09 moved verbatim to `docs/decisions/2026-09.md` (DECISIONS.md now 246 KB; months rotate the
same way, `docs/TOOLING.md` "Decisions log: monthly archive"); `tools/decisions_index.py --check` confirms all 216 pre-split entries are present exactly
once and identical after CRLF normalisation (checked against f6504f3); `tools/native_index_find.py` prints matching index rows. `docs/TOOLING_TRIM_PROPOSAL.md`
lists repeated sections of TOOLING.md for the maintainer to approve; nothing was deleted. Checks: ctest 15/15, verify_packages 9/9. All tracked, so the
setup persists in new worktrees.


