# Weapon balances, card stats and loot for the Sanctuary slice (2026-10-01)

AI-assisted (Claude). Everything below was recovered from the installed packages with
`ow-package --properties` / the new `--properties-batch`, and checked against oracles named in each
section. Game-derived output (recipes, tables, audit reports) stays under ignored `local/`; this
record holds only rules, counts and identities. Automated checks, oracle checks and in-game checks
are reported separately. **No in-game check was made in this pass** (the game was not launched).

Tools: `tools/weapon_balance.py` (legal parts, runtime part-list crosscheck),
`tools/weapon_card_audit.py` (card audit), `tools/loot_pools.py` (pool expansion, display oracle,
seeded rolls), `tools/weapon_slice_gear.py` (slice guns and loot table), and changes to
`tools/weapon_recipe.py` / `tools/weapon_stats.py`. Reader: one additive CLI mode,
`ow-package <pkg> --properties-batch <index-file> --property-offset 4 [--array-schema f]` (same
decoder as `--properties`, one process for many exports; `src/cli.cpp` only).

## 1. The lent mission pistol

`GD_Z1_RockPaperGenocideData.MW_RockPaper_Fire` (`MissionWeaponBalanceDefinition`, export 26179 of
`Startup.upk`) has an empty own part list and `BaseDefinition`
`GD_Weap_Pistol.A_Weapons_Elemental.Pistol_Maliwan_2_Fire`. Chain, root first:

| balance | part list mode | enabled slots |
|---|---|---|
| `GD_Weap_Pistol.A_Weapons.Pistol_Maliwan` | Additive (omitted) | Body, Grip, Barrel, Sight, Elemental, Material |
| `..._Maliwan_2_Uncommon` | Selective | Body, Barrel, Sight, Accessory1, Material |
| `GD_Weap_Pistol.A_Weapons_Elemental.Pistol_Maliwan_2_Fire` | Selective | Elemental |
| `MW_RockPaper_Fire` | Additive (omitted, empty) | none |

Weapon type `GD_Weap_Pistol.A_Weapons.WeaponType_Maliwan_Pistol`, manufacturer
`GD_Manufacturers.Manufacturers.Maliwan` (root `Manufacturers[0]`, one grade, `ExpLevel` 0).

Legal parts (`tools/weapon_balance.py parts`):

- **Fixed by the balance:** Body `Pistol_Body_Maliwan_2`, Elemental `Pistol_Elemental_Fire`,
  Material `Mat_Maliwan_2`.
- **Rolled:** Grip (8 manufacturers' grips) and Barrel (8 barrels), every entry weight 0 in the data;
  Sight (None 200, eight others 10 each: 71.4% / 3.6%); Accessory1 (None 200, seven others 10 each:
  74.1% / 3.7%). 4,608 combinations. Stock and Accessory2 are disabled.

Oracle: the game's own merged list for this balance (`RuntimePartListCollection`, see section 2)
matches ours entry for entry. The selection inside a rolled slot is native code
(`ItemPool`/balance spawning functions are `native` in `WillowGame.upk`); the slice uses a seeded
weighted pick, uniform when a slot weighs 0 in total: **UNVERIFIED**.

Level: the mission is `bGameStageLocked` to region `GD_GameStages.Zone1.Sanctuary`, whose playthrough-1
band in `GD_GameStages.Balance.Balance_P1_Zone1` is 7-9 (mission override for
`M_Ep4_WelcomeToSanctuary`: 8-11). The slice uses level 8, inside both. How the game picks the level
of a lent weapon is native: **UNVERIFIED**.

The roll used for the slice (seed 1, level 8) and the card numbers over all 4,608 legal combinations
are in `local/items/slice/slice_mission_pistol_fire.json` (`stats.card`, `stat_ranges`). Shape of
the result: fire element (`DmgType_Incendiary_Impact`, status `STATUS_EFFECT_Ignite`, 5 s), two ammo
per shot (type `WeaponShotCost` +1 on the class default 1), bullet speed 12,000 from
`GD_Weap_Pistol.FiringModes.Bullet_Pistol_Maliwan` unless the barrel overrides the firing mode.

## 2. Part-list merge: crosscheck with the running game (249 balances)

The running game keeps every balance's merged list in `RuntimePartListCollection` (built at load,
not cooked). OpenBLCMM's object dumps (the game's own `obj dump`, local only; provenance in
`docs/verification/BLCMM_DUMP_CROSSCHECK.md`) contain it. `weapon_balance.py crosscheck` compares, per
slot: enabled flag, part order, and each entry's min/max stage and weight data resolved through
each list's own `ConsolidatedAttributeInitData`.

Result over all 249 weapon balances in `Startup.upk` (242 `WeaponBalanceDefinition`, 7
`MissionWeaponBalanceDefinition`; 1,824 slots, 7,340 entries): **243 identical, 6 differ, 0 without a
dump**. The six:

- `RL_{Bandit,Maliwan,Tediore,Torgue,Vladof}_4_VeryRare`: the Material slot. The cooked own part
  list has the `_4` material with the slot *disabled*; the dumped own part list has it enabled. The
  object itself differs between cooked data and the dumping session.
- `SMG_Maliwan_3_Rare`: Body. Cooked and dumped own lists agree (`VarC`), but the runtime list holds
  `VarC` with different stage indices plus `VarB`.

Both kinds of difference are in data the running game changed, not in our merge; the likely cause is
Gearbox's online hotfixes being active when the dumps were taken (**UNVERIFIED cause**). The merge
rule (root first; Selective replaces enabled slots, Additive appends) is therefore checked on 243
balances. `EPRM_Complete` does not occur in this package (209 Selective, 40 Additive lists), so its
reading stays **UNVERIFIED**.

## 3. Card stats: audit and fixes

### Oracle

`tools/weapon_card_audit.py` reads weapon cards from the existing UI trace (the game's own
`ItemCardGFxObject` callbacks; local `local/ui/traces/`). Six distinct weapon cards were available
(levels 43-51): a Bandit launcher, a Maliwan shock sniper rifle, a Dahl SMG, a Vladof fire pistol, a
Bandit slag SMG and a Dahl assault rifle. The cards are the player's own items, so per-card values
stay in ignored `local/weapon_audit/`. Parts are not on a card: for each card the tool takes every
balance of the card's type and manufacturer whose title list can produce the title, enumerates every
legal part combination (all stages; element restricted to the card's element icon), evaluates each
at the card's level requirement, and compares the numbers **as the card prints them**.

### Before (HEAD `weapon_stats.py`)

1 of 6 cards had a combination reproducing damage, fire rate, reload and magazine together (the
assault rifle). Status rows (chance, damage per second) and projectile count were not modelled at
all. Wrong or missing pieces found:

1. **Scale combination.** `(base + PreAdd) * (1 + sum Scale)` cannot reproduce the two elemental
   cards: their fire rate, reload, magazine and accuracy all match one part set, but damage does
   not. Splitting the scales, `(base + PreAdd) * (1 + sum of positive Scales) / (1 + sum of |negative
   Scales|) + PostAdd`, reproduces them. (Ablation: with every other fix below but the old rule,
   still 1 of 6.)
2. **The weapon type's own `WeaponAttributeEffects` were ignored** (Maliwan status chance and status
   damage, +1 shot cost, Jakobs/Hyperion projectile speed).
3. **Fields the type omits** (cooked data drops class defaults) became `None`: the Maliwan pistol
   type sets neither `FireRate` nor `ReloadTime`, so the mission pistol had no fire rate. Defaults now
   come from `WillowGame.upk` `Default__WeaponTypeDefinition` (decoded: FireRate 0.4, ReloadTime 2.1,
   Spread 0.01, ProjectilesPerShot 1, BaseStatusEffectChanceModifier 1).
4. **No projectile count, no status rows, no display rounding.**

### After

| card | main four stats | accuracy | projectiles | status chance | status dmg/s | sale value |
|---|---|---|---|---|---|---|
| launcher (Bandit) | reproduced (6 combos) | yes | yes | n/a | n/a | **no** (launchers still fail) |
| sniper (Maliwan, shock) | reproduced (25) | yes | n/a | yes | yes | yes (3 of 25) |
| SMG (Dahl, unique) | damage **1 point off**, rest yes | yes | yes | n/a | n/a | yes (somewhere) |
| pistol (Vladof, fire) | reproduced (9) | yes | n/a | yes | yes | yes (2 of 9) |
| SMG (Bandit, slag, legendary) | fire rate, reload yes; **damage and magazine no** | yes | yes | yes | n/a | yes (somewhere) |
| assault rifle (Dahl) | reproduced (120) | 90 of 120 | n/a | n/a | n/a | yes (2 of 120) |

4 of 6 cards are reproduced in every printed stat by one part set; 6 of 6 for accuracy and for
the status rows they carry. Independent check: the audit picks parts only by stats, yet in all
four full matches the matched accessory/grip is exactly the part whose `PrefixList` gives the card's
prefix word.

Rules now in `weapon_stats.py`, with their status:

- Scale rule `split` (`SCALE_RULE`, `combine()`): fitted on these cards, **UNVERIFIED** (the
  combining code is native). The old rule stays selectable for comparison.
- Status chance % = the status effect's `DMGSURFACE_Generic` `BaseChance` *
  `WeaponBaseStatusEffectChanceModifier` * `WeaponStatusEffectChanceModifier` (class default 1);
  status damage/s = `WeaponStatusEffectDamage` (type `StatusEffectDamage` = `Init_WeaponDamage`, plus
  effects), only for damage-over-time effects. Reproduces three cards (fire, shock, slag; one of them
  through the Maliwan sniper type's `PreAdd` 0.2 on status damage). Per-surface chances and duration
  are emitted for gameplay; how the native code uses them per shot (`GetFireIntervalChanceModifier`
  and friends are `native`) is **UNVERIFIED**.
- Projectiles = `WeaponProjectilesPerShot` (type `ProjectilesPerShot` + part effects): reproduced on
  three cards.
- Display: damage rounded up, magazine rounded down, the rest to one decimal: consistent with every
  card, **UNVERIFIED** as a rule.
- `accuracy_known` is now true (accuracy reproduced on 6 of 6); pistol and sniper price calculators
  are now "checked" (integer-exact matches among otherwise matching combinations).
- Damage type: the elemental part's `CustomDamageTypeDefinition`, else the type default (precedence
  between parts **UNVERIFIED**); firing mode: a barrel's `CustomFiringModeDefinition`, else the type
  default; `projectile_speed` = firing-mode `Speed` * `WeaponProjectileSpeedMultiplier` (**UNVERIFIED**
  product).
- Class defaults `ShotCost` 1, `StatusEffectChanceModifier` 1, `ProjectileSpeedMultiplier` 1 come from
  the OpenBLCMM dump of `Default__WillowWeapon`: the cooked object holds them as
  `IntAttributeProperty`/`FloatAttributeProperty` tags that `ow-package` does not decode yet.
- The attribute -> `WillowWeapon` property mapping is decoded from each attribute's
  `ObjectPropertyAttributeValueResolver.PropertyName` (e.g. `WeaponReloadSpeed` -> `ReloadTime`).

### Failures, enumerated

- Dahl SMG (unique, `PreAdd` -0.4 on damage): one point low after rounding up. Ignoring that PreAdd,
  or applying it after scaling, would match; one card cannot decide. **Open.**
- Bandit slag SMG (legendary): no legal combination reproduces damage and magazine together with the
  other stats; the barrel's cooked and dumped effects agree. **Open** (a runtime change to this item,
  or a missing term).
- Launcher sale value (third launcher failing, after two on 2026-09-29). **Open.**
- Jakobs pistols/rifles: types carry `AutomaticBurstCount` 1 and `FireRate` 0.06; no Jakobs card was
  observed, so their fire-rate display is unchecked.
- Not derived: white fun-text lines, critical-hit and burst values, player-side modifiers (no card
  needed them).

No community/wiki formula was adopted; nothing here comes from one.

## 4. Loot

- `GD_Population_Psycho.Balance.PawnBalance_TargetDummy` has **no item pools** (no
  `DefaultItemPoolList`, `DefaultItemPoolIncludedLists` or playthrough custom pools), and its
  `CharClass_TargetDummy` has none. **The target dummy drops nothing in stock data.**
- `GD_Population_Loader.Balance.Unique.PawnBalance_TargetDummyBot` lists only
  `GD_ItempoolsEnemyUse.Shields.Pool_Shields_Standard_EnemyUse` (probability 1), an enemy-use
  equipment pool.
- Slice loot source chosen: **`GD_Itempools.ListDefs.StandardEnemyGunsAndGear`**, the list a standard
  enemy includes (e.g. `GD_Population_Psycho.Balance.PawnBalance_Psycho`). Placed Sanctuary
  containers (storage lockers, dumpsters, laundry machines, Dahl ammo crates, strong box) exist
  through `InteractiveObjectBalanceDefinition` loot lists; they are identified, not decoded.

`tools/loot_pools.py` expands a source into a tree and a flat table (expected items per firing) and
rolls it from a seed. Weight evaluation covers constants, `ValueFormula`, enabled range restrictions,
`ConstantAttributeValueResolver`, `ConditionalAttributeValueResolver` (conditions on
`NumberOfPlayers`, default 1) and `DesignerAttributeDefinition` `BaseValue`; anything else is listed
as unresolved and never enters a roll.

Oracle: each cooked `BalancedItems` entry stores `ProbabilityDisplayString`, the share the editor
computed. Recomputed in an editor-like context (designer attributes 0): **728 pools, 2,554 entries:
2,505 agree to the printed 0.01%, 48 unresolved** (ammo-need and health odds that depend on the
player's state), **1 differs**: a single-entry pool whose only weight is a designer attribute; the
editor prints 100% where our share is 0.

For a standard enemy at stage 8, one player (in `local/items/slice/slice_manifest.json`, `loot`):
`Pool_GunsAndGear` fires with 0.085 (conditional on player count); inside it weapons 50.8%, shields
22.8%, grenade mods 15.2%, class mods 11.2% (the common class-mod pool is empty in cooked data, so that
branch yields nothing), relics gated off by game stage. Money 0.25, eridium 0.0015 / 0.008, vehicle
skins 0.05; health and ammo odds unresolved.

**UNVERIFIED** (native `ItemPool.SpawnBalancedInventoryFromPool`): each list entry firing independently
with min(1, probability); a pool picking `Quantity` entries by weight among eligible ones; game-stage
gates applying to a whole pool; the designer attribute keeping its default for a standard enemy.

## 5. Slice gear (local manifest)

`python tools/weapon_slice_gear.py --game <BL2> [--gestalt local/gestalt --gltf <UModel gestalt dir>]`
writes `local/items/slice/`:

- `slice_mission_pistol_fire.json` and four pool-rolled guns (`slice_pistol`, `slice_smg`,
  `slice_assault_rifle`, `slice_shotgun`: one per `GD_Itempools.WeaponPools.Pool_Weapons_<Type>`,
  seed 1-4, level 8), each with `<id>.gltf` when the gestalt inputs are given;
- `load_order.txt` (mission pistol first);
- `slice_manifest.json` (no `stats` key, so the host's recipe loader skips it).

Recipe schema: unchanged fields as read by `UOpenWillowInventory::LoadRecipes` (`name`, `balance`,
`gestalt_fragments`, `stats.card.{damage, fire_rate, reload_time, magazine, spread, shot_cost, spin_up,
spin_mode, spin_start_interval_scale, element, rarity, manufacturer, accuracy, accuracy_known,
sale_value, sale_value_known, fun_stats}`), plus additive fields:

- recipe: `type` (card type label), `legal_parts` {stage, combinations, slots{slot: {fixed,
  all_zero_weight, candidates[{part, weight, share}]}}}, `provenance` {kind `mission_weapon` |
  `pool_roll`, mission/objective or pool_chain, seeds, level, level_rule, note}; the mission pistol
  also `stat_ranges` {combinations, stats{field: {min, max}}} as printed;
- `stats.card`: `projectiles`, `damage_type`, `firing_mode`, `projectile_speed`, `status_effect`,
  `status_effect_definition`, `status_chance` (%), `status_chance_by_surface` {generic, flesh, armor,
  shield}, `status_dps`, `status_duration` (s), `display` {damage, magazine, fire_rate, reload_time,
  accuracy, status_chance, status_dps, projectiles} as the card prints them.

Manifest: `{schemaVersion: 1, kind: "openwillow.slice_gear", level, level_rule, items[{id, recipe,
balance, name, type, provenance, mesh, card{...}}], loot{source, note, stage, pools[{pool,
probability, by_class, tree, items{balance: expected count}}], unresolved}}`.

The existing demo recipes under `local/items` change numbers when re-evaluated with this
`weapon_stats.py` (scale rule, type effects, Maliwan shot cost 2); their schema does not change.

## Reproduce

```powershell
cmake --build build --config Release
$G = "$env:OPENWILLOW_BL2\WillowGame\CookedPCConsole"
python tools/weapon_balance.py --reader build/Release/ow-package.exe --package "$G\Startup.upk" --output local/weapon_balance/crosscheck.json crosscheck
python tools/weapon_balance.py --reader build/Release/ow-package.exe --package "$G\Startup.upk" parts GD_Z1_RockPaperGenocideData.MW_RockPaper_Fire
python tools/weapon_card_audit.py --reader build/Release/ow-package.exe --package "$G\Startup.upk" --trace local/ui/traces/<trace>.jsonl
python tools/loot_pools.py --reader build/Release/ow-package.exe --package "$G\Startup.upk" --output local/loot/display_check.json check
python tools/loot_pools.py --reader build/Release/ow-package.exe --package "$G\Startup.upk" table GD_Itempools.ListDefs.StandardEnemyGunsAndGear --stage 8 --seed 1
python tools/weapon_slice_gear.py --game $env:OPENWILLOW_BL2
```

## Automated checks (synthetic fixtures)

`tests/weapon_recipe_test.py` (7), `tests/weapon_stats_test.py` (18), `tests/weapon_balance_test.py`
(6, includes the card-trace parser), `tests/loot_pools_test.py` (8). Not registered with CTest
(`CMakeLists.txt` unchanged); run them with `python tests/<name>.py`.
