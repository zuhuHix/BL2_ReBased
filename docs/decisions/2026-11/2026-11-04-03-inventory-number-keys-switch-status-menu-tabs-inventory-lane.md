## 2026-11-04: inventory number keys switch status-menu tabs (inventory lane, round 24)

AI-assisted (Claude). Record: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` "Round 24". Page behaviour change, read from
script bytecode (`StatusMenuExGFxMovie.HandleInputKey` -> `TrySwitchToScreen`); not yet seen in the running game (UNVERIFIED).

- Keys 1-5 in the inventory page switch to Missions, Map, Inventory, Skills, Challenges, as the status menu's script does; the
  pages the host lacks (Map, Challenges) do nothing. The host's own slot picker on 1-4 is removed: no inventory panel handles
  digits, and the transfer's slot is chosen with Up and Down.
- The inventory action suite follows: its digit presses became Up/Down in the transfer and one step was added (50 steps,
  50 PASS on a bounded set rebuilt on pc-hummer).
- Follow-up: Comma and Period are the script's previous and next tab (`GotoPrevTab` / `GotoNextTab`); from Inventory, Map
  (no host page: nothing happens) and Skills. UNVERIFIED in game.

