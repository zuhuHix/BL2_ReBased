# Loot on death on the VM: a killed bullymong rolls its item pools (plan step S-g, 2026-10-09)

AI-assisted (Claude), implementer lane Sg. Worked from the own-words note [NATIVE_LOOT.md](NATIVE_LOOT.md), the live records [COMBAT_GOLDEN_TRACE.md](COMBAT_GOLDEN_TRACE.md)
(section A step 7, lane L3; the bullymong paragraph, lane L5b) and the installed script (`research/script_disasm.py`: `WillowPawn.DropLootOnDeath`, `WillowPawn.AddPoolItems`,
`PopulationFactoryBalancedAIPawn.SetupBalancedPopulationActor`). No decompiler output was read. Every rule below is `UNVERIFIED` as a statement about the native code unless a row says
which live observation it matches; "reproduces a live row" means the VM's flag or number equals the recorded live one, not that the native does exactly this.

## What drops now

`--combat-run` spawns a bullymong with its pawn balance (`PawnBalance_PrimalBeast`, `_PrimalBeastAdult`, `_PrimalBeast_Monglet`, `_PrimalBeast_Slinger`), which runs the stock spawn's loot
setup, and the lethal hit of the installed `Died` chain now reaches a real death roll instead of a log line:

1. **Spawn (`CombatSystem::setupLoot`).** The pawn's item game stage is its experience level when the balance's `BaseItemGameStage` is 0, otherwise its game stage (the class default 1 is
   used by all four bullymong balances), plus the truncated `ItemGameStageOffset` (the adult's is a random integer, `Random_Minus02-Plus01`; the others have none). Then the script's call
   `AIPawnBalanceDefinition.SetupPawnItemPoolList(pawn)` runs through the VM and the bound native fills the pawn's `ItemPoolList`: the included lists first (`StandardEnemyGunsAndGear`,
   depth first), then the own pool list; the playthrough entry's custom lists replace the default ones when not empty; a balance with no playthrough entry gets no list.
   The `spawn` step reports `loot_setup` (item stage and the pool list).
2. **Death (`CombatSystem::dropLoot`, replaces the installed script `WillowPawn.DropLootOnDeath`).** The loop is the script's: for every pool of the list whose `bAutoReadyItems` is false
   (a true one is carried gear rolled at spawn, skipped here) it draws `FRand` first, then evaluates the pool's probability with the pawn's controller as context (a probability definition
   named `AmmoDrops_PerPlayer` is multiplied once per death by 1.0, the first-playthrough value), and *tries* the pool when the chance is above 0 and the draw is at most the chance:
   `ItemPool.SpawnBalancedInventoryFromPool(pool, item stage, awesome level 0, pawn)`. Of the items it returns, one with `bDropOnDeath` is dropped (unless the pawn is below the level's KillZ
   and the item is a weapon), the others destroyed.
3. **The pool roll (`CombatSystem::spawnFromPool`, bound as the native `ItemPool.SpawnBalancedInventoryFromPool`).** NATIVE_LOOT.md's rules: `Quantity` evaluated and truncated; the stage
   gate (minimum / maximum stage attributes, then at least one reachable entry; a failing gate returns false and draws nothing); one flat weighted list (nested pools with weight 0 unless
   they pass the gate, an empty nested pool counts as absent so a balance on the same entry is used, a balance entry becomes one candidate per manufacturer with weight
   modifier x entry weight / count, the modifier interpolated across the grade's stage window, candidates of 1e-8 or less dropped); `Quantity` picks with replacement by
   `total x rand() / 32767` and the first inclusive interval; a nested pick rolls the inner pool again with `bDropOnDeath` = entry flag AND the caller's and its result ignored; an item has
   the level `min(stage, 50)` and `bDropOnDeath` = entry flag AND caller's flag.
4. **No inventory, no pickup.** An item is a record: balance, manufacturer, inventory class (from the inventory definition), level, `drop_on_death`, `dropped`. `InitializeInventory` (parts,
   stats) is not run. The native called from outside (the `rollpool` step) makes stand-in objects of the item's class with `bDropOnDeath` set.
5. **The random.** One seedable stand-in for the engine's `rand()`: the Microsoft C runtime's linear congruential generator (15 bits, `rand() / 32767`), default seed 20261009 (fixed per
   run, so a run repeats), `seed:<n>` restarts it. It feeds the death roll's `FRand`, the pool picks and the random variance of a quantity or of the stage offset. The game's own seed is
   unknown; `FRand` and `Rand` of the VM core (a different generator, used by other scripts) are not rebound, so nothing else in a run changes.

## The data the bullymongs roll (`GD_Itempools.ListDefs.StandardEnemyGunsAndGear`, one player)

| Pool | Chance per death | Stage gate | Entries |
|---|---|---|---|
| `Pool_GunsAndGear` | 0.085 (the one-player entry of a number-of-players table) | none | guns, shields, grenade mods (weapon balances) |
| `Pool_VehicleSkins_All` | 0.05 | none | vehicle skins, `bDropOnDeath` false |
| `Pool_Money_1or2` | 0.25 | none | `Money_Little` (weight 100), `Money_Big` (weight 10, modifier 0.5 to 1 across the window); quantity a random integer in -1 to 2, at least 1 |
| `Pool_Eridium_Bar` | 0.0015 | none | eridium bar |
| `Pool_Eridium_Stick` | 0.008 | minimum stage 7 | eridium stick |
| `Pool_Health_All` | 0.03 at 40 percent health or more, up to 0.25 below (stand-in, below) | none | two nested drink pools |
| `Pool_Ammo_All_NeedOnly` | 0.2 (`AmmoDrops_PerPlayer` 1.0 x 0.2) | none | ammo pools weighted by need |
| `Pool_Ammo_All_Emergency` | 0.2 | none | ammo pools weighted by need (emergency weights) |

The evaluator (`src/progression.*`) gained what these need: the random variance of a definition (and a purely random definition), `RandomAttributeValueResolver`,
`ConstraintAttributeValueResolver`, `ConditionalAttributeValueResolver` (the list is read although its `bEnabled` is false in the data, the reading `tools/loot_pools.py` checks against
the editor's own shares), `AmmoDropWeightAttributeValueResolver`, designer attributes (they keep their base value), and the game-info and designer context resolvers. All of it throws
"unsupported" as before when no random source is set, which only the death setup does.

## Reproduce

```
cooked="$OPENWILLOW_BL2/WillowGame/CookedPCConsole"
# one kill: loot appears as "loot" on the hit step and as rows loot.<class>.L<level>.<k>.*
build/Release/ow-package.exe "$cooked/Sanctuary_Dynamic.upk" --combat-run --cooked "$cooked" maya:8 spawn:adult:7:6 \
    hit:0:9999:GD_Impact.DamageType.DmgType_Normal:Shotgun > kill.json
# the loot rows of tools/verify_combat_pawn.py (seeded; about 2 minutes, most of it the first load of Ice_Dynamic)
OUT=loot.json local/p2/Sg/run_loot.sh        # a local script holding the step list described below
python tools/verify_combat_pawn.py --vm loot.json
python tests/combat_test.py build/Release/ow-package.exe     # synthetic: the toy item data (hand-worked rules, mutation-checked)
```

The loot run is `seed:20261009 maya:8 spawn:adult:7:6`, then `rollpool:0:<pool>:<stage>:200` for `Pool_Eridium_Stick` (6 and 7), `Pool_Ammo_All_Emergency`, `Pool_Ammo_All_NeedOnly`,
`Pool_Health_All`, `Pool_GunsAndGear`, `Pool_VehicleSkins_All` (6), `Pool_Money_1or2` (6 and 7), then `lootbatch:adult:7:6:500:adult`, `lootbatch:baby:6:6:500:baby`, `mayahealth:0.2`,
`lootbatch:baby:6:6:300:low`. A batch is that many fresh pawns each killed by one lethal shotgun hit of Maya's (the same spawn and hit steps as a single kill), the death rolls summed.
`--vm` takes several JSONs (`--vm all_rows.json loot.json`) and merges their rows.

## The live rows and what the VM does with them

Rows of `tools/verify_combat_pawn.py` (`loot.*`; the comparator also reports the pool chances against the numbers above). Live: L5b (eight AI deaths, five by the player) and L3.

| Live observation | Source | VM | Status |
|---|---|---|---|
| `Pool_Ammo_All_Emergency` tried and returned false, 3 times (stages 4 and 6) | L5b x2, L3 | 200 calls at stage 6: 0 true; in the batches tried 89 / 500 (adult), 102 / 500 (baby) and 0 true | reproduces (all weights are 0 for a player whose ammo is not low) |
| `Pool_Ammo_All_NeedOnly` tried and false | L5b | 200 calls: 0 true; tried 96 / 500, 101 / 500, 0 true | reproduces |
| `Pool_Eridium_Stick` tried and false at stage 6 | L5b | 200 calls at stage 6: 0 true (minimum stage 7); at stage 7 200 / 200 true (rule row, no live observation) | reproduces |
| `Pool_Health_All` tried and true, twice, a health pickup each | L5b | 200 / 200 true; items dropped = tried (15, 9, 35) | reproduces the return and the drop; how often it is tried live is not comparable (below) |
| `Pool_Money_1or2` true | L3 (adult stage 7, baby stage 6) | 200 / 200 true at stages 6 and 7; items dropped = made | reproduces the flag |
| `Pool_Money_1or2` made 1 item for a baby | L3 | quantity 1 is 3 in 4 of the calls | reproduces (reachable) |
| `Pool_Money_1or2` made **3** items for an adult | L3 | quantities seen: 1 (92) and 2 (34) in 126 calls; 3 is not reachable | **OPEN: not reproduced** |
| `Pool_GunsAndGear` true (a boss, one weapon) and `Pool_VehicleSkins_All` true with an item that got no pickup | L3 | 200 / 200 true each; skins: 23 items, 0 dropped (`bDropOnDeath` false); a gun is a weapon balance at the stage, `AR_Torgue@6` in a probe | reproduces |
| Item stage of a baby: 6 (its game stage) | L3, L5b | 500 / 500 at 6 | reproduces |
| Item stage of an adult at game stage 6: 4 and 7 both seen | L3 (7), L5b (4) | stages 4 to 7 with 114 / 113 / 148 / 125 of 500 (each a quarter within the band) | reproduces both ends seen |
| Item level equals the roll stage (Money 7 at 7, a pistol 6 at 6) | L3 | 0 mismatches in 500 adult rolls | reproduces |

Empirical rates against the data chances (500 adult kills, 500 baby kills, seed 20261009; "tried" is the chance roll passing): tried / 500 for adult and baby: Guns 41 and 41 (42.5 expected),
Skins 23 and 23 (25), Money 126 and 125 (125), Bar 0 and 1 (0.75), Stick 2 and 5 (4), Health 15 and 9 (15), NeedOnly 96 and 101 (100), Emergency 89 and 102 (100). All lie inside the band
of four binomial standard deviations plus one that the comparator uses (over the three batches the 18 pool-and-batch pairs with an expectation of 10 or more have a chi-square of 18.1 for
18 degrees of freedom, mean z -0.67; the generator itself, run alone for 200,000 draws per pool, is within 1.4 standard deviations everywhere). Returned true / items made (adult and baby):
Guns 41 / 41 and 41 / 41, Money 126 / 160 and 125 / 161, Health 15 / 15 and 9 / 9, Skins 23 / 23 each (none dropped), Ammo 0 true, Eridium Stick 1 true of 2 tried (adult, only a roll at item
stage 7 passes the gate) and 0 true of 5 (baby, stage 6). The adult money quantities were 1 x 92 and 2 x 34 (0.27 of the calls made 2; the data's range gives 0.25).

## UNVERIFIED, stand-ins and what does not reproduce

- **Quantity 3 of the money pool (OPEN).** The data makes the quantity an integer random between -1 and 2, at least 1; no mapping of `rand()` onto that range can give 3. The item stage offset
  (-2 to +1) reached both ends live, which fits an inclusive range, so the inclusive reading is kept and the live 3 is reported as OPEN rather than adjusted. Either the integer
  randomization maps wider than its bounds, or the quantity is changed after the evaluation (by the pawn, the stage, the number of players, a multiplier of the money). Analyst read below.
- **The health drop weight.** `AmmoDropWeightAttributeValueResolver` reads the local player's pool of a resource. The VM's reading: at or above the resource threshold (0.4 for health) the
  above-threshold weight (0.03); below it a weight rising linearly from the minimum (0.1 at the threshold) to the maximum (0.25 at empty); no such pool the no-pool weight (0 here), so a
  pawn killed with no player gets no health or ammo drops. The shape and the direction are guesses. At 20 percent health it gives 0.175 (a rule row; 35 tried of 300 in the seeded batch,
  expected 52 +- 7, which is the one batch figure a little outside two standard deviations of the 18; the other pools of that batch are within 1.8). The live game's two health drops in eight
  deaths cannot be compared: Maya's health at those kills was not recorded, and she was down repeatedly.
- **The ammo pools.** Only the health pool of the local player is modelled, so every ammo weight is the no-pool weight (0 for need and emergency): the ammo pools always return false. The live
  game rolled the same false results while Maya's ammo was high; what a player with low ammo gets is not modelled.
- **`Pool_Eridium_Stick` tried once in the 13 live deaths with a death roll** (eight in L5b, five in L3) against a chance of 0.008 here (the chance of at least one in 13 is 10 percent).
  Possible, but the live chance could be higher (a multiplier not found in the data); the live draws were not recorded.
- The **random** is a stand-in; the exact mapping of a draw onto an integer range and onto the weighted pick (inclusive intervals, single precision) is NATIVE_LOOT.md's, `UNVERIFIED`.
- **Not run:** `InitializeInventory` (weapon parts, item stats and names), `AddPoolItems` and `AddDefaultInventory` (carried gear: the bullymong list has no `bAutoReadyItems` pool, so none is
  skipped), the stage variance path (a variance formula is reported as not implemented), the third-playthrough level cap, the chest and mission-reward rolls, shop quantities, the item
  toss and the pickup (`DropFrom`, `CreatePickup`): the host side.
- **Natives bound** (`src/combat_loot.cpp`, only with `enableDeaths`, so the slice is unchanged): `ItemPool.SpawnBalancedInventoryFromPool`, `AIPawnBalanceDefinition.SetupPawnItemPoolList`;
  `AddToItemPoolList`, `GetPlayThroughIndex`, `GetAmmoDropsPerPlayerMultiplier` and the two balance getters have their rule inside the C++ and are not bound as natives (nothing calls
  them). `WillowPawn.DropLootOnDeath` is overridden (script), as before.

## Analyst-read requests (names; what the code expects)

1. The integer branch of the attribute definition's random variance (and `RandomAttributeValueResolver`): the mapping of `rand()` onto an inclusive range, and whether anything is added to
   a pool's `Quantity` afterwards. Live: `Pool_Money_1or2` made 3 items for an adult at item stage 7 (L3).
2. `AmmoDropWeightAttributeValueResolver`: which player's pool (the local one, or the killer's), the threshold comparison, the interpolation below it, and the no-pool case; the pools of
   the ammo resources (what the weights of the need and emergency pools are for a player with low ammo).
3. `ConditionalAttributeValueResolver`: whether the `bEnabled` flag of its expression struct matters (the gun drop odds list is disabled in the data).
4. `ItemPool.SpawnBalancedInventoryFromPool`: the return value for a quantity of 0 or less and for a partly failing roll; the optional `OuterPoolChance` (read as having no effect).
5. `InventoryBalanceDefinition` item initialisation for the dropped item (parts, stats), already NATIVE_WEAPON_RULES.md's; the host needs it to show an item, not this step.
