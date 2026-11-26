## 2026-11-26: Instruction files cut for tokens; GATES.md rows moved to an archive

AI-assisted (Claude Code, Fable 5.2). Lane T (TOKEN-OPTIMISATION), asked for by the maintainer. Docs only: no code, no
hook and no setting changed, and no rule was dropped or weakened on purpose. **The changes to `AGENTS.md` and
`CLAUDE.md` wait for the maintainer's explicit yes** (the PR is a draft until then).

- **Why.** Every session loads `CLAUDE.md` and `AGENTS.md`, and every lane job re-reads `docs/LANE_BRIEF.md` and
  `docs/verification/GATES.md`. `GATES.md` had grown to about 281 KB (284 dated rows), with its gate commands at
  line 2,747, below the rows.
- **What was decided.**
  - `docs/verification/GATES.md`: the gate commands and a new "Standing baselines" block come first, then the dated
    rows, then the Rust host's gates and "Tests outside ctest". Ten rows stay; the other 274 moved, byte for byte and
    in their order, to `docs/verification/gates-archive/2026-11-26.md`. The line a lane inserts its row under, the
    blank line after it and the newest row stand together and unchanged, so the append point and the row format are
    as they were. `docs/verification/gates-archive/README.md` proposes how later rows move out (by hand; no tool was
    added). Two lines are new in the command sections: a pointer to the Rust `ow-package --slice-gate` the rows quote
    (the command is `docs/TOOLING.md`'s; that it runs under the machine lock is what the rows report), and the leak
    gate under "Records" (`docs/TOOLING.md`, "Leak gate").
  - `docs/LANE_BRIEF.md` no longer repeats the rules of `AGENTS.md`. It keeps what only it holds (the clean room, the
    note scan, the evidence wording, the gates, how to finish a lane) and the two rules a brief must carry (process
    safety, the tools line). An agent that cannot see "Rules that do not bend" is told to read `AGENTS.md` first, and
    `CLAUDE.md` now tells whoever briefs a sub-agent to point it at `AGENTS.md` too.
  - `AGENTS.md`: rules 1 to 11 are byte for byte what they were. The long form of "External tools and extraction"
    moved, unchanged, to `docs/TOOLING.md`, "External tools", and `AGENTS.md` keeps a short binding form. The pointer
    to the legal text now names the part to read: "Rules" and "Analysing the executable" for native analysis, the
    whole of `docs/LEGAL.md` for licences, provenance, third-party code, releases, money or moving game-derived data.
    `README.md`'s "Is this legal?" is named as the public summary and no longer as required reading. Four clauses came
    in from `LANE_BRIEF.md` (restore what the game rewrote and say so; never hand an error back unread; never claim
    more than you checked; do not open `NATIVE_INDEX.md` whole), and "anything public (a gist, a chat)" from
    `docs/LEGAL.md`.
  - Stale text corrected, each against a record: the host "will be" our own engine (it exists: `crates/ow-host`,
    decision 2026-11-16-44); "Rust host runs once they exist" (they exist: `GATES.md`, the host sections);
    `docs/TOOLING.md`'s "the Rust port is not started" for `ow-package` (the Rust reader gives the same 3,339 class
    bodies: row PHASE-0R-READER-CHECK).
  - `CLAUDE.md`, `DECISIONS.md` and `docs/TOOLING.md`'s "Main tools at a glance" are tighter, with the same content.
  - Left alone, on purpose: `docs/LEGAL.md` and `README.md` (public policy text; rewording it for tokens is not this
    lane's call), `.claude/settings.json` and `.claude/hooks/sensitive_guard.py`, `docs/NATIVE_ANALYSIS.md`,
    `THIRD_PARTY.md`, `LICENSE`, `ROADMAP.md`.
- **Measured** (bytes of the committed text; tokens estimated as bytes / 4, which is rough):

  | File | Before | After |
  |---|---|---|
  | `AGENTS.md` | 16,467 | 15,303 |
  | `CLAUDE.md` | 1,448 | 1,389 |
  | `docs/LANE_BRIEF.md` | 5,879 | 3,470 |
  | `docs/verification/GATES.md` | 280,618 | 32,573 |
  | `DECISIONS.md` | 1,479 | 1,242 |
  | `docs/TOOLING.md`, "Main tools at a glance" | 3,142 | 2,495 |

  `GATES.md` up to its first row is 3,081 bytes; before, the first command stood behind 259,737 bytes of rows.
  `docs/TOOLING.md` as a whole grew by 1,571 bytes, the rules that came from `AGENTS.md`.
- **Verified.**
  - A script (kept under ignored `local/token-opt/`) rebuilt `GATES.md` and the archive from `origin/main` and
    checked that every old line is in one of the two files as often as before, that the kept rows, the moved rows and
    each kept section are unbroken runs in the old order, and that the insertion context is unchanged.
  - A synthetic row inserted at the old append point on `origin/main` merges onto this layout without a conflict
    (`git merge-tree` on the real file). A real row landed on `main` while this lane worked, and the rebase over it
    was clean too.
  - An independent read of the old and new texts (a second assistant, given the files and not the conclusion) found
    two losses and several weaker wordings. They were fixed before the PR: rule 6 went back to its exact text, and
    "nothing counts as verified until it has been checked against the real game" was restored.
  - The guard hook was run by hand on nine paths: it prints nothing for an ordinary file, about 400 characters for
    one of the four C++ sensitive files, and nothing for their Rust ports, which its list does not hold.
  - `tools/decisions_index.py --check`, the leak gate and `tests/real_game_migration_test.py` (the one test that
    reads a changed file): results are in the PR.
- **UNVERIFIED.**
  - The daily saving. It rests on assumed loads per day and on how much of `GATES.md` a lane reads, which nobody
    measured.
  - That every sub-agent has `AGENTS.md` in its context. Claude Code and Codex load it for a chat; for a sub-agent
    type that does not, the brief's first paragraph and the briefing line in `CLAUDE.md` are the only guards.
  - That the hand procedure in `gates-archive/README.md` will be kept up. At the present rate of about 70 rows a day
    the rows section passes thirty rows within a day.
- **Open, for the maintainer.** Whether the narrower "what to read in `docs/LEGAL.md`" is acceptable; whether
  `LANE_BRIEF.md` may lean on `AGENTS.md` in this way; whether the hook should also name the Rust ports; whether
  moving rows out should become a small script; and two lines this lane did not touch, which the independent read
  found at odds with `crates/ow-host` (`docs/LEGAL.md`, "its host has no engine yet"; `README.md`'s FAQ, "The Rust
  host doesn't run yet").
