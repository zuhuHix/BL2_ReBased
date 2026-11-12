## 2026-10-05: native notes on loot rolls (UNVERIFIED) and a native index

AI-assisted (Claude), analyst lane G1, Ghidra; own-words note `docs/verification/NATIVE_LOOT.md`, no listing text. How an
item pool picks entries (one flat weighted list per pool, stage window, `Quantity` draws with replacement, nested pools
recurse), how a balance becomes an item (first grade window containing the capped stage, spawn-modifier interpolation,
level = capped stage), enemy drop lists and the death roll, mission reward items. Confirms that the Fire mission's stock
data carries no reward item, so the turn-in loot remains a labelled host stand-in. The note lists corrections to
`tools/loot_pools.py` and NATIVE_WEAPON_RULES section 4 (not applied yet). All UNVERIFIED in game.
`docs/verification/NATIVE_INDEX.md` starts a one-row-per-native index over the note files (mission script bridge and loot).

