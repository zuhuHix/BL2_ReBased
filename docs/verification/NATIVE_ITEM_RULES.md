# Non-weapon items: the relic part-weight hook, item rarity, and how the class mod pools are filled (draft, 2026-11-13)

AI-assisted (Claude Code), analyst lane K. Written in our own words for a later implementer, who works from this note only. No listing,
pseudo-code, script dump or extracted table is reproduced here; tables below are summaries with the few values an implementer needs.

**Status, once for the whole note.** No real-game session was available. Unless a line says otherwise, every rule here is **read from data or
code and UNVERIFIED in the running game**. Each statement carries one of these source tags:

- **[record]**: a tracked record on main (`NATIVE_WEAPON_RULES.md`, `NATIVE_LOOT.md`, `NATIVE_PROGRESSION.md` section 1, `WEAPON_PARTS_CATALOG.md`,
  `WEAPON_BALANCE_DECODE.md`, `VENDORS.md`, `NATIVE_INVENTORY_EQUIP.md`).
- **[data]**: the installed packages read with `ow-package` (`--exports`, `--object-dump`): `Startup.upk` for the item definitions, parts, part
  lists, balances, pools, the four player class identifiers and `GD_Globals.General.Globals`; `WillowGame.upk` and `Engine.upk` for class
  declarations, class defaults and enums. A scan of the installed DLC packages looked for further class identifier objects. Counts are of
  `Startup.upk` unless a line says otherwise.
- **[native]**: native analysis of `Borderlands2.exe` with Ghidra (`tools/ghidra/run.ps1`, read-only queries against the saved project, 14 runs on
  2026-11-13, granted by the hub; the first failed before producing output because the native name tables were missing from its output
  folder, and the second repeated it): the shared part-entry weight rule, the relic definition's override of the weight hook, the item rarity
  calculation, the routine that builds the class mod pools and its teardown, the two class-weight resolvers and the player-controller context
  resolver. Where a line says **layout checked**, the fields the code reads were matched to the package layout with
  `tools/ghidra/class_layout.py`; that output is kept, with the list of fields matched, under the ignored `local/analysis/items/layout/`.

It builds on `NATIVE_WEAPON_RULES.md` (part pick, weapon rarity) and `NATIVE_LOOT.md` (pool rolls, balance candidates) and does not repeat them.

## 1. The relic part-weight hook

**Where it sits.** The entry weight of a part candidate is the rule of `NATIVE_WEAPON_RULES.md` section 3: a flat 100 when the entry has no
manufacturer list, otherwise the manufacturer's weight index or the default index, and 0 outside the entry's game-stage window [record]. Items
use the same entry rule (`NATIVE_LOOT.md`, "WillowItem.ChooseRandomParts") [record]. After that rule, and only when the weight is above 0, the
definition of the item being rolled is asked to adjust the weight if it sets `bAllowInventoryDefToModifyPartWeight` [native, layout checked].
The definition's answer replaces the weight; the candidate floor of 1e-8 then applies as for any other weight [record].

**Who sets the flag.** The class default of `ArtifactDefinition` sets it, and the defaults of `ShieldDefinition`, `GrenadeModDefinition` and
`ClassModDefinition` do not. None of the 30 relic, 33 shield, 12 grenade mod or 44 class mod definitions in `Startup.upk` overrides its class
default [data]. Every weapon type leaves it off (`NATIVE_WEAPON_RULES.md` section 12) [record]. So the hook acts on relics only.

**What the relic answer is** [native, layout checked]:

- Inputs: the part and its weight; the part's `AttributeSlotUpgrades` (the slot names it upgrades) and the relic definition's
  `AttributeSlotEffects` (the slot names the relic defines).
- A part with no slot upgrades keeps its weight.
- A part whose slot upgrades name at least one slot the relic defines keeps its weight.
- Any other part gets weight 0, so it is not a candidate.
- Slot names are compared whole (`Effect3` matches only `Effect3`). Nothing else is read: not the stage, not the manufacturer, not the slot the
  part is drawn for.

**Where it bites in the data** [data]. Relics carry 1 to 8 effect slots (11 of the 30 have exactly `Effect1` and `Effect2`; 5 have `Effect1`
only). All 30 relic definitions point their alpha and beta slots at shared "enable effect" lists (`GD_Artifacts.Enable1st` and `Enable2nd`);
two also point gamma at `Enable3rd` and one delta at `Enable4th`. These lists offer an enabler for every effect number, for example
`EnableFirst_Effect1` to `EnableFirst_Effect7`, and each enabler upgrades one slot. Summed over the 30 definitions, the hook removes 109 of the
210 first-effect and 109 of the 210 second-effect entries from those shared lists, and 3 of 14 and 3 of 7 in the third and fourth. It removes
no candidate that comes from a relic's own part-list collection, no body part, and no upgrade part (the grade parts `Upgrade_Grade0` to
`Upgrade_Grade15` name `Effect1` to `Effect7`, and every relic has `Effect1`).

**Which lists reach the pick is not resolved.** All 53 relic part-list collections in `Startup.upk` use the replacement mode `EPRM_Complete`
and list some slots only (21 list alpha, beta, eta and theta) [data]. That mode's merge is not read for weapons (`WEAPON_BALANCE_DECODE.md` section 2)
or items (`NATIVE_LOOT.md`) [record]. Two readings fit:

- (a) a slot the collection does not list falls back to the definition's shared list, and the hook then limits the enablers to the relic's own
  effects (the elemental relic, which defines `Effect1` and `Effect2` and whose collections list no beta slot, then draws a beta enabler
  from `EnableSecond_Effect1` and `EnableSecond_Effect2` only);
- (b) the collection alone supplies the slots, and in this data the hook then never changes a pick.

Reading (a) is the one that gives the flag a purpose; it is still a reading.

## 2. Rarity of shields, grenade mods, class mods and relics

**Rule** [native, layout checked]. Rarity is not rolled; it follows from the parts, with the same shape as the weapon rule
(`NATIVE_WEAPON_RULES.md` section 4) [record]:

- rarity level = the definition's `BaseRarity`, truncated to an integer, plus the truncated `Rarity` of each filled part slot (alpha to theta
  and material, nine slots). Each value goes through the shared evaluator (`NATIVE_PROGRESSION.md` section 1) [record].
- The two name parts (prefix, title) are counted only when the part walk is set to include them. Which setting the rarity sum uses was not
  resolved in the item code. Two things point to "counted", and both are readings: the weapon rarity on record sums both name parts
  (`NATIVE_WEAPON_RULES.md` section 12) [record], and six legendary shield titles carry a rarity (section below) [data].
- **Exception:** when the item's definition is a `MissionItemDefinition` or a subclass of it, the level is `GlobalsDefinition.MissionItemRarityLevel`
  instead of the sum: **500** in this data (the class default; `GD_Globals.General.Globals` does not override it) [native, data].
- The level is stored on the item by the same routine that starts the part value total from the definition's
  `BaseMonetaryValueModifier`, as for weapons [native]. That this happens when the item is initialised is a reading; the routine's callers were
  not followed.

**Which parts carry rarity** [data]. A part's `Rarity` points at one of the `GD_Balance_Inventory.Rarity_Item` attributes, constants 1
(`ItemRarity1_Common`) to 5 (`ItemRarity5_Legendary`); `ItemRarity4_VeryRare_Alien` is 2. Per kind, in `Startup.upk`:

| Kind | Where the rarity comes from | Range |
|---|---|---|
| Shield | the material part (`IP_Material`); parts in the other slots carry 0. Six titles of legendary or raid shields carry a rarity, five of them 5 and one 4 (below) | 1 to 5 |
| Grenade mod | one part in the epsilon slot (8 parts); the 9 legendary definitions under `GD_GrenadeMods.A_Item_Legendary` set `BaseRarity` 5 | 1 to 4, base 5 for the legendary ones |
| Class mod | the alpha-slot part (20 parts carry a rarity, 18 carry 0) | 1 to 5 |
| Relic | the theta (upgrade) part, `Upgrade_Grade0..3` = 1, `4..7` = 2, `8..11` = 3, `12..15` = 4; one body part carries 5 | 1 to 4, one body 5 |

No definition of these four kinds sets a `BaseRarity` other than the 9 legendary grenade mods [data].

**The legendary grenade mods.** By the rule, a legendary grenade mod sums its base 5 and its epsilon part's 1 to 4, so 6 to 9. At 6 it lies in
the band that `NATIVE_WEAPON_RULES.md` section 4 names Very Rare (rated VeryRare); 7 to 9 are Legendary. Which epsilon parts reach the pick is
the open merge question of section 1: the definitions' own epsilon lists offer all four rarities, while their part-list collections are
narrower (six of the nine offer only rarity-4 parts, so 9; `GrenadeMod_FlameSpurt`'s offers only rarity-1 parts, so 6) [data].

**The shield titles.** Of the 498 item name parts in `Startup.upk`, six carry a rarity, all titles of legendary or raid shields under
`GD_Shields.Titles`: five with 5 and `Title_FireNova02_Phoenix` with 4 [data]. Their six shields and their materials:

- the Phoenix (`Shield_Nova_Fire_Phoenix`, material 4, title 4): 8 with the title counted, 4 without;
- the two legendary absorption shields (`Shield_Absorption_05_LegendaryNormal` and `_LegendaryShock`, material 4, title 5): 9 or 4;
- the legendary chimera, the legendary corrosive spike and the Thresher raid shield (`Shield_Chimera_05_Legendary`,
  `Shield_Spike_CorrosiveLegendary`, `Shield_Roid_05_ThresherRaid`, material 5, title 5): 10 or 5, Legendary either way.

So the open name-part question changes a band for the first three only (Legendary with the titles counted, Very Rare without), and these
are the only shields in this data where it does.

**Bands** [data]. The band is the first `GlobalsDefinition.RarityLevelColors` entry whose `[MinLevel, MaxLevel]` holds the level; the
bands for levels 1 to 10 are in `NATIVE_WEAPON_RULES.md` section 4 [record]. An entry's `RarityRating` names the tier (`EItemRarity`: 0 Unknown,
1 Common, 2 Uncommon, 3 Rare, 4 VeryRare, 5 Legendary, 6 Seraph, 7 Rainbow). New here: level 0 has its own entry, rated Unknown, and level
500 (mission items) has one rated Unknown. The colours are read from the install by `tools/game_colours.py` and are not recorded in the tree
(decision `docs/decisions/2026-11/2026-11-08-02-the-game-s-ui-colours-move-out-of-the-code-into-an-ignored.md`).

Level 11 lies in no entry (the next one starts at 12). Further entries from 12 to 506 serve other inventory (usable items set base rarities
such as 171, 176, 181 and 502 to 504) and are not covered here. A pickup is announced from rarity level 2 up
(`AnnouncePickupRarityThreshold`, `NATIVE_INVENTORY_EQUIP.md`) [record].

## 3. How the class mod pools are filled

**What the packages hold** [data]. The global per-rarity pools that `Pool_ClassMod_All` draws from are named by the globals definition
(`CommonClassMods` to `SlayerOfTerramorphousClassMods`: `GD_Itempools.ClassModPools.Pool_ClassMod_01_Common`, `_02_Uncommon`, `_04_Rare`,
`_05_VeryRare`, `_06_Legendary`, `_06_SlayerOfTerramorphous`). They have no entries, only their stage gates (`LootSchedule_ClassMod_01_Common`
= 8, `_02_Uncommon` = 10, `_03_Rare` = 13, `_04_VeryRare` = 16, `_05_Legendary` = 20). Each player class identifier (`GD_PlayerClassId.Siren`
and three more) names six per-class pools with the same roles (`Pool_ClassMod_Siren_01_Common` to `_06_SlayerOfTerramorphous`), each holding
one class mod balance (`GD_ItemGrades.ClassMods.BalDef_ClassMod_Siren_01_Common` and so on). Its `ClassDropInitializationDefinition` is empty.
`WillowGlobals.PlayerClassIdentifiers` is empty in the class default, so the list is filled at run time (by what was not read).

**What holds once the pools are built** [native, layout checked except where noted]. A guarded initialisation of the globals object
builds them, and a matching cleanup undoes the build and clears the guard. The build's immediate caller, a wider set-up routine of the globals
object, runs it only under conditions, among them that the globals definition is present. What triggers that caller, and whether the build
can run more than once in a session, were not resolved. After the build:

- Each of the six global class mod pools holds one entry per class in `PlayerClassIdentifiers` whose six per-class pools are all set, in the
  list's order, and nothing else. The entry nests that class's pool of the same role, names no balance and drops on death. The global
  `Pool_ClassMod_04_Rare` pairs with each class's `RareClassMods`, which is its `_03_Rare` pool [data].
- Each entry's probability is the class's drop weight **A × B**, evaluated when the pool is rolled (`NATIVE_PROGRESSION.md` section 1). A is
  the player-class match and B the class drop weight, both below. The same weight is what each identifier's `ClassDropInitializationDefinition`
  holds after the build, so a live read can look for it there.
- When the Aster DLC's pools are loaded, `GD_Aster_ItemPools.ClassModPools.Pool_ClassMod_00_Aster` follows the same scheme: one entry per class
  for its per-class Aster sub-pool (`Pool_ClassMod_<Class>_Aster`, for Assassin, Mechromancer, Merc, Psycho, Siren and Soldier), weighted with
  that class's drop weight. Which class a sub-pool belongs to goes through the class its single class mod balance requires; that association
  is inferred and not resolved. Likewise `GD_Lobelia_Itempools.ClassModPools.Pool_ClassMod_Lobelia_All`, when loaded, holds one entry per class
  whose `LobeliaClassMods` is set; no identifier in `Startup.upk` sets it, and who does was not read.
- After the cleanup the identifiers hold no drop weight and these pools are empty again.

**The two resolvers** [native]:

- **`ClassDropWeightValueResolver`** (B, no context needed). Let n be the number of players in the game's player list and k the number of them
  whose character's class identifier is this class (layout checked: the player list, the character name identifier and its class identifier;
  the step from the world to that list was not). Let f be
  `GlobalsDefinition.ClassDropChance[n - 1]` (layout checked), which is **0.5, 0.33, 0.25, 0.2** for 1 to 4 players [data].
  - k at least 1: B = k × f.
  - k = 0: B = (1 − n × f) / C, where C is the number of entries in `PlayerClassIdentifiers` (all classes, present or not); 0 when the list is
    empty.
  - n outside 1 to 4, or no game: B = 0.
- **`PlayerClassAttributeValueResolver`** (A). It answers 1 when its context is a `WillowPlayerController` whose character's class is this
  class, else 0. (The controller-side fields could not be layout checked: the layout tool stops on that class.)
- **`PlayerControllerAttributeContextResolver`** gives A its context: the roll's context object itself if it implements `IPlayerBehavior`,
  else the controller returned by `IPlayerMaster` if it implements that, else nothing. In `WillowGame`, `Engine` and `GearboxFramework` only
  `WillowPlayerController` implements both, and `MissionTracker` implements `IPlayerBehavior` [data]. So A depends on what the context
  resolves to:
  - nothing: the attribute does not apply and A keeps its constant 1 (`NATIVE_PROGRESSION.md` section 1) [record];
  - a player controller, given directly or handed out through `IPlayerMaster`: A is 1 for that player's class and 0 for every other;
  - any other object (a `MissionTracker`): A is 0 for every class, so nothing can be drawn; no roll is known to pass one.

**What that means for a roll** (derived from the rules above; nothing here was seen in the game):

- An enemy's death roll passes the pawn as context (`NATIVE_LOOT.md`) [record]. When the pawn resolves to no controller, which holds if it
  implements neither interface (no pawn class declared in those three packages carries either), A = 1 and each class weighs B. Solo with
  four identifiers: the player's class 0.5, every other class (1 − 0.5) / 4 = 0.125 (together 0.875, not 1), so a common class mod is the
  player's class with probability 0.5 / 0.875 = 4/7, about 57 %.
- With a player controller as the context, A is 1 only for that player's class: every class mod from the pool is for that player's class.
  Mission reward rolls pass the player controller (`NATIVE_LOOT.md`, "MissionDefinition.GetItemRewardsForPlayer") [record], so a class mod
  reward drawn from these pools is always the player's class. Which context vendors and chests pass was not read.
- The weights are evaluated when a pool is rolled, so they follow the party as it is then; only the entries are fixed at build time.
- At stage 8, the slice's stage, the global common pool passes its gate (minimum 8), so `Pool_ClassMod_All`'s common branch is reachable once
  the pool is filled. Uncommon and above are gated off until stages 10 to 20 [data].
- Which class mod comes out of a per-class pool is the ordinary balance roll (`NATIVE_LOOT.md`), except the choice among the class mod
  definitions a class mod balance lists (see "Open").

**On this install** [data]: `Startup.upk` holds the four base-class identifiers (Assassin, Mercenary, Siren, Soldier). No installed package
holds another identifier object: the Mechromancer and Psycho DLC folders (`Tulip`, `Lilac`) hold customization packages, and the scan found no
identifier in them. So C is 4 here unless something adds identifiers at run time.

## Corrections to records on main

- `NATIVE_LOOT.md`, "Corrections to earlier notes", item 1: the common class mod pool is empty in the packages but filled at run time (section 3),
  and its gate passes at stage 8. The weapon share computed in that item (57.1 %) rests on the pool being empty, so it needs reassessing. A
  replacement share is not established: it also depends on the build having run before the roll and on `PlayerClassIdentifiers` being
  filled, both open here.
- `VENDORS.md` section 6 says the running game fills these pools "in some way that is `UNVERIFIED`", and the comment in
  `tools/prepare_vendors.py` says "filled at runtime; how is UNVERIFIED". How they are filled is now read from native code (section 3); it is
  still UNVERIFIED in the game.
- `NATIVE_WEAPON_RULES.md` section 3 ("not read") and `WEAPON_PARTS_CATALOG.md` section 2: what the relic definition does with the weight is now
  read (section 1).
- `NATIVE_LOOT.md`, "WillowItem.ChooseRandomParts", "Open": item rarity is now read (section 2).

## What an implementer needs to decide

1. **The relic hook.** Implement it as one check in the shared part pick: when the item's definition sets the flag and the weight is positive,
   drop a part that has slot upgrades, none of which names a slot the definition defines. A part with no slot upgrades keeps its weight. It is cheap and harmless under both readings of the merge mode. Which lists
   reach the pick under `EPRM_Complete` is a separate decision (section 1); reading (a) is the plain stand-in, marked UNVERIFIED.
2. **Item rarity.** One function for the four kinds: truncated base plus the truncated rarity of each filled part slot, the band from
   `RarityLevelColors`; mission items take the globals' 500. Decide whether name parts count: counting them is the reading the data supports,
   and it moves three legendary shields of section 2 from Very Rare to Legendary.
3. **Class mod pools.** At load, for each class identifier the host knows, append one entry per global pool as in section 3, and evaluate the
   weight A × B at each roll. In the slice (Maya alone, enemy drops) that is Siren 0.5 and each other base class 0.125. Decide:
   - whether the host keeps the other classes' entries (the stock rule drops their class mods too; equipping one is refused by the class
     requirement);
   - what the host passes as the roll's context (a pawn that resolves to no controller gives the mixed weights, a controller gives only the
     player's class);
   - the class count C: the four base classes on this install. A host that adds the DLC classes changes every weight for absent classes.
4. **Which class mod definition** a class mod balance yields is still open (below); a stand-in is needed before a class mod can be rolled.

## What a session should read

1. **Relic slots:** spawn many drops of `GD_Artifacts.A_Item.A_Elemental` at one stage and read the beta slot. Its relic defines exactly
   `Effect1` and `Effect2`, and its collection lists alpha, eta and theta but no beta [data], so beta is where the readings differ:
   - reading (a) predicts a beta part on every drop, always `EnableSecond_Effect1` or `EnableSecond_Effect2`, about equally often (neither
     entry in the shared list has a manufacturer list, so each weighs a flat 100); an `EnableSecond_Effect3` to `_Effect7`
     would mean (a) without the hook;
   - reading (b) predicts an empty beta slot on every drop.
   The alpha and theta parts come from the collection under both readings, so their frequencies check only the weights.
2. **Rarity:** the `RarityLevel` of spawned shields, grenade mods, class mods and relics against the part sum above, one legendary grenade mod,
   and one mission item (500). One spawned Phoenix or legendary absorption shield settles the name-part question (8 or 9 if titles count,
   4 if not). One `GrenadeMod_FlameSpurt`, whose collection lists only rarity-1 epsilon parts, should read 6 under either merge reading; that
   checks the sum rule and the level-6 band, not a reading.
3. **Class mod pools:** live, after loading a save: `WillowGlobals.PlayerClassIdentifiers` (count and order), the entries of
   `Pool_ClassMod_01_Common` (one per class, each with its nested pool and a probability whose definition is a run-time object), each
   identifier's `ClassDropInitializationDefinition`, and the value that definition gives with a pawn and with the player's controller as context.
   Then the class of class mods from many solo enemy drops: about 4/7 Siren with four classes if the pawn resolves to no controller, all
   Siren if it resolves to one.
4. **Contexts:** many class mods from one vendor and from chests: only Siren means the roll passes a controller, a mix as in item 3 means it
   does not. One mission reward that offers a class mod should be Siren under the recorded controller context.

## Open

- **Which class mod definition a class mod balance yields.** A class's balances list nine `ClassModDefinitions` on a shared base
  (`BalDef_ClassMod_Siren` and the others), with eight manufacturers on `BalDef_ClassMod_AllParts` [data]. The choice is made by
  `ClassModBalanceDefinition`'s own `GetInventoryDefinitionForManufacturerGrade` (a native override, not read here). This blocks a class mod roll.
- **The manufacturer of an item with several:** `NATIVE_LOOT.md`'s candidate expansion already covers it (one candidate per manufacturer entry,
  weighted by its grade) [record]. The stock shield card of 2026-11-13 takes the first entry, a stand-in its decision file already names.
- The `EPRM_Complete` merge for items (section 1); whether name parts count in item rarity (section 2); how a level in no band (11) is shown,
  since the `GlobalsDefinition` rarity lookups were not read.
- What triggers the build's caller, whether the build can run more than once, and what fills `PlayerClassIdentifiers` and `LobeliaClassMods`
  (section 3).
- The same pass also builds keyed customization pools from each identifier's `CharacterCustomizations` with the class drop weight; not described.
- Card rounding of item stats: not read.
- Counts and bands are of `Startup.upk`; DLC item definitions (for example bands above 500) were not surveyed.
- Everything above is UNVERIFIED in the running game.
