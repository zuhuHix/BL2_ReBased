# Vending machines, currency and interactive objects in Sanctuary (vendors lane)

AI-assisted (Claude), 2026-11-03. Our own words. Package facts come from `ow-package --properties` through
`tools/prepare_vendors.py`; live facts come from one read-only real-game dump (`tools/real_game/vendors_capture.ps1`
running `tools/real_game/scripts/vendors_probe.py`). Both outputs are game data and stay under ignored `local/`. Each
rule below is marked **confirmed (live)**, **package data**, or `UNVERIFIED`.

## 1. Where the machines come from

**Package data.** Sanctuary places no vending machine actor. The machines are spawned at
`WillowPopulationOpportunityPoint` actors whose `PopulationDef` names a `GD_Population_Shopping.VendingMachine.*`
population. That population's `PopulationFactoryVendingMachine` holds:
- the shop type: absent means weapons, `SType_Items` means the Ammo Dump, `SType_Health` means Dr. Zed;
- a commerce-markup initialisation for the stock and another for the featured item;
- the object balance (`InteractiveObjectBalanceDefinition`).

The balance's `DefaultLoot` lists two loot configurations by name: `Inventory` (the stock pools) and `FeaturedItem`
(the item of the day). Each lists `ItemAttachments`, which are an item pool and a pool probability. The shop pools live in
the map package under `GD_ItemPools_Shop.*`, and their entries point at objects in `Startup.upk`.

Opportunity points in the Sanctuary packages give 2 weapon machines (Marcus Munitions), 3 Ammo Dump machines, 2 Dr. Zed
machines and Crazy Earl's Black Market (`InteractiveObj_CrazyEarl_BlackMarket`, in `Sanctuary_Dynamic`).

**Confirmed (live), with one open gap.** The running game had 8 `WillowVendingMachine` actors and 1
`WillowVendingMachineBlackMarket`. Seven machines and the Black Market sit exactly (distance 0) on the opportunity
points above, with the same definitions. An eighth machine, a Dr. Zed `HealthItems` machine, stands about 7 m from the
other Zed machine near the Ammo Dump, and no opportunity point the tool reads accounts for it. Where it is placed is
still open: it may be in a streamed level the tool does not visit, or placed some other way.

## 2. Stock

**Confirmed (live).** A machine's stock is a fixed array of 30 slots, of which only the first few are filled:
- weapons: 7 items plus the featured one;
- Ammo Dump: 10 to 12 items (grenade mods and ammo items);
- Dr. Zed: 8 items (shields, class mods and health items).

Dr. Zed sells shields and class mods as well as health items. The item levels seen at player level 8 were 6 to 8, which
fits the package's loot game-stage variance of a random integer offset with lower bound −2 (`UNVERIFIED` as a rule).
`GlobalsDefinition.MinutesBetweenShopResets` read 20 live. `ShopResetCost` is 0 in the class default.

How many items a machine rolls, and how it rolls them, is native code that was not read. The host rolls the observed
counts from the decoded pools with its own seeded stream: the pool weights are package data, but the roll order and
de-duplication are `UNVERIFIED`.

## 3. Prices

**Confirmed (live), 79 of 79 rows.** These were every stock and featured item of the eight machines whose fields read
completely, at player level 8. The rest of each 30-slot array is empty. The shop's price for an item is

    price = floor(value * (level * 0.14 + 4.1))

Here `value` is the item's monetary value (the card's sale value) and `level` its item level. The two constants are the live
values of `GD_Economy.VendingMachine.Att_SellExponentializer` (0.14) and `Att_BaseVendingMarkupIncrease` (4.1). The
package's `Init_MarkupCalc_P1` / `P2` give this shape: offset plus unowned item level times exponentializer. The two
attributes carry no value in the packages, which is why the live read was needed. The arithmetic is done in single
precision, and the featured item uses the same markup. The player's own commerce markup read 1.0.

`UNVERIFIED`: the price the game pays when the player sells. The same native quote, asked about the player's own
backpack items, returned the buy formula, not a sale price. The host pays the item's monetary value on a sale until a
live sale is observed. Buy-back is also `UNVERIFIED`.

## 4. Host design (PR 1)

- `tools/prepare_vendors.py` writes `local/slice/vendors.json`: the machines, their kind, pose and the expanded stock
  pools.
- `AOpenWillowVendingMachine` actors are spawned from it by `UOpenWillowVendorSubsystem` when `-owvendors=<file>` is
  given. The use key opens the nearest machine within 350 uu, a stand-in for the native use trace
  (NATIVE_USE_INTERACTION.md).
- The page `tools/hud_overlay/vending.html` runs the stock `UI_VendingMachine.VendingMachineInterface` movie in Ruffle
  as frame art (converted by `tools/prepare_vending_movie.py`) with HTML lists over it. The host pushes state with
  `window.owVending(state)`, and the page answers with `OWVEND {serial, action, id}` console lines.
- The machine visual is a placeholder box and the page layout is not fitted to the game: both `UNVERIFIED`.
- Host rules, all host choices and `UNVERIFIED` against the game:
  - A restock that has fallen due is applied before the next action, never after it. A buy aimed at the shelf from
    before the restock is refused with "The stock changed".
  - The host pushes the state every second while the shop is open, so the countdown follows host world time.
  - The weapon the mission lends is refused for sale while the quest owns it (the quest's own flag, not an item id).
  - Money is added with saturation, and manifest numbers are range-checked before use.
  - The page gets UI-only input only after it reports ready (`OWVENDREADY`). A load error closes the shop, and Escape or
    a second use press closes it from the host side.
  - The use key goes to the nearest of vending machine (350 uu), door (220 uu) and pickup (220 uu), each within its own
    reach.
- **Persistence (PR 2a):** with `-owcurrencysave=<file under local/>`, the host saves money and eridium, plus each
  machine's restock count and remaining timer, after every action and at teardown. On load it re-rolls the shelf from the
  machine's seed and restock count and removes the items already bought. `-owmoney` / `-owerid` override the saved purse.
  Host choices, all `UNVERIFIED` against the game:
  - Buy-back is not saved.
  - The timer does not run while the game is closed.
  - The file (schema `openwillow-currency/2`) carries a manifest identity: a SHA-1 over the stage, the reset interval and
    each spawned machine's id, kind, seed and pools. A save from a different machine set, or with any row that would not
    restore, is refused whole. Nothing is applied and saving stays off, so the file is never overwritten.
  - Writes go to a unique temp file and replace the save in one step (Windows `ReplaceFileW`, which keeps the previous
    save as `.bak`).
  - The new file is flushed first, and if the replace stops half way the host finishes the move or puts the backup back.
    A load that finds only the `.bak` reads it, and logs that it did.
  - The path must resolve, links followed, inside this checkout's `local/`. The deepest existing folder is checked
    before any folder is created, and the target folder is checked again before every write. A small check-then-use
    gap remains: a link swapped in between that check and the write is not caught. That is accepted for a local
    development tool; the checks are not handle-based.
  - With no vendor machines spawned (no usable `-owvendors`) there is no manifest identity, so saving stays off.
  - A `-owmoney=` / `-owerid=` value that is not a whole number in range is ignored with a warning.
  - Restock counts are bounded on load (1,000,000) and the increment saturates.
- **PR 2b:** the Ammo Dump and Dr. Zed are stocked from the decoded item blocks (section 6). All host choices, `UNVERIFIED`:
  - Shelf sizes 11 and 8 follow the live row counts.
  - The Ammo Dump has one row per ammo type, then grenade mods by weight. Dr. Zed has one row per health item, then
    shields by weight.
  - Buying ammo adds the item's rounds to the pool, capped at its maximum, and is refused with "Ammo full". Health Now!
    heals 25 % of max health and is refused at full health. Ammo and health rows stay on the shelf.
  - Shields and grenade mods are bought as stand-in gear with no stats and rarity 1.
  - Class mods are left out: their shop pools are empty in the packages, and how the game fills them is open.
  - Item value is evaluated in single precision, in the tool's order.
- PR 1 stocked guns only. Ammo, grenade mods, shields, class mods and health items need item kinds the host inventory
  does not have yet (PR 2).

## 5. Open items
- The eighth machine's placement (section 1).
- The sale price, buy-back, and the stock roll rule (sections 2 and 3).
- The Black Market (eridium upgrades): placed in the data, marked unsupported.
- Currency HUD readout and save/load, ammo pickups and purchases, chests and lootables: later PRs.

## 6. Non-weapon stock

AI-assisted (Claude), 2026-11-04. Package data through `tools/prepare_vendors.py` (no disassembly); live values from
the same read-only dump as section 3 (`local/realgame/vendors/sanctuary.json`, player level 8). The tool writes one
`item` block per non-weapon leaf balance of `local/slice/vendors.json` and, with `--oracle <dump>`, counts exact live
matches into `item.verified.value`.

**Monetary value, package data, confirmed (live) on 63 of 63 rows that the packages describe.** Every non-weapon
item in these shops has the same shape as the guns (NATIVE_WEAPON_RULES.md section 6):

    value = trunc(part_total * 1.12 ^ item_level * base_cost)

- `1.12` is `GD_Economy.Universal.Att_UniversalPriceIncreasePerLevelScaler`.
- `item_level` is the item's `ExpLevel`. The calculators read it through `ShieldItemLevel`, `GrenadeModItemLevel` or
  `UnownedItemLevel`, which all name that property.
- `base_cost` is a constant `GD_Economy.BaseCost.Att_BaseCost_*` attribute that the definition's `MonetaryValue`
  calculator names. Observed values: standard, spike and juggernaut shields 28; grenade mods 25; standard class mods 40,
  "team" class mods (Init_ClassMods_Team) 42; Health Now! 3; pistol, SMG, rifle, shotgun and sniper ammo 0.9 each;
  grenades 5; launcher ammo 10.
- `part_total` is the definition's `BaseMonetaryValueModifier` multiplied by the `MonetaryValueMod` of each drawn
  part, in single precision. Ammo and health calculators do not read a part total, so they use 1. For shields,
  grenade mods and class mods it depends only on the grade, because every part a slot can draw carries the same
  modifier (the `Att_Price_RarityMultiplier_*` constants). Totals: shields 0.96 / 1.6 / 2.6 / 3.6 / 5.6 (common to
  legendary); grenade mods 0.9216 / 1.536 / 2.496 / 3.456, which is two modified parts; class mods 0.96 (common). Some
  legendary grenade balances can draw parts with different modifiers. For those the block lists every distinct
  total, and the host must pick one by the parts it rolls.
- The tool evaluates each calculator with `weapon_stats.price_init`, the evaluator that matches 69/69 gun cards. It
  checks that the reduced form above gives the same integer at levels 1 to 80 for every total (`shape_checked`).

Live rows, by kind (exact integer `monetary_value`):

| kind | rows | match |
|---|---|---|
| ammo and grenade ammo (`ItemGrade_AmmoShop_*`) | 18 | 18 |
| Health Now! (`ItemGrade_HealthDrops`) | 3 | 3 |
| grenade mods (`GM_Standard`, `GM_Standard_2_Uncommon`) | 18 | 18 |
| shields (standard, spike, juggernaut, common to rare) | 18 | 18 |
| class mods, Siren and Assassin | 4 | 4 |
| class mods, Mechromancer and Psycho (DLC packages) | 2 | not described |

Class mod rows are matched to a variant by the name word that ends the definition's object name (`ClassMod_Siren_Nurse`
for "Nurse Class Mod"). The two DLC class mod balances live in packages under `DLC/`, which the tool does not read, so
they stay `UNVERIFIED`. Every leaf balance with no live row (rare and better grades, other grenade and shield
types, launcher ammo) is labelled `UNVERIFIED (no live row)` in the JSON. Its value comes from the same calculator
shape.

**Ammo, package data.** A shop ammo item (`GD_Ammodrops.Shop.AmmoShop_*`, a `UsableItemDefinition` in the map package)
has one `ExternalAttributeEffects` entry. It is a `MT_PostAdd` on a `ResourcePoolAttributeDefinition` whose
`Resource` is the ammo pool (`D_Resources.AmmoResources.Ammo_*`), and the amount is the `AmmoAmountShop_*`
constant: pistol 54, SMG 72, rifle 54, shotgun 24, sniper 18, launcher 12, grenades 3. The item's card text agrees
("Contains 72 SMG bullets."). The tool maps the pool to the host's ammo key (`pistol`, `smg`, `ar`, `shotgun`,
`sniper`, `launcher`, `grenade`). `UNVERIFIED`: that a purchase applies the effect at once (the definition sets
`bPlayerUseItemOnPickup`), and how the `OnUseConstraints` "pool is full" test refuses a buy.

**Health, package data.** Health Now! (`GD_BuffDrinks.A_Item.BuffDrink_HealingInstant`) heals through its behaviours,
not through an attribute effect. The tool reports the multiplier of the `Behavior_SimpleMath` multiply under its
behaviour provider (0.25), and the card text agrees ("Instantly heals 25% of your Max Health"). The behaviour graph
that applies it is not decoded, so the heal is `UNVERIFIED`.

**Stock, package data.** `GD_Itempools.ClassModPools.Pool_ClassMod_01_Common` and `_02_Uncommon`, which
`HealthShop_Items` lists, have no entries in the packages. The live Dr. Zed machines still sold class mods, so the
running game fills these pools in some way that is `UNVERIFIED`. The tool therefore lists the Startup
`BalDef_ClassMod_*_01_Common` / `_02_Uncommon` balances under `unpooled_items`, each with its `ClassModDefinitions`
as `variants`. The Ammo Dump pools also name `GD_CustomItems.Items.CustomItem_AmmoSDU_*` balances (ammo capacity
upgrades). Their definition has no `MonetaryValue` calculator in the packages and no live row was seen, so they are
`UNVERIFIED` and carry no value.

Not settled here: the item's display name (the block gives the definition's base `ItemName`; the card name comes from
name parts), part rolls and stats of shields and grenade mods, rarity, and the sale price (section 3).

## 7. Lootables and pickups (PR 3)

AI-assisted (Claude), 2026-11-06. Package data through `tools/prepare_interactives.py`, written to ignored
`local/slice/interactives.json`. **Nothing in this section has been seen in the running game: every rule about how the
game uses this data is `UNVERIFIED`.**

**Placement.** 97 objects: 94 lootables (41 cardboard boxes, 22 strong boxes, 21 storage lockers, 7 laundry machines, 3
dumpsters), 1 Dahl ammo crate and 2 chests. The host spawns 95. It does not spawn the golden chest (it costs a golden key)
or the Bearer of Bad News chest (a mission object).

**Loot, read from the balances.** One configuration is picked by weight. Each attachment then fires with its probability
and gives one leaf by weight. Pools whose weights depend on runtime attributes (`DropODDS_Health`, `DropWeight_*_Need`)
have no value in the packages, so they drop nothing here.

**Cash.** `calc(L) = 1.25 * 1.12^L`; a little pickup is `calc(L) * (1 + 0.01 * grade)` and a big one
`calc(L) * (10 + 0.1 * grade)`, with the grade 0 to 49 from the drawn parts. The constants are package data. Rounding
down, the uniform grade and the item level (a seeded value in the point's game stage range, 7 to 9 in Sanctuary) are host
choices.

**Other pickups, package data.** Ammo amounts: pistol 18, assault rifle 18, SMG 24, shotgun 8, sniper 6, grenade 1,
launcher 4. Eridium: stick 1, bar 4. Host choices: auto pickups collect within 120 uu and the others by the use key within
220 uu. A full ammo pool, or full health or being down, leaves the pickup in place. Lootables open once per session;
this is not saved, and what the real game does here is `UNVERIFIED`.

**Use key.** Every target in its own reach is measured and offered the press, nearest first: vending machine, lootable or loot pickup, door, dropped gun or gear (ties in that order). A pickup that would refuse, such as ammo on a full pool, is not a target at all, so what lies behind it keeps the distance order. Any other target that refuses lets the next one try. A refused dropped gun still takes the press, because the player meant the gun and "backpack full" is its feedback. Marcus's talk is checked first on purpose, because it opens and turns in the slice's mission and his shop must not lose the press to a gun on the counter. All of this stands in for the native use trace (`UNVERIFIED`).

**Stand-ins.** Shield and grenade mod drops are gear with no stats and rarity 1.

**Assets.** `tools/import_lootables.ps1` (UModel 1590, then two editor imports) imported 36 of 36 assets on 2026-11-06: the
6 lootable meshes, the 12 pickup meshes and 18 clips. The meshes are untextured: they render as plain pale surfaces. 15 of the
clips were first refused by the converter's check that the MD5-to-UE map is the expected mirror:
those skeletons have 2 or 3 bones, too few to determine the fitted map (rank 2 or 3 of 4), although the expected mirror
matches each of their bones to within 0.00001 cm. `convert_subset_rank_deficient`, which only this import uses, now tests the mirror directly on those skeletons at the same 0.05
tolerance and logs it per clip. It proves
less than a fit, because directions the bones do not span are not tested, so these clips stay `UNVERIFIED` until a frame
shows the lid opening the right way: `-owinteractivetest` captures `OWLootLid_<balance>_closed`, `_mid` and `_opened` for each
type with a clip. A lootable shows its imported mesh only when both the mesh and its open clip load; otherwise the placeholder, with its own visible lid, stays.

Frames of the 2026-11-06 capture, judged by eye by the hub, frames only and `UNVERIFIED` against the real game: the DahlAmmo chest's and the dumpster's lids rise from the rear, and the laundry machine's round door swings out on its right hinge. Each meets the condition above. The storage locker's door opens, but its hinge cannot be told in white, so that clip stays unchecked until textures or a real-game frame. The strong box and cardboard box frames showed neither object, so those clips are unchecked too.

For each type the test tries its lootables, leaving the first to the test's own open unless it is the only one, and on each of eight sides needs Maya's capsule to fit on walkable floor with a clear view as wide as she is. It logs which lootable it used and what failed on each side. With no such side on any of them, the checker reports that type `INCOMPLETE`, which fails the run. Which types must reach the capture is read from the assets on disk at grading, not from an earlier import's report. How the imported assets look and play is
`UNVERIFIED`.
