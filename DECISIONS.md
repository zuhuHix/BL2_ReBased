# Decisions and evidence

This file holds no entries. Since 2026-11-12 every decision is its own file under `docs/decisions/`, named
`docs/decisions/YYYY-MM/YYYY-MM-DD-NN-slug.md`. The 393 entries that stood here and in the September archive were moved
there byte for byte, so a citation such as "DECISIONS.md 2026-09-26" still leads to the right date: look the date up
with `python tools/decisions_index.py --grep 2026-09-26` or in that month's folder.

Reading: `python tools/decisions_index.py --tail N` prints the last N entries in full (by date, then `NN`), `--grep WORD`
lists the entries whose title or date holds WORD, and no argument lists every entry with its file.

Adding: one new file per decision. `python tools/decisions_index.py --name "<title>"` prints its name (`NN` is one more
than the highest number `main` holds for that date; the slug is the title in lower case, every run of other characters
one hyphen, cut at a hyphen to at most 60 characters). Line 1 is `## YYYY-MM-DD: Title`, then a blank line, then the
body, which opens with "AI-assisted (<tool>)". Never delete or rename a decision file, and never change one unless a
file added in the same change names it on a line `Edits: <path>` and says why; a decision that replaces an older one is
a new file with a line `Supersedes: <path>`. `python tools/decisions_index.py --check --baseline origin/main` checks
these rules. The full text is in `docs/TOOLING.md`, "Decisions log: one file per decision".
