## 2026-10-03: weapon card rules: level line, single-precision rounding, float bases; golden cards 69/69 (weapon lane)

AI-assisted (Claude). Tools, tests and notes only: no `src/`, no `CMakeLists.txt`, no host or page code; package
parsing is unchanged. The rules are written in our own words in `docs/verification/NATIVE_WEAPON_RULES.md`. Raw
decompiler output stayed in the analysis store. Evidence: the 69 golden cards (exact rolled parts) recorded by the
real-game lane.

- **Level line** (read from script and data, section 4). A mission-balance weapon requires level 0. Any other weapon
  requires its item level minus the floor of the player's level-requirement bonus (0 in this data unless the player has
  something that raises it), at least 1. The card prints the line only when the requirement is above 1, so every
  mission weapon and every level-1 weapon prints none. New `weapon_stats.level_requirement` and the card fields
  `level_requirement` and `level_line`. Not modelled: the over-level text and the DLC message.
- **Rounding precision** (section 2). The Float rounding scales, adds the half and floors on the x87 unit. A golden
  status chance that is exactly a float tie (stored 33.349998, printed 33.4) shows the unit runs at single precision.
  `half_up` now rounds `f32(value * 10^p)` before adding the half. How the game sets the x87 precision was not read
  (UNVERIFIED; Direct3D 9's default). Consequence: an invented 87.35 accuracy now prints 87.4, not 87.3.
- **Float bases** (section 1). Plain weapon-type fields enter the stack as floats. The class-default 2.1 reload is
  2.0999999, so a -20 % scale gives 1.7499998, which prints 1.7 as in the game, not exactly 1.75 printed as 1.8.
- **Launcher sale value** (section 6). The evaluator was right. The golden comparison tool does not pass the recorded
  prefix and title, so the launcher prefixes' price multiplier was missing. The value function itself is still not
  read: its vtable could not be resolved, a second time.
- **Status rows** come from their own presentation data: Float, one decimal. The chance row is a remap whose slope equals
  the Generic BaseChance, except for slag (30.03 against 30; no slag card, UNVERIFIED).
- **Golden counts**, tracked comparison tool, main four / every printed stat / every field:

  | data | before | after |
  |---|---|---|
  | live | 69 / 68 / 54 | 69 / 69 / 54 |
  | cooked | 52 / 52 / 40 | 53 / 53 / 41 |

  Per field (live), status chance went from 15/16 to 16/16. On cooked data reload went from 63 to 64/69.
  A local copy of the tool that passes prefix, title and balance and uses `card['level_line']` gives every field on
  68/69 live (sale value 69/69, level line 69/69) and 52/69 on cooked data. The one remaining live miss is the host's
  own slice name. That tool change belongs to the real-game lane.
- **Tests and shared cases.** `tests/card_rounding_cases.json` gains two reload cases at the half (1.7499998 prints
  1.7; 1.75 prints 1.8). Its 87.35 accuracy case moves to `pending_page_change` with the new text 87.4, which only the
  Python test reads. `inventory.js` `cardRound` still prints 87.3 for it. To match, the page would round
  `Math.fround(Math.fround(stored * scale) + 0.5)` before the floor, with `scale = Math.fround(10 ** decimals)`. That
  change needs the in-engine suite and was not made. `weapon_card_audit.py` audits cards without a level line at level 1
  instead of failing.
- **Checks:** weapon_stats 28, weapon_recipe 14, weapon_balance 7, weapon_paint 29, golden_card_compare 12 tests OK;
  navigation 23/23; CTest 10/10; `verify_packages.py` 9/9. **In-game:** none beyond the golden cards above. No
  sensitive files touched.

