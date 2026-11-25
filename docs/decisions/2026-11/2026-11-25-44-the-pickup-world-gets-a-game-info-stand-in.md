## 2026-11-25: The pickup world gets a game-info stand-in

AI-assisted (Claude Code, Opus 5.5). Lane LINK-6-PICKUP-GAMEINFO, on main 5168bb63. It follows lane LINK-6-OWNER-CHECK's
finding (recorded in decision 2026-11-25-41).

- **Why:** the pickup world of slice links 6 and 7 (`ow-cli` `equip_run::pickup_world`) spawns no game info, so its
  `WorldInfo.Game` is None. `DroppedPickup.Pickup.ValidTouch`'s last test is
  `WorldInfo.Game.PickupQuery(Other, self)` (read with `research/script_disasm.py`). Through None it answers false
  silently, with no error and no `NotYet`. So a pawn's touch never registered, the controller's
  `CurrentTouchedPickupable` stayed None, and the key took nothing.
- **What was decided:** the pickup world's set-up gives its `WorldInfo` a game info (`equip_run::pickup_game_info`),
  the same harness stand-in the combat run's deaths install (`ow_combat::death::enable_deaths`):
  - a class-default `WillowGame.WillowGameInfo`, with `Role` authority and its `WorldInfo`, written into the
    `WorldInfo`'s `Game`;
  - done at set-up, before the run writes anything;
  - logged as a harness input, and listed in links 6 and 7's harness lines;
  - a world whose `Game` is already set is left as it is.

  In the game the level's game mode spawns its own game info, so this one is a stand-in, **UNVERIFIED** as the
  game's. `GameInfo.PickupQuery` and what it calls are the installed script, run by the VM; nothing is bound or
  stubbed.
- **The slice gate** (installed Off strict, under the machine lock, on main 5168bb63 with this change): 5 ok / 8
  NotYet / 1 error. That is main 5168bb63's own reading; this change moves no row. Links 6 and 7 still stop in `touch`
  at `GetBoundingCylinder`'s skeletal bounds, which come before `ValidTouch`'s `PickupQuery`. Current main reads 5 ok /
  9 NotYet / 0 errors (GATES row SLICE-GATE-READING), and links 6 and 7 stop at the same place there. The change shows
  in the gate only once a link gets past that box.
- **Where link 6 goes next** (a throwaway local stack, never pushed: this change on #707's first version, whose box
  for the pickup's skeletal component was answered): link 6 runs `make` and `touch` and stops at the unbound native
  `WillowInventory.GetInventorySpaceRequirement()`. That is the inventory manager's `HandlePickupQuery`, behind
  `PickupQuery`. Its own-words row is in NATIVE_INVENTORY_EQUIP.md ("default 1; usable and mission items 0",
  medium-high, UNVERIFIED), so it is the next implementation lane. Link 7 stops at the touch trace filter, as before.
- **Tests:** synthetic (`equip_run_game_info_tests.rs`):
  - a world with no `Game` gets the stand-in, with `Role` and `WorldInfo`, logged once;
  - a world with a `Game` is left unchanged, with nothing written or made;
  - a world with no placed `WorldInfo` is an error of the set-up;
  - the line has single spaces.
- **Sensitive files:** none. `crates/ow-package` and `Cargo.toml` are unchanged.
- **Verified:** nothing in the running game.
