# Loot on a kill, the spawn side: the drop, the pickup, InitializeInventory and the loot-spawn behaviors (draft, 2026-11-19)

AI-assisted (Claude), analyst lane FIX-POOL-SPAWN-A (decision 2026-11-19-23). Script first: the installed packages
read with `research/script_disasm.py`. The natives the script reaches were then read from the game executable with
Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock). Written in our own words: no decompiler output,
pseudo-code, addresses or listing structure is reproduced here. Field names come from the cooked packages
(`tools/ghidra/class_layout.py`). **Every rule is UNVERIFIED in the running game.** The last section lists the live
observations that would confirm them.

**Clean room.** This lane implements nothing. Everything here stays `NotYet` in Off (or is installed script the VM
already runs) until a fresh implementer codes the native rules from this note alone.

**What it extends.** NATIVE_LOOT.md reads the pool roll: `ItemPool.SpawnBalancedInventoryFromPool`, its candidate
expansion, and the death roll's choice of pools in `WillowPawn.DropLootOnDeath`. This note reads what happens to the
spawned inventories after the roll, and the loot-spawn behaviors NATIVE_LOOT.md left unread (decision 2026-11-16-41,
SLICE_RUN_PLAN.md section 3.4 lane 2).

## Summary

| Piece | Kind | On the slice's kill (link 5) | Confidence | Status |
|---|---|---|---|---|
| The bullymong's death behaviors | data | none spawns loot | high | UNVERIFIED |
| `WillowPawn.DropLootOnDeath`, the per-item part | script | yes | high | UNVERIFIED |
| `Weapon.DropFrom`, `WillowWeapon.DropFrom`, `WillowItem.DropFrom`, `WillowInventory.DropFrom`, `CreatePickup` | script | yes | high | UNVERIFIED |
| `WillowPickup.InitializeFromInventory` | script | yes | high | UNVERIFIED |
| `WillowPawn.GetItemTossVelocity`, `WillowAIPawn.GetItemTossVelocity` | native | yes (the base rule, for the bullymong) | high | UNVERIFIED |
| `WillowAIPawn.GetGameStageForSpawnedInventory` | native | yes | medium (field mapping) | UNVERIFIED |
| `InitializeInventory` on `WillowInventory`, `WillowWeapon`, `WillowItem` | script | yes, run by the pool native | high | UNVERIFIED |
| `WillowInventory.GenerateUniqueID`, `ComputeMonetaryValue`, `GetManufacturer` | native | yes, inside `InitializeInventory` | high | UNVERIFIED |
| `Behavior_SpawnLoot`, `Behavior_SpawnLootAroundPoint`, `Behavior_SpawnLootAtPoints`, `Behavior_SpawnItems`, `Behavior_DropItems` | script, no native | no (chests and other providers) | medium | UNVERIFIED |

## The kill on the slice: no loot behavior

The stock regular bullymong (`GD_PrimalBeast`, the `--combat-run` `regular` spawn) has one behavior provider, on its
character class. Its behaviors are `Behavior_ChangeRemoteBehaviorSequenceState`, `Behavior_CompareFloat`,
`Behavior_Conditional`, `Behavior_DamageSourceSwitch`, `Behavior_SpecialMove`, `Behavior_Transform` and
`Behavior_UpdateMissionObjective`, on the events `OnSpawned`, `OnTakeDamage` and `OnBehaviorSequenceEnabled`. None of
them spawns loot. The pawn and its population balance name no behavior. So link 5's loot comes only from the death
roll, `WillowPawn.DropLootOnDeath` (script), and the five loot-spawn behaviors below are not on the slice path.

## What DropLootOnDeath does with each spawned inventory

NATIVE_LOOT.md covers the pool choice. For each firing pool the script calls the pool native with the pool, the item
game stage, the awesome level and the dead pawn as context (no variance formula, default flags), and takes the spawned
list back. It then walks that list in order, before the next pool rolls:

1. An inventory whose `bDropOnDeath` is set, and which is either not a `WillowWeapon` or belongs to a pawn that died at
   or above the level's `KillZ`, is dropped:
   1. a telemetry record (`WillowLeviathanService.RecordItemSpawnedEventForAllPlayers`, not gameplay);
   2. its `ItemLocation` becomes 2 (the enum is not named in the package);
   3. its owner and `Instigator` become the dead pawn;
   4. it is dropped at the pawn's location with the pawn's item toss velocity (`DropFrom`; the physics and torque
      arguments are left at their defaults).
2. Every other inventory is destroyed.
3. The local array is emptied before the next pool.

The item game stage comes from `GetGameStageForSpawnedInventory()`, and the awesome level from `GetAwesomeLevel()`.

### WillowAIPawn.GetGameStageForSpawnedInventory

A plain read of the pawn's `GameStageForSpawnedInventory`. The setter and the population script write it
(NATIVE_LOOT.md). The layout tool does not place this class's fields exactly (the class has native-only fields the
tool cannot see), so the field the native reads is identified by its meaning and its neighbours' names, not by the
tool's layout (UNVERIFIED).

### GetItemTossVelocity(optional bool)

Two rules. `WillowAIPawn` overrides the `WillowPawn` one.

- **`WillowAIPawn`:**
  - The target is the nearest non-empty entry of the pawn's `ItemTossTargetActors`, by straight-line distance; the
    first of equals wins.
  - With no target, the `WillowPawn` rule below runs. The stock bullymong's `ItemTossTargetActors` is empty, so its
    drops take that rule.
  - With a target, the answer is a ballistic toss from the pawn's location to the target's location. The flight time
    `t` is the distance divided by `ItemTossSpeed` (the field is identified by meaning, as above; the bullymong's
    default is 1024). Gravity `g` is the pawn's `WorldInfo.WorldGravityZ`, or -500 when the pawn has no `WorldInfo`.
    - Horizontal velocity: the horizontal direction to the target times (horizontal distance / `t`).
    - Vertical velocity: ((target Z - pawn Z) - `g` × 0.5 × `t`²) / `t`.
    - With `t` below 0.0001, the answer is a fixed engine vector (read as zero: UNVERIFIED).
- **`WillowPawn`:**
  - The forward direction `F` is the unit vector of a rotation the pawn reports through one of its engine interfaces
    (read as its facing: UNVERIFIED). It is built with the engine's usual pitch and yaw conversion.
  - With the bool argument set: velocity = `F` × 200 + (0, 0, 150), with no randomness.
  - Without it, which is what `DropLootOnDeath` uses:
    1. a random direction `R`: three C-runtime `rand()` draws per try, the first for x, the second for y and the third
       for z, each mapped to [-1, 1] as rand × (1 / 32767) × 2 - 1. A try is kept when x² + y² + z² is at most 1 (a
       point on the sphere's surface is kept), otherwise all three are drawn again; the kept point is then normalised;
    2. velocity = `F` × (the pawn's `Velocity` · `F` + 200) + `R` × 50 + (0, 0, 150);
    3. a result longer than 400 is scaled down to length 400.
  - The draws come from the same C-runtime generator as the other native rolls: three numbers per try, for each item
    dropped (see "Draw order" below for where they fall).

## DropFrom and the pickup

What dropping an inventory does, as outcomes:

- **A weapon that cannot be thrown** (`CanThrow` false) is not dropped: nothing changes and the answer is None.
  Otherwise a weapon stops zooming (`WillowWeapon`), goes inactive, stops firing and detaches before the shared drop,
  and its `AIController` is cleared afterwards. An item (`WillowItem`) goes straight to the shared drop.
- **No pickup, no inventory.** The inventory is destroyed, and the answer is None, when a mission denies the pickup
  (`MissionDenyPickup`), when the owner may not drop it (`CanInventoryBeDroppedByOwner` false), when it has no
  `DroppedPickupClass`, or when the pickup actor cannot be spawned.
- **The pickup.** A `DroppedPickupClass` actor is spawned for the inventory through its `ISpawnActor.SpawnForMap`
  (the dropped-pickup variant, at the drop's start location; the remaining arguments were not all decoded:
  UNVERIFIED). Before that, the inventory leaves its instigator's inventory manager, and its network update flags are
  set.
- **The inventory afterwards.** It records the pickup as its `AdditionalQueryInterfaceSource`, is based on the pickup,
  and has no owner, no `Instigator` and no state.
- **The pickup's motion.** It gets physics `PHYS_Falling` and the drop's start velocity. When the drop forces rigid-body
  physics off, the pickup's `bUseRBPhysics` is cleared.
- **Torque, in precedence order:**
  1. torque passed with the drop: the pickup takes it, with `bTorqueTemporary` set;
  2. otherwise the inventory definition's `bTorque`: the pickup takes the definition's `Torque`, with
     `bTorqueTemporary` cleared;
  3. otherwise the pickup's `bTorque` is cleared.
- **Last,** the pickup is initialised from the inventory, with the instigator and pickup enabled, and the pickup is
  the answer.

What initialising the pickup does (`WillowPickup.InitializeFromInventory`):
- it takes the inventory's `RarityLevel` as its `InventoryRarityLevel`, and the inventory, instigator and the
  inventory's manufacturer;
- a `MissionItemDefinition` registers with the mission tracker (directive, objective and waypoint) and may disable
  pickup; ordinary loot skips this;
- it then sets its drop lifespan, marks the inventory associated, and sets pickupability as asked.

The pickup's rigid-body set-up, particles and lifespan are presentation and timers, not read further here.

## InitializeInventory (what a spawned inventory gets)

The pool native runs the script event `InitializeInventory` on each new inventory, passing the balance, the
manufacturer, the grade-or-level word and the context source (NATIVE_LOOT.md step 5.5). The installed script:

- **`WillowWeapon`**, in order:
  1. `DefinitionData.UniqueId` = `GenerateUniqueID()`, and `DefinitionData.GameStage` = the inventory's `GameStage`;
  2. with a balance: `WeaponTypeDefinition` is the balance's inventory definition for the given manufacturer and
     grade (`GetInventoryDefinitionForManufacturerGrade`); `BalanceDefinition`, `ManufacturerDefinition` and
     `ManufacturerGradeIndex` are set from the arguments; `ExpLevel` is the balance's level for that manufacturer and
     grade (`GetExpLevelFromManufacturerData`, NATIVE_LOOT.md);
  3. the `WillowInventory` part (below);
  4. with a `WeaponTypeDefinition`: `bSelectRandomPartsOnInitialization` is set and `InitializeInternal()` runs. That
     is the weapon build: base values, ammo, part list collection, `ChooseRandomParts` (NATIVE_ITEM_RULES.md,
     NATIVE_WEAPON_RULES.md), part values, attribute slots and effects, name parts, the item card, and the firing
     mode's explosion data.
- **`WillowItem`**: nothing without an `InventoryBalanceDefinition`.
  1. `UniqueId` and `GameStage` as for weapons;
  2. `ItemDefinition` = the balance's definition for the manufacturer and grade; nothing more without one;
  3. `BalanceDefinition`; `ManufacturerDefinition` is the definition's override for the given manufacturer, or that
     manufacturer (`GetManufacturerOverrideOrDefault`);
     `ManufacturerGradeIndex`; `ExpLevel` as for weapons;
  4. the `WillowInventory` part;
  5. `bSelectRandomPartsOnInitialization` and `InitializeInternal()` (the item version of the build above).
- **`WillowInventory`** (both call it):
  1. `AdditionalQueryInterfaceSource` = the context source;
  2. `MonetaryValue` = `ComputeMonetaryValue()`;
  3. `ClientInitializeInventoryFromDefinition()`, which copies the card frames from the definition's presentation.

  It runs **before** the parts are chosen.

### The natives inside it

- **`GenerateUniqueID()`**: two C-runtime `rand()` draws, `first | (second << 16)`. An answer of 0 becomes 1.
  `rand()` gives 0 to 32767, so bits 15 and 31 are always clear. These are two more draws from the shared generator
  per inventory, taken before the parts roll.
- **`ComputeMonetaryValue()`** (one body for weapons and items):
  1. 0 without an inventory definition;
  2. otherwise the definition's `MonetaryValue` (an attribute initialization value, evaluated by the engine's
     initialization evaluator with the inventory itself as context: NATIVE_INVENTORY_VALUE.md section 3), truncated toward zero to an
     integer;
  3. then capped by the definition's `FormOfCurrency`: `CURRENCY_Credits` at 99,999,999, `CURRENCY_Eridium` at 500,
     `CURRENCY_SeraphCrystals` and the ten reserved values at 999, and a value above the enum's last entry gives 0. A
     negative result is not raised.
- **`GetManufacturer()`**: the inventory's `DefinitionData.ManufacturerDefinition` (weapon or item data), as it is.

## The loot-spawn behaviors (not on the slice's kill)

All five are script classes with no native. The kernel runs their `ApplyBehaviorToContext` as installed script:

- **`Behavior_SpawnLoot.ApplyBehaviorToContext`**:
  1. does nothing on a network client (`NetMode` 3), or when the context object is not an `IBalancedActor`;
  2. for each of its `ItemPools`, calls the pool native with the pool, the context's game stage and awesome level, and
     the context object as context (no variance formula, default flags), and takes the spawned list back;
  3. wraps each item in a `SpawnedDroppedLootData` (with the behavior's `bDisableRigidBodyPhysics`) and hands the
     batch, with the context object, to its `PlaceSpawnedItems`;
  4. with `bIncludeLootFromLootableContext`, the context's `ILootable.GetDroppedLoot` items are placed the same way.
- **`Behavior_SpawnLootAroundPoint.PlaceSpawnedItems`**:
  1. the centre is the behavior's attachment location, or the context object's location when that is zero; the
     orientation comes from the attachment data or the context;
  2. each item is scattered: a yaw of `Rand(32768)` × 2, a point `CircularScatterRadius` out along it from the centre,
     and a launch velocity of `SpawnVelocity` turned by one of four rules (`SpawnVelocityRelativeTo`);
  3. then its `ItemLocation` becomes 2 and it is dropped at the scatter point with the scatter velocity, passing on
     the behavior's rigid-body and torque settings. An item whose scatter fails is destroyed.
- **`Behavior_SpawnLootAtPoints`** attaches or drops each item at the context's spawn points in turn.
- **`Behavior_SpawnItems`** and **`Behavior_DropItems`** were read only as far as the natives they call: pool spawns at
  a world-body attachment location, and a drop of the context's own items at its world-body location and rotation.

Their placement details were read only to this level.

**In `ow-behavior` terms.** These behaviors need no handler of their own: their rule is their installed script. The
kernel has to call the behavior object's `ApplyBehaviorToContext` through the VM, inside `kernel::natives::collect` so
the output links are gathered, with the resolved context object and the consumer. The consumer is what
`ow_behavior::registry::HandlerFn` does not receive today (decision 2026-11-15-30). That is why `ow-combat::loot`
leaves the five classes unregistered, and the kernel's unregistered-class default (a `NotYet` of kind `NotPorted`) stops
their branch. Nothing in them is a native rule to port, beyond the pool native, `DropFrom`'s natives and those above.

## For an implementer, in the port's terms

- The slice's kill needs:
  - the pool native in Off (NATIVE_LOOT.md, and the contract items in decision 2026-11-16-41);
  - `GetGameStageForSpawnedInventory`, `GetItemTossVelocity` (the `WillowPawn` rule for the bullymong) and the three
    `WillowInventory` natives above;
  - `ISpawnActor.SpawnForMap` for the pickup (NATIVE_INDEX.md lists its rows).

  The rest is installed script.
- **Draw order** on the shared C-runtime generator, per pool: for each pool the death roll considers (those with
  `bAutoReadyItems` false), in list order, first the `FRand` that decides whether it fires (drawn whether or not it
  does); then, for a firing pool, that pool's roll, with each new inventory's two `GenerateUniqueID` draws and its
  parts roll inside the pool native's loop; then the toss draws of each item dropped from that pool, in list order;
  then the next pool. Whether `ow_core::rand`'s streams should share one generator here is the port's choice. The
  game's order is as above (UNVERIFIED).
- Where the pool native places the new inventory actor is still not read (NATIVE_LOOT.md "Spawning one inventory",
  and the UNVERIFIED choice in `ow_inventory::spawn`). On the kill path it does not decide where loot lies: `DropFrom`
  puts the pickup at the pawn's location with the toss velocity.

## Open questions

1. The `ItemLocation` value 2's name (the property declares no enum in the package).
2. The context the `MonetaryValue` evaluation is given. Answered by NATIVE_INVENTORY_VALUE.md section 3: the inventory
   itself.
3. Which rotation the `WillowPawn` toss uses (facing, view or controller), and the vector returned for a toss time
   under 0.0001.
4. `WillowPickup.InitializeDropLifespan`, `SetPickupability` and `InitializePickupForRBPhysics`: lifespans and
   physics, not read.
5. Where the pool native spawns the inventory actor, and by whom.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Kill a bullymong, hook `DropFrom` and `GetItemTossVelocity`, and record:
  - the velocity's length (at most 400) and its Z;
  - the pickup's start location (the pawn's);
  - which inventories were destroyed instead of dropped.
- Read a dropped item's `UniqueId` (never 0, bits 15 and 31 clear), its `MonetaryValue` against its definition's, and
  its pickup's `Manufacturer` and `InventoryRarityLevel`.
- Read `GameStageForSpawnedInventory` on a spawned bullymong, against the stage its drops carry.
- Open a chest with `Behavior_SpawnLootAroundPoint` and record each item's start point (on the scatter circle) and
  velocity.
