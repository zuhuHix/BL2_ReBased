# Weapon item-card extras: accuracy, sale value, red text

> Update 2026-10-01: superseded in part by `WEAPON_BALANCE_DECODE.md`. With the weapon type's own
> effects applied and negative Scales dividing instead of subtracting, the modelled spread reproduces
> the printed accuracy on all six newly audited cards (`accuracy_known` is now true), and pistol and
> sniper price calculators are checked. Launcher prices still fail. The text below is the 2026-09-29
> state.

Date: 2026-09-29. Scope: what `tools/weapon_stats.py` now adds to a weapon
recipe's `stats.card`, how each field is derived, and how far each was checked
against real Borderlands 2 item cards. No package parser changes; everything is
read with `ow-package --properties` from the installed `Startup.upk` and the
installed `WillowGame/Localization/INT/*.int` files. Nothing game-derived is
committed.

Automated checks and in-game checks are reported separately. The "real cards"
below are nine weapon cards captured from the game's own UI (movie callback
trace, ignored `local/inventory-research/trace_cards.json`, Maya level 45).
Seven of them (the ones used for price and spread) have level and stats
recorded; their rolled parts are unknown, so a comparison tries every part combination the
balance allows and asks whether any of them reproduces the card.

## Card layout

The real weapon card's top stats, in order (observed in the trace, not read from
a game data list): Damage, Accuracy, Fire Rate, Reload Speed, Magazine Size,
then element/status rows (Slag Chance, Burn Damage / sec., Ignite Chance).
Below them: fun text (first line red, then white stat lines), level
requirement, value. The Accuracy label and icon are decoded from
`GD_AttributePresentation.Weapons.AttrPresent_WeaponSpread`
(`Description = "Accuracy"`, `Icon = "weaponAccuracy"`). I did not find or decode
a `UIStatList` in the game data; the order above is observation only.

## Fields

| card field | source | status |
| --- | --- | --- |
| `accuracy` | remap in `AttrPresent_WeaponSpread` applied to the modelled `spread` | formula decoded; spread input UNVERIFIED; `accuracy_known = false` |
| `sale_value` | weapon type `MonetaryValue` calculator | verified for shotgun, assault rifle, SMG; launcher failed; pistol/sniper unchecked |
| `fun_stats` | red text of the title part's `CustomPresentations` | text verified; white lines not derived |

### accuracy

`AttrPresent_WeaponSpread` has `bValueRemappingEnabled` with
`RemappingData`: `InputValueMx = 15`, `OutputValueMn = 100`; the other two
bounds are absent from the cooked data (default 0). Read as a linear remap of
spread 0..15 onto accuracy 100..0, so `accuracy = 100 * (1 - spread / 15)`.
The orientation is inferred: the reverse would put every real card near 10%,
and the real cards read 70 to 93. Clamping to 0..100 is UNVERIFIED.

The spread itself comes from the existing combination rules (`WeaponSpread`
base, part effects, slot grades). Those rules reproduce damage, fire rate,
reload time and magazine size of real cards, but not spread:

- Conference Call (level 42, real 70.4% = spread 4.44): the part combinations
  that also reproduce price, damage, fire rate, reload and magazine all give
  spread 3.90 (74.0%).
- Striker (level 42, real 86.7% = spread 1.995): the matching combinations give
  1.02 (93.2%).
- For KerBlaster, Veruc and Slagga (price-matching combinations) and the two
  launchers (all combinations examined), no combination gives the observed
  spread either.

So the modelled spread is missing some term (not identified; candidates are
manufacturer grades, which the evaluator ignores, and player-side effects such
as Badass Rank). `accuracy` is therefore emitted with `accuracy_known = false`,
and the host forwards `accuracyKnown = false`. It is a plausible number of the
right magnitude (within about 3 to 7 points in the two cases above), not a
verified one.

### sale_value

`WeaponTypeDefinition.MonetaryValue` is `{BaseValueAttribute:
Att_Manufacturer_<Maker>, InitializationDefinition:
Init_Gun_<Type>_PriceCalculator}`. The calculator has mode
`InitializationDefScalesBaseValue`: the manufacturer cost modifier is scaled by
`Init_UniversalGun_PriceCalculator * Att_BaseCost_Guns_<Type> ^ 1`, and the
universal calculator reads `InventoryPartMonetaryValueModifierTotal` and
`WeaponLevel`. `InventoryPartMonetaryValueModifierTotal` is taken as the product
of each chosen part's `MonetaryValueMod` constant (rarity multipliers). The
result is rounded down.

Check against real cards (integer-exact, floor):

| weapon | level | real price | modelled |
| --- | --- | --- | --- |
| Conference Call (shotgun) | 42 | 62713 | 62713.475 |
| KerBlaster (AR) | 41 | 90490 | 90490.404 |
| Veruc (AR) | 40 | 63525 | 63525.591 |
| Striker (shotgun) | 42 | 91352 | 91352.220 |
| Slagga (SMG) | 43 | 68725 | 68725.030 |
| Nukem (launcher) | 42 | 133468 | no combination within 1 |
| Pyrophobia (launcher) | 43 | 173544 | no combination within 1 |

Five of seven match to the dollar, and Veruc rules out rounding to nearest, so
rounding down is the supported reading. Each match is one specific value among
a few dozen possible per balance, so this is not chance. The launcher failures
mean the launcher calculator (or a launcher-specific term) is not understood.
`sale_value_known` is true only for the shotgun, assault rifle and SMG
calculators (`PRICE_CALCULATORS_CHECKED`); pistols and sniper rifles were not
observed and are emitted as `sale_value_known = false` (number present, flagged).
Range restrictions and conditional initialization in the calculators are ignored.
The recipes' level is their roll level (`game_stage`), so a level 30 recipe
prices as level 30, not as the level 42 real cards.

### fun_stats (red text)

The red line is the title part's `CustomPresentations`
(`AttributePresentationDefinition`): `NoConstraintText` is the line and
`TextColor` is (220, 70, 70), the `#dc4646` in the real cards' HTML. Among the
74 legendary and unique weapon titles in `Startup.upk`, 73 have exactly one such
presentation and one has none. All nine red lines seen on real cards exist
verbatim in the cooked data (Conference Call "Let's just ping everyone all at
once.", KerBlaster "Torgue got more BOOM!", Nukem "Name dropper.", and so on).
`<Package>.int` overrides the cooked string when present (Striker's line lives
there), as `tools/skill_stats.py` does.

The output is the gear schema's `funStats` string; `;` inside a line becomes `,`
because the HUD splits on `;`. The colour cannot travel in that string: the
first line is the red one.

Not derived: the white lines (`5.0x Weapon Zoom`, `Consumes 4 ammo per shot.`,
`Deals bonus explosive damage.`, `-20% Weapon Recoil Reduction`, ...). They come
from attribute presentations of the parts with computed numbers; that mapping
was not decoded. Green/purple/orange status lines are likewise absent.

## Host forwarding

`OpenWillowInventory.cpp` reads `accuracy`, `accuracy_known`, `sale_value`,
`sale_value_known`, `fun_stats` from `stats.card` and writes weapon snapshot
items with `accuracy` and `value` only when present, `accuracyKnown` and
`valueKnown` always (false when absent or unverified), and `funStats` only when
non-empty. Items whose recipe has no display type (the Infinity recipes) now get
a `type` label (`Pistol`, `SMG`, `Assault Rifle`, `Shotgun`, `Sniper Rifle`,
`Launcher`) resolved from the balance/ammo type; a recipe-provided type is
unchanged.

## Populated for the local recipes

Re-run `tools/weapon_stats.py --reader ... --package Startup.upk --recipe
local/items/<id>.json` after this change (the recipes are ignored local files).

- proof_conference_call (shotgun): accuracy 74.0 (unverified), value 17108
  (known), red text "Let's just ping everyone all at once."
- proof_hammerbuster (AR): accuracy 93.2 (unverified), value 25083 (known), red
  text from its title.
- infinity_1..8 (pistol): accuracy about 91 to 93 (unverified), values 12683 to
  14632 flagged `valueKnown = false`, red text "It's closer than you think!
  (no it isn't)" (the cooked string contains a run of spaces).

## Still unknown

- Why spread differs from real cards (accuracy).
- Launcher pricing; pistol and sniper pricing accuracy.
- White fun-text lines, status lines, level requirement text for weapons.
- Whether the real card's accuracy uses player-side modifiers.

## Automated checks (synthetic)

`tests/weapon_stats_test.py` (11 tests, 8 new) covers the remap, clamping and
absence, the scaled-base price calculator with rounding, the checked/unchecked
flag, unresolved attributes, red-only fun text and the INT override. It is not
registered with CTest (that needs `CMakeLists.txt`, a sensitive file); run it
with `python tests/weapon_stats_test.py`.

In-game checks: none by this change; a capture of the card with the new fields
is a separate step.
