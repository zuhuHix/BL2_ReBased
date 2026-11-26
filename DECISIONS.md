# Decisions and evidence

This file holds no entries. Since 2026-11-12 every decision is its own file,
`docs/decisions/YYYY-MM/YYYY-MM-DD-NN-slug.md`; the 393 older entries were moved there byte for byte, so a citation
such as "DECISIONS.md 2026-09-26" is found with `python tools/decisions_index.py --grep 2026-09-26` or in that month's
folder.

- **Read:** `python tools/decisions_index.py --tail N` prints the last N entries in full (by date, then `NN`),
  `--grep WORD` lists the entries whose title or date holds WORD, and no argument lists every entry with its file.
- **Add:** one new file per decision; `python tools/decisions_index.py --name "<title>"` prints its name. Line 1 is
  `## YYYY-MM-DD: Title`, then a blank line, then the body, which opens with "AI-assisted (<tool>)".
- **Change:** never delete or rename a decision file, and never change one unless a file added in the same change
  names it on a line `Edits: <path>` and says why. A decision that replaces an older one is a new file with a line
  `Supersedes: <path>`.
- **Check:** `python tools/decisions_index.py --check --baseline origin/main`.

The full text, with how `NN` and the slug are formed, is in `docs/TOOLING.md`, "Decisions log: one file per decision".
