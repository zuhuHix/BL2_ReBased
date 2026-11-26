# GATES.md rows that moved out

`docs/verification/GATES.md` keeps about ten dated rows under "Dated rows, newest first". The rows that stood below
them are here, byte for byte, one file per move, named for the day of the move (`YYYY-MM-DD.md`; a second move on
that day is `YYYY-MM-DD-2.md`). Nothing here is a current baseline and no gate needs these files. Open one only to
follow a citation: `grep -rn "<lane name>" docs/verification/gates-archive/` finds a row.

## Moving rows out

Proposed on 2026-11-26 with the decision that moved the first rows out (2026-11-26-74). The hub, or whoever lands a
batch, does it when "Dated rows" has grown past about thirty rows.

1. Work on a fresh `origin/main`, in a change that touches nothing else.
2. **First bring "Standing baselines" in `GATES.md` up to date, before any row is cut.** For each gate in the command
   sections (ctest, `verify_packages`, `cargo test --workspace`, `crate_edges_test.py`, the installed Off strict
   `--slice-gate`, the Fire event order, `verify_combat_pawn`, and any gate added since), find the newest row that
   reports it, in `GATES.md` or in this folder. Write its result, the row's name and date, and what it ran on (machine,
   and branch or commit). A gate's last result must stand at the top of `GATES.md` and never only in an archive file.
3. Cut every row below the newest ten, from the bold name that opens the first of them to the last line of the oldest
   row, and paste the block unchanged at the end of a new file here, under a short header. Leave the line that starts
   "Update this file", the blank line after it and the rows above the cut exactly as they are: lanes insert their rows
   under that line, and an unchanged context is what lets their branches rebase without a conflict.
4. The rows were written in `docs/verification/`, so a relative link in a moved row no longer resolves from here. Do
   not edit the row. Repeat each such link in the new file's header with `../` in front.
5. Commit, then compare the bytes of what left `GATES.md` with what the new file holds. With `<first>` the bold name
   that opens the first moved row and `<file>` the new file:

   ```
   git show origin/main:docs/verification/GATES.md | sed -n '/^\*\*<first> (/,/^## Older rows$/p' | head -n -2 > local/removed.txt
   git show HEAD:docs/verification/gates-archive/<file>.md | sed -n '/^\*\*<first> (/,$p' > local/archived.txt
   cmp local/removed.txt local/archived.txt && echo identical
   git show HEAD:docs/verification/GATES.md | grep -c '^\*\*<first> ('
   ```

   `cmp` must print nothing before "identical", and the last line must print 0 (the row is no longer in `GATES.md`).
6. Land it as a Markdown-only PR.

The first file, `2026-11-26.md`, holds two blocks, because rows had also been appended at the end of `GATES.md`.

A branch that edits a row which has moved gets a conflict on rebase: make that edit in the archive file. That is the
only reason an archive file changes after it is written.
