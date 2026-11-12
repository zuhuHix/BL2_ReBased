## 2026-11-06: the shop's serial moves only when what a request refers to changed

AI-assisted (Claude). From Luna's review of PR #127. A refusal bumped the serial, so a second press on a full pool looked stale and could say the stock changed when nothing had. The serial now moves on a success, a restock or an opening, and a refusal keeps it, so a repeat press gets the refusal again. A press that is really stale changes nothing: it keeps a refusal already showing, otherwise says "Try again", and does not move the serial, so a held key cannot keep missing. This replaces "The stock changed" from a stale press in the entry above; that text now only means a restock did happen.

- **Verified:** synthetic tests, including the real page code in Node with stub elements. `tools/test_vendors.ps1` gains a `stale_press` row through the host's own message path, which has not run yet.
- **`UNVERIFIED`:** real key input and how the text looks on screen.

