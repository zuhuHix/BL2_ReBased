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
- PR 1 stocks guns only. Ammo, grenade mods, shields, class mods and health items need item kinds the host inventory
  does not have yet (PR 2).

## 5. Open items
- The eighth machine's placement (section 1).
- The sale price, buy-back, and the stock roll rule (sections 2 and 3).
- The Black Market (eridium upgrades): placed in the data, marked unsupported.
- Currency HUD readout and save/load, ammo pickups and purchases, chests and lootables: later PRs.
