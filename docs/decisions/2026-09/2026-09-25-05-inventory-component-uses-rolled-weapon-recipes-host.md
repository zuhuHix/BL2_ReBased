## 2026-09-25: Inventory component uses rolled weapon recipes (host)

`UOpenWillowInventory` loads every recipe under `local/items` (or
`-owitems=<dir>`) that `tools/weapon_stats.py` has evaluated. It keeps a
backpack plus BL2's four weapon slots, and the first four items are equipped
in order. Keys 1-4 select a slot and 0 holsters; an empty slot keeps the
current weapon. The walker fires the active item's evaluated values:
- interval 1 / fire rate
- spin-up delay
- per-shot damage (no criticals, element or resistance yet)
Each item's mesh is `SK_<recipe id>`, imported by
`host/ue5/import_weapon_items.py` from its `filter_gestalt_gltf.py --recipe`
output. Only the Infinity MIC has a material approximation; other materials
get a neutral grey stand-in with a logged warning. Item rarity is the highest
part `Rarity` (`ItemRarity5_Legendary` = 5, resolved through its constant
attribute), which gives the HUD's rarity colour. Taking the max is
UNVERIFIED.

Runtime check: 8 Infinity recipes (seeds 1-8, level 30) loaded. Slot 1 fired
"Despair Infinity" at 740 per hit and 8/s; switching to slot 4 loaded "Angry
Infinity" (753, 9.1/s) with its own rolled Tediore sight on screen. The host
dummy's health was raised to 20,000 so the capture sequence survives level-30
damage.

Inventory screen (same day): `UOpenWillowInventoryWidget`, UMG built in C++
so the layout is reviewable text rather than a Blueprint asset. It opens with
I; Tab stays the dev map selector. The layout follows BL2's inventory:
equipped slots and backpack on the left in rarity colours, and an item card
with level, manufacturer, damage, fire rate, reload, magazine, spread,
element, "Consumes no ammo" and spin-up. Stats are compared against the
targeted slot, green up and red down, with lower reload and spread counted as
better. Clicking a slot targets it; clicking a backpack item equips it there.
Fonts, frames and icons are host stand-ins, not BL2's Scaleform art. The
module now depends on UMG, Slate and SlateCore. Captures now request the UI
layer so screenshots include UMG widgets. The unattended sequence adds
`OWCombat_6_Slot4` and `OWCombat_7_Inventory`; the latter was inspected and
shows the card for "Extended Infinity" (835, 7.3/s) compared with slot 1.

Item level (same day): `weapon_stats.py --level` now defaults to the recipe's
`game_stage`, the level it was rolled at. `Init_WeaponDamage` scales by
`1.13 ^ WeaponLevel`; for Extended Infinity (seed 5) the tool gives 24, 246,
835, 9,621 and 141,555 damage at levels 1, 20, 30, 50 and 72. The level-50
value looks high for a BL2 pistol. The likely suspect is the summed slot-grade
bonus (+3% per WeaponDamage grade), whose combination rule is UNVERIFIED. A
real item card at a known level is needed to calibrate.

