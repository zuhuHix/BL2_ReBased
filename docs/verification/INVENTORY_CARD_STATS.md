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

OpenWillowInventory.cpp reads element, accuracy, accuracy_known, sale_value,
sale_value_known, fun_stats, and fun_stats_markup from stats.card. It copies the
element key to the weapon snapshot and writes accuracy/value only when present,
their known flags always, and non-empty fun text/markup. inventory.js lowercases
the element key for the movie's SetTitle element argument and passes funStatsMarkup
to the movie. No adapter or page change is needed for the explosive badge; a host
capture with the regenerated recipes remains unverified.

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

- Whether the inferred zoom reference conversion matches the native attribute resolver.
- Whether the real card's accuracy uses player-side modifiers beyond the item spread.
- Host rendering of the regenerated element badge has not been captured.

## Automated checks (synthetic)

`tests/weapon_stats_test.py` (36 synthetic tests) covers the remap, clamping and
absence, damage-type element mapping, the scaled-base price calculator with name-part
modifiers, the checked/unchecked flag, unresolved attributes, red flavour and card
text sources, zoom, ammo and markup. It is not
registered with CTest (that needs `CMakeLists.txt`, a sensitive file); run it
with `python tests/weapon_stats_test.py`.

In-game checks: none by this change; a capture of the card with the new fields
is a separate step.

## What the page prints on a weapon card (2026-10-04)

AI-assisted. Page and host forwarding only; `tools/weapon_stats.py` is unchanged. The page's printed text for the six
local slice recipes (their `stats.card` rounded the way the page rounds, single precision) was compared with the real-game
cards recorded for the same guns (`local/realgame/cards/golden_cards.json`, entries `slice_exact`): **damage, accuracy,
fire rate, reload, magazine and sale value are equal for the SMG, the pistol, the rifle and the mission pistol; the two
shotgun recipes differ** (slice shotgun: reload 4.4 against 4.1 and magazine 9 against 10; reward-roll shotgun: reload 3.7
against 3.5 and magazine 13 against 14; re-running `weapon_stats.py` on the same recipe gives the same numbers, so the
recipe's parts or a shotgun accessory term differ from the roll the game made, not the page). That belongs to the weapon
lane. The page did not print some things the real cards have, and now does:

- **Projectile count.** `card.projectiles` above 1 is forwarded as `projectiles`; the damage row prints "21" followed by a
  smaller gold "x10" (size 10, `#e6d223`, both measured on a real capture and `UNVERIFIED` as exact values). The real
  card's text carries a `[projectilecount]` tag; the page writes its own markup instead.
- **Status rows.** `status_dps` and `status_chance` are forwarded as `statusDps`/`statusChance`; fire, shock and corrosive
  weapons get two rows after the five ("Burn Damage / sec." and "Ignite Chance", "Shock Damage / sec." and "Electrocute
  Chance", "Corrode Damage / sec." and "Corrode Chance"; icons `elementFire/Shock/Corrosive`), Float with one decimal, the
  chance with a percent sign. Labels are the ones the golden cards carry; slag has no card, so no rows. The card movie has
  rows for up to seven stats.
- **Value colour.** Row values print in the label cyan (`#a4e8f3`), as the real card does; they were white.
- **Compare arrows.** Arrow only, no difference figure; damage compares as damage x projectiles.

At the time of these captures the page did not receive the white flavour lines, zoom, sustained-fire note or orange
effectiveness line, so the cards carried fewer lines than the real ones. The text-source and recipe-card work below
supersedes that text gap. One other difference from this earlier comparison remains: a captured real-game name can
differ from the bounded recipe's name-part combination.

## Weapon card text and refreshed recipes (2026-10-22)

The bounded recipe cards now retain their existing stat keys and add `weapon_zoom`, `fun_stats_lines` and
`fun_stats_markup`. Text comes from installed definitions rather than per-item literals:

- Red flavour is selected by the presentation's red `TextColor`, across the balance, weapon type, parts and name parts.
  `NoConstraintText` and `Description` are both accepted; whitespace is collapsed before sending the line so long gaps
  in flavour text do not consume the card's width.
- Other text-only `CustomPresentations` from those same definitions are included when `bDontDisplayNumber` is set.
  The selected damage type's `WeaponCardPresentations` supplies its bonus/effectiveness phrases. The definition's
  `TextColor` sets the line color unless `bEnableTextColor` is false; the embedded damage-type color tag is preserved
  as nested Scaleform markup.
- A sight or accessory requests the zoom row through `WeaponCardAttributes.Attribute`. The type's `ZoomedEndFOV` is
  modified by the activated `WeaponZoomFOV` slot effect using the sum of the selected parts' grade increases. The
  model uses the presentation's inverse-zoom form (`80 / effective FOV`) and its one-decimal precision. The 80-degree
  reference conversion is inferred from card captures; the native resolver's unit conversion is still **UNVERIFIED**.
- The generic ammo line is made from the evaluated `shot_cost` when it exceeds one. Phrases such as reduced ammo use
  are read from the part's own text-only presentation. Lines are ordered flavour, zoom, numeric ammo cost, part/type
  presentations, then damage-type presentations.
- Existing `status_dps` and `status_chance` are emitted from the selected damage type and its status effect. They are
  not synthesized as text; the host already turns them into status rows. The same applies to `accuracy` and `sale_value`:
  they remain numeric card fields consumed by the host.

The recipe refresh uses the installed type, balance, chosen parts and the evaluator's single-precision modifier stack.
For a fire-rate attribute, the card presentation is inverse: first form the final `WeaponFireInterval` from its base,
part effects and activated slot grade using `(base + PreAdd) * (1 + positive scales) / (1 + abs(negative scales)) +
PostAdd`, then display `1 / interval` to one decimal. This is why the corrected rifle rate comes from the installed
type and parts, rather than visually adjusting the displayed digit. Damage, accuracy, reload and clip size follow the
same decoded attribute/presentation path; accuracy remaps spread to the card percent, damage rounds up, and magazine
size rounds down. The comparison report under ignored `local/orch/D20/` records the numeric capture audit.

The Round 1 comparison log from 2026-10-22 reported all fields matching across 67 supported records, with 2
unsupported. That run used the `--live-data` overlay (`local/realgame/cards/live_weapon_data.json`, values read from the
running game): rerunning it with the overlay on the review's request reproduces it (every printed stat 67/67, status
16/16; report `local/realgame/cards/golden_compare_r2_live.json`). The host's recipes are generated from the cooked
packages without that overlay, so the Round 2 cooked-package counts below are the ones that describe the host's cards.
Neither run validates the new element string or zoom parity.

The adapter reads stats.card.fun_stats_markup into Item.FunStatsMarkup, and
inventory.js passes funStatsMarkup to the movie. The forwarding path is present;
its in-game rendering has not been checked.

Recipe-only regeneration (never the separately owned `weapon_view.json`):

```powershell
$weaponStatsReader = (Resolve-Path build/Release/ow-package.exe).Path
$weaponStatsPackage = Join-Path $env:OPENWILLOW_BL2 'WillowGame/CookedPCConsole/Startup.upk'
Get-ChildItem local/inventory-actions/bounded-recipes -Filter '*.json' |
  Where-Object Name -ne 'weapon_view.json' |
  ForEach-Object { python tools/weapon_stats.py --reader $weaponStatsReader --package $weaponStatsPackage --recipe $_.FullName --game $env:OPENWILLOW_BL2 }
```

The earlier zoom and price caveats are updated by the Round 2 audit below. Card-text
markup forwarding is in the adapter, but its in-game rendering has not been checked.

## Round 2 audit (2026-10-22)

The card's `element` is the resolved damage type's elemental kind. `card_element()` maps the installed `DamageType`
enum to the card's existing element names, then falls back to the selected `Elemental` part name. This preserves
the existing Fire icon and also handles explosive weapons whose `Elemental_None` part leaves the type's default
explosive damage active. The D20 capture shows the real explosive badge; the host already forwards
`stats.card.element` and lowercases it for the movie's `SetTitle` element key, so this field requires no adapter or
page change. The new recipe output has not been checked in a host capture.

The sale calculator is unchanged. The D20 spawned record has the bounded launcher recipe's same balance, level and
non-name parts, but a different prefix. Evaluating an in-memory copy with the captured prefix reproduces the
recorded sale value exactly. This isolates that price difference to the bounded recipe's name-part choice; the
calculator already multiplies every selected part and name-part `MonetaryValueMod`. The bounded recipe set remains
unchanged, so its computed value still describes its selected prefix.

The zoom model still computes `80 / effective ZoomedEndFOV`, including the selected parts' active
`WeaponZoomFOV` slot grade. The D20 comparison matches three of the five zoom labels; two differ by one tenth.
The paired capture uses the same weapon parts as the bounded recipes, and neither one global nearest-rounding rule
nor floor explains all five labels. The checked type FOV and slot-grade inputs do not establish why those two
values differ, so the native resolver conversion and those recipe zoom outputs remain **UNVERIFIED**. No per-item
zoom adjustment was added. The synthetic zoom test checks the implemented FOV/grade arithmetic, not game parity.

The Round 2 cooked-package golden comparison used
tools/real_game/golden_card_compare.py without a --live-data overlay, writing
local/realgame/cards/golden_compare_r2.json. It compared 69 entries: 2 unsupported;
among 67 supported, damage matched 63, projectile count 67, accuracy 67, fire rate
67, reload 62, magazine 55, and sale value 67. Status DPS matched 16/16 and status
chance 15/16. Within slice_exact, damage, projectile count, accuracy, fire rate and
sale value matched 6/6; reload and magazine matched 4/6. This automated run does
not verify the new element string or zoom parity.

Synthetic regression coverage now includes a default explosive type with an `Elemental_None` part, the existing Fire
icon mapping, prefix and title price multipliers together, and the current zoom FOV/grade calculation. In-game
validation remains pending because this lane does not own the host/page or run a UE capture. Regenerating the six
bounded recipes changed only the element field in the three Torgue recipes; numeric card fields did not change, and
weapon_view.json was excluded and left untouched. Automated checks: weapon_stats_test.py
36/36, CTest 10/10, and verify_packages.py 9/9.
