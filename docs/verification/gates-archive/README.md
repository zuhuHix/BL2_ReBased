# GATES.md rows that moved out

`docs/verification/GATES.md` keeps about ten dated rows under "Dated rows, newest first". The rows that stood below
them are here, byte for byte, one file per move, named for the day of the move (`YYYY-MM-DD.md`; a second move on
that day is `YYYY-MM-DD-2.md`). Nothing here is a current baseline and no gate needs these files. Open one only to
follow a citation: `grep -rn "<lane name>" docs/verification/gates-archive/` finds a row.

## Moving rows out

Proposed on 2026-11-26 with the decision that moved the first rows out. The hub, or whoever lands a batch, does it
when "Dated rows" has grown past about thirty rows.

1. Work on a fresh `origin/main`, in a change that touches nothing else.
2. Cut every row below the newest ten, down to the line above `## Older rows`, and paste the block unchanged into a
   new file here, under a short header like the one in the existing files. Leave the line that starts "Update this
   file" and the rows above the cut exactly as they are: lanes insert their rows under that line, and an unchanged
   context is what lets their branches rebase without a conflict.
3. Check that no line was lost: the new file's rows are the removed lines, in the same order
   (`git diff --stat` shows the two counts).
4. Land it as a Markdown-only PR.

A branch that edits a row which has moved gets a conflict on rebase: make that edit in the archive file. That is the
only reason an archive file changes after it is written.
