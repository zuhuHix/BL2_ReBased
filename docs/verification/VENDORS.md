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
  given. The use key opens the nearest machine within 350 uu that Maya looks at, a stand-in for the native use trace
  (NATIVE_USE_INTERACTION.md; CONTROLS.md, "The use target follows the view").
- The page starts its selection over on each opening. A refusal shows in the stock shop's labels (`INSUFFICIENT FUNDS`, `FULL`), read from script; which reasons the game sends to `FULL` is native and `UNVERIFIED`. The stock feedback clip and its sound are not driven. The state's serial moves only on a success, a restock or an opening. A refusal keeps it, so a repeat press gets the refusal again; a stale press changes nothing, keeps a refusal already showing, and otherwise says "Try again".
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
  - Shields and grenade mods are bought as stand-in gear with no stats and rarity 1. A shield's capacity comes from the walker's stand-in rule when equipped (`COMBAT_DOWN_RESPAWN.md`, "Stand-in shield capacity").
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
`local/slice/interactives.json`. **Section 9 records what two real-game runs later saw. Nothing else in this section has been seen in the running game: every rule about how the
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

**Where drops lie.** A drop lies at the socket its attachment point names, on the imported mesh. `tools/prepare_interactives.py` lists each mesh's `SkeletalMeshSocket` objects, and every attachment point of the six supported types names one on its mesh. That sockets are how the game places loot is well supported, not proven. Besides the matching names, 10 of 10 pickups read live in a real-game run lie at a socket's distance from their container, within rounding, on five types: the DahlAmmo chest, dumpster, storage locker, cardboard box and laundry machine. That is distance only, not direction, with other SDK mods on, and one pickup each on four of the types. Frames of two real-game runs, judged by eye, also show loot inside those five. The strong box's hinge and loot stay `UNVERIFIED`: it did not open in the real game and the host's frame looks like a rear view, so a frame from the door side, in the real game and the host, is what is missing. A placeholder has no sockets, so its drops stay on the floor in front. Only the position is used: the socket's rotation is not.

The imported bone takes the package's offset unchanged. Checked offline against the imported reference pose: the laundry machine's sockets land on its door's side and the locker's and strong box's at positive height, and a mirrored offset does neither. The dumpster and chest are too symmetrical to tell from that. For the dumpster, the real distance fits the unmirrored socket by a narrow margin on one pickup, and real frames show that pickup on the lid side, where the unmirrored socket puts it: this favours no mirroring and does not prove it. For the chest it cannot be told, and it barely matters, because its sockets are nearly symmetric: a flip would mostly change which item sits where. The pickup sight check ignores the lootable that holds the pickup, so a gun inside can be taken. Every other trace still stops on the lootable, shots included.

Checked against the regenerated data and the open clips, offline. No used socket sits on a lid bone. Those on the locker's and the DahlAmmo chest's base bones do not move. Those on the dumpster's, laundry machine's, strong box's and cardboard box's base bones move while the clip plays, because the container shakes, at most about 9 uu and for about a second, and end within 0.3 uu of where they began. The closed and opened idle poses agree to within 0.02 uu. So a drop placed when the lootable opens is right once the clip ends, and is not attached: it stays still during the shake. No configuration uses a point twice, so two drops of one roll never share a socket; nothing is built for that case.

**Stand-ins.** Shield and grenade mod drops are gear with no stats and rarity 1. A dropped shield gets its capacity the same way as a bought one, from the walker's stand-in rule when equipped (`COMBAT_DOWN_RESPAWN.md`, "Stand-in shield capacity"); the item itself still holds no stats, so its value and price do not change.

**Assets.** `tools/import_lootables.ps1` (UModel 1590, then two editor imports) imported 36 of 36 assets on 2026-11-06: the
6 lootable meshes, the 12 pickup meshes and 18 clips. The meshes are untextured: they render as plain pale surfaces. 15 of the
clips were first refused by the converter's check that the MD5-to-UE map is the expected mirror:
those skeletons have 2 or 3 bones, too few to determine the fitted map (rank 2 or 3 of 4), although the expected mirror
matches each of their bones to within 0.00001 cm. `convert_subset_rank_deficient`, which only this import uses, now tests the mirror directly on those skeletons at the same 0.05
tolerance and logs it per clip. It proves
less than a fit, because directions the bones do not span are not tested, so these clips stay `UNVERIFIED` until a frame
shows the lid opening the right way: `-owinteractivetest` captures `OWLootLid_<balance>_closed`, `_mid` and `_opened` for each
type with a clip. A lootable shows its imported mesh only when both the mesh and its open clip load; otherwise the placeholder, with its own visible lid, stays.

Frames of the 2026-11-06 captures, judged by eye by the hub, frames only and `UNVERIFIED` against the real game. The DahlAmmo chest's and the dumpster's lids rise from the rear, the laundry machine's round door swings out on its right hinge, and the cardboard box's flaps fold out: each meets the condition above. In white, two could not be judged: the storage locker's hinge and the strong box's direction.

For each type the test tries its lootables, leaving the first to the test's own open unless it is the only one, and on each of eight sides needs Maya's capsule to fit on walkable floor with a clear view as wide as she is. It logs which lootable it used and what failed on each side. With no such side on any of them, the checker reports that type `INCOMPLETE`, which fails the run. Which types must reach the capture is read from the assets on disk at grading, not from an earlier import's report. No imported lootable at all fails the run; a machine without the import passes `tools/test_interactives.ps1 -AllowNoLootables`, and the summary then shows `allow_no_lootables=1`. Frames are named `OWLootLid_<full balance with dots as underscores>_<phase>.png`, and the checker requires each on disk. How the imported assets look and play is
`UNVERIFIED`.

**Materials.** `tools/import_lootables.ps1 -Steps materials` binds the diffuse and normal textures of UModel's export to each mesh's one slot, through one instance per material of a minimal lootable material (the method of `tools/slice_npc_editor.py`, no original graph). It does not re-import a mesh, so clips stay valid. Emissive is left out, because some of UModel's bindings there look wrong. Other inputs are not reproduced either: the health vial's material is masked, and the eridium materials have pattern and energy textures. Pickup meshes get a second UModel export (`-psk`), because their glTF job brings no materials or textures. Textures, instances and this run's folders are saved before any mesh is touched. A planned mesh that does not end bound fails the step, after the report is written; the host logs the material each lootable type renders, and `check_interactives.py` reports a type that does not render its planned instance as `INCOMPLETE`. An instance's name includes a hash of its textures, so two exports with one name and different textures do not share one; the 10 instances of the run below had the older plain names. The first editor run bound only the 12 pickups, because the skeletal path changed a copy of the slot, and that run still read as a pass.

With the slot written back, the editor run on 2026-11-06 bound 18 of 18 meshes through 10 instances (54 imported, 0 not imported), and `test_interactives.ps1` gave `SUMMARY passed=11 failed=0 missing=0 skipped=0 incomplete=0`.

The frames of that run, host frames only and not compared with the real game, so all `UNVERIFIED` against it:

- Textures: every lootable type is textured. "DAHL" reads correctly on the dumpster and the ammo chest, so no flipped V was seen where text can be read. Pickups in those frames are textured and show no floating label.
- Storage locker: the handle is on the right edge when closed, and the door swings out on a hinge on its left edge, showing the inside. It reads as correct from these frames.
- Strong box: its hinge stays `UNVERIFIED`. The only clear view is from behind, where the door stands off to the side of the box with its outer panel facing the camera. Whether that is right waits for `tools/real_game/loot_capture.ps1`'s side frame and its real-game frames.
- Laundry machine, dumpster, DahlAmmo chest and cardboard box: as judged above, now textured.
- Not read by anyone: the `mid` frames of the cardboard box, dumpster, DahlAmmo chest and laundry machine. The health vial and eridium pickups do not appear in the frames that were read, so how they look is unknown.
- The saturated blue streak and green edge on a wall in the strong box frames are existing world, not this work: they are in the same place in a capture from before any lootable had a material. They have not been triaged.

The glTF slot names the material. `Skel_StorageLocker`'s slot is a dummy, since it was cooked into `Sanctuary_Dynamic.upk` while the material is exported from `Sanctuary_P.upk`. Its import table holds `Prop_Lootables_02.PhAT.Mati_StorageLocker` and UModel's log names that import as unloadable while loading the mesh, so it takes the same material as `Skel_StrongBox`. That comes from package data, not from the running game; a live read of both slots is queued. The package also exports `Mati_StorageLocker_GreenGlow`, whose use was not read.

**Pickup labels.** The floating text on a loot pickup is a host stand-in that tells placeholder spheres apart. It is hidden once the real mesh loads. Whether the game shows any text on cash and ammo pickups is `UNVERIFIED`; a real-game frame is queued.

## 8. Quick Change respec

AI-assisted (Claude), 2026-11-06. `tools/prepare_interactives.py` lists the placed `CharacterCustomizationMachine` under
`stations` (one in Sanctuary); its mesh is already part of the imported scene. The use key near it calls
`UOpenWillowSkills::PurchaseReset` and charges the money that takes. The cost is `ResetCost`: 100 x 1.12^level, read from
data by the skills lane and `UNVERIFIED` in game.

The customization menu is not built. A two-press confirmation stands in for its dialog (host choice, `UNVERIFIED`): the
first press shows the cost on a label, and a second within 5 s does the reset. The second press counts only from the same walker at the price shown; a changed price asks again. The prompt and label end when it expires, or when the station stops being that walker's selected target, either by walking out of reach or because something else is now nearer. It also ends when Maya is down or dead, and when a press goes to Marcus, whose talk takes the use key first. The use key does nothing at all while she is down or dead, for every target (host rule, `UNVERIFIED` against the game's own checks). An empty tree can still be bought back, as
the script allows; whether the game's dialog stops that was not read. Reach is 350 uu, as for lootables.
**Machine missing from a host scene (2026-11-08, PR #106 lane).** The showcase frames showed no machine in Sanctuary where the real game shows one. Cause, on the machines checked: the Sanctuary data was older than the importer that places the machine. The local manifest `local/sanctuary/scene.json` of one PC was dated 2026-09-23 and logged every `WillowInteractiveObject` component as `Unsupported component owner`; the importer places them from 2026-10-21. A regenerated manifest (same code, full pipeline: prepare_level, terrain, BSP, pillar, collision, then prepare_interactives) places 11 interactive components, with the one Quick Change station `Sanctuary_P:WillowInteractiveObject_6` (CharacterCustomizationMachine) unhidden and unique by name across levels. No tracked code changed for this.

A second cause was on a worktree whose UE map was older than its manifest: the map had no interactive actors at all, though its manifest had them. The same name check ran on this worktree's map before and after re-import.

**How a machine finds out the map is older than its manifest.** Read-only, no editor. Three checks, in order:
1. Modification times: `Content/OpenWillow/<Map>/<Map>.umap` older than `local/<map>/scene.json` means the map was saved before the manifest was written. A heuristic; it does not prove the content differs.
2. Name check: the map's binary holds the names of its imported actors. Count `WillowInteractiveObject_` in the `.umap`. A manifest with interactive placements and a map with zero means the import predates them. Unreal stores a name with a numeric suffix split off (`...StaticMeshComponent` plus a number), so search the base name, not the full tag.
3. Counts: `local/<map>/ue-import.json`'s `source_placements` against the manifest's `actors`. Different means the last import used another manifest.
A guard that runs these three before a suite would catch both cases; not added yet.

**Re-import and checks (worktree t3-7991f5d9, 2026-11-08, in-engine, one machine).** The UE module built here after one retry (the first failed on the paging file while another editor ran; the second compiled cleanly). `tools/run_ue_level.ps1 -ImportOnly -SkipBuild` took about 5 min 47 s from start to finish with editor start, and about 20 s of that was the commandlet. The re-imported map holds the interactive components (nine name matches). `tools/test_interactives.ps1 -SkipBuild`: 11 pass, 2 fail, 0 missing. `quick_change` passed (walk-away cancel, low-money refusal, reset charged). `lid_capture` and `lootable_materials` failed because this worktree has no imported lootables (`tools/import_lootables.ps1` not run here); the suite's own message names that step. Those two say nothing about the machine.

**Standard views, before and after (same module, same camera poses in both runs).** Eight standard views plus four Quick Change views. The before run used the old map (2026-09-25) and the after run the new import. Manifest diff against the September manifest: 0 placements vanished, 0 moved more than 1 uu, 16 added (11 interactive, 5 collection actors). The new manifest also carries a lighting section the old one lacks. Visually: the machine is absent in the before frame and visible in the after frame of the standard customization view. Sun, sky and shadows differ across the standard views (the new lighting data); the pixel differences are large, and this is not yet a verified visual match. `UNVERIFIED`: whether the game shows the machine the same way, the Quick Change camera framing, and the lighting change.

## 9. Real-game lootable capture

AI-assisted (Claude), 2026-11-07. `tools/real_game/loot_capture.ps1` drives the installed game, in Sanctuary with saves blocked, and restores and compares the player's files afterwards. Output stays under ignored `local/realgame/loot_capture/`. It ran twice. Both runs exited 1 as incomplete, and both left the saves identical after their restores, with no recovery needed. Everything below is read from live values and from frames by eye, and only for what those two runs did. The frames read were a sample of each burst, not all of them. This is not parity.

**What ran.** The runs were at earlier commits, and three things changed after them, not yet run in the game. Stage problems also go into the status, as in the phaselock capture. A pickup attempt takes its baseline right before the attempt, needs the chosen pickup still there and counts only its own ammo pool, matched by name; the ammo pickup below would still count, since its own pool rose. The ammo stage needs a pool to have fallen.

**Correction.** The first version of the capture counted a lootable as done when its frames existed. Three of six had not opened and still passed. A lootable stage now needs the object's own state to have changed.

**SDK mods stay on.** `Install-Driver -Exclusive` only refuses a driver folder that already exists. It does not disable other SDK mods, and the capture disables nothing. The second run recorded these as enabled: Python SDK, Apple's Borderlands Cheats, Be Gone Out Of Bounds Loot, Better UI Controls, Bossbar, BSABT, Commander, Dialog Skipper, Item Lights, Loot Collector, No Ads, Python Part Notifier, RTSE, Sanity Saver, Quick Startup, and the run's own driver. Drop Chance Multiplier and OpenWillow UI Trace were listed but disabled.

Loot Collector, Better UI Controls, Dialog Skipper, Item Lights and Apple's Borderlands Cheats may affect what was read here, and whether any of them did has not been checked. So everything below about pickups and prompts is `UNVERIFIED` as stock behaviour, including the use prompt, the beams, the icons and the item card. That a press collected a pickup is what happened with these mods on.

**Shop keys are not stock.** The page's keys in `vending_model.js` (W, S and arrows move, Enter or E acts, Tab, Escape closes) are host choices, and `tests/vending_test.js` only holds them in place. No live session opened a shop, so the stock shop's keys are unobserved. Per another lane's read of its code, Better UI Controls makes E and WASD work in shop menus, so seeing them work on this install would not confirm them.

**What opening changes.** On each of the five types that opened, `SimpleAnimState` and `RepSimpleAnimState` went from 4 to 7 and `bCanBeUsed` went from true to false. On the seven tries that did not open, none of the three moved. This is what those types did in this one run, not a rule for every lootable.

**A signal before the press.** `CachedUsableObject` on the player controller was that object on all five tries that opened, and empty on all seven that did not. It stays empty for a pickup even with the prompt showing, so it says nothing about pickups.

**Lids, second run:**

- **Opened:** the DahlAmmo chest, storage locker and cardboard box from the first side tried, the dumpster from the second and the laundry machine from the third.
- **Against the host:** the laundry machine's round door swings out on its right, as the host's does. The dumpster's lid rises on a long edge; whether that is the rear is `UNVERIFIED`. The first run had the DahlAmmo chest, storage locker and cardboard box opening as the host's do.
- **Strong box:** its hinge is still not observed. No side offered the prompt. Its bounds read as unusable, so the aim fell back to a guess above its origin. An orange panel under a shelf in one frame may be the box, `UNVERIFIED`.

**Where loot lies.** In all five types that opened, the loot lies inside the container. Comparing this with the host's placement is a follow-up after PR #120.

**Pickups:**

- **Use key, ammo:** one grenade ammo pickup was collected by the use key. The prompt showed, and on the press the pickup flew at the camera and was gone within a few frames. The grenade count on the HUD and its pool each rose by one.
- **Message:** none in the frames read of that one pickup. Other pickups were not read, so this is no rule.
- **Walk-over:** not answered. The ammo pickup was not collected after the fall, but nothing shows the player touched it, and the position after the fall was not recorded. The other fall landed on top of a machine with the pickup inside it.
- **Cash: not tested.** The capture mistook eridium for cash: its check puts an eridium stick in that group, and it chose one inside a laundry machine, from a side with no prompt. Eridium would not move money anyway. No money pickup was tried.
- **In the open: not met.** Both chosen pickups lay within a lootable's bounds. Those bounds are large spheres and may overcount.

**Materials, first run, read live:**

- **Slots:** the storage locker and the strong box both have `Prop_Lootables_02.PhAT.Mati_StorageLocker` in their first slot, which agrees with what PR #120 took from package data. The cardboard box, laundry machine, dumpster and DahlAmmo chest render their slot's material directly.
- **Per-object instances, `UNVERIFIED`:** placed lockers and strong boxes render a `MaterialInstanceConstant` of their own, not the slot's material. Their parents were not read, so whether `Mati_StorageLocker_GreenGlow` is involved is unknown, and the host's plain instance may differ from them.
- **No labels:** no frame read shows floating text on loot, which agrees with PR #120 hiding the host's stand-in labels once the real mesh loads.

**For a later run, not done.** A third run was judged not worth a launch before the pending merges.

- Press only when `CachedUsableObject` is this object, and try other strong boxes until one offers the prompt.
- Keep eridium apart from cash, and judge each by the value it moves.
- Record the player's position after the fall, so a walk-over shows whether it touched.
- Choose pickups by a tighter test than the sphere, and stand on the side with the prompt.
- A pickup truly on open ground may not exist here, since all of them came from containers. A walk-over answer may need one dropped some other way.
