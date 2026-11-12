## 2026-10-01: weapon balances, card stats and loot pools decoded for the slice

AI-assisted. The mission pistol `MW_RockPaper_Fire` resolves through `Pistol_Maliwan` -> `_2_Uncommon` ->
`Pistol_Maliwan_2_Fire`; body, Fire element and material are fixed, grip/barrel/sight/accessory roll (4,608
combinations). Our part-list merge equals the game's `RuntimePartListCollection` (OpenBLCMM dumps) for 243 of
249 Startup balances; the 6 others differ in data the running game changed. `tools/weapon_stats.py` now
applies the weapon type's own effects and class defaults, divides by negative Scales instead of subtracting
them (FITTED: 4 of 6 observed cards fully reproduced, was 1), and emits projectiles, status chance/damage,
firing mode and card rounding. Still failing: launcher sale value, one Dahl SMG (1 damage point) and the
Bandit slag SMG (damage, magazine). `tools/loot_pools.py` reproduces 2,505 of 2,554 cooked
`ProbabilityDisplayString` shares; the target dummy has no loot in stock data, so the slice uses
`StandardEnemyGunsAndGear`. Selection/roll rules, the scale rule and rounding are native and stay UNVERIFIED.
Reader: additive `--properties-batch` CLI mode. Record: `docs/verification/WEAPON_BALANCE_DECODE.md`.

