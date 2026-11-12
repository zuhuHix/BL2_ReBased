## 2026-10-03: Maintainer README/plan edits, written stop conditions dropped, worktree provisioning from a source worktree

Maintainer decision. Docs and tooling only; no change to `src/`, `CMakeLists.txt`, the reader or package parsing.

- **Maintainer edits kept as written:** the README intro, status-table notes for Phases 1, 3 and 4, the FAQ and
  "Who's making this" wording, and the plan's opening/credit lines. The Phase 3 and Phase 4 notes ("working well",
  "roughly 6/10 parity") are the maintainer's own testing and have no verification record; they are worded as such.
- **Stop conditions dropped:** the plan's section 9 (now "Commitment") and ROADMAP's "Kill criteria" section no longer
  list stop conditions. Links and descriptions that pointed at them were updated (README FAQ, document table, the
  Phase 1 gate sentence in ROADMAP). Verification rules are unchanged.
- **Worktree provisioning:** `tools/provision_worktree_assets.ps1 -SourceWorktree <path>` copies missing files from
  a populated worktree's Content and every `local/` folder (the shared seed only holds four folders and goes stale).
  Documented in AGENTS.md and `tools/worktree-assets.md`. Parse-checked only; not run in a new worktree.

