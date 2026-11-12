## 2026-11-12: One file per decision, and a landing script without the union merge

AI-assisted (Claude Code). The maintainer decided on 2026-11-11 that one file per decision replaces the single
`DECISIONS.md`. The layout, the migration, lanes committing their own decision file, the edit rule and the landing
script's check wait were the hub's recommendations (plan of 2026-11-12); the maintainer approved all five on
2026-11-12. This entry records three PRs that land one after the other.

- `tools/land_batch.sh` runs its rebases with `merge.conflictStyle=diff3` itself. The union merge driver for
  `DECISIONS.md`, `tools/merge-union.attributes` and the script's `DECISIONS.md` special cases are gone; decision files
  are ordinary files under the full diff-identity check. A new synthetic test, `tests/land_batch_test.py` (throwaway
  repositories, a local bare origin, a stub for `gh`), found that rerere replays a fix only when it was recorded under the
  same conflict style: when two appended entries start with the same blank line, the default style moves that line out
  of the conflict and diff3 keeps it in, so the two conflicts differ (git 2.54). The plan had said the styles replay
  each other's fixes; they did not in this case, so fixes should be recorded under diff3.
- `tools/decisions_index.py` reads `docs/decisions/YYYY-MM/YYYY-MM-DD-NN-slug.md` and still reads the old layout, in the
  working tree and at a baseline revision. `--check [--baseline REV]` checks the names, month folders, the one heading
  on line 1, that nothing else lives under `docs/decisions/` and that `DECISIONS.md` holds no entry; against the
  baseline, that no decision file was deleted or renamed, that a changed one is named on an `Edits:` line of a file
  added in the same change, and that each added file opens with "AI-assisted (". Against an old-layout baseline, every
  old entry must be present exactly as often, byte for byte after CRLF normalisation. `--name TITLE` prints the next
  free file name. `tests/decisions_index_test.py` is rewritten on invented entries, one case per rule.
- The 393 entries (88 from `docs/decisions/2026-09.md`, 305 from `DECISIONS.md`) were split by a one-off script into 393
  files in one commit that holds only its output. Its `--verify` rebuilt both old files from the new blobs byte for
  byte, each blob used once, with no CR byte; `decisions_index.py --check` against the commit before agreed. The
  listing is now in date order, and within each day in landing order. In the old files the date stepped backwards 17
  times and the September archive ran newest first, so 168 of the 393 entries print at another place in the plain
  listing (at least 50 would have to move to restore the old order), and `--tail N` differs from the old tool's from
  N = 8 on. `DECISIONS.md` is a stub. The script was deleted in the next commit and stays in the history.

Verified: the synthetic tests named above, the split's proof and the leak gate on each branch. `UNVERIFIED`: whether
GitHub's web blame follows the moved lines; plain `git blame` and `git log --follow` stop at the migration commit,
while `git blame -C` follows the lines back (the plan's trial, not repeated here).
