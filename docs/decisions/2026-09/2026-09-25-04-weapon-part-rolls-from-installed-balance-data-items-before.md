## 2026-09-25: Weapon part rolls from installed balance data (items before UI)

The maintainer chose to build the item/part layer before the inventory and
HUD. Item cards need a gun's name, rarity, parts and stats, and a UMG
rebuild of BL2's UI was chosen over running its Scaleform movies.
`UI_HUD.HUD` and `SharedWillowInventory` `SwfMovie`s exist in `Startup.upk`.
SWF playback remains an optional later benchmark.

`tools/weapon_recipe.py` reads a `WeaponBalanceDefinition` through
`ow-package --properties` with an array schema; parser code is unchanged. It
follows `BaseDefinition` to the root, whose `InventoryDefinition` is the
`WeaponTypeDefinition` and whose `Manufacturers[0]` is the manufacturer. It
merges each balance's `WeaponPartListCollection` per slot, filters by game
stage, weights, rolls from a seed and writes a JSON recipe: parts, gestalt
fragments, material instance and name. `filter_gestalt_gltf.py --recipe`
builds that roll's mesh.

Decoded and used:
- `WeightedParts` entries index `ConsolidatedAttributeInitData` for min/max
  game stage and weight.
- Rarity weights such as `GD_Balance.Weighting.Weight_1_Common` are
  `ValueFormula` Multiplier 100 x Level 1 ^ Power 1, clamped at a 100 minimum.
- Name parts carry `Priority`. Manufacturer variants carry an `Expressions`
  entry `Weapon_Is_<Maker> == 1`. For example, "Xtra Fast" is the Bandit
  spelling of Vladof's "Rapid".

For `Pistol_Vladof_5_Infinity` the chain is `Pistol_Vladof` (Additive), then
`_2_Uncommon`, `_3_Rare`, `_4_VeryRare` and `_5_Infinity`, all Selective.
Barrel and material are fixed, so the Infinity's paint does not roll. Grip,
sight, element and accessory roll across manufacturers. Seeds 1-5 gave
Burning, Caustic, Discharge, Angry and Discharge Infinity. Every rolled
fragment exists in the pistol gestalt.

UNVERIFIED, flagged in each recipe:
- The replacement-mode semantics (Selective replaces enabled slots,
  Additive appends, Complete replaces all).
- The uniform pick when every candidate weighs 0. The Infinity barrel and the
  root body and grip entries do weigh 0.
- Manufacturer grade restrictions are ignored.
- The name rule: highest priority, ties broken by the seed.
Recipes have not been compared with in-game drops.

Also recorded for the weapon pass: `WeaponType_Vladof_Pistol` gives
FirstPersonMeshFOV 45, PlayerViewOffset (20, 4, 2), FireRate 0.125,
ClipSize 20, Spread 2.1 and the WeaponKick values. The separate 45 degree
weapon FOV likely explains the oversized gun in the host view.

Automated: `python tests/weapon_recipe_test.py` 5/5 on synthetic data.

Follow-up, part weights and item stats (same day):

- Weights: the maintainer expected every part to carry a drop weight. In the
  cooked `Pistol_Vladof.PartList`, `ConsolidatedAttributeInitData` is
  [1, 100, 0]. Every grip, sight and body entry points at the 0 with no
  InitializationDefinition. Elements and accessories do use rarity formulas
  (`Weight_1_Common` = 100 and so on). What BL2 does with an all-zero slot is
  native code; the host picks uniformly, still UNVERIFIED. The OpenBLCMM dump
  oracle is not installed on this machine, so no in-engine cross-check was
  made.
- `tools/weapon_stats.py` evaluates a recipe into item-card numbers. Base
  values come from the weapon type (`InstantHitDamage`, `FireRate`,
  `ClipSize`, `ReloadTime`, `Spread`). Damage is `Init_WeaponDamage` = 8 x
  `Att_UniversalBalanceScaler` ^ `WeaponLevel`, minimum 5, times the type's
  1.45. The scaler resolves through a ConstantAttributeValueResolver to 1.13.
  Part `WeaponAttributeEffects` are added, along with the type's
  `AttributeSlotEffects` at the summed `AttributeSlotUpgrades` grade.
  `Weapon_Is_<Maker>` operands are 1 only for the weapon's own manufacturer.
  Infinity seed 1 at level 30: damage 649, fire rate 8/s, magazine 1, shot
  cost 0, spin-up 0.55 s. The combination (base + PreAdd) x (1 + Scale) +
  PostAdd with a 0 clamp, the grade sum, the unapplied balance manufacturer
  grades and the missing accuracy-percentage conversion are all UNVERIFIED.
  No value has been compared with an in-game item card.
- `attribute_value` now applies BaseValueScaleConstant to formula and
  attribute bases too, and resolves attribute operands from a supplied map.

Automated: `tests/weapon_recipe_test.py` 5/5 and `tests/weapon_stats_test.py`
2/2, synthetic.

Follow-up, cross-checked against OpenBLCMM (same day):

The maintainer approved downloading OpenBLCMM. Installed outside the repo
at `%USERPROFILE%/Tools/OpenBLCMM/`:
- OpenBLCMM v1.4.1 (`OpenBLCMM-1.4.1-Windows.zip`, SHA-256
  `bbe9d09a3373de7f20b2f138b865baed762f2a8e6ef9b50738966f4095bc4000`) from the
  official BLCM/OpenBLCMM release.
- Datapack `blcmm_data_BL2-2023-04-20-01.jar` (SHA-256
  `8bf07971904ed9d511586e11adcc4e676fbeda546994b439456860fcce2457bc`) from
  BLCM/OpenBLCMM-Data.
- Its `data.db` matched the shipped `.sha256sum` and was extracted to
  `%LOCALAPPDATA%/OpenBLCMM/extracted-data/BL2/`, where `tools/blcmm_dumps.py`
  looks for it.
No OpenBLCMM code is used and no dump text enters the repository
(THIRD_PARTY.md already records this relationship). Subobject names in the
dumps use a colon (`Pistol_Vladof:PartList`).

Results:
- Part lists agree. All 45 part-list slots across the five Infinity-chain
  balances match our decode exactly: part order, per-part `Manufacturers`
  overrides, stage and weight indices, `ConsolidatedAttributeInitData`
  constants and `PartReplacementMode`. The game itself holds weight 0 for
  every grip and body.
- A clamp bug in our evaluator was fixed. Cooked data omits false booleans;
  the dumps show `RangeRestriction.bEnableMinValueRestriction=False` on
  `Weight_*` and `Init_WeaponDamage`. We had applied those minima anyway,
  which flattened `Weight_2_Uncommon` (10) and `Weight_4_Rare` (1) to 100.
  Restrictions now apply only when enabled; the formula also adds `Offset`
  and honours `ValueFormula.bEnabled`. A definition with another
  `BaseValueMode` or an enabled `ConditionalInitialization` is reported as
  unresolved instead of guessed.
- Manufacturer weight overrides now apply only when an entry names the
  weapon's manufacturer. Entries with `Manufacturer=None` are not wildcards;
  as wildcards they would make every `DefaultWeight` formula unused, whereas
  BL2 elemental-chance mods work by editing those formulas. UNVERIFIED.
- Resulting Infinity odds: element None 76.9%, Fire, Shock and Corrosive
  7.7% each; accessory None 74.1%, the other seven 3.7% each; sight 12.5%
  each of eight. Level-30 damage over seeds 1-8 is 649-835. No value has
  been compared with an in-game card.

