## 2026-11-12: The host's default shield recharge becomes an invented stand-in

AI-assisted (Claude Code). Rule 1 says no stats copied from a player's save are tracked. The host's default recharge delay
and rate for a shield whose card has none (`FOpenWillowMayaStats` in `OpenWillowMayaState.h`) were the live read of the
maintainer's own shield, one item's rolled stats. They applied to every stand-in shield: the start shield, vendor and loot
shields, and the slice gate's fallback shield. The old values are in the maintainer's private record. History is not
rewritten.

Edits: docs/decisions/2026-11/2026-11-03-11-maya-s-shield-and-survival-state-in-the-host-maya-lane-item.md
Edits: docs/decisions/2026-11/2026-11-07-19-the-stand-in-shield-rule-takes-the-maintainer-s-numbers-maya.md

Those entries gave the old delay and rate, and the old capacity anchor at level 8; they now read as before without the amounts.
Both were save values: the delay and rate were the read of the maintainer's own shield, and the level 3 shield behind the old
capacity anchor was the maintainer's own shield too. The delay and rate are replaced by this change; the anchor was replaced on
2026-11-07 by the fit.

- **Now:** a delay of 2.0 s, then 40 per second whatever the capacity. Round numbers chosen for the host and labelled in code
  as invented. They are not game data and not any player's shield. A card's own `Recharge Delay` and `Recharge Rate` still win.
- **Why not game data:** no prepared data gives a stock shield's delay or rate. The player shield pool
  (`D_Resourcepools.PlayerPools.ShieldPool`) was read from the installed packages on 2026-11-12 with `ow-package`, through
  `tools/weapon_recipe.py`'s `Package` and the schema of `tools/slice_values.py`: the pool object carries no base delay or rate.
  That the shield's parts at the item's level supply them is a reading of that data, `UNVERIFIED` in the running game. No prep
  tool evaluates the parts' delay or rate (`tools/prepare_vendors.py` resolves
  shield parts only for the price). Doing so is new decoding work, so it is left as the follow-up that would replace both.
- **Capacity unchanged:** the stand-in fit in ignored `local/slice/stand_in_shield.json` (`COMBAT_DOWN_RESPAWN.md`, "Stand-in
  shield capacity"), already `UNVERIFIED`. The slice gate's fallback keeps her maximum health as its capacity and takes the new
  recharge.
- **Behaviour change:** a stand-in shield waits a little longer after a hit and refills faster than before. Nothing in a checker
  or a suite's expected numbers pinned the old delay or rate; the shield check waits the delay it reads from the host.

Host test (it runs in the in-engine batch, not run by the lane): `OpenWillow.MayaState` (synthetic) now also checks that a card without recharge stats keeps the
stand-ins and that a card's own recharge stats win. `UNVERIFIED` against the game: the delay, the rate and the capacity of
every stand-in shield.
