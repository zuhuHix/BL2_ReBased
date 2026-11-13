# Item roll: shields, grenade mods, class mods and relics from the registry (2026-11-13)

AI-assisted (Claude Code, lane B of the hub run of 2026-11-13). Counts, class names and rules in our own words only. The rolls and
the census output (part paths, frequencies) are game data and stay under ignored `local/items/roll/`. **Nothing in this record was
checked in the running game.** The census proves that every roll keeps to the rules below; it does not prove those rules are the
game's.

## 1. What the tool does

`tools/item_roll.py` rolls one item of a shield, grenade mod, class mod or relic balance at a game stage from a seed
(`roll(registry, balance, stage, seed, lookup)`), and proves a roll legal (`check(registry, roll, lookup)`). The registry is
`tools/item_registry.py`'s output (`ITEM_REGISTRY.md`); `lookup` is item_registry's package census, which answers what the registry
does not hold (attribute values, name parts, part rarities, slot upgrades, the globals). The pick, the random generator, the name
choice and the attribute evaluator are imported from `weapon_recipe`; no existing tool is edited.

A roll, in order, drawing from one `CrtRand(seed)` stream:

1. **Manufacturer and grade.** Per manufacturer entry, the first grade whose stage window holds the stage, weighted by the spawn
   modifier interpolated across the window (`NATIVE_LOOT.md`, "Balance candidate expansion"); a grade weighing 0 is no option. One
   option: no draw. Several: one weighted pick. No option: the balance yields nothing at that stage.
2. **Definition.** An interpolating balance (`bInterpolateExpLevel`, class default true) yields its own definition; one that does not
   yields the grade's own definition when set. A class mod picks among its balance's `ClassModDefinitions` (stand-in below).
3. **Parts.** The registry's candidates of that definition's variant at the stage for the manufacturer (`item_registry.candidates_at`),
   then the relic hook (`NATIVE_ITEM_RULES.md` section 1): when the definition sets `bAllowInventoryDefToModifyPartWeight` (class
   default true for `ArtifactDefinition` only), a part with slot upgrades, none of which names a slot in the definition's
   `AttributeSlotEffects`, weighs 0. Then one pick per slot, Alpha to Theta and Material; a slot with no candidate takes no part.
4. **Level.** The stage for an interpolating balance, else the grade's `ExpLevel`. No stage cap here.
5. **Name parts.** The definition's prefix and title lists, then each chosen part's in slot order, as `prepare_stock_shield` does.
6. **Rarity.** Truncated `BaseRarity` of the definition plus the truncated `Rarity` of each chosen part and of the two name parts;
   the band is the rating of the first `GlobalsDefinition.RarityLevelColors` entry holding the level, by name only (no colours,
   decision 2026-11-08-02). A level in no entry has no band.

The checker recomputes the legal set independently of the draws: the manufacturer has a covering grade of positive weight and the
grade is the first covering one; the definition is one the balance yields; the level follows the rule; every chosen part is a
candidate of its slot at that stage for that manufacturer with a weight above 0 after the hook; a slot is empty only where it has no
candidate; each name part is in the definition's or a chosen part's list, qualifies at the level and is the one the name rule picks;
the rarity level and band equal the rule's.

## 2. Stand-ins (each UNVERIFIED, each where `NATIVE_ITEM_RULES.md` leaves the question open)

- **Which class mod definition:** one uniform pick among the balance's `ClassModDefinitions`. The game's
  `ClassModBalanceDefinition` override was not read.
- **`EPRM_Complete` for relics:** the registry's merge (a Complete collection replaces every slot), which is the note's reading (b);
  reading (a), where an unlisted slot falls back to the definition's shared list, is not modelled.
- **Name parts in item rarity:** counted, the reading the note says the data supports. It moves three legendary shields from Very
  Rare to Legendary.
- **Manufacturer pick:** drawn in the roll, weighted by the grade's spawn modifier. The game draws it in the pool roll, one candidate
  per manufacturer; the roll accepts a manufacturer from the caller to skip the draw.
- **Runtime state:** a stage-window bound that needs runtime state is no bound (as `prepare_vendors` and `prepare_stock_shield` read
  it); a weight that needs it would be taken as 1 with a note (none occurred). The registry keeps only the leaf-most
  `Manufacturers` list, so the game's walk to a base balance when that list has no covering grade is not modelled.

## 3. Census of the installed game (zuhu desktop, 2026-11-13)

`python tools/item_roll.py --census`: 20 seeds per balance at stages 1, 8, 15, 30, 50 and 80, over the registry of the same day
(2,010 packages scanned, 2 not valid packages skipped as in `ITEM_REGISTRY.md`). 62 s in all, 44 s of it opening the package census.
The seeds are spread over the 32-bit range: the C runtime generator's first draw grows by about 3.3 per seed step, so seeds 1 to 20
would all draw near 0 first and favour each list's first candidate (the synthetic test caught this).

| kind | balances | rolls | not rollable | checker failures |
|---|---|---|---|---|
| shield | 120 | 14,380 | 20 (no grade at the stage) | 0 |
| grenade mod | 67 | 6,300 | 1,740 (no grade at the stage) | 0 |
| class mod | 56 | 6,600 | 120 (the one balance naming no `ClassModDefinitions`) | 0 |
| relic | 71 | 8,380 | 140 (no grade at the stage) | 0 |

**35,660 rolls, 0 checker failures.** Slots filled (rolls with a part in the slot):

| kind | Alpha | Beta | Gamma | Delta | Epsilon | Zeta | Eta | Theta | Material |
|---|---|---|---|---|---|---|---|---|---|
| shield | 13,900 | 13,900 | 13,900 | 13,900 | - | - | - | - | 13,900 |
| grenade mod | 5,020 | 6,300 | 6,300 | 6,300 | 6,300 | 6,300 | 2,740 | 5,940 | 6,300 |
| class mod | 6,600 | 6,600 | 6,600 | - | - | - | - | - | 6,600 |
| relic | 7,036 | 5,100 | 2,256 | 1,548 | 996 | 810 | 8,380 | 8,380 | - |

(480 shield rolls lack every slot: 4 enemy shield balances with no part slot, 20 seeds, 6 stages.) Distinct parts drawn per slot, against
the registry's distinct candidates (`ITEM_REGISTRY.md` section 2): shields 15/16, 21/21, 12/13, 45/45, 68/68; grenade mods every slot
equal but Theta 10/11; class mods all equal; relics Alpha 11/11, Beta 13/14, Gamma 8/9, Delta 10/10, Epsilon 3/4, Zeta 2/3,
Eta 29/29, Theta 46/56. A part never drawn is either outside every census stage's window or rare at 20 seeds; the checker does not
need every candidate drawn.

Bands (rolls): shields Common 2,022, Uncommon 3,936, Rare 2,604, Very Rare 2,958, Legendary 1,320, Seraph 840, Rainbow 220,
Unknown 480 (the enemy shields, level 0). Grenade mods, class mods and relics fall in the same tiers; the Seraph (level 501) and
Rainbow (level 506) rolls are DLC items whose definitions carry those base rarities. No roll fell in a level without a band.

Other counts: 4,758 grenade mod rolls drew at least one part whose stage window needs runtime state (treated as no bound; the
registry's 1,035 such entries); the relic hook removed candidates in 240 rolls, all from two DLC unique relics. Under the
registry's Complete merge no base-game relic loses a candidate to the hook, which is what the note predicts for reading (b).

## 4. The stock shield

`tools/prepare_stock_shield.py`'s default balance, level 8 and seed 1: its balance has one manufacturer option at stage 8, so the
roll draws no manufacturer and takes the same `CrtRand(1)` sequence. `roll()` gives the same part in every slot as that tool's own
roll and as the card it wrote (`local/slice/stock_shield_gear.json`, regenerated in this worktree), and the same prefix and title.
`prepare_stock_shield.py` is unchanged.

## 5. Tests

`tests/item_roll_test.py` (ctest `item-roll-synthetic`, 12 cases, no package reader needed) builds invented registries and a
dictionary lookup: a shield (reproducible rolls, a slot empty below its window, rarity from parts and a name part, a level in no band),
a grenade mod (two manufacturers with grade windows and a grade weighing 0, a part weighted for one manufacturer only, base rarity 5,
a non-interpolating balance taking the grade's level and definition), a class mod with three definitions, the relic hook (kept, removed,
and off for a shield class and for a relic that clears the flag), and refusals. The checker must reject a part from another slot, a part
out of its window, an emptied slot, a wrong level, rarity, manufacturer, name part or definition, and a part of another manufacturer.

## 6. UNVERIFIED and open

- Every rule above, in the running game. The census proves internal consistency only.
- The four stand-ins of section 2.
- A live check: spawned drops of one shield, one class mod and one relic balance, compared with these rolls' slot frequencies and
  rarity levels (`NATIVE_ITEM_RULES.md`, "What a session should read", items 1 and 2), and the `GrenadeMod_FlameSpurt` rarity 6.
