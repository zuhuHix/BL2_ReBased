# Native loot: how item pools and balances are rolled (2026-10-05)

AI-assisted (Claude), analyst lane G1. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Field names were matched to the native code with
`tools/ghidra/class_layout.py`: every offset used landed on a field of the expected type, including the three
`BalanceModifierDefinition` override fields and the `AIPawnBalanceDefinition` playthrough struct, which is the strongest
oracle this note has. Script behaviour (spawn setup, death drop) was read with `ow-package --disasm` from the installed
package and is labelled "script" where it is not native. Data counts were decoded from `Startup.upk` with the project
reader and are quoted where a rule needs them. Game-derived output (decompilation, listings, my own census scripts and
their results) is under ignored `local/p2/G1/` and `C:\Users\yorad\bl2-analysis-2\out`.

Scope: how a pool or a balance becomes inventory (`ItemPool` natives, the balance helpers they use), where a mission
reward's items come from (pool-roll side only; lane C1 owns the tracker's turn-in chain), and the enemy drop on death at
a summary level. Weapon part choice, card stats, level requirement and sale value are in
[NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) (read first; this note extends it and points to it). The shared attribute
evaluator is [NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) section 1. The region game stage that decides a mission's
loot level is NATIVE_PROGRESSION.md section 5 ("Region game stage"; the mission's game-stage getter).

## Summary
| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| ItemPool.SpawnBalancedInventoryFromPool | `static bool SpawnBalancedInventoryFromPool(ItemPoolDefinition Definition, int GameStage, int AwesomeLevel, Object ContextSource, out array<Inventory> SpawnedInventory, optional AttributeInitializationDefinition GameStageVarianceFormula, optional float OuterPoolChance, optional bool bInventoryMayDropOnDeath)` | Every enemy drop, chest and mission reward roll | high | UNVERIFIED |
| ItemPool.SpawnBalancedInventoryFromInventoryBalanceDefinition | `static bool SpawnBalancedInventoryFromInventoryBalanceDefinition(InventoryBalanceDefinition InvBalanceDefinition, int Quantity, int GameStage, int AwesomeLevel, Object ContextSource, out array<Inventory> SpawnedInventory)` | Lent mission weapon, reward balances | high | UNVERIFIED |
| (internal) balance candidate expansion | not script-visible | One candidate per manufacturer: grade, level, weight | high | UNVERIFIED |
| InventoryBalanceDefinition.GetInventoryDefinitionForManufacturerGrade | `InventoryDefinition GetInventoryDefinitionForManufacturerGrade(ManufacturerDefinition Manufacturer, int ManufacturerGradeIndex)` | Which weapon type / item definition a balance yields | high | UNVERIFIED |
| InventoryBalanceDefinition.GetExpLevelFromManufacturerData | `int GetExpLevelFromManufacturerData(ManufacturerDefinition Manufacturer, int ManufacturerGradeIndex)` | Dropped item's level | high | UNVERIFIED |
| InventoryBalanceDefinition.GetInventoryPartListCollection | `InventoryPartListCollectionDefinition GetInventoryPartListCollection(class PartListCollectionClass, ManufacturerDefinition Manufacturer, int ManufacturerGradeIndex)` | Which part list a balance rolls from | medium | UNVERIFIED |
| (internal) game-stage level cap | not script-visible | Caps every rolled stage (50 in the slice) | medium | UNVERIFIED |
| ItemPoolListDefinition.AddToItemPoolList | `void AddToItemPoolList(out array<ItemPoolInfo> Out_ItemPoolList)` | Builds an enemy's pool list from `StandardEnemyGunsAndGear` | high | UNVERIFIED |
| AIPawnBalanceDefinition.SetupPawnItemPoolList | `bool SetupPawnItemPoolList(WillowAIPawn SpawnedPawn)` | Which pools a spawned enemy rolls | high | UNVERIFIED |
| AIPawnBalanceDefinition.GetPlayThroughIndex | `int GetPlayThroughIndex()` | Picks the playthrough entry (0 in the slice) | medium | UNVERIFIED |
| MissionDefinition.GetItemRewardPools | `void GetItemRewardPools(out array<ItemPoolDefinition> ItemPools, bool bAltReward)` | Fire mission has none | high | UNVERIFIED |
| MissionDefinition.GetItemRewardsForPlayer | `void GetItemRewardsForPlayer(WillowPlayerController WillowPC, out PendingMissionRewardData MissionReward)` | Reward item choices (none for Fire) | medium | UNVERIFIED |
| AMissionTracker.GrantMissionWeapon (pool-roll side only) | internal, called from the status change | Level and roll of the lent Maliwan pistol | medium | UNVERIFIED |
| WillowPawn.GetGameStageForSpawnedInventory / SetGameStageForSpawnedInventory | `int GetGameStageForSpawnedInventory()` / `void SetGameStageForSpawnedInventory(int)` | Enemy loot level | low (accessors not read) | UNVERIFIED |
| BalanceModifierDefinition.GetAmmoDropsPerPlayerMultiplier | `float GetAmmoDropsPerPlayerMultiplier(Object Context, int GameStage)` | 1.0 in the slice | medium | UNVERIFIED |
| BalanceModifierDefinition.GetUncommonChestItemPoolWeightMultiplier | `float GetUncommonChestItemPoolWeightMultiplier(int GameStage)` | Chests only | medium | UNVERIFIED |
| BalanceModifierDefinition.GetCommonGearDropWeightBaseValue | `float GetCommonGearDropWeightBaseValue(...)` | Data attribute helper, not in the slice path | low | UNVERIFIED |
| ItemPool.ToggleAllItemTypesDebug / IsAllItemTypesDebugEnabled | `void` / `bool` | Debug switch; documented to be ignored | medium | UNVERIFIED |
| InteractiveObjectBalanceDefinition.SetupInteractiveObjectLoot | `bool SetupInteractiveObjectLoot(WillowInteractiveObject SpawnedInteractiveObject, int GradeIndex)` | Chests and lockers (summary only) | low | UNVERIFIED |
| WillowItem.ChooseRandomParts (item part weights) | `void ChooseRandomParts(...)` | Shields, grenade mods, class mods | low-medium (weight rule only) | UNVERIFIED |

## Overview of the flows (what calls what)

**Enemy drop on death** (script, with the natives below):

1. When a balanced enemy spawns, script stores its game stage and exp level, works out the *item game stage* (below), and
   calls `AIPawnBalanceDefinition.SetupPawnItemPoolList`, which writes the pawn's `ItemPoolList` (pool plus probability).
2. Pools in that list whose `bAutoReadyItems` is **true** (class default true) are rolled once when the pawn's default
   inventory is added (script `AddDefaultInventory` calls `WillowPawn.AddPoolItems` with "readied" true) and the items are
   given to the pawn: this is the gear the enemy fights with. If the pawn still has no weapon afterwards, script
   `AddDefaultWeapon` rolls the pawn class's `DefaultWeapon` balance once through
   `SpawnBalancedInventoryFromInventoryBalanceDefinition` at the pawn's own `GameStage` (not the item game stage).
3. On death `WillowPawn.DropLootOnDeath` rolls the pools whose `bAutoReadyItems` is **false** (667 of the 826 pools in
   `Startup.upk` set it false explicitly). For each such pool: the pool's `PoolProbability` is evaluated with the pawn's
   controller as context (a pool whose probability definition is named `AmmoDrops_PerPlayer` is scaled once per death by
   `GetAmmoDropsPerPlayerMultiplier`); the pool fires when the chance is above 0 and a uniform random float `FRand()` is
   **less than or equal to** it. A firing pool calls `SpawnBalancedInventoryFromPool` with the pawn's item game stage, the
   pawn's awesome level, the pawn as context, **no variance formula** and the default flags. Every spawned inventory whose
   `bDropOnDeath` is true is dropped at the pawn with a toss velocity; the others are destroyed. A pawn that died below the
   level's `KillZ` still drops non-weapon items but destroys weapons.
4. Item game stage (script `PopulationFactoryBalancedAIPawn.SetupBalancedPopulationActor`): the stage is the pawn's
   exp level when the balance's `BaseItemGameStage` is `IGSS_ExpLevelOfBalancedActor`, otherwise the population's game stage
   (`IGSS_GameStageOfBalancedActor`, the class default), plus the truncated `ItemGameStageOffset` evaluated with the pawn as
   context (class default 0).

**Mission reward items:** `MissionDefinition.GetItemRewardsForPlayer` rolls the mission's `Reward` (or `AlternativeReward`)
`RewardItems` balances and `RewardItemPools` at the mission's game stage and keeps at most two results (below). Lane C1
owns the call chain around it.

**Lent mission weapon:** `AMissionTracker.GrantMissionWeapon` rolls the mission's `MissionWeapon` balance once per local
player at the region's game stage (below).

## ItemPool.SpawnBalancedInventoryFromPool
- **Signature:** as in the table. Defaults when omitted: no variance formula, `OuterPoolChance` 1.0,
  `bInventoryMayDropOnDeath` true. Returns whether the roll produced what it was asked to.
- **Reads:** the pool (`ItemPoolDefinition`: `Quantity`, `MinGameStageRequirement`, `MaxGameStageRequirement`,
  `BalancedItems`, flags `bSupportsGameStageVariance`, `bEligibleForUncommonWeightMultiplier`,
  `bShopsHaveInfiniteQuantity`; each `BalancedInventoryData` entry has `ItmPoolDefinition`, `InvBalanceDefinition`,
  `Probability`, `bDropOnDeath`), the context source (for attribute evaluation), the global level cap.
- **Does (in order of effect):**
  1. Evaluates `Quantity` (an `AttributeInitializationData`, so it can be a random definition: `Pool_Money_1or2` uses an
     integer random variance) with the context source and truncates it to an integer. Evaluates the two stage-requirement
     attributes with the same context and truncates them; an unset attribute is 0. In `Startup.upk` these attributes are
     constants such as `Gamestage_07` (7), 163 pools have a minimum and one pool has a maximum.
  2. **Pool gate.** The pool rolls only if its stage window admits the caller's stage and at least one entry can be reached
     (below). Window: minimum 0, or (minimum <= stage and minimum <= level cap), and maximum 0, or stage <= maximum.
     Reachable: some entry is a *real nested pool* that itself passes the same gate for the same stage, cap and context
     (recursively), or some entry has a non-null balance. Failing the gate returns false and spawns nothing; no random number
     is drawn.
  3. **Candidate list.** Entries are visited in `BalancedItems` order and turned into weighted candidates (the sequence
     matters for the pick):
     - **Nested-pool entry**: `ItmPoolDefinition` set **and that pool has at least one entry of its own**. Weight = the
       entry's `Probability` evaluated with the context source. If the *outer* pool has
       `bEligibleForUncommonWeightMultiplier` and the entry's probability definition is named `Weight_2_Uncommon`, the weight
       is multiplied by the uncommon chest multiplier (below). The weight becomes 0 unless the nested pool passes the gate
       at the caller's original stage. One candidate.
     - **Balance entry**: `ItmPoolDefinition` unset **or pointing to a pool with no entries**, and `InvBalanceDefinition`
       set. The entry weight is the `Probability` evaluated with the context source. The entry's stage is the caller's
       stage, adjusted only when the pool has `bSupportsGameStageVariance` (class default true) **and** a variance formula was
       passed: add the truncated evaluated variance; if the caller's stage was above 0 the result is at least 1, otherwise at
       least 0; then, **only on this variance path**, raise it to the pool's minimum stage and lower it to the pool's
       maximum stage when those are above 0. The balance is expanded into candidates at that stage (section "balance
       candidate expansion"). Each candidate's weight = its grade modifier x entry weight / number of candidates from this
       entry. A balance that yields no candidates contributes nothing.
     - An entry with neither a usable pool nor a balance is ignored. If an entry has both a non-empty pool and a balance,
       the pool wins.
     - A candidate weighing 1e-8 or less is not added. A candidate identical in all identity fields to one already present
       replaces its weight and keeps its position (cannot arise from ordinary data).
  3b. If a nested pool is empty or ineligible it takes **no** share: remaining candidates are chosen among themselves.
  4. **Quantity times:** draw `r = total weight x rand() / 32767` with the C runtime `rand()`, walk the candidates in list
     order adding weights, and take the first whose interval `[sum, sum + weight]` (both ends inclusive, single precision)
     contains `r`. The same candidate can be picked again (with replacement); the list is never rebuilt. With no candidate
     the iteration fails (counts against the return value) and nothing is spawned for it.
     - **Picked nested pool:** the function runs again on that pool with the same stage, awesome level, context source,
       output array and variance formula; `OuterPoolChance` becomes (picked weight x current `OuterPoolChance` / total);
       `bInventoryMayDropOnDeath` becomes (entry `bDropOnDeath` AND the current flag). The nested call's own result is
       ignored by the caller. `OuterPoolChance` has no effect on what spawns in any path read (it only travels down).
     - **Picked balance candidate:** spawn one inventory (next paragraph).
  5. **Spawning one inventory (pool variant).**
     1. Re-resolve the inventory definition with `GetInventoryDefinitionForManufacturerGrade` on the entry's balance, using the
        candidate's manufacturer and stored grade-or-level word. Its `InventoryClass` must be non-null, otherwise this pick fails.
     2. Create an actor of that class and verify it is a `WillowInventory`.
     3. Bookkeeping: `SourceDefinitionName` = the pool's object name; `SourceResponsibleName` = a name supplied by an engine
        helper that was not resolved (the balance variant takes it from the context source instead). Both are only recorded
        (telemetry); nothing in the roll reads them.
     4. `SetGameStage(candidate level)`, where the candidate level is the entry's stage capped by the level cap (so the item's
        stored game stage never exceeds the cap); `SetAwesomeLevel(awesome)`.
     5. Run the script event `InitializeInventory(balance, manufacturer, grade-or-level, context source)` on the new
        inventory. For weapons and items this is the script that sets the definition data, exp level
        (`GetExpLevelFromManufacturerData`), rolls parts and name parts, and so on (NATIVE_WEAPON_RULES.md).
     6. `bDropOnDeath` of the inventory := (entry `bDropOnDeath` AND `bInventoryMayDropOnDeath`).
     7. `bShopsHaveInfiniteQuantity` := the inventory definition's flag OR the pool's flag.
     8. Append to the output array.
- **Calls into script:** `Inventory.InitializeInventory` (via the event mechanism, by name) per spawned inventory, then
  everything that script runs; `SetGameStage`, `SetAwesomeLevel` through the balanced-actor interface.
- **Calls other natives:** `GetInventoryDefinitionForManufacturerGrade`, the balance candidate expansion, the attribute
  evaluator (`AttributeInitializationDefinition.EvaluateInitializationData` family), the global level cap, the uncommon chest
  multiplier, `rand()`.
- **Constants / formulas:** flat random `rand()/32767` (1/32767 = 3.0518509e-05); weight floor 1e-8; level cap 50 (below).
  Default `OuterPoolChance` 1.0.
- **Edge cases:** `Quantity` <= 0 spawns nothing. A nested pool with an empty `BalancedItems` is treated as absent
  (46 of the 591 pool-to-pool entries in `Startup.upk` point at one). A pool whose every entry is a balance with no
  candidate at the stage rolls but spawns nothing, and it still consumed its share at its parent. Only `Quantity` repeats
  the draw; nothing else re-rolls.
- **Implementer checklist:**
  - A pool with no reachable entry, or outside its stage window, returns false and draws no random number.
  - Nested pools and balance entries compete in one flat weighted list per pool (order of `BalancedItems`).
  - Empty nested pools and balances with no candidate at the stage take no share.
  - The stage written to a spawned item is `min(stage, cap)`; with the interpolating balances of the base game its exp level
    is the same number (section "GetExpLevelFromManufacturerData").
  - Entry `bDropOnDeath` AND the caller flag is what the death roll tests; a false item is destroyed, not dropped.
  - Stage clamping to the pool's min/max happens only when a variance formula is supplied and `bSupportsGameStageVariance`.
  - Enemy drops pass no variance formula, so enemy stage is the unmodified item game stage.
- **Open:** the engine helper behind `SourceResponsibleName`; the return value when only some picks fail (read: false if any
  pick failed to spawn, nested results ignored).

## Balance candidate expansion (internal; used by both spawn natives)
- **Inputs:** a balance, a stage, a level cap (a cap of 0 or less means the global cap).
- **Reads:** the balance's `Manufacturers` (each: `Manufacturer`, `Grades`), each grade's `GameStageRequirement` window
  (inclusive `MinGameStage`..`MaxGameStage`), `MinSpawnProbabilityModifier`, `MaxSpawnProbabilityModifier`, the balance's
  `bInterpolateExpLevel` (class default true; 1 balance in `Startup.upk` sets it false: `GD_GrenadeMods.A_Item_Custom.GM_SkyRocket`), and its `BaseDefinition`.
- **Does:** let `s` = `min(stage, cap)`. Start with the balance itself, then its `BaseDefinition`, and so on (a balance
  already being expanded in this call yields nothing; this is the cycle guard). For each balance in that chain, for each
  manufacturer entry whose manufacturer is set: choose the **first** grade whose window contains `s` (min <= s <= max); no
  grade means skip. Ask the **original** (first) balance for the inventory definition of that manufacturer and grade
  index (section below); a null answer means skip. Otherwise add one candidate:
  - definition, manufacturer;
  - "grade-or-level" word = `s` when the original balance interpolates exp level (the base-game case), otherwise the grade
    index;
  - stage stored = `s`;
  - weight = the grade's modifier at `s`: evaluate `MinSpawnProbabilityModifier` and `MaxSpawnProbabilityModifier`
    (the context is the local player's data when a game is running, otherwise none); if the window has a positive span
    (`max - min`) the weight is `minMod + (s - windowMin) / span x (maxMod - minMod)` in single precision, else `minMod`.
  The walk **stops at the first balance in the chain that produced at least one candidate**; later bases are not consulted.
  Returns the number of candidates.
- **Constants / data:** over the 1,091 inventory balances in `Startup.upk`: 969 manufacturer entries have one grade;
  windows are mostly 0..100 (562) or 1..100 (322), 34 are 1..10000; Min and Max modifiers are 1 for the great majority,
  0.5 for 89 grades (e.g. `ItemGrade_Currency_Money_Big`, `ItemGrade_BuffDrink_HealingInstant`: 0.5 at the window start
  rising to 1 at its end, about 0.535 at stage 8), a few 0.25, 0.2, 0.6 or 0; 256 balances list no manufacturers of their own
  (they rely on their base).
- **Debug switch:** when the "all item types" debug flag is on (`ItemPool.ToggleAllItemTypesDebug`), the grade is picked
  uniformly at random from the entry's grades instead of by window and every modifier is 1.0. Ignore it for the port.
- **Edge cases:** at stage 8, 20 grenade-mod balances in `GD_GrenadeMods.A_Item` (Mirv, Bouncing Betty, Singularity, Area
  Effect, Transfusion at Common, Uncommon, Rare and Very Rare) have no grade covering the stage and so yield no candidate; they
  take no share.
- **Implementer checklist:** one candidate per manufacturer entry with a covering grade; weight is the linear interpolation of
  the two modifiers across the window; first balance in the chain with any candidate wins; stage is capped before windows
  are tested.
- **Open:** the context used while evaluating the modifiers (only constants occur in the data read).

## InventoryBalanceDefinition.GetInventoryDefinitionForManufacturerGrade
- **Signature:** as in the table.
- **Reads:** the balance's `Manufacturers`, `bInterpolateExpLevel`, `InventoryDefinition`, each grade's
  `GradeModifiers.CustomInventoryDefinition`, `BaseDefinition`; the definition's `InventoryClass`.
- **Does:** find the entry whose `Manufacturer` equals the argument in **this** balance's own list; if none, defer to the
  base. If the balance interpolates exp level the answer is the balance's own `InventoryDefinition`. Otherwise take the
  grade index from the low 16 bits of the second argument; it must be below the entry's grade count (else defer to the
  base); the answer is the grade's `CustomInventoryDefinition` when set, else the balance's `InventoryDefinition`. If the
  resulting definition's `InventoryClass` is null the answer is null. If the answer is null and the balance has a
  `BaseDefinition`, the base's answer is returned (same arguments).
- **Edge cases:** a manufacturer that only a base lists resolves through the base; a recursion guard returns null for a
  balance already in the middle of this call.
- **Implementer checklist:** resolve against the original balance first; base only on a null result or a missing
  manufacturer; an interpolating balance ignores the grade argument for the definition.
- **Open:** none beyond UNVERIFIED.

## InventoryBalanceDefinition.GetExpLevelFromManufacturerData
- **Signature:** as in the table.
- **Does:** default result 1. If the balance interpolates exp level, the result is the second argument unchanged, which is
  the level stored by the candidate expansion: **`min(stage, cap)`**, no further clamp. Otherwise: find the manufacturer in
  this balance's list; if the grade index (low 16 bits) is within its grades, then when the high 16 bits are zero the result
  is that grade's `GradeModifiers.ExpLevel`, else the high 16 bits capped at 63. If not resolved that way and the balance has
  a base, the base's answer is used.
- **Implementer checklist:** an item rolled at stage 8 has level 8; at stage 60 with cap 50 it has level 50. The weapon note's
  "level = the game stage it spawned at" holds with this cap.
- **Open:** none beyond UNVERIFIED.

## InventoryBalanceDefinition.GetInventoryPartListCollection
- **Signature:** as in the table.
- **Does:** the balance answers with its own part-list collection (through its internal collection getter, read as the
  `PartListCollection` field) when it lists **no manufacturers at all**, or lists the argument manufacturer and either
  interpolates exp level or has the grade index in range. A manufacturer the balance does not list, or an out-of-range grade
  on a non-interpolating balance, sends the question to the `BaseDefinition`. The collection must be an instance of the
  requested collection class, otherwise it counts as no answer and the base is asked; no base means null. The merged runtime
  list is described in [WEAPON_BALANCE_DECODE.md](WEAPON_BALANCE_DECODE.md) section 2.
- **Open:** the collection getter itself was not read; the weapon note's merge rule is unchanged.

## Level cap (internal, used by both spawn natives and the mission reward roll)
- **Does:** returns 50 for every playthrough except the third. In the third playthrough, with the level-cap upgrades present,
  the result is 80 (50 plus 11, 11 and 8) plus the player's overpower level capped at 10 (8 plus 2); without the upgrades the
  intermediate value is returned. The third-playthrough branch was read at overview level.
- **Implementer checklist:** pass 50 as the cap for the slice (playthrough 1); a stage of 8 is untouched.
- **Open:** exactly how the intermediate values are chosen in the third playthrough.

## ItemPool.SpawnBalancedInventoryFromInventoryBalanceDefinition
- **Signature:** as in the table.
- **Does:** expands the balance into candidates (above) at `min(stage, cap)`, each weighted by its grade modifier divided by
  the candidate count. Then `Quantity` times: pick one candidate by the same weighted draw (`total x rand() / 32767`, first
  inclusive interval) and spawn one inventory exactly as the pool variant does, with these differences: the inventory's
  `SourceDefinitionName` is the balance's name, `bDropOnDeath` is **always set to true** (the pool's flags do not apply),
  and `bShopsHaveInfiniteQuantity` is not touched. When a context source is given, the new inventory's
  `SourceResponsibleName` is taken from it. Returns true when every requested spawn succeeded.
- **Calls into script:** `Inventory.InitializeInventory`, then what it runs.
- **Constants / formulas:** same as the pool variant.
- **Edge cases:** zero candidates (e.g. a grade window that does not contain the stage) spawns nothing and returns false.
- **Implementer checklist:** `Quantity` times, with replacement; `bDropOnDeath` true; stage and level `min(stage, cap)`.
- **Open:** none beyond UNVERIFIED.

## ItemPoolListDefinition.AddToItemPoolList
- **Signature:** as in the table.
- **Does:** for each list in `ItemPoolIncludedLists` in order, calls the same function on it (so included lists are expanded
  depth first, and a cycle would not terminate), then appends this list's own `ItemPools` entries (pool plus probability).
  Included lists therefore come first.
- **Implementer checklist:** order is included lists (recursively, in array order) then own pools.
- **Open:** no cycle guard was seen.

## AIPawnBalanceDefinition.SetupPawnItemPoolList
- **Signature:** as in the table.
- **Does:** takes the playthrough entry selected by `GetPlayThroughIndex`. If the index is below 0 or beyond the entry
  array it returns false and sets nothing (so a balance with an empty `PlayThroughs` array gets no pool list at all, not even
  the defaults). Otherwise builds one list: first the included lists, then the pool list, where **the playthrough entry's
  `CustomItemPoolIncludedLists` replace `DefaultItemPoolIncludedLists` when the custom array is non-empty**, and
  independently **`CustomItemPoolList` replaces `DefaultItemPoolList` when non-empty**. Each included list is expanded with
  `AddToItemPoolList`. The result (included lists' pools first, then the chosen own list) is stored on the pawn's
  `ItemPoolList`; the function returns true when that list is not empty.
- **Edge cases:** `PlayThroughs` empty; the game not running (index -1).
- **Implementer checklist:** replace-not-merge for the playthrough custom lists; included-before-own order; empty
  `PlayThroughs` means no loot.
- **Open:** how often a balance has both default and custom lists (not counted: AI balances are not in `Startup.upk`).

## AIPawnBalanceDefinition.GetPlayThroughIndex
- **Does:** if a game is running and a local player exists, takes that player's current playthrough index; a negative value
  returns -1; otherwise the smaller of that index and (number of `PlayThroughs` entries - 1). No game or player: -1.
- **Implementer checklist:** playthrough 1 of the game (index 0) is the first entry.
- **Open:** the exact player getter (read as the first local player controller's playthrough).

## MissionDefinition.GetItemRewardPools
- **Does:** clears the output array and copies the `RewardItemPools` array of `Reward` (when `bAltReward` is false) or
  `AlternativeReward`. Nothing else.
- **Slice:** `GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire` has `RewardData` with only
  `ExperienceRewardPercentage` and a `CreditRewardMultiplier` of 0 in the closure
  (`local/missions/rpg_fire/closure.json`); there is **no `RewardItems` and no `RewardItemPools`**, so this returns empty and
  the turn-in grants no item in stock data.

## MissionDefinition.GetItemRewardsForPlayer
- **Signature:** as in the table; the output struct is `PendingMissionRewardData` (`Mission`, `WeaponRewards[2]`,
  `ItemRewards[2]`, `bGrantAltReward`).
- **Reads:** the struct's `bGrantAltReward` (set by script before the call) to choose `Reward` or `AlternativeReward`;
  the player's `PlayerReplicationInfo` and its character identifier (a class id); the mission's game stage and awesome
  level through its balanced-actor interface; the global level cap.
- **Does (in order):**
  1. Returns without effect when the player controller is null, or has no replication info or character identifier.
  2. Rolls every `RewardItems` balance whose required player class is 0 or equals the player's class id, once each
     (quantity 1), at the mission's stage and awesome level with the **player controller as context** (the balance
     spawn path: always `bDropOnDeath` true).
  3. Rolls every `RewardItemPools` pool at the same stage and awesome level, context the player controller, **no variance
     formula**, outer chance 1.0, may-drop true.
  4. Walks the spawned inventories in spawn order. The **first two** become reward slots: a weapon's definition data is
     copied into `WeaponRewards[slot]`, an item's into `ItemRewards[slot]` (each slot holds one of the two; the slot counter
     advances for either). `SourceResponsibleName` is set from the mission. A third and later results are destroyed. Items of
     other classes are skipped. Every kept actor is destroyed after the copy (the reward is stored as definition data, not as
     a live item).
- **Calls into script:** the balanced-actor getters on the mission (`GetGameStage`, `GetAwesomeLevel`); inventory
  initialisation per roll.
- **Edge cases:** more than two results; classes other than weapon and item; the alternative reward chosen by the caller.
- **Implementer checklist:** this is where "how many choices" is decided: at most **two** reward entries in total, the first
  two spawned, balances before pools. The Fire mission produces none.
- **Open:** whether a third result is destroyed or leaked (read as destroyed); the telemetry call per kept item was not read.

## AMissionTracker.GrantMissionWeapon (pool-roll side only; C1 owns the tracker)
- **Reads:** the mission's `MissionWeapon` balance and `GameStageRegion`; the list of player controllers.
- **Does:** asks the mission's region for its game stage and awesome level (the region's own game-stage routine with the
  default flag off; the stage is fixed per player and playthrough by the first query, NATIVE_PROGRESSION.md), then for each
  player controller rolls the balance once with the balance spawn (quantity 1, stage, level cap, awesome, the controller as
  context). The first result becomes a marked mission weapon (mark value 2), is given to the player through the inventory's
  own `GiveTo` event, and the controller receives the script event `ShowMissionWeaponTraining`. The rolled item is a normal
  inventory spawn: its game stage and level are `min(region stage, cap)`.
- **Slice:** `MW_RockPaper_Fire` interpolates exp level (class default), so the lent pistol's level is the Sanctuary region
  stage (7 to 9 from the playthrough-1 band; the slice's 8). A mission weapon balance has no level requirement
  (NATIVE_WEAPON_RULES.md section 4).
- **Open:** the party of players it is given to beyond the single local player.

## WillowPawn.GetGameStageForSpawnedInventory / SetGameStageForSpawnedInventory
- **Does:** these accessors hold the *item game stage* a pawn rolls its loot at (field `GameStageForSpawnedInventory` on
  `WillowAIPawn`). The value is written by script at spawn (overview above). The accessors themselves are virtual calls that
  could not be resolved from the class tables; they are read as plain field reads and writes.
- **Open:** whether the base `WillowPawn` version returns its own `GameStage` for non-AI pawns (read as likely).

## BalanceModifierDefinition natives
- **GetAmmoDropsPerPlayerMultiplier(Context, GameStage):** returns 1.0 unless a game is running and the local player's
  playthrough index is 2 or more; then the last `BalanceModifiers` entry (searched from the end) whose `MinEffectiveLevel` is
  not above the stage supplies `AmmoDropsPerPlayerMultiplier` evaluated with the context. In the slice: 1.0.
- **GetUncommonChestItemPoolWeightMultiplier(GameStage):** for playthrough 0 and 1 reads
  `ChestItemPool_Weight_2_Uncommon_PT1_Multiplier` / `..._PT2_Multiplier` from the third playthrough's modifier object
  (evaluated without context); for 2 or more, the entry's `ChestItemPool_Weight_2_Uncommon_PT3_Multiplier`. A result
  with magnitude below 1e-8, or a missing object, is returned as 1.0. It only matters for a pool with
  `bEligibleForUncommonWeightMultiplier`; **no pool in `Startup.upk` sets that flag**, so it cannot affect the slice.
- **GetCommonGearDropWeightBaseValue:** same shape for `GearDrops_CommonWeightModifier_PT1/PT2/PT3_BaseValueOverride`. The
  call site that uses it was not found in the script read.
- **Open:** the stored values of the PT1 and PT2 overrides (they live in a package other than `Startup.upk`; the local
  OpenBLCMM jar needed to read the observed dump is absent on this machine).

## ItemPool.ToggleAllItemTypesDebug / IsAllItemTypesDebugEnabled
- **Does:** toggle and read the global "all item types" debug flag used by the grade selection (see candidate expansion). Off
  by default. Not part of the port.

## InteractiveObjectBalanceDefinition.SetupInteractiveObjectLoot (summary)
- **Does:** for a spawned chest, locker or similar, picks the grade's loot lists: the grade's included loot lists and its own
  `LootConfigurationData` entries; when the grade has neither, the balance's defaults (`DefaultIncludedLootLists`,
  `DefaultLoot`). It appends the entries to the object's loot array and returns whether there was any. The actual item rolls
  happen later in the object's `GetDroppedLoot` / `GetAttachedLoot`, which could not be resolved from the class tables and
  were not read. The objects use `DefaultLootGameStageVarianceFormula` as the pool variance formula (NATIVE_WEAPON_RULES.md
  section 4).
- **Open:** all of it beyond this summary.

## WillowItem.ChooseRandomParts (items other than weapons)
- **Does:** the per-slot choice for shields, grenade mods, class mods and artifacts has the same shape as the weapon's: a
  candidate list per slot from the "default" and "custom" part lists, then the same weighted draw. The entry weight rule
  that was read for items is the weapon rule of NATIVE_WEAPON_RULES.md section 3 (flat 100 when the entry has no manufacturer
  list; otherwise the manufacturer's weight index else the default; 0 outside the entry's stage window; an optional hook
  when the inventory definition sets `bAllowInventoryDefToModifyPartWeight`). Slot names differ per item type.
- **Open:** the item slot order, the "default" versus "custom" selection modes, item rarity, name parts and value.

## Not read yet
- `WillowInteractiveObject.GetDroppedLoot`, `GetAttachedLoot`, `Behavior_SpawnLoot` / `Behavior_SpawnItems` natives'
  surroundings (script passes the object's stage; they use `SpawnBalancedInventoryFromPool` without variance).
- `WillowAIPawn` / `WillowPawn` virtual accessors for the item game stage; the pawn `SetItemPoolList` setter.
- The third-playthrough level-cap branch in detail; `AWillowPlayerController.GrantChallengeCompletionItemRewards`,
  `GetPlayerLoot`, vending machine `GenerateInventory`, black market natives (`AddBlackMarketItem` family).
- Item rarity, name parts and value for non-weapons; ammo, money and health pickup inventory natives.
- The values behind `GetUncommonChestItemPoolWeightMultiplier` and `GetCommonGearDropWeightBaseValue`.
- `MissionDefinition.ShouldGrantAlternateReward` (compares per-objective counters with the objectives' thresholds; read only
  at overview level, lane C1's side) and `GetMissionRewardPresentation` (UI).
- `URecentDropList.Add` / `Contains`: the inventory manager's list of items the player has just dropped (used to avoid
  re-picking them up); unrelated to loot rolling.

## Corrections to earlier notes
Against `tools/loot_pools.py`, WEAPON_BALANCE_DECODE.md section 4 and NATIVE_WEAPON_RULES.md section 4 (not edited here):

1. **Empty nested pools take no share.** An entry whose `ItmPoolDefinition` has no entries is dropped from the candidate
   list (46 such entries in `Startup.upk`). The tool and the editor's `ProbabilityDisplayString` keep their share. At stage 8
   `Pool_GunsAndGear` lists `Pool_ClassMod_All` (10.63 % of the editor's display) and `Pool_ArtifactsAll` (4.83 %); at stage 8
   neither is reachable (the Common class-mod pool is empty and the other class-mod and artifact pools are gated by their
   loot-schedule or game-stage attributes). The tool keeps the class-mod branch's share as a branch that yields nothing, so
   its table sums to 0.888; in the game as read the firing always yields one item and the weapon share moves from 50.8 % to
   57.1 % (pistols 14.5 %, assault rifles, SMGs and shotguns 11.6 % each, snipers 8.0 %, shields and other `GD_ItemGrades`
   items 25.7 %, grenade mods 17.1 %, from my local cross-model `local/p2/G1/model.py`, which implements this note's rules
   over the same decoded data).
2. **Balance weights are not the entry weight.** The weight of a balance is its entry weight x the grade's spawn probability
   modifier (linearly interpolated across the grade's stage window) per manufacturer candidate, and a balance with no grade
   window covering the stage has weight 0. The tool treats every balance as weight 1 times the entry weight. Affected at
   stage 8: `ItemGrade_Currency_Money_Big` (about 0.535), the healing buff drinks (0.5 to 1 interpolation), and 20 grenade-mod
   balances with no candidate.
3. **`bDropOnDeath` gates real drops.** An item from a death roll whose effective flag (entry flag AND every outer entry's
   flag) is false is destroyed. `Pool_VehicleSkins_All` has no entry with the flag, so a standard enemy drops no vehicle
   skin (the tool lists them at 5 %). 497 of the 1,683 entries in death-roll pools have the flag false; the tool ignores it.
4. **Pools with `bAutoReadyItems` true are not part of the death roll.** They are rolled at spawn as the enemy's carried gear.
   (A death roll must test `bAutoReadyItems` false; the class default is true, so a pool without the key counts as true.)
5. **Playthrough custom lists replace, not extend, the defaults.** `tools/loot_pools.py` `source_lists` appends the first
   playthrough entry's custom lists to the defaults. The native keeps the custom included lists **instead of** the default
   included lists (when non-empty) and the custom pool list **instead of** the default pool list (when non-empty); an empty
   `PlayThroughs` array yields no list. Order: included lists first.
6. **Chance comparison and ammo scaling.** The per-pool chance fires when `FRand() <= chance` (inclusive); the chance is the
   attribute evaluated with the pawn's controller as context; `AmmoDrops_PerPlayer` pools are scaled once per death by
   `GetAmmoDropsPerPlayerMultiplier` (1.0 outside the third playthrough). The tool's `min(1, chance)` is right for chances
   above 1.
7. **Stage clamp (NATIVE_WEAPON_RULES.md section 4).** The note says the stage is "clamped into the pool's
   `MinGameStageRequirement` / `MaxGameStageRequirement` attributes when those are positive" after the variance. That clamp
   happens **only on the variance path** (variance formula supplied and `bSupportsGameStageVariance`). Without a variance
   formula (every enemy drop) the stage is unchanged; the min and max attributes still decide whether the pool is eligible.
8. **Stage and level cap.** Both spawn natives cap the stage used for grade windows, the stored game stage and the
   interpolated exp level at the global level cap (50 outside the third playthrough). NATIVE_WEAPON_RULES.md section 4's
   "level = the game stage" is right below the cap.
9. **Lent mission weapon (WEAPON_BALANCE_DECODE.md section 1, "how the game picks the level of a lent weapon").** Read: the
   mission's region stage (NATIVE_PROGRESSION.md) goes through the balance spawn with quantity 1; with the interpolating
   balance that stage is the item level.
10. **Turn-in loot.** In stock data the Fire mission has no `RewardItems` or `RewardItemPools`, so the turn-in grants no item;
    the labelled stand-in in SANCTUARY_RPG_MISSION.md is not a stock behaviour. The target dummy (`PawnBalance_TargetDummy`,
    no pools) drops nothing: the death roll of an enemy with an empty `ItemPoolList` does nothing. (AI pawn balances are not
    in `Startup.upk`; this restates the earlier decode.)
