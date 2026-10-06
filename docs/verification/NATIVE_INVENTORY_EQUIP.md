# Native inventory, pickup and equip: loot, backpack, weapon slots, mission weapon (2026-10-05)

AI-assisted (Claude), analyst lane G12. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script behaviour was read in the local WillowGame and Engine
listings (`research/script_disasm.py`; the VM already runs all of it), script signatures come from the package
declarations, field names from `tools/ghidra/class_layout.py`, data values from `ow-package --properties`. Related notes:
[NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) (lane G8: the use key, `IUsable`; pickups are excluded from that
walk), [NATIVE_LOOT.md](NATIVE_LOOT.md) (lane G1: what a roll produces, the lent pistol's roll),
[NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) (lane C1: when the tracker grants and removes the mission
weapon), [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) (lane G4: currency, the cost query, `GetPawnInventoryManager`),
[NATIVE_INVENTORY_SORT.md](NATIVE_INVENTORY_SORT.md) (backpack list order), [NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md)
(item attribute effects), [INVENTORY_MOVIE_PROTOTYPE.md](INVENTORY_MOVIE_PROTOTYPE.md) (the menu). The GFx side belongs to lane G5,
the behavior kernel to G2, damage to G11.

## The slice answer in one page

Almost all of "pick up, keep, equip" is **script the VM already runs** (WillowPlayerController, WillowInventoryManager,
WillowPickup, Engine.DroppedPickup/Inventory/WillowInventory, WillowEquipAbleItem, WillowPawn). The natives are thin:
counters and limits on the inventory manager, three "fire this behavior event" natives per item type, the cost pair for pickups,
two predicates, the mission-weapon helpers on the tracker, and the pickup interface accessors. What a clean-room host must
therefore get right is the **script contract around them** (section "Script flow"), plus the following native facts:

1. The backpack is a **counter plus a list**: `CountUnreadiedInventory` returns a stored count that script increments and decrements;
   `GetUnreadiedInventoryMaxSize` returns a stored limit whose default is **12** and which `SetInventoryMaxSize` clamps to at
   least 12 unless told not to.
2. Weapon slots are the `WeaponReadyMax` attribute: a stored in-place value, read as is by `GetWeaponReadyMax` (the optional
   "want base value" flag is accepted and ignored by this native). `CountReadiedWeapons` counts weapons whose readied flag is set
   in the inventory chain. A weapon goes into the readied set only while `CountReadiedWeapons < GetWeaponReadyMax`.
3. Four quick slots (Up, Down, Left, Right) are the weapon field `QuickSelectSlot` 1..4 (0 = not slotted). Slots above
   `WeaponReadyMax` are not assignable.
4. Picking up gear is done by the **pickup key** (script `PickupSomething`), not by the use key; money, ammo and similar items are
   picked up **on touch** when their definition says `bAutomaticallyPickup`.
5. The mission weapon is a normal weapon with a `MissionWeaponBalanceDefinition` balance: it cannot be dropped, sold, stored or saved,
   has no level requirement, is granted by the tracker (C1/G1) and removed by a script event the tracker fires at the player's
   inventory manager.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| WillowInventoryManager.CountUnreadiedInventory | `native final function int CountUnreadiedInventory()` | backpack-full test, UI count | high | UNVERIFIED |
| WillowInventoryManager.GetUnreadiedInventoryMaxSize | `native final function int GetUnreadiedInventoryMaxSize()` | backpack-full test, save | high | UNVERIFIED |
| WillowInventoryManager.SetInventoryMaxSize | `native final function SetInventoryMaxSize(int NewSize, optional bool bOverrideDefaultMin)` | save load, SDU | high | UNVERIFIED |
| WillowInventoryManager.CountReadiedWeapons | `native final function int CountReadiedWeapons()` | slot-full test | high | UNVERIFIED |
| WillowInventoryManager.GetWeaponReadyMax | `native final function int GetWeaponReadyMax(optional bool bWantBaseValue)` | slot count | high (reads), medium (SDU wiring) | UNVERIFIED |
| WillowInventoryManager.FindLeastValuableWeapon / FindLeastValuableItem | `native final function WillowWeapon FindLeastValuableWeapon(optional bool bIncludeUnreadied, optional bool bIncludeReadied)` | auto-slot of a new weapon | medium-high | UNVERIFIED |
| WillowInventoryManager.ItemActors | `native final iterator function ItemActors(class<Inventory> BaseClass, out Inventory Inv, optional bool bOnlyReadied)` | item iteration | medium | UNVERIFIED |
| WillowInventoryManager.FindBestHolsteredWeapon / ReplaceHolsteredWeapon / SetHolsteredWeapon | `native function WillowWeapon FindBestHolsteredWeapon(optional byte SizeFilter)` etc. | body holsters (visual only) | medium | UNVERIFIED |
| WillowPlayerController.CanAffordToPickUpPickupable / PayForPickupable | `native function bool CanAffordToPickUpPickupable(IPickupable Pickupable)` / `native function PayForPickupable(IPickupable Pickupable)` | only for priced pickups; not Fire | medium-high | UNVERIFIED |
| WillowPlayerController.CanHoldWeapon (Controller virtual) | `native function bool CanHoldWeapon(Pawn Holder, Weapon TestWeapon, bool bHoldInOffHand)` | weapon switch / ready gate | medium-low | UNVERIFIED |
| WillowPlayerController.ConditionalFixWeaponReadyMax | `native function ConditionalFixWeaponReadyMax(PlayerSaveGame SaveGame)` | save load only | medium | UNVERIFIED |
| WillowPlayerController.UpdateAmmoCounts | `native function UpdateAmmoCounts(bool bSilent)` | HUD ammo refresh after pickup | low | UNVERIFIED |
| DroppedPickup / WillowPickup IPickupable accessors (GetPickupableInventory, GetPickupableInventoryDefinition, Pickupable_IsEnabled, IsDiscovered, MarkAsDiscovered, TouchPickupTrace) | `native function ... ` on the pickup | every pickup | medium | UNVERIFIED |
| WillowInventory.GetInventorySpaceRequirement | `native final function int GetInventorySpaceRequirement()` | backpack-full test | medium-high | UNVERIFIED |
| WillowEquipAbleItem.OnEquipped / OnUnequipped, WillowWeapon.OnEquip / OnUnequip, WillowItem.OnPickupAssociated, UsableItemDefinition.OnUsed, EquipableItemDefinition.OnEquipped / OnUnequipped, WeaponTypeDefinition.OnEquip / OnUnequip, ItemDefinition.OnPickupAssociated | `native function OnEquipped(Object EventInstigator, optional out array<BehaviorProviderDefinition> Providers)` and the definition forms `(out BehaviorConsumerHandle, Object Instigator)` | equip events of weapons and gear | high (what), medium (payload) | UNVERIFIED |
| WillowItem.IsEquipped / WillowEquipAbleItem.IsEquipped | `native function bool IsEquipped()` | UI marker | low | UNVERIFIED |
| WillowPawn.ShouldAutoReadyMissionWeapon | `native function bool ShouldAutoReadyMissionWeapon()` | mission weapon ready | medium | UNVERIFIED |
| WillowWeapon.IsMissionWeapon | `native function bool IsMissionWeapon()` | sort header, drop rules | medium | UNVERIFIED |
| MissionTracker.IsValidMissionWeapon | `native function bool IsValidMissionWeapon(MissionWeaponBalanceDefinition MissionWeaponBalanceDef)` | ready-from-backpack gate | high | UNVERIFIED |
| MissionTracker.GrantMissionWeaponsToClientPlayer | `native function GrantMissionWeaponsToClientPlayer(WillowPlayerController WillowPC)` | re-grant on load | medium | UNVERIFIED |
| AMissionTracker removal helper (no script name; fires `RemoveMissionWeapons`) | none | Fire: take the lent pistol back | medium | UNVERIFIED |

Natives belonging to other lanes that the equip path touches: `WillowInventory.ApplyExternalSlotEffectModifiers` /
`ApplyInternalSlotEffectModifiers` (attribute slots, see "Not read yet"), `WillowInventory.GetInventoryDefinition`/`GetExpLevel*`
(trivial field reads, loot lane), `WillowPlayerController.GetPawnInventoryManager` / `GetInventoryPawn` (G4).

## Script flow (what the VM already runs; read first, in our own words)

### Finding and taking a pickup

* **Candidates.** A `WillowPickup` is a world actor with a cylinder collision component. Stock UE3 touch events decide candidacy,
  not a camera ray: when a pawn that may pick up inventory (`bCanPickupInventory`, on foot or a vehicle occupant who can) touches
  the pickup, the pickup's `Touch` asks `ValidTouch` (pawn allowed, not driving, a short trace from the pickup to the pawn must
  not be blocked, and the game's `PickupQuery` must allow it) and, if so, tells the pawn's controller `TouchedPickupable`. The
  controller stores it as `CurrentTouchedPickupable` and clears it on `UnTouchedPickupable`. A pickup that is touching but
  whose touch was refused is re-checked every 0.5 s by timer.
* **Touch radius.** For pickups of automatically picked-up inventory (and for mission items and mission-director pickups) the
  pickup's collision cylinder radius is set to `GlobalsDefinition.PlayerInteractionDistance` (class default **250**; the Globals
  instance sets **350**, so 350 in the shipped game); a manual-pickup pickup keeps its mesh-sized collision. `PickupRadius`
  (class default **200**) is the gather radius for bulk pickup below.
* **Seen pickup.** A second candidate, `CurrentSeenPickupable`, is set by the script events `SawPickupable` / `ClearSeenPickupable`
  that the native HUD/targeting code raises (the raiser is a one-line native that fires the event with the pickup; who decides
  "seen" was not read, see Open). On the server it is only accepted when the pickup has an inventory and a definition.
* **Which one is current.** `GetCurrentPickupable`: nothing touched means nothing is current. If the touched pickup's definition has
  `bAutomaticallyPickup`, the touched one. Otherwise the seen one if there is one, else the touched one.
* **Auto-pickup.** `TouchedPickupable` ends by calling `PickupSomething(False)` when the definition's
  `ShouldPlayerAutomaticallyPickup(controller)` is true (default: the `bAutomaticallyPickup` flag; a `MissionItemDefinition` with a
  mission directive answers true when the player is not in a vehicle).
* **Manual pickup.** The pickup key is the exec `PickupSomething` (binding not traced). `WillowPlayerController.Use` does nothing
  while a pickupable is current (G8), so the one E key takes the pickup first.
* **`PickupSomething`, effects in order.** (1) take the current pickupable and the pawn that will own it; nothing happens unless
  both exist and the pickup's own `DenyPickupAttempt` (a mission-directive pickup opens its mission UI and denies) allows it;
  (2) `CanAffordToPickUpPickupable` (native, below); not affordable: `NotifyUnableToAffordPickupable`, stop; (3) decide
  `bReadyAfterPickup`: kept as passed, but `AllowReadyAfterPickup` forces it false when the picked item is a weapon and the pawn's
  action skill is the Buzzaxe skill; (4) `HasRoomInInventoryFor` true: send `ServerPickupSomething(bReady)` to the authority and
  stop; (5) no room: if the item may be used by the pawn and bReady was set: a weapon swaps with the pawn's current weapon
  (`ServerThrowPawnActiveWeapon` when the active weapon `CanThrow`, then `ServerPickupSomething`), a gear item swaps with what sits in
  its equipment location (`ServerConditionalThrowPawnEquippedItem`, then `ServerPickupSomething`); (6) otherwise, with no swap
  possible: `ClientDisplayPickupFailedMessage` (HUD message "FULL") and, if the item needs space (`GetInventorySpaceRequirement` > 0),
  the stat `STAT_PLAYER_PICKUP_FAILS_FULL` is incremented on the authority; (7) after a swap or a plain pickup,
  `PayForPickupable` (native, below).
* **`PickupPickupable` (authority), effects in order.** gathers, when the definition `CanPickupInBulk` and the pickup is a plain
  pickupable, every other enabled pickup of the same kind within `PickupRadius` of it (the list is capped at 4 total); refreshes
  ammo counts; for each pickupable in turn: affordability, `bReady` kept only if the item `CanBeReadiedOnPickup(pawn)`
  (default true; a usable item answers true only when `IsItemAutoUsedBy`), room check, the game's `PickupQuery`; then
  `ClientSpawnPickupableMesh` (the flying-into-the-pawn mesh), for a weapon that will be readied and is usable the pawn's
  `bGrabWeaponPickup` is set (drives the grab animation and the rare-weapon dialog), then either `CloneAndGiveToCoopPawns` (usable
  items flagged co-op range and used on pickup: every local co-op pawn gets a copy) or `GiveTo(pawn, bReady)` on the pickup. The
  pickup is then cleared from the touched and seen slots and every player controller is told `UnTouchedPickupable`. A failed room
  or query check shows the full message and calls the pickup's `FailedPickup` (a little random impulse so it does not sit still).
  Ends with `UpdateAmmoCounts`.
* **`DroppedPickup.GiveTo(pawn, bReady)`:** announces the pickup to the pawn (`AnnouncePickup` -> `HandlePickup`; weapons also
  `UpdateStatsOnWeaponPickup`; `WillowPlayerController.ServerAnnounce*Pickup` raises the on-screen message when the item's
  `RarityLevel` >= `GlobalsDefinition.AnnouncePickupRarityThreshold` (default **2**) and the definition does not set
  `bNeverDisplayPickupMessage`), then `Inventory.GiveTo`, clears its own inventory pointer and calls `PickedUpBy` (destroys the
  pickup actor). `WillowPickup.GiveTo` first detaches it from a vending machine it was based on and removes waypoint components.
* **`Inventory.GiveTo(other, bReady, bPlayPickupSound)`:** `ClientConditionalIncrementPickupStats` then
  `InvManager.AddInventory(item, bReady, bDoNotActivate, bPlaySound)`.

### `WillowInventoryManager.AddInventory(item, bReady, bDoNotActivate, bPlayPickupSound)` (authority, script)

Returns false for no owner pawn, a non-WillowInventory, or an item the pawn cannot use that is not auto-used.

* **Gear (WillowItem), effects in order:** if an identical definition (the same item definition) is already carried and the definition
  has `bDuplicatePickupJustAddsQuantity`, the new actor adds its quantity to the old one, plays the pickup sound and is destroyed
  (done). Otherwise `bReady` becomes true when `InventoryShouldBeReadiedWhenEquipped(item)`: the pawn exists and is not a vehicle, the
  pawn can use the item, and for gear the equipment-location slot on the pawn is **empty** (a filled slot keeps the backpack choice).
  A gear item not readied goes to the backpack (`AddBackpackInventory`; telemetry event "to backpack"). A readied gear item is given
  to the pawn (`GivenTo`, which calls `Ready`, which calls `Readied`, which calls `WillowPawn.EquipItem`), the manager notes a
  grenade mod (`GiveGrenadeToPlayerIfGrenadeMod`: a grenade mod carrying a stored grenade gives one grenade to the grenade ammo pool
  and clears the flag), and `WillowInventoryManager.OnEquipped` runs the matched-set stats.
* **Weapons:** `bReady` becomes true when `InventoryShouldBeReadiedWhenEquipped`: the pawn can use it **and**
  `CountReadiedWeapons < GetWeaponReadyMax`. Not readied: `AddBackpackInventory`. Readied: `Engine.InventoryManager.AddInventory`
  (the chain add + `GivenTo` + `Ready`), and, when `bAutoSwitchWeaponOnPickup` is set, `bDoNotActivate` is false and either the pawn
  holds nothing or the held weapon has a lower `Priority` than the new one, the new weapon is made current (`ClientWeaponSet`);
  failing that, with no weapon held and the owner's menu closed, `SwitchToBestWeapon` runs.
* **Stats:** the definition's `PickedUpStatID` stat is incremented on the owner's `PlayerStats`; if readied the definition's
  `GetEquippedStat` stat is incremented and the mission tracker `EvaluateStat` is called with it (this is what Kismet/mission
  stat objectives hear). `ServerIncrementWeaponPickedUpStats` bumps weapons-picked-up, tech weapons, and the rarity stat.
* **Backpack storage is server-light:** `InternalAddBackpackInventory` hands any `StoredAmmo` of a weapon to the ammo pool, sends the
  definition data to the owning client (`ClientAddWeaponToBackpack` / `ClientAddItemToBackpack`, which spawn a transient backpack actor
  on the client, call `AddInventoryToBackpack` and set the `Mark`), destroys the server actor and increments
  `BackpackInventoryCount`. `RemoveInventoryFromBackpack` decrements by re-reading the array length. The host may keep a single list.
* **`AddBackpackWeaponFromDefinitionData` / `AddBackpackItemFromDefinitionData`** (local controller only, used by rewards and
  the mission weapon grant path): spawn the actor from definition data, initialise it with quantity 1, run the pickup stats, test
  `InventoryShouldBeReadiedWhenEquipped`, hand weapon `StoredAmmo` (from the weapon type's `StartingAmmoCount`) to the ammo pool,
  destroy the actor and call the client add with Mark 3 (a "new" mark) and the ready flag; increment the count.

### Readying, slots and unreadying

* **`ReadyBackpackInventory(inv, weaponSlot)`:** finds the item in the backpack. Weapon: `SendSlottedThingToBackpack(slot)` first (the
  weapon currently in that quick slot, if any, is removed from the manager and goes back into the backpack), remembers the incoming
  one in `BackpackInventoryBeingEquipped`, then `ServerReadyWeaponFromBackpack(definitionData, slot, mark)`. Gear: the same with
  the equipment location (`SendEquipLocThingToBackpack`), `ServerReadyItemFromBackpack`. Then the client decrements the backpack
  entry's quantity and removes it at zero.
* **`ServerReadyWeaponFromBackpack`:** refuses if a trade is pending for that data. A weapon whose balance is a
  `MissionWeaponBalanceDefinition` is only accepted when `MissionTracker.IsValidMissionWeapon` says so (i.e. the mission is still
  active; otherwise nothing is equipped). Spawns the weapon, sets `StoredAmmo` to 0, sets `PendingQuickSlot` to the chosen slot,
  calls `AddInventory(weapon, True, True)` (ready, do not auto-activate), sets the mark, decrements `BackpackInventoryCount`.
  `InventoryReadied` then assigns the quick slot: `PendingQuickSlot` (if non-zero, `SafelySetQuickSlot`, which evicts any other
  weapon holding that slot) or, when zero, `AssignNextAvailableDefaultQuickSlot`.
* **`AssignNextAvailableDefaultQuickSlot(weapon)`:** if `bLimitedInventory` and the readied count already exceeds the maximum, an
  existing weapon is unreadied first (the pending weapon, else the current weapon, else `FindLeastValuableWeapon(False, True)`, the
  cheapest readied weapon). Then it scans the readied weapons for used slots and gives the new weapon the **first free slot of 1..4
  in the order 1, 2, 3, 4, but slots 3 and 4 only while `WeaponReadyMax` >= 3 / >= 4**. Telemetry follows.
* **Unreadying** (`Inventory.Unready(bPlaceInBackpack=True)` -> `InventoryUnreadied`): the item goes back into the backpack
  list (`AddBackpackInventory`) unless told otherwise; `Unreadied` resets `QuickSelectSlot` to 0; gear `Unreadied` calls
  `WillowPawn.UnequipItem`. `SwitchQuickSlot` swaps the slots of two weapons.
* **Removal from the manager** (`RemoveFromInventory`): a held weapon gets `OnUnequip`, the manager switches to the best weapon, and
  a holstered weapon is replaced on the body.
* **Equipment location values:** Shield 0, GrenadeMod 1, ClassMod 2, Artifact (relic) 3, "none" 4 (the default for items that
  cannot be equipped). `WillowPawn.EquippedItems[0..3]` holds the four gear slots.
* **`WillowPawn.EquipItem(item)`:** nothing for location 4; if a different item is in that slot it is unreadied (goes to the
  backpack) first; the slot and `EquippedItemDefs` are set; `item.ItemEquipped()`; gear likenesses are refreshed.
  `ItemEquipped`: applies the item's external attribute effects, activates its attribute-slot skill, plays the definition's
  equip sound (player-owned only), then the native `OnEquipped`. `ItemUnequipped` is the mirror (`OnUnequipped`).
* **What "equip" does to attributes (script):** `WillowItem.ApplyAllExternalAttributeEffects` applies the definition's and each
  part's `ExternalAttributeEffects` to the **owning controller** as the context, then calls the native
  `ApplyExternalSlotEffectModifiers` for attribute slots (class mods' skill bonuses). The item's own (internal) effects are applied
  at initialisation, not on equip. A weapon applies `ApplyAllExternalAttributeEffects` and activates its slot skill when it enters
  the `WeaponEquipping` state, and removes them in `UninitializeAfterPutDown` / `ItemRemovedFromInvManager`; `WeaponEquipped` fires the
  native `OnEquip` and, for the first-time rare weapon on the local player, a dialog event.

### Eligibility and droppability (script, Engine.WillowInventory)

* **`CanBeUsedBy(pawn)`:** the item's DLC requirement; the definition's `OnUseConstraints` (an attribute-expression evaluation with
  `OnUseConstraintsMode`); then, for a human-controlled pawn and a definition with `bUsesPlayerLevelRequirement`, the level
  rule below. Failure bits are recorded in `LastCanBeUsedByResult` (1 = level, 2 = constraints, 4 = DLC; bit 1<<0, 1<<1, 1<<2 respectively
  as the script reads them, translated by `TranslateUseFailure`). Weapons and equippable gear also refuse when the controller
  is using a vehicle; gear additionally honours `IsPlayerRestricted`.
* **Level rule:** required level = max(item `ExpLevel` - floor(EvaluateInitializationData(`PlayerUseLevelBonus`)), 1); the pawn's
  `GetExpLevelForEquip` must be >= required (a required level of 1 or less always passes). A **mission-weapon balance returns
  required level 0**, so the lent pistol is always usable.
* **Droppability:** the definition carries `PlayerDroppability` (seen: `EPD_CannotDropOrSell` on the weapon-slot SDU). Two script
  predicates, `CanInventoryBeDroppedByOwner` and `CanInventoryBeSoldOrStoredByOwner`, combine that value with the controller's
  `CanDrop(item)`; the exact truth table was read only roughly (UNVERIFIED). **A weapon with a mission weapon balance answers false to both
  and is not saved** (`CanBeSaved` false); this one is certain from script.
* **Filter levels:** `GetWeaponList` / `GetItemList` take a `MaxDroppability` filter (2 = no check, 1 = must pass the sell/store
  predicate, 0 = must pass the drop predicate) and skip items failing it (the default is 2; the weapon save call also passes 2 and relies on `CanBeSaved` per weapon, so mission weapons
  are left out of a save by that check, not by the filter).

## Native sections

## WillowInventoryManager.CountUnreadiedInventory / GetUnreadiedInventoryMaxSize / SetInventoryMaxSize
- **Signature:** as in the table.
- **Reads:** manager fields `BackpackInventoryCount` (the stored count, not the length of `Backpack`) and `InventorySlotMax_Misc` (the
  stored limit).
- **Does:** `CountUnreadiedInventory` and `GetUnreadiedInventoryMaxSize` return those fields unchanged. `SetInventoryMaxSize(new,
  bOverrideDefaultMin)` first runs the common native-call epilogue, then, when `bOverrideDefaultMin` is false (the default) and
  `new` < **12**, stores 12; otherwise stores `new`. Class default `InventorySlotMax_Misc` = **12** (read from
  `Default__WillowInventoryManager`), `bLimitedInventory` default true.
- **Calls into script:** none.
- **Constants / formulas:** minimum 12. Room for a pickup: `bLimitedInventory && space > 0` requires `count + space <= max` (script).
- **Edge cases:** count and list can disagree on a client until `UpdateBackpackInventoryCount` replicates the length; on the
  authority the count is recomputed from the array in `AddInventoryToBackpack` and `RemoveInventoryFromBackpack` and incremented or
  decremented by the add/ready paths in between.
- **Implementer checklist:** keep one integer count and one limit; load `SetInventoryMaxSize(saved)` clamped to >= 12;
  full means `count + spaceRequirement > max`; do not recompute the count from visible rows.
- **Open:** how SDU upgrades raise the limit (they are item `ExternalAttributeEffects` on the SDU usable items against the
  `InventorySlotMax_Misc` attribute; arrays of that kind are not decodable by `ow-package`). Black Market backpack upgrades use the constant `Att_BackPackSlotsPerUpgrade` = **3** per upgrade (decoded constant; the
  wiring is the undecoded behavior effect).

## WillowInventoryManager.CountReadiedWeapons / GetWeaponReadyMax
- **Reads:** the manager's `InventoryChain` (the engine's linked list of held inventory), each weapon's `bReadied`; the
  `WeaponReadyMax` attribute value.
- **Does:** `CountReadiedWeapons` walks the chain and counts the `WillowWeapon` entries whose readied flag is set; a chain that
  loops back to itself yields -1. `GetWeaponReadyMax` returns the stored attribute value, ignoring `bWantBaseValue`.
- **Script setter:** `SetWeaponReadyMax(n)` stores `max(n, class default)` as the base and then `UnreadyExcessWeapons` unreadies
  readied weapons (never the one in hand) from the end of the list until the count fits. AI pawns get 4 in `SetupFor`.
- **Constants:** the class default of the attribute is not decodable (attribute properties are unsupported). Save fix-up rule below
  gives 2 as the floor for players.
- **Implementer checklist:** keep `WeaponReadyMax` (default 2 for a player, UNVERIFIED), count readied weapons from the weapon
  list, never exceed 4, unready excess weapons when it shrinks.

## WillowPlayerController.ConditionalFixWeaponReadyMax (save load only)
- **Does:** repairs old saves. Starting from 2, +1 for each completed (status 4) plot-critical, non-DLC mission whose `MissionNumber`
  is 5 or 10, capped at 4, then raises the save's `InventorySlotData.WeaponReadyMax` to at least that. `ApplyInventorySlotSaveGameData`
  then feeds the saved value to `SetInventoryMaxSize` and `SetWeaponReadyMax`.
- **Open:** `MissionNumber` is not stored in the decoded mission properties (the episode missions' tagged data lack it), so which
  missions are numbers 5 and 10 is not confirmed; the file names suggest `M_Ep5_ThePhoenix` ("Hunting the Firehawk") and
  `M_Ep10_BirdISTheWord` ("Wildlife Preservation") only if the number follows the episode number. A weapon-slot SDU item pool
  (`GD_Itempools.sdu.Pool_SDU_EquipSlot`) and `INV_SDU_WeaponEquipSlot` (auto-picked-up, bulk, used on pickup, cannot be dropped or
  sold) exist; the mission `RewardItemPools` arrays that would name the reward missions are not decodable here.

## WillowInventoryManager.FindLeastValuableWeapon / FindLeastValuableItem
- **Does:** walks the chain (and, for items, also the item chain) and returns the inventory with the **lowest monetary value** among
  entries that pass a per-class eligibility virtual and whose readied flag matches the two booleans (unreadied allowed when
  `bIncludeUnreadied`, readied allowed when `bIncludeReadied`). Defaults: unreadied true, readied false. The slot logic calls
  `FindLeastValuableWeapon(False, True)`: the cheapest **readied** weapon. Ties keep the first found.
- **Implementer checklist:** strict less-than on `MonetaryValue`; None when nothing qualifies.

## WillowInventoryManager.ItemActors
- **Does:** iterator like `InventoryActors` over the manager's inventory restricted to `WillowItem` classes (optionally readied
  only); body not read in detail. Script treats it as "all gear the pawn holds equipped".

## WillowInventoryManager.FindBestHolsteredWeapon / ReplaceHolsteredWeapon / SetHolsteredWeapon
- **Does:** presentation. The manager forwards to its pawn only if the pawn's holster flag is set. `FindBestHolsteredWeapon(size)`
  picks the first weapon from the pawn's inventory list that is not the weapon or off-hand weapon in hand, is not flagged as already
  holstered, and matches the optional size filter; if none, retries ignoring the held-weapon exclusion. `Replace` and `Set` forward.
- **Implementer checklist:** no gameplay effect; may be a no-op in the host.

## WillowPlayerController.CanAffordToPickUpPickupable / PayForPickupable
- **Does:** the same shape as G4's use-cost pair, but through `IPickupable`'s cost query, which fills (costs?, currency type, amount)
  for the pickup (fields `bCostsToPickUp`, `CostsToPickUpType`, `CostsToPickUpAmount` on `WillowPickup`). Free or null: affordable,
  Pay no-op. Type 3 (golden keys): amount <= key count; Pay fires `SpendGoldenKey` per unit. Otherwise amount <= `GetCurrencyOnHand(type)`
  and Pay calls `AddCurrencyOnHand(type, -amount)`. Never refuses by itself.
- **Slice:** pickups in the Fire path cost nothing.

## WillowPlayerController.CanHoldWeapon (Controller virtual, thin front end)
- **Does (reading):** with a holder pawn that passes a pawn-kind predicate: a null weapon is refused and the weapon must be a
  WillowGame-package weapon class; for an off-hand request the holder must be a player pawn that has an action skill (dual wield),
  else refused. Finally the answer is the inverse of a controller state flag (set by cinematic or vehicle states; flag not named).
- **Confidence:** medium-low; used by `ClientWeaponSet`, `PickupPickupable` via `SwitchToBestWeapon` and the readied weapon logic.

## Pickup interface accessors (Engine.DroppedPickup / WillowPickup)
- **Does:** `GetPickupableInventory` returns the pickup's `Inventory`; `GetPickupableInventoryDefinition` that inventory's
  definition; `Pickupable_IsEnabled` is the pickup's pickup-allowed state (for `WillowPickup` set by `SetPickupability(bool)` and
  initially false while a mission-director item's mission dependencies are not met or while its objective is inactive);
  `IsDiscovered` / `MarkAsDiscovered` the discovery flags (call-out dialog once per pickup, `WillowCalloutDefinition.DialogEvent`);
  `TouchPickupTrace(from, to)` is the blocking check used by `ValidTouch`. Reading, confidence medium (the implementers sit behind the
  interface table and were not each read).
- **`WillowPickup` data defaults:** `bPickupable` true, life spans very short 60 s / short 600 s / long 1800 s, shrink 15 s before
  destruction, rigid-body awake time 15 s, always-relevant distance squared 64,000,000.
- **Inventory space:** `GetInventorySpaceRequirement` asks the definition; the default is **1**; `WeaponTypeDefinition` and
  `EquipableItemDefinition` use the default; a `UsableItemDefinition` returns 0 when either of its two low flag bits is set (used-on-pickup
  items take no space), else 1; a `MissionItemDefinition` returns 0.

## Equip, unequip and pickup events (WillowEquipAbleItem / WillowWeapon / WillowItem / definitions)
- **Does:** each is "raise a named behavior event on the behavior consumer with the instigator as the event instigator":
  item-level natives fire `OnEquipped`, `OnUnequipped`, `OnPickupAssociated` on the item's consumer (the item's behavior provider plus
  the optional extra `Providers` array); `WillowWeapon.OnEquip` / `OnUnequip` fire `OnEquip` / `OnUnequip` on the weapon's consumer;
  the definition forms (`EquipableItemDefinition`, `WeaponTypeDefinition`, `ItemDefinition`, `UsableItemDefinition.OnUsed`) activate the
  same-named event on the definition's own behavior provider through the behavior kernel (G2). All use link filter "any",
  with no payload beyond the instigator.
- **Ordering:** gear: `ItemEquipped` runs attribute effects and skill, plays the sound, then `OnEquipped`. Weapon: attribute effects
  at `WeaponEquipping` start; `OnEquip` when `WeaponEquipped` completes (after a reload is started if the magazine is empty).
- **Slice:** Maya's skills that react to equipping a weapon go through these events; a host without the kernel can ignore them for
  the Fire mission but must not skip the attribute effects in script.
- **Open:** whether the Providers argument is ever filled by script callers (none of the read callers pass it).

## WillowPawn.ShouldAutoReadyMissionWeapon / WillowWeapon.IsMissionWeapon / MissionTracker.IsValidMissionWeapon
- `ShouldAutoReadyMissionWeapon`: true unless the pawn is in the injured (Fight For Your Life) state (`bIsInjured`).
  No script caller; the native grant path asks it (see below, UNVERIFIED).
- `IsMissionWeapon`: asks the `IMissionInventory` interface of the weapon (the `WillowGame` weapon whose balance is a
  `MissionWeaponBalanceDefinition`); the sort note uses it for the MISSION WEAPONS header.
- `IsValidMissionWeapon(balance)`: true when the balance is in the tracker's `ActiveMissionWeapons` array (membership test by
  reference). Used to refuse readying a mission weapon whose mission is no longer active.

## Mission weapon: grant, ready, remove (pointer for the tracker side: NATIVE_MISSION_SCRIPT_BRIDGE.md, NATIVE_LOOT.md)
- **Grant at accept (C1/G1):** `ActivateMission` -> `GrantMissionWeapon` rolls `MissionWeapon` once per local controller, marks it
  (mark value 2), gives it to the pawn through the inventory's `GiveTo`, and calls the controller's script
  `ShowMissionWeaponTraining(weapon)`. Path through this note: `GiveTo` -> `AddInventory(bReady = ...)` ->
  `InventoryShouldBeReadiedWhenEquipped` (readied only if a weapon slot is free, otherwise it lands in the backpack) -> quick slot
  by `AssignNextAvailableDefaultQuickSlot`. `ActiveMissionWeapons` receives the balance.
- **Re-grant on load:** `GrantMissionWeaponsToClientPlayer(pc)` runs after the save's missions are applied (primary player only).
  For every balance in `ActiveMissionWeapons` it rolls the balance again at the owning mission's `GameStageRegion` game stage and
  awesome level, sets the new weapon's `SourceResponsibleName` to the mission's name, and calls `GiveTo(pawn, False)` on it (not
  readied). An early exit (a controller predicate plus a net-mode helper) was not resolved. Mission weapons are never saved
  (`CanBeSaved` false), which is why this exists.
- **Remove (tracker native, fires on Complete or when the mission ends):** for every player controller, fires the script event
  `RemoveMissionWeapons(MissionWeaponBalance)` on the controller's pawn inventory manager, then removes the balance from
  `ActiveMissionWeapons`. The script `RemoveMissionWeapons` (authority only) takes every readied and backpack weapon whose balance
  equals it: records the telemetry event, `RemoveFromInventory`, destroys; then `ClientRemoveMissionWeapons` does the same for the
  client-side backpack copies and the transient "being equipped / swapped" slots. If it was the held weapon, removal makes the manager
  switch to the best remaining weapon (script `RemoveFromInventory` path).
- **Implementer checklist (Fire mission):** (1) grant adds a weapon with mark 2 and is not droppable/sellable/savable; (2) readied
  only when a slot is free, otherwise backpack (the player may ready it from the backpack while the mission is active);
  (3) at turn-in the weapon disappears from slots and backpack at once; (4) the quick slot it held becomes free (`QuickSelectSlot`
  reset by `Unreadied`); (5) no level requirement; (6) reading it from the backpack after the mission ended is refused.

## Ammo pools and money (brief)
- Ammo pools are `ResourcePool`s found with `Controller.GetResourcePoolForResourceDefinition(weaponType.AmmoResource)`. A weapon's
  `AssociateAmmoPool` (on `GivenTo`) caches the pool and **moves any `StoredAmmo` into the pool**, zeroing it; a weapon going to the backpack
  does the reverse (`GiveStoredAmmoBeforeGoingToBackpack` adds the weapon's stored rounds to the pool; the weapon is created with the type's
  `StartingAmmoCount`). `GiveGrenadeToPlayer` adds exactly 1 to the `Grenade_Protean` pool.
- Ammo, money, health and SDU pickups are `UsableItemDefinition`/`WillowUsableItem` items: `bAutomaticallyPickup` (touch to take),
  `bPickupInBulk` (gather within `PickupRadius`), `bPlayerUseItemOnPickup` (used on the spot), and their effect is the behavior event `OnUsed`
  fired by the native `UsableItemDefinition.OnUsed` on the definition's provider; the actual pool or currency change is in the
  behavior data (G2/G11), not in these natives. `ScriptAnnounceAmmoGain` is a script event raised natively for the HUD message.
  Currency caps and `AddCurrencyOnHand` are in the G4 note.

## Not read yet
- The native that decides `SawPickupable` / `ClearSeenPickupable` (which pickup the crosshair "sees"), the pickup-key binding for
  `PickupSomething`, and the native input path.
- `ApplyExternalSlotEffectModifiers` / `ApplyInternalSlotEffectModifiers` (class-mod and weapon attribute slots), `InitializeAttributeSlots`.
- How weapon-slot, backpack and bank SDUs apply their numbers (`ExternalAttributeEffects` arrays are not decodable); the real new-character
  `WeaponReadyMax` default.
- The native tick that raises `bIsDead`/injured transitions and weapon-switch input (`NextWeapon`, `PrevWeapon`, `SwitchQuickSlot` are script).
- `GrantMissionWeaponsToClientPlayer` early-exit predicates; where `ShouldAutoReadyMissionWeapon` is consulted.
- `WillowPlayerController.UpdateAmmoCounts` body (a virtual call that refreshes the ammo HUD).
- The sell, buy-back, bank/stash and trade paths (`PlayerSoldItem`, `TheBank`, `TheStash`, trade manager).

## Corrections to earlier notes
- NATIVE_USE_INTERACTION.md says the pickup binding "was not traced": still true. Its statement that a current pickupable wins the key
  matches script; the manual pickup is `PickupSomething`, and the auto pickup is the touch path above (no per-frame ray for pickups).
- NATIVE_LOOT.md, `GrantMissionWeapon`: the grant leaves readiness to `AddInventory`'s slot test; the mission weapon is not forced into a slot.
