# Native weapon generation rules: attribute stack, card display, part pick, level, names, value (2026-10-02)

AI-assisted (Claude). Behaviour notes written from a local reading of `Borderlands2.exe` in Ghidra and of the installed
script with `ow-package --disasm`, under the policy in [LEGAL.md](../LEGAL.md) ("Analysing the executable") and
[NATIVE_ANALYSIS.md](../NATIVE_ANALYSIS.md). Nothing below is listing, pseudo-code or an address. Functions are named by
their registered native name, their script name, or by what they do. Enum orders and class defaults were decoded from
the installed packages with the project reader and are quoted only where a rule needs them. Field names were matched to
the native code with `tools/ghidra/class_layout.py`; every field used below landed on a field of the expected type.

**Every rule here is `UNVERIFIED` in game.** "Read" means read from native code or script, not confirmed by running the
game. The only running-game evidence is the set of weapon cards in the local UI traces (section 8); each section ends
with the in-game observation that would confirm it. Game-derived output (decompilation, audits, census) is under ignored
`local/weapons/` and `%USERPROFILE%\bl2-analysis\out\weapons`.

Implemented in `tools/weapon_recipe.py`, `tools/weapon_stats.py`, `tools/weapon_card_audit.py` (section 9).

| # | Rule | Source | Confidence |
|---|---|---|---|
| 1 | Modifier stack: `(base + PreAdd) * (1 + up) / (1 - down) + PostAdd`, no clamp, single precision, integers truncate | native | high (read; 9 of 9 cards) |
| 1 | Effect order: type, parts in slot order, attribute slots, prefix, title | script | high (read) |
| 1 | Slot value = `BaseModifierValue + PerGradeUpgrade * sum(GradeIncrease)` on activated slots | grade bookkeeping native; value formula not read | medium (form fits 9 of 9; adding the base grade fits 0 of 9) |
| 2 | Card rounding from each stat's presentation data; Float = half up to `FloatPrecision` decimals | native + data | high (read; 9 of 9 cards) |
| 2 | Status rows one decimal, half up | observed only | medium |
| 3 | Part weight: empty `Manufacturers` list = flat 100; game-stage window on truncated bounds | native | high (read; no running-game check) |
| 3 | Weighted pick: zero weights dropped, duplicates keep the later weight, running-interval walk | native | high (read; no running-game check) |
| 4 | Item level = the game stage it spawned at (`bInterpolateExpLevel` default true) | native + data | high (read) |
| 4 | Rarity level = sum of truncated part rarities; tier from `RarityLevelColors` | native + data | high (read) |
| 5 | Prefix/title: deterministic, type lists first, then parts in slot order, highest priority, later wins ties | native | high (read; 9 of 9 card titles) |
| 6 | Sale value: name parts' `MonetaryValueMod` in the part product; integer truncation | script order + data | medium (inferred; 2 launcher cards) |

## 1. The attribute modifier stack

Each weapon stat is an attribute property on `WillowWeapon` (for example `InstantHitDamage`, `ClipSize`) with a stored
base value and a stack of modifier objects (`Core.AttributeModifier`: a type byte and a float value). The type enum,
decoded from `Core.upk`, is `MT_Scale` = 0, `MT_PreAdd` = 1, `MT_PostAdd` = 2.

**Combination.** Whenever a modifier is added or removed, the property recomputes its value from the base and the whole
stack:

1. Walk the stack once and keep four single-precision sums: PreAdd values, PostAdd values, Scale values greater than 0
   (`up`) and Scale values less than or equal to 0 (`down`, a non-positive number).
2. `value = (base + PreAdd) * ((1 + up) / (1 - down)) + PostAdd`, evaluated in extended precision and stored as a float.
3. There is **no clamp**: a stack can produce zero or a negative value.
4. Integer attributes (`IntAttributeProperty`; on `WillowWeapon`: `ClipSize`, `ProjectilesPerShot`, `ShotCost`,
   `AutomaticBurstCount`, `AdditionalRicochets`) use the same formula on the integer base converted to float, and the
   result is truncated toward zero. Byte attributes do the same.

**This settles the fitted rule.** The rule fitted on 2026-10-01 (positive scales multiply, negative scales divide) is
exactly what the game computes; the `max(0, ...)` clamp the tool applied was not. A zero-valued Scale lands in `down` and
has no effect either way.

**Where the stack comes from** (script `WillowWeapon.InitializeInternal` and helpers):

1. `CalculateWeaponBaseValues` sets every base from the weapon type: `InstantHitDamage`, `InstantHitMomentum` (then times
   100), `MeleeDamage`, `BaseStatusEffectChanceModifier`, `StatusEffectDamage` and `ExtraShotDelay` through
   `AttributeInitializationDefinition.EvaluateInitializationData` with the weapon as context; `ShotCost`, `ClipSize`,
   `ProjectilesPerShot`, `Spread`, `FireRate` (into `FireInterval`), `ReloadTime`, `BurstInterval`,
   `AutomaticBurstCount` and the rest copied from the type's plain fields.
2. Parts are chosen (section 3), then `CalculatePartDependentWeaponBaseValues` sets the barrel and body spin and flap
   durations.
3. `ApplyAllWeaponAttributeEffects`: the type's `WeaponAttributeEffects`, then each part's, in slot order (Body, Grip,
   Barrel, Sight, Stock, Elemental, Accessory1, Accessory2, Material).
4. The internal attribute slots (below).
5. Name parts are chosen (section 5) and `ApplyNamePartWeaponAttributeEffects` adds the prefix's then the title's
   `WeaponAttributeEffects` (none of the 748 name parts in `Startup.upk` carries any; the path is real but empty in this
   data).
6. Each effect's value is `EvaluateInitializationData(BaseModifierValue, weapon)`, the shared evaluator of
   [NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) section 1. Re-read for this note: base (attribute value if the
   attribute's context resolves, else the constant), then the definition's formula combined by `BaseValueMode` (enum
   from `Engine.upk`: 0 sets, 1 adds, 2 scales, 3 formula minus base; a definition with neither an enabled formula nor an
   enabled conditional leaves the base alone), **then** `BaseValueScaleConstant`, **then** the enabled min/max clamp,
   then the definition's `RoundingMode` (0 float, 1 round half up, 2 floor, 3 ceiling). All in single precision.

**Attribute slots.** For each `AttributeSlotUpgrades` entry, the weapon type's list first and then each part's, the named
slot's grade grows by `GradeIncrease`, whether or not the entry activates the slot. The first entry with `bActivateSlot`
also activates the slot and adds the type's `AttributeSlotBaseGrade` (an initialization; the class default on
`Default__WillowInventoryDefinition` is the constant 1), rounded half up. At most `AttributeSlotMaxActivated` slots are
considered (class default 19, values outside 1..19 become 19). Only activated, non-external slots add a modifier to the
weapon; external slots (`bExternalSlot`, for example the accuracy pool's min and max) go to the owner. Each slot's
modifier value is computed once into the slot; that computation was **not read** (it is reached through an interface the
analysis could not resolve). The tools use `BaseModifierValue + PerGradeUpgrade * sum(GradeIncrease)`: this form
reproduces all 9 observed cards, and adding the activation base grade to it reproduces none, so the value apparently
counts grades relative to the base grade. Census of `Startup.upk`: 1,002 activating and 57 non-activating part upgrades,
no type upgrades, every slot effect's `BaseModifierValue` zero, 99 external slots, 33 slots with `bEnforceMinimumGrade`.

**Confirmation:** a card whose stats depend on a negative scale and a PostAdd (for example a launcher with a clip PreAdd
and a magazine scale): the integer truncation shows directly in the magazine line; a stack that would go below zero (if
any exists in data) printing a negative or zero value instead of 0 after a PostAdd.

## 2. Card display

The card's numbers come from each stat's `AttributePresentationDefinition`
(`GD_AttributePresentation.Weapons.AttrPresent_*`). Applying a presentation to a value:

1. Values smaller than 1e-8 in size become 0.
2. If `bValueRemappingEnabled`: a linear remap from `[InputValueMn, InputValueMx]` onto `[OutputValueMn,
   OutputValueMx]`, the input **clamped** to its range (an empty input range maps everything to `OutputValueMn`). The
   slope is computed once in single precision.
3. Sign handling (`SignStyle`), and `1 / value` when `bDisplayAsInverse`.
4. Rounding with the presentation's `RoundingMode` and `FloatPrecision` (clamped to 0..10): Float rounds half up to that
   many decimals; IntRound half up; IntFloor down; IntCeil up. The rounding works on the stored float.

Class defaults (`Default__AttributePresentationDefinition`): `RoundingMode` IntRound, `FloatPrecision` 1. The weapon
presentations in the data set: damage IntCeil; clip size IntFloor; fire rate inverse of `WeaponFireInterval`, Float;
reload Float; accuracy = remap of `WeaponSpread` (0..15 onto 100..0), Float. So the card shows:

| line | value | rounding |
|---|---|---|
| Damage | `InstantHitDamage` per projectile | up to an integer |
| `xN` after the damage | `ProjectilesPerShot` when above 1 (the card text carries a `[projectilecount]` tag) | integer |
| Accuracy | remapped spread | one decimal, half up |
| Fire Rate | `1 / FireInterval` | one decimal, half up |
| Reload Speed | `ReloadTime` | one decimal, half up |
| Magazine Size | `ClipSize` (already an integer) | down |
| status chance and damage per second | status-effect rows | one decimal, half up (observed; their presentation class was not read) |

Single precision matters at the ceiling: one observed launcher's damage is 140,522.014 in double precision (would print
140,523) and just under 140,522 in single precision (prints 140,522, as the card does).

Card value: `ItemCardGFxObject.SetItemCardEx` prints an override price when one is given (shops), else the item's
`GetMonetaryValue` (section 6).

**Confirmation:** a card whose damage lies within a few thousandths above an integer in double precision; any accuracy
value ending in 5 in the second decimal (87.35 as a float is 87.3499985 and prints 87.3).

## 3. The weighted part pick

The pick (registered `WillowWeapon.ChooseRandomParts`) runs over the part list collection chosen for the item (script
`ChoosePartListCollection`; the merged runtime list, see `WEAPON_BALANCE_DECODE.md` section 2) once per slot in slot
order, at the item's game stage. Per slot:

1. **Slot use.** For a collection whose mode is Additive or Complete the slot is always considered; for Selective only
   when the slot is enabled; in every case the slot's part data must be enabled for any candidate to exist.
2. **Entry weight.** For each `WeightedParts` entry:
   - if the entry's `Manufacturers` list is **empty**, or the item has no manufacturer, the weight is a **flat 100**; its
     `DefaultWeightIndex` is not read. 1,572 of the 2,473 weapon entries in `Startup.upk` have an empty list;
   - otherwise the entry for the item's manufacturer gives its weight index (exact match: a `Manufacturer=None` entry is
     not a wildcard), and a list without the manufacturer falls back to `DefaultWeightIndex`;
   - the weight index evaluates `ConsolidatedAttributeInitData[i]` with the item as context;
   - the entry weighs 0 when the stage is below `trunc(min)` or above `trunc(max)` of its game-stage window;
   - a positive weight may be adjusted by an optional per-type hook (not read; behind a definition flag).
3. **Candidates.** Entries weighing at most 1e-8 are not added. A part already in the list has its weight replaced by
   the later entry's (the slot total adjusted accordingly); its position stays.
4. **Pick.** `r = total * rand() / 32767` (C runtime `rand`); walk the candidates accumulating weights and take the first
   whose interval `[sum, sum + weight]` contains `r`. With no candidate the slot gets no part.

Consequence for the old tool rule "a slot whose candidates all weigh 0 picks uniformly": the slots it meant (for example
the slice pistol's grips and barrels, `DefaultWeight` 0 with empty manufacturer lists) weigh 100 each, so the pick is
uniform anyway; a slot that really weighs 0 stays empty.

**Confirmation:** many drops of one balance at one stage (a farm, or the sdk spawning items): part frequencies within a
slot should follow the weights above, for example equal frequencies for entries without manufacturer lists whatever
their `DefaultWeight`.

## 4. Level, grade and rarity

**Item level.** Every inventory balance has `bInterpolateExpLevel`, true by class default (`Default__InventoryBalanceDefinition`).
When a pool spawns an item, the balance's candidate list stores, per eligible manufacturer grade, the stage in place of
the grade index; `GetExpLevelFromManufacturerData` then returns that value as the item's experience level. So **a
dropped weapon's level is the game stage it spawned at**. Without the flag the level would be the grade's
`GradeModifiers.ExpLevel`, or a level packed into the high 16 bits of the grade index (read as capped at 63); the base
game's weapon balances do not use that path.

**Stage of a pool roll** (`ItemPool.SpawnBalancedInventoryFromPool`): the caller's game stage (an enemy's or object's
`GetGameStage`); if the pool has `bSupportsGameStageVariance` (class default true) **and** the caller passed a
variance formula, the evaluated variance is added (then at least 1 when the stage was at least 1, else at least 0); then
clamped into the pool's `MinGameStageRequirement` / `MaxGameStageRequirement` attributes when those are positive. Enemy
drops (`WillowPawn.DropLootOnDeath`) pass no variance formula; interactive objects have
`LootGameStageVarianceFormula`. The candidate list also caps the stage at a maximum level (an argument, or a global
default, not read).

**Grade.** Each manufacturer entry of a balance lists grades with a game-stage window; the eligible grade's weight is its
spawn probability modifier at that stage. Every weapon balance in `Startup.upk` has one grade covering stages 1..10000.

**Rarity.** Rarity is not rolled: the pool picks a balance (by its probabilities), the balance yields parts, and the
rarity follows from them. Rarity level = `trunc(type BaseRarity)` (class default 0) plus `trunc(Rarity)` of every part
(each evaluated through its `ItemRarity` attribute). The tier is the first `GlobalsDefinition.RarityLevelColors` entry
whose `[MinLevel, MaxLevel]` contains the level; in this data 1 Common, 2 Uncommon, 3 Rare, 4 Very Rare, 5 Legendary,
6 Very Rare in E-tech colour, 7..10 Legendary. A legendary body (4) and legendary barrel (5) sum to 9, inside 7..10. The
tools used the maximum part rarity before.

**Confirmation:** the level requirement of an enemy drop equals the area's stage (with an enemy whose stage is known);
an E-tech item reporting rarity level 6 in an sdk trace; a gun whose parts sum to a level that a max rule would place
in another tier.

## 5. Prefix and title

`WillowWeapon.ChooseRandomNameParts` has **no random draw**. Within one `PrefixList` or `TitleList`, an entry qualifies
when the item's experience level lies in `[MinExpLevelRequirement, MaxExpLevelRequirement]` (class defaults 1 and 100),
its `Priority` is greater than 0 (class default 1; cooked data omits it when it is 1) and its `Expressions` hold with the
item as context; the qualifying entry with the highest priority wins, a later entry on ties. The weapon type's lists are
taken first; then each part in slot order Body .. Material replaces the current pick when its own best has a priority at
least as high. The name is the prefix's `PartName` then the title's.

The tools evaluate only expressions of the form `Weapon_Is_<Manufacturer> == 1` (any other form excludes the entry); the
game evaluates them all.

**Checked on cards:** for all 9 observed cards, a part combination that reproduces every number also reproduces the title
exactly through this rule.

**Confirmation:** a gun whose parts carry two name parts of equal priority (the later slot's should win), or a captured
gun's prefix and title with its full part list.

## 6. Sale and buy value

The item's value is stored as an integer. The value code itself (`WillowInventory.ComputeMonetaryValue`, a virtual on
the item) was **not resolved**; what is known:

- script `InitializeInternal` recomputes the part value (`ComputeValueOfParts`) after the parts and again after the name
  parts are chosen;
- launcher prefixes carry `MonetaryValueMod` = `GD_Economy.Rarity.Att_Price_RarityMultiplier_01_Common` (67 name parts
  have one, all others none);
- with every part's and both name parts' `MonetaryValueMod` multiplied into
  `InventoryPartMonetaryValueModifierTotal`, the type's calculator (`Att_Manufacturer_<Maker> * PartTotal *
  Att_UniversalPriceIncreasePerLevelScaler ^ level * Att_BaseCost_Guns_<Type>`) reproduces both observed launcher cards,
  truncated to an integer. This closes the "launcher sale value" failure: it was the prefix's modifier.

Buying and selling: the card prints the value unless a shop passes an override; the buy-back list prices an item at its
value; a vending machine's featured item costs `int(value * FeaturedItemCommerceMarkup)` (or a fixed cost) capped by the
currency cap; the regular shop price goes through `GetSellingPriceForInventory`, an interface call that was not read.

**Confirmation:** a launcher whose prefix is known, value on the card versus the formula; the shop price of a known gun
next to its card value.

## 7. Runtime data (Gearbox hotfixes)

The 2026-09-26 traces were captured on a game with online hotfixes active. OpenBLCMM's copy of the game's own `obj dump`
(local; `tools/blcmm_dumps.py`, `docs/verification/BLCMM_DUMP_CROSSCHECK.md`) shows the running state. Census over the
1,439 weapon parts, types and name parts of `Startup.upk` (local `local/weapons/runtime_changes_filtered.json`), after
discarding class defaults that cooked data omits: 39 objects differ from the cooked data in stat-relevant properties
(24 `WeaponAttributeEffects`, 7 `ClipSize`, 5 `ReloadTime`, 4 `AttributeSlotUpgrades`, 2 `ZoomWeaponAttributeEffects`,
2 `InstantHitDamage`, 1 `ExternalAttributeEffects`).
Four of the observed cards are hotfixed legendaries: for two of them the cooked-to-runtime difference in one barrel effect
equals the whole damage gap the audit saw.

The tools therefore treat runtime data as an **input**, not a rule: `tools/weapon_card_audit.py --runtime-overlay`
replaces the stat-relevant properties of weapon parts, types and name parts with the dumped values. Whether the port
should apply hotfix data (and from where; the game downloads it, it is not in the packages) is a maintainer decision.

**Real-game check, 2026-10-02** ([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md)): the live weapon types do differ
from the cooked decode (Bandit pistol `ClipSize` 36 vs 30, Dahl pistol 16 vs 12, Bandit shotgun 10 vs 9 and
`ReloadTime` 4.1 vs 4.4), but none of the 23 entries the game's `Micropatch` service held that day touches a weapon
(they cover population weights, skills, a shield projectile and rare-enemy balances). So "online hotfix" is not shown
to be the source of these type-level differences; another package overriding the cooked objects, or older hotfix
content captured in the 2026-09-26 dump, are the open alternatives. On the 69 golden cards (exact rolled parts,
cooked data) this commit's evaluator matches 52 in every printed stat (fire rate now 69/69). One card regressed:
a stage-15 Maliwan pistol whose reload evaluates to exactly 1.75 prints 1.8 here and 1.7 in the game, while a Bandit
shotgun's fire rate of 1.25 prints 1.3 in the game; the game's float operation order evidently puts one just below
and the other just above the half (UNVERIFIED which order).

## 8. Card audit (oracle: 9 distinct cards in the local 2026-09-26 UI traces, levels 34-43)

| code | data | main four stats | every printed field | every field and the name |
|---|---|---|---|---|
| HEAD (fitted rules) | cooked | 4 of 9 | 4 of 9 | not measured |
| HEAD (fitted rules) | runtime overlay | 8 of 9 | 7 of 9 | not measured |
| read rules | cooked | 5 of 9 | 5 of 9 | 5 of 9 |
| read rules | runtime overlay | **9 of 9** | **9 of 9** | **9 of 9** |

Ablation with runtime data: without single precision 8 of 9 (the launcher's damage ceiling); without name parts 9 of 9
on the main four but 7 of 9 numerically (both launchers' value); adding the slot base grade to the slot value 0 of 9.
Remaining failures on cooked data are exactly the four hotfixed legendaries.

The 2026-10-01 record audited a different set of six cards (a Bandit launcher, a Maliwan shock sniper, a Dahl unique SMG,
a Vladof fire pistol, a Bandit slag SMG, a Dahl assault rifle); those traces are not on this machine and were not
re-run. Its open items map onto this note as follows: the launcher value is the name-part modifier (section 6); the Bandit
slag SMG matches with runtime data; the Dahl SMG's one-point damage gap is the kind single precision or runtime data
explains, but that card was not re-checked.

Parts are inferred from stats in this audit; a match is evidence for the evaluator, not for the roll.

## 9. Implementation (2026-10-02)

- `tools/weapon_recipe.py`: `f32`, `formula_value` and `attribute_value` in the game's order and precision
  (BaseValueMode, scale, clamp, rounding); `entry_weight`, `slot_candidates` (flat 100, truncated stage window), `pick`,
  `choose_name_parts` (deterministic names); `roll` uses them.
- `tools/weapon_stats.py`: `combine` without the clamp and with integer truncation; effects in game order including
  slots (activation, all increases, min/max grade) and name parts; `rarity_of` (sum and table; `rarity` keeps the host's
  1..5, new `rarity_level`, `rarity_rating`, `rarity_color`); `present`/`display` from the presentation data; name parts
  in the sale value; launchers' calculator now checked.
- `tools/weapon_card_audit.py`: predicted name parts per combination (effects, value and a `name` comparison),
  `--runtime-overlay`, counts of combinations reproducing every field.
- `tools/weapon_balance.py`: docstring only (weights follow `entry_weight`).

## 10. Validation plan against real-game golden cards (W3, plan only)

**What each golden record must hold** (one record per captured weapon; captured by the real-game lane, kept under
`local/realgame/cards/`): balance path; manufacturer and grade index as stored; game stage and experience level; every
slot's part path (Body, Grip, Barrel, Sight, Stock, Elemental, Accessory1, Accessory2, Material, empty slots explicit);
prefix and title name-part paths; rarity level if the trace exposes it; the item's stored monetary value; the session's
hotfix state (on or off, and a dump of the changed objects if on); and every card line exactly as the card printed it:
title, level requirement, value, each top stat's label and value text (projectile tag included), every status row,
every white and red flavour line, element icon, manufacturer and type icons.

**Strata** (census of `Startup.upk`, local `local/weapons/plan_census.json`): 242 weapon balances in 33 weapon type x
manufacturer strata; 2,175,422 part combinations over all stages (base game only; DLC packages not counted).

1. **Every part at least once per stratum.** A gun fixes one part per slot, so a stratum needs as many guns as its
   largest slot (7 to 15). Total: **307 guns**.
2. **Interacting pairs.** Barrel x firing mode: each barrel has one firing mode (its own or the type's), so (1) covers
   it. Elemental x body (the element's damage scale meets the body's slot grades): 0 to 16 pairs per stratum; covering
   them as well raises the total to **444 guns**. Also pair the accessory with the body when both change the same slot.
3. **Every unique and legendary balance**: 79 (46 unique, 33 legendary), plus the 7 mission-weapon balances; at least one
   gun each, with and without hotfixes where a dump shows a runtime change.
4. **Levels**: spread the strata over low (1-10), slice (7-11), mid (25-35) and cap (50) stages, and include stages at
   the edge of part game-stage windows.
5. **Precision cases**: guns whose double-precision damage sits within 0.02 above an integer, and accuracy values ending
   in 5 in the second decimal (section 2).

## 11. Brand mechanics census

| mechanic | answer | where |
|---|---|---|
| Tediore throw-reload | **behavior** sequence, triggered by reload | the Tediore weapon types' `BehaviorProviderDefinition` holds spawn-projectile chains (the other brands' providers are empty); the trigger and the thrown damage are native/script (`BeginReload` is script, `IncrementPlayerTedioreReloadDamageStat` native); the projectile definitions are data |
| Vladof spin-up | **native** on data | barrel `bIsSpinningEnabled`, `SpinUpDuration`, `StartingSpinUpFireIntervalMultiplier`, type `BarrelSpinMode`; `TickBarrelSpinUp` and `GetFireInterval` are native |
| Torgue gyrojet | **data** plus the script firing path | type `DefaultFiringModeDefinition` (a gyrojet firing mode) and its projectile definition; `FiringModeDefinitionFire` / `ProjectileDefinitionFire` are script; projectile flight is native |
| Dahl burst while aiming | **data** applied by script, timing native | `ZoomWeaponAttributeEffects` add `AutomaticBurstCount` on Dahl barrels/types (runtime data adds more); `ApplyAllZoomWeaponAttributeEffects` is script; `GetBurstInterval` native |
| Hyperion reverse recoil | **native** on data | type `bAlternativeKickEnabled`, `AlternativeWeaponKick`, accuracy impulse effects; the kick (`AddWeaponKick`) and accuracy pool are native |
| Bandit magazines | **data** | type `ClipSize`, PreAdd and slot effects; the evaluator above |
| Jakobs fire rate | **data** plus the firing loop | type `FireRate` (interval) and `AutomaticBurstCount` 1, `MinFireAnimDuration`; `ShouldRefire` is script, `GetFireInterval` native; the card's fire rate is the inverse interval (one observed Jakobs card shows it) |
| Maliwan elements | **data** for the card, **native** per shot | fixed elemental parts in the balances, type status-chance and status-damage effects, status-effect definitions; the per-shot chance (`GetStatusEffectChanceModifier` and friends) is native |

## What was not read

The slot modifier value computation; the value function (`ComputeMonetaryValue`) and the regular shop price; the per-type
weight hook in the part pick; the grade choice and the stage cap in the pool's candidate list; the status rows'
presentation; expression evaluation for name parts beyond the manufacturer form; the game's random sequence. Everything
above is `UNVERIFIED` until a golden card or a trace confirms it.
