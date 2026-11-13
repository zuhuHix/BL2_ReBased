## 2026-11-13: Class mod pools are filled at run time; item rarity and the relic weight hook read

AI-assisted (Claude Code). Analyst lane K of the hub run of 2026-11-13, for the item registry the maintainer put before more item logic.
The new note `docs/verification/NATIVE_ITEM_RULES.md` answers three rules a non-weapon roll needs. It was read from the installed packages
and from 14 read-only Ghidra runs (the first failed before producing output and the second repeated it); nothing in it was seen in the
running game, so every rule is UNVERIFIED there. No code changed.

- **Class mod pools.** The global per-rarity class mod pools (`GD_Itempools.ClassModPools.Pool_ClassMod_01_Common` and five more) are empty
  in the packages. A guarded initialisation of the globals object fills them: one entry per player class, nesting that class's own pool.
  What triggers it and whether it can run more than once were not resolved. Each entry's weight is A × B, evaluated at the roll, where A
  depends on what the roll's context resolves to (the note's section 3). B comes from `GlobalsDefinition.ClassDropChance` f (0.5, 0.33,
  0.25, 0.2 for n = 1 to 4 players): k × f for a class with k players in the party, and (1 − n × f) / C for each class with no player, where C
  counts every class identifier. Solo with the four base classes the B values add to 0.875, not 1. When the context resolves to nothing,
  A is 1 for every class; with a player controller as the context only that player's class is drawn.
- **Item rarity.** For shields, grenade mods, class mods and relics: the definition's base rarity plus the rarity of each part, banded by
  `RarityLevelColors`; mission items take `MissionItemRarityLevel` (500). Whether the two name parts count was not resolved. Six shield
  titles carry a rarity, and the question changes the band of three of those shields (the Phoenix and the two legendary absorption shields).
- **Relic weight hook.** For relics only, a part that has slot upgrades, none of which names a slot the relic defines, gets weight 0. A part
  with no slot upgrades keeps its weight.

What this changes in records on main (those files are not edited here; the note's "Corrections to records on main" lists the points):

- `NATIVE_LOOT.md`, "Corrections to earlier notes", item 1, says the common class mod pool is empty, so at stage 8 the class mod branch of
  `Pool_GunsAndGear` takes no share. Filled at run time, that pool passes its stage gate (8) at stage 8. The 57.1 % weapon share computed
  there rests on the empty pool and needs reassessing; a replacement share is not established, since it also depends on the build having run
  before the roll and on the class identifier list being filled, both open.
- `VENDORS.md` section 6 says the game fills those pools "in some way that is `UNVERIFIED`", and the comment in `tools/prepare_vendors.py`
  says "filled at runtime; how is UNVERIFIED". How they are filled is now read from native code, and still UNVERIFIED in game.
- `NATIVE_WEAPON_RULES.md` section 3 and `WEAPON_PARTS_CATALOG.md` section 2 list the relic hook as not read, and `NATIVE_LOOT.md` lists item
  rarity as open. Both are now read.

Still open and needed before a class mod can be rolled: which of a class mod balance's nine definitions it yields (a native override, not
read). The note's last three sections name the live reads that would confirm each rule.
