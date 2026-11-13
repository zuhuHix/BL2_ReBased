## 2026-11-13: The slice gate's -Shield run equips one generated stock shield card

AI-assisted (Claude Code). Lane S, from the plan the hub accepted on 2026-11-13. The gate's `-Shield` run used to equip a shield
from the observed gear manifest, cards seen in someone's game. On the zuhu PC every one of those was above Maya's level 8, so the
gate equipped the lowest one under a fixture that overrode the level rule. On pc-hummer it fell back to a pool sized to her
maximum health. Neither is a shield evaluated from the game's data at her level.

- **Now:** `tools/prepare_stock_shield.py` writes one gear card, `local/slice/stock_shield_gear.json` (ignored; game data), with
  `Capacity`, `Recharge Rate` and `Recharge Delay`. The roll uses the balance `GD_ItemGrades.Shields.ItemGrade_Gear_Shield_Standard_01_Common`
  (the common standard shield Dr. Zed stocks most), item level 8, seed 1. Seed 1 is the one `tools/weapon_slice_gear.py` uses for
  the slice guns, and it was not chosen for how the fight goes. When the file exists, `tools/test_slice_gate.ps1 -Shield` passes
  it as `-owgear=`. The gate then equips it under the normal level rule and never reaches the override. The launcher prints which
  shield source was offered and which one the run used. No C++ changed: the host already reads those three card labels in
  `AOpenWillowWalker::ReadShieldStats`.
- **The numbers are data-derived and UNVERIFIED in game.** The card is one seeded roll of one balance under rules read from script
  and from the native notes. It is not the shield the game would give Maya: the game rolls its own parts with its own random
  sequence. The card's provenance block, the tool's output, the launcher's line and `SLICE_GATE.md` all say so.

**How the numbers are formed (step 0, read from script with `research/script_disasm.py`; listings only under ignored `local/`).**
An item's initialization picks its parts. It then builds the attribute slots and applies the item's own internal slot modifiers
through the same `WillowInventory` slot natives a weapon uses (`InitializeAttributeSlots`, `ApplyInternalSlotEffectModifiers`).
It runs the slots again for the name parts. When the item is equipped, its external effects go to the owner. First the
definition's `ExternalAttributeEffects`, then every part's, each through `AttributeEffect.ApplyAttributeEffects` with the
controller as source and the item as override. After those come the external slots, through `ApplyExternalSlotEffectModifiers`.
That native skips slots that run as a skill (`NATIVE_ATTRIBUTES.md`). So the item path matches the weapon slot rule (outcome
(i)), and the existing evaluators are reused unchanged:

- each pool attribute starts at the shield pool's base, 0;
- the definition adds `MT_PreAdd` bases:
  - capacity `80 x 0.8 x 1.13^level` (minimum 1; `Att_UniversalBalanceMultiplier_HealthShields`, `Att_UniversalBalanceScaler`,
    the item level through `ShieldItemLevel`);
  - delay `3.5` s (`Att_UniversalShieldBaseDelayConstant`);
  - rate `0.16 x` the unscaled capacity base per second;
- the parts carry no effects of their own, only `AttributeSlotUpgrades`. The definition activates the slots `Capacity`,
  `RechargeRate` and `RechargeDelay` (external `MT_Scale` slots). Each part's grade increases add up, and the value is
  `weapon_stats.slot_value`;
- the stack is `weapon_stats.combine` (split rule, single precision).

Part weights are read with `weapon_recipe.entry_weight` and the stage window; they are not flat. The pick is `weapon_recipe.pick`
from `CrtRand(seed)`. The tool evaluates every drawable combination (891 for this balance at stage 8), prints where the roll lies
in each range as a fraction, and refuses to write a card that falls outside its own range. Keep or refuse: an existing card that
differs is not written over without `--replace`. The card writes an empty `assetId` (no verified mesh preview). The inventory
page reads that as "no preview", and `EquipGearById` does not need a mesh.

**Verified:** the synthetic `tests/prepare_stock_shield_test.py` (invented package, now `py-prepare_stock_shield` in ctest). It
checks:
- the base formula, activation by the definition, part grade sums, and both scale sides;
- that internal and skill-run slots stay off the pool;
- weights and the stage window;
- seeding, and that the roll lies inside the range for 39 seeds;
- the refusals (not a shield, above level 8, output outside `local/`, a different existing card).

On the install the tool's roll and range agree with an independent probe written for the plan.

**UNVERIFIED:**
- that items merge part lists, weigh and pick parts, and form slot values as weapons do (the slot value computation itself is
  only a fit for weapons, `NATIVE_WEAPON_RULES.md`);
- the name choice;
- the card's display rounding (the shield card presentation was not read; values are written unrounded);
- the item level the game would pick;
- every number against the running game.

No existing record can confirm the card exactly: the live shield reads are pool maxima of random spawns with the player's own
modifiers in them, and they carry no recharge. Confirming it needs a live read of a spawned shield's parts and its own capacity,
delay and rate. Not run here: the gate itself. The hub's runner runs `slice_gate_shield` on the zuhu PC.
