# Weapon parts catalog and the loot generator (2026-10-23)

AI-assisted (Claude Opus 5.5 orchestrating; Opus and Haiku sub-agents; review GPT-6-Luna). This record holds counts,
coverage and rules in our own words only. The catalog itself (object paths, values), the generator manifest, parity
reports and live dumps are game-derived and stay under ignored `local/` (`local/weapons/catalog/`,
`local/weapons/gen/`, `local/realgame/weapons/`, `local/realgame/gen_confirm/`).

Automated checks, live-game observations and rules read from native code are reported separately. Every rule
read from native code is `UNVERIFIED` in game unless a section says how it was confirmed.

## 1. Census

Tools: `tools/weapon_catalog.py` (`packages`, `crosscheck`, `summary`) over every installed package with
`ow-package`; `tools/real_game/scripts/weapon_parts_census.py` (bl-sdk `find_all` in the running game, read-only,
the same way as the RTSE catalog scripts); OpenBLCMM's dump database as the third source.

Scope: 2,010 `.upk` files scanned (base game and every installed DLC), 315 of them with weapon-generation data.
Two remaster texture packages are not valid packages (bad magic) and were skipped. Packages repeat export paths
heavily, so every count is of unique object paths.

| class | packages | OpenBLCMM | live (in game) |
|---|---|---|---|
| Weapon balances (incl. mission weapon balances) | 369 | 369 | 359 |
| Weapon parts | 908 | 908 | 886 |
| Weapon types | 89 | 89 | 35 |
| Manufacturers | 22 | 22 | 22 |
| Name parts (prefix and title) | 876 | 876 | 876 |
| Part list collections (not runtime) | 377 | 377 | 359 |
| Part list definitions | 28 | 28 | 18 |
| Gestalt meshes | 17 | 10 | 16 |
| Item pools (incl. keyed) | 2,014 | 2,015 | 1,739 |
| Item pool lists | 61 | 61 | 60 |

Parts per slot (packages): Body 168, Grip 39, Barrel 208, Sight 45, Stock 33, Elemental 32, Accessory 64 (Accessory1
and Accessory2 share parts), Material 310, unclassified 9. The slot comes from the part list that names the part,
else `PartType`, else the path. Per DLC (balances / parts / weapon types / item pools): base game 253 / 662 / 61 / 994;
Anemone 27 / 79 / 5 / 349; Aster 38 / 46 / 0 / 94; Orchid 18 / 37 / 5 / 69; Sage 12 / 30 / 8 / 66; Iris 7 / 10 / 6 / 165;
Gladiolus 8 / 24 / 0 / 39; Lobelia 4 / 13 / 3 / 24; Lilac 1 / 7 / 1 / 7; Allium 1 / 0 / 0 / 53; the other DLC folders
have no weapon balances.

**Coverage.**
- Packages and OpenBLCMM agree on every class except gestalts and one pool. The 7 gestalts OpenBLCMM lacks are in a
  remaster startup package; one path is a pool in one package and a balance in another, and the census keeps the
  first owner (4 such paths are listed).
- The live dump was taken in game (Maya loaded, saving blocked, saves unchanged afterwards). It sees only loaded
  objects, so its counts are lower bounds. That the missing ones are unloaded DLC content is `UNVERIFIED`.
- Live runtime part lists against the packages' part-list chains: 359 balances compared, 37 identical, 322 where the
  package chain is larger (consistent with slot replacement); in 2 Maliwan SMG balances the runtime list holds a
  body part absent from the package chain (`UNVERIFIED`, not explained).
- 17 parts are listed by no balance (mostly DLC vehicle and Anemone sniper parts).

## 2. Rules newly read from native code (2026-10-23)

Read statically from the executable (lane notes under ignored `local/analysis/weapon_gaps/`). The REA MCP tools
were tried first. `open_binary` loaded the executable but refused the project's existing analysis database, and
the analysis calls failed or timed out, so the reading went through the project's own headless Ghidra project
(`tools/ghidra/run.ps1`). The value function was finally reached through the class registration records, which
name each class's internal constructor; that constructor sets the vtable, and the exec thunks' vtable slots
resolve from it. The full list is in [NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) section 12. In short:

| gap (was "not read") | now | effect on cards and parity |
|---|---|---|
| sale value function | part total starts at the type's `BaseMonetaryValueModifier`, then parts, prefix, title (float steps); trunc; capped by the currency (Credits 99,999,999) | none: every gun type sets the modifier to 1 |
| attribute-slot value | grade = base grade (half up) + increases, max clamp then min clamp on the full grade; value = (grade - 1) * PerGradeUpgrade + BaseModifierValue | none on the golden cards; differs only when a clamp bites (negative zoom sums) |
| part-pick weight hook | gated by a definition flag that every weapon type leaves off (only relics use it) | none for weapons |
| pool grade choice and stage cap | first grade whose window holds the stage, weight interpolated across it; stage = min(stage, cap), cap 50 in normal and TVHM, the level cap in UVHM | stage 80 rolls change when capped (opt-in) |
| status chance on the card | the damage type's mapped presentation remaps the combined modifier, input clamped (slag [0, 3.33]) | slag chances print 0.1 higher; no golden card has slag |
| name-part expressions | all expressions must hold; `WeaponDamage_Is_<Element>` compares the weapon's damage type | 170 of 36,675 parity guns (elemental SMG barrel titles) change name; one confirmed live |

None of these is confirmed by a running-game check yet except where section 4 says so.

## 3. The generator

**Python** (unchanged rules, new plumbing): `tools/weapon_recipe.roll()` takes an `rng` and a `stage_cap`. Its
default generator is now `CrtRand`, the Microsoft C runtime `rand()` (state times 214013 plus 2531011, bits 16-30),
because the game's pick uses `rand() / 32767` and because it is trivially reproducible in C++. The seeding is ours: the
game's own random sequence is not reproduced. The existing slice tools (`weapon_slice_gear.py`,
`slice_npc_assets.py`) pass Python's generator explicitly, so the slice's pre-rolled guns do not change.
`tools/loot_pools.py` gained the same CRT-based pool roll (a listed pool fires when `rand() / 32767` is below its
probability; each Quantity draw walks the eligible entries' shares like the part pick).

**Manifest.** `tools/weapon_gen_manifest.py` decodes `Startup.upk` with the existing helpers and writes one
line-based text file (format in the tool's docstring) holding everything `roll()` and `evaluate()` read: 249
balances with merged slot candidates, 652 parts, 748 name parts, 33 weapon types, 261 attribute initializers, 46
attributes (10 of them runtime: weapon level, the part price total and eight `Weapon_Is_<Maker>`), 826 pools and 12
loot sources. It builds in about 16 s. Base game only: DLC balances are not in the manifest yet.

**C++.** `src/weapon_gen.{hpp,cpp}` (standard C++, in `ow-core`, so the UE module links it like
`inventory_navigation`) and the `ow-weapongen` command-line tool. Given a balance, pool or loot source, a game stage,
an optional stage cap and a seed it returns the parts per slot, manufacturer, type, gestalt fragments, material,
prefix and title, rarity, level, level line and the card stats with the card's rounding.

**Parity, Python against C++** (`tools/weapon_gen_parity.py`; it checks the identity fields the C++ echoes against each
request and exits 1 on any mismatch, Python error, missing comparison or misaligned output): every Startup weapon balance at stages 1, 8, 15, 30, 50
and 80 with 20 seeds each (29,880 guns), 7,680 pool rolls over 64 weapon pools and lists and the 6,795 guns they
dropped: 2,919,834 fields compared, strings and integers equal, floats bit for bit. **0 mismatches**, uncapped and
with a stage cap of 50. Not ported: the evaluator's diagnostic outputs (modifier list, unresolved attributes, merge
history, rule notes). Bitwise equality assumes no fused multiply-add and the same C runtime `pow`; the UE module
builds with the default precise floating point.

**Golden cards** (`tools/real_game/golden_card_compare.py`, live overlay): 69 / 69 / 68 of 69 in the main four stats /
every printed stat / every field, 0 unsupported (before this work 67 / 67 / 66 with 2 unsupported; one Vladof minigun
barrel lists an empty card presentation that made the evaluator raise).

## 4. Host and live confirmation

**Host.** `OpenWillowWeaponGen.{h,cpp}` loads the manifest once (`local/weapons/gen/weapon_gen_manifest.txt`, or
`-owweapongen=<path>`; a missing file turns the feature off with one log line) and converts a result into the
inventory's `FOpenWillowWeaponItem` the same way recipe-loaded items are filled. Shared item fields added for the
weapon-assembly lane: `Parts` (slot and part object path, slot order Body .. Material), `Material`, `GestaltMesh`.
Loot: the slice quest's turn-in reward is rolled from the stand-in's source pool at the mission level (seed from
`-owrewardseed`, default 1); when the manifest is missing or the roll drops no weapon the pre-rolled stand-in is used,
as before. `ow.RollGun <balance or pool> [stage] [seed]` puts a rolled gun in the backpack. Pool rolls in the host cap the gun's stage at 50 (normal mode and
TVHM; section 2, `UNVERIFIED` in game; UVHM is not modelled); the drops themselves use the raw stage.

Checks (in engine, this machine): `-owweapongentest` rolls a fixed list and logs the converted items; against
`ow-weapongen` 7 balance cases and a pool case agree on all 245 compared fields (floats within a relative 1e-4, since the
item struct stores floats; strings and integers exact). Quest reward path: seed 3 logs a generator-rolled Dahl assault
rifle, seed 1 (no weapon from the pool) and a missing manifest log the fallback. The quest self-test itself stops before
its turn-in checks on this machine because Marcus is not imported here, so pickup and turn-in with a generated gun are
**not** checked. `tools/test_inventory_actions.ps1`: 49 of 49.

**Live confirmation** (2026-10-23, `tools/real_game/gen_confirm*.py` and `gen_confirm_capture.ps1` driving the real game
through `tools/real_game/realgame.ps1`; saves backed up, saving blocked, saves identical afterwards; outputs under ignored
`local/realgame/gen_confirm/`). A sample of 44 generated guns (all six weapon types, eight manufacturers, common to
legendary and unique balances, stages 5 to 80, four extra Dahl assault rifles) was spawned in the game with exactly the
generated parts, name parts and level.

| check | result | what it confirms |
|---|---|---|
| spawned gun against the generator (balance, type, manufacturer, level, all nine parts, prefix, sale value) | 40 of 40 (first sample) | the game accepts the generated definition as is |
| title | 39 of 40, then 40 of 40 after the name-expression rule (section 2) | the element-gated title rule, on one gun |
| part frequencies, two common balances at stage 30, 400 game-rolled guns each | every rolled slot consistent with the generator's weights (chi-square, p from 0.17 to 0.67 over two sessions) | the weights, the flat 100 and the pick shape for these two balances only; 400 rolls cannot see a 10 to 20 % weight shift |
| item level of a game-rolled gun | equal to the stage on 1,600 of 1,600 | rule 4 (level = stage) |
| name choice of game-rolled guns | matches the name rule on 1,600 of 1,600 | rule 5, for manufacturer-only expressions |
| evaluated stats of the 44 spawned guns, read from the readied weapon | cooked generator equal bit for bit on 366 of 484 compared fields; with the live overlay 392 | see below |

**Stats, honestly.** The evaluated attributes of each spawned gun (damage, fire interval, reload, magazine, spread,
projectiles, shot cost, status values, value, rarity) were read from the game. With cooked data 366 of 484 compared
values equal the generator's exactly; 26 more mismatches are runtime data (the live overlay reproduces them: magazine,
damage, reload). **92 mismatches are not triaged.** Some are implausible for a rule error (a damage value off by a factor
of about 75, a shotgun read with one projectile), which points at the read (wrong or stale weapon object, a value read
before initialization) rather than the generator; until they are triaged, the stat rules stay `UNVERIFIED` on these guns.
The level requirement (44) and status chance (8) were not readable this way. No item card was captured: the session
could not open the inventory card, so card rounding was not checked live in this work (the 69 golden cards remain the
card evidence).

**Dahl assault rifle (cross-lane report, Weapons 3: live damage 9.5 % above the card).** The lane's coverage gun reads a
live `InstantHitDamage` 9.5 % above the cooked evaluation and bit-for-bit equal to the evaluation with the live overlay; a
rare Dahl rifle is 6.6 % above, also equal with the overlay; two rolled common Dahl rifles match cooked data exactly. So
the gap is the runtime data of section 7 of [NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) (origin still
`UNVERIFIED`), not a stack or card rule. On all 44 guns the damage attribute's value equalled its base value with Maya's
gear as is, so no skill or item modifier was on the attribute during the read. Aiming (zoom) effects were not measured.

## 5. Open

- DLC balances in the manifest; the census counts them, the generator does not roll them yet.
- Overpower levels at the full stage cap; the pick's use of the capped stage is implied, not read.
- The native code that walks the status presentation mapping (the mapping itself is data).
- The game's random sequence (ours is seeded per gun).
- Triage of the 92 untriaged live stat mismatches; a live card capture; Dahl and other aiming (zoom) effects.
- Runtime data: the generator evaluates cooked data. Whether the port should use the live values (they explain the Dahl
  gap) is the maintainer's decision recorded in NATIVE_WEAPON_RULES.md section 7.
