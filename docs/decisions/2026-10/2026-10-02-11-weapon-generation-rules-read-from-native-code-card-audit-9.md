## 2026-10-02: weapon generation rules read from native code; card audit 9 of 9 with runtime data (UNVERIFIED in game)

AI-assisted. Tools and notes only: no host C++, no `src/`, no `CMakeLists.txt` change; package parsing is unchanged.
Read locally from `Borderlands2.exe` in Ghidra and from script, written in our own words in
`docs/verification/NATIVE_WEAPON_RULES.md`; every rule is UNVERIFIED in game and each section names its confirmation.

- **Attribute stack (settles the fitted rule).** The game sums PreAdd, PostAdd, positive and non-positive Scales in
  single precision and computes `(base + PreAdd) * (1 + up) / (1 - down) + PostAdd`, with no clamp; integer stats (clip,
  projectiles, shot cost, burst count) truncate. The 2026-10-01 "split" rule was right; its clamp at 0 was not. Enum
  orders and class defaults were decoded from the packages.
- **Effect order** (script): type, parts in slot order, attribute slots (activated ones; grades count every increase),
  then prefix and title. **Card rounding** comes from the presentation data (damage up, clip down, the rest half up to
  `FloatPrecision`); single precision decides the damage ceiling on one observed launcher.
- **Part pick.** An entry without a `Manufacturers` list weighs a flat 100 (1,572 of 2,473 weapon entries); stage
  windows use truncated bounds; zero weights are dropped, a duplicate keeps its later weight, and a slot with nothing
  left stays empty (not a uniform pick). **Names** are deterministic (type lists first, then parts in slot order, highest
  priority, later wins ties; class defaults priority 1, level window 1..100). **Level** = the spawn game stage
  (`bInterpolateExpLevel` default true). **Rarity** = sum of truncated part rarities looked up in
  `RarityLevelColors` (was: max). **Value**: the prefix's `MonetaryValueMod` is in the part product; this was the
  launcher value gap (inferred from script order and data; the value function itself was not resolved).
- **Runtime data.** The observed cards were captured with Gearbox hotfixes active; OpenBLCMM's dumps of the running game
  show 39 weapon objects with changed stat data, which explain the four hotfixed legendaries. `weapon_card_audit.py
  --runtime-overlay` reads them locally as an oracle input; whether the port applies hotfix data is a maintainer decision.
- **Card audit** (9 distinct cards in this machine's 2026-09-26 traces; the record's 6-card set is not on this PC):
  before (HEAD) 4 of 9 cards on the main four stats and on every printed field; HEAD rules with runtime data 8 / 7 of 9;
  read rules on cooked data 5 of 9; read rules with runtime data **9 of 9 on every printed field, name included**.
  Ablation: without single precision 8 of 9; without name parts 7 of 9 numerically; adding the slot base grade 0 of 9.
- **Changes:** `tools/weapon_recipe.py` (evaluator order and precision, `entry_weight`, `pick`,
  `choose_name_parts`), `tools/weapon_stats.py` (`combine`, effect order, slots, `rarity_of`, `present`/`display`, name
  parts in the value, launchers' calculator checked; new card fields `rarity_level`, `rarity_rating`, `rarity_color`),
  `tools/weapon_card_audit.py` (name parts and `name` check, `--runtime-overlay`, every-field counts),
  `tools/weapon_balance.py` (docstring), tests, `WEAPON_BALANCE_DECODE.md` pointer. Regenerating `local/items/slice`
  changes the slice guns' parts and names (new pick sampler and name rule); it was not regenerated in place.
- **Checks:** weapon_recipe 14, weapon_stats 25, weapon_balance 7, loot_pools 9, weapon_paint, skill_stats, slice_world
  and golden_card_compare tests pass; CTest and packages in the lane report. Not done: any in-game check, the host
  changes listed in the lane report (magazine and shot-cost truncation, HUD card rounding, E-tech colour).

