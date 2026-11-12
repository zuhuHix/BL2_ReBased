## 2026-11-07: Inventory and Skills refuse while Maya is down (inventory lane)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/CONTROLS.md`.

- **Change:** the I and K keys refuse while she is down, as M and L already did, and a page already open cannot route to another page then. It can still be closed.
- **Why:** the set of actions the data allows while down holds no menu. M and L were read live refusing; I and K were not pressed while down in the real game, so for them this rests on that set and stays **UNVERIFIED**.
- **Host choice:** what a stock page does when she goes down under it was not read; a page stays open then. A respawn closes the status pages and gives the input back: she is a new body at a station. A focused page takes its own keys, so with #158 I and K stay on it and Escape and Tab close it.
- **Checked:** four new controls rows, `i_refused_while_down`, `k_refused_while_down`, `escape_closes_page_while_down` and `respawn_closes_page`; synthetic checker tests. A passing row means the host did this, not parity.
