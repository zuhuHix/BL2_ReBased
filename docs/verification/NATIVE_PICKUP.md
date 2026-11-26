# Native and script rules of a dropped item's pickup: from the roll to the backpack (draft, 2026-11-19)

AI-assisted (Claude), analyst lane ANALYST-PICKUP (decision 2026-11-19-64), for slice links 6 (pick up the gun) and 7
(the aimed pickup wins over the nearest). Records first: NATIVE_LOOT_SPAWN.md (the drop and the pickup's spawn),
NATIVE_USE_INTERACTION.md (the key and `GetCurrentPickupable`) and NATIVE_INVENTORY_EQUIP.md (the inventory
natives). The script was then read with `research/script_disasm.py`, and the natives the records did not cover were
read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock). Written in our
own words, as behaviour only: no decompiler output, pseudo-code, quoted script statements, addresses, offsets or call
structure is reproduced here. **Every rule is UNVERIFIED in the running game.** The last section lists the live
observations that would confirm them.

**Clean room.** This lane implements nothing. Everything here stays unported, or `NotYet` in Off, until a fresh
implementer codes it from this note alone.

## Summary

| Step | Kind | Already recorded | Confidence | Status |
|---|---|---|---|---|
| The roll's item becomes a pickup actor (`DropFrom`, `CreatePickup`, `InitializeFromInventory`) | script | NATIVE_LOOT_SPAWN.md | high | UNVERIFIED |
| The pickup comes to rest (`Landed`, `PickupAtRest`) | script | here | high | UNVERIFIED |
| The pawn touches it (`DroppedPickup` state `Pickup`: `Touch`, `ValidTouch`, `UnTouch`) | script | here | high | UNVERIFIED |
| The player looks at it (`SawPickupable`, `ClearSeenPickupable`) | native targeting, then script | here | medium | UNVERIFIED |
| Which one the key takes (`GetCurrentPickupable`) | script | NATIVE_USE_INTERACTION.md | high | UNVERIFIED |
| The take (`PickupSomething`, `PickupPickupable`, `GiveTo`, `AddInventory`) and the pickup's destroy | script | here | high | UNVERIFIED |

## 1. From the roll to a pickup actor (NATIVE_LOOT_SPAWN.md, summarised)

Scoped to ordinary droppable slice loot (a gun with a `DroppedPickupClass`, not a mission item). Each rolled
inventory is dropped at the dead pawn's location with the pawn's item toss velocity. `DropFrom` spawns the
inventory's `DroppedPickupClass` (a `WillowPickup`), gives it falling physics and the toss velocity (and torque by the
drop's or the definition's rule), links the inventory to it, and initialises it from the inventory with pickup
enabled. A mission that denies the pickup, an inventory its owner may not drop, a missing pickup class or a failed
spawn destroys the inventory instead. NATIVE_LOOT_SPAWN.md (section "DropFrom and the pickup") has the full rule and
its two exceptions outside this scope: a weapon that cannot be thrown makes no pickup and stays intact, and a mission
item's initialisation can leave pickup disabled.

## 2. The pickup comes to rest

When the pickup lands (`Landed`), it is marked at rest (`bPickupAtRest`). The first time, it is also handed to the
population master as an external actor of the inventory factory, made to survive level transitions, and registers
as a mission director. On the `DroppedPickup` side, landing adds it to navigation, which does nothing in this
build (an empty native). The touch rules below do not depend on being at rest.

## 3. The touch

While the pickup is in its `Pickup` state:
1. **A pawn touches it.** A vehicle passes the touch to an occupant who can pick up, if any. Then, when the touch is
   valid (below), the toucher's controller is told it touched the pickup (`TouchedPickupable`). When it is not valid
   but the definition is picked up automatically, the touch is checked again after 0.5 seconds.
2. **A valid touch** needs all of:
   - a toucher that can pick up inventory (`bCanPickupInventory`) and that has a controller or drives a vehicle.
     **Correction** to NATIVE_INVENTORY_EQUIP.md ("not driving"): the script refuses only a toucher with neither a
     controller nor a driven vehicle (UNVERIFIED in play);
   - not the pickup's own instigator while the pickup is still falling and moving upward (a pawn cannot catch what
     it just tossed on the way up);
   - a clear trace from the toucher to the pickup (`TouchPickupTrace`, indexed). **Direction resolved** by the
     package declaration, whose parameters are `StartLoc` then `EndLoc`: the script passes the toucher's location as
     the start and the pickup's touch trace point as the end. That point is the pickup's location raised by half its
     bounding cylinder's height. This matches NATIVE_INVENTORY_EQUIP.md's `TouchPickupTrace(from, to)` and corrects
     its prose ("from the pickup to the pawn"). What blocks the trace inside the native was not read: a named `NotYet`
     in Off (Off contract, below). **Superseded** by NATIVE_TOUCH_PICKUP_TRACE.md: the native's line runs from `EndLoc`
     to `StartLoc`, and its rule is read there. If the trace fails, the touch is checked again after 0.5 seconds;
   - the game info's `PickupQuery` agreeing (section 5, "PickupQuery").
3. **The controller's side** (`WillowPlayerController.TouchedPickupable`): with the pickup enabled, it becomes the
   controller's `CurrentTouchedPickupable`. When the definition's `ShouldPlayerAutomaticallyPickup(controller)` answers
   true, the controller takes it at once, as the key would (section 5). That query defaults to the definition's
   `bAutomaticallyPickup` flag, but a mission item with a mission directive answers true when the player is not in a
   vehicle (NATIVE_INVENTORY_EQUIP.md). It is not the same test as the flag `GetCurrentPickupable` reads (section 4).
   A gun has neither.
4. **The touch ends** (`UnTouch`): the controller forgets it if it was the touched one.

## 4. The look: which pickup the player sees (link 7)

A per-tick native chooses at most one **seen** pickup. **[NATIVE_PICKUP_TARGETING.md](NATIVE_PICKUP_TARGETING.md)
(decision 2026-11-19-74, review r1 of #461) is the rule now**, and wins where this summary is shorter. In brief:
- **The pass is the player HUD's** (`WillowHUD`), not the controller's. It reads the HUD's owner, the owner's view
  point and the HUD's `HUDDef`, and raises its events on the HUD's `WPlayerOwner` (the owner controller).
- **Candidates** are the `bPickupable` pickups of the globals' list (`WillowGlobals.PickupList`; a destroyed pickup
  leaves the list, section 6). A pickup is scored when it lies within the HUD definition's range,
  **`HUDDefinition.PickupRange`** (not `GlobalsDefinition.PickupRadius`, as an earlier draft of this note said), or
  always when the globals definition has a hover particle, and inside a horizontal cone (a cosine above 0.9, yaw only).
- **The score and the ranking** are resolved there: the view ray against a vertical segment through the pickup's
  centre, a capture radius from `GlobalsDefinition.HoverWidthPickup`, a bonus and a wider radius for the pickup
  already hovered, divided by the distance, the highest value winning, ties to the earlier list entry.
- **A line check** from 25 units back from the winner toward the viewer, to the view point, must not hit anything but
  the pickup or its base, **except that no trace runs when the winner is within a quarter of `PickupRange`
  horizontally**.
- **The events.** When the winner is within `PickupRange` and differs from the last reported one, `SawPickupable` is
  raised with it. When there is no winner and one was reported, `ClearSeenPickupable` is raised. Each is raised before
  the HUD records the change. The same pass also drives the interaction highlight and icon, which are presentation.

On the controller, `SawPickupable` records the pickup as `CurrentSeenPickupable` (on a client, after asking the
server, which records it only when it has an inventory and a definition).

**What the key takes** (`GetCurrentPickupable`, NATIVE_USE_INTERACTION.md and NATIVE_INVENTORY_EQUIP.md):
1. **No touched pickup: nothing**, even when a pickup is seen. A touch is the prerequisite; the look only chooses
   once the player touches some pickup.
2. The touched pickup, when its definition has `bAutomaticallyPickup` (the flag itself, not
   `ShouldPlayerAutomaticallyPickup`).
3. Otherwise the seen one when there is one, else the touched one.

So, while the player touches some pickup, the one they aim at wins over the touched one. That is link 7's rule, as
read.

**The slice fixtures** (SLICE_GATE.md, the pickup links). Each fixture must establish the touched slot itself: a
valid touch, which includes the touch trace and `PickupQuery` passing (section 3).
- **Link 6** (one gun dropped 150 uu ahead of Maya, aimed at): **the touch source is that gun**, the only pickup.
  Whether its manual-pickup collision (mesh-sized, NATIVE_INVENTORY_EQUIP.md) reaches Maya at 150 uu was not read
  (open question 5).
- **Link 7** (the aimed gun, plus a nearer decoy 45 uu to Maya's side, about 90 degrees off her aim): **the touch
  source is the decoy**, the nearer gun, which fills the touched slot when its touch is valid. The aimed gun then
  wins as the seen one **only with a supplied view** that puts the decoy outside the cone and the aimed gun inside it,
  and only when the aimed gun passes NATIVE_PICKUP_TARGETING.md section 11's scoring and reporting gates (a positive
  score, the pawn test, the trace beyond a quarter of the range, its definition, the 0.5 check, and `PickupRange`).
  If the aimed gun also touches Maya, the answer is the same.
- **Link 7's look** is NATIVE_PICKUP_TARGETING.md section 11: with a supplied view at Maya's location aimed at the
  gun, the decoy fails the cone and the aimed gun is the only scored candidate. The fixture's placements alone give no
  camera geometry.
- **What Off answers** (Off contract, below): a touch whose validity cannot be settled is a named `NotYet`. An
  observed empty touched slot gives no pickup, which is the game's answer and not a `NotYet`. **The touched slot stays
  the key's prerequisite** (rule 1 above): the look only matters once the player touches some pickup. The look itself
  answers as NATIVE_PICKUP_TARGETING.md section 10 says, in its order: the touched-slot check first, then no HUD, a HUD
  with no owner or definition (nothing seen), and no camera.

## 5. The take

1. **The key** (`PickupSomething`) uses the current pickup (section 4). Nothing happens without one, when the
   pickup refuses the attempt (`DenyPickupAttempt`), or when the controller has no inventory pawn
   (`GetInventoryPawn`). It also needs that the player can afford it (`CanAffordToPickUpPickupable`, indexed;
   loot is free).
   - **With room** (`HasRoomInInventoryFor`, through the inventory manager): the server take runs (step 2).
   - **Without room**, only when ready-after-pickup is set and the pawn can use the item (the readiness gate of
     NATIVE_INVENTORY_EQUIP.md, step 5): a weapon swaps with the held one when that can be thrown (or none is held),
     and an equippable item with the item at its equipment location; then the take runs.
   - **Otherwise** the "pickup failed" message shows. The full-backpack statistic is updated only when the item needs
     space (`GetInventorySpaceRequirement` above 0).
   - On success the price is paid (`PayForPickupable`, indexed).
2. **The server take** (`PickupPickupable`) takes the current pickup, plus nearby bulk pickups when its definition
   allows bulk pickup:
   - a bulk pickup is another live, enabled `WillowPickup` of the same kind whose definition also allows bulk
     pickup, within the pickup radius of the first (NATIVE_INVENTORY_EQUIP.md has the full condition);
   - for each pickup taken, while there is room and `PickupQuery` agrees, the pickup is given to the pawn: `GiveTo`,
     or a clone for each co-op pawn when the co-op range rule applies (the bulk list is capped at 4,
     NATIVE_INVENTORY_EQUIP.md);
   - a weapon taken with ready-after-pickup is marked for the pawn to grab;
   - after each give, the pickup is no longer the touched or the seen one, and every player controller is told
     `UnTouchedPickupable`;
   - no room, or a refused query: the failed message, and the pickup's `FailedPickup`.
3. **`DroppedPickup.GiveTo`:** with an inventory, the inventory announces the pickup, the inventory's `GiveTo` gives
   it to the pawn, the pickup forgets the inventory, and `PickedUpBy` **destroys the pickup**.
4. **`Inventory.GiveTo`:** with a pawn that has an inventory manager, the manager's pickup statistics are updated and
   its `AddInventory` runs.
5. **`WillowInventoryManager.AddInventory`** (script, NATIVE_INVENTORY_EQUIP.md has its natives):
   - an item merges into a stack of the same definition already held only when the definition sets
     `bDuplicatePickupJustAddsQuantity` (the quantity adds up and the new one is destroyed);
   - otherwise it goes to the backpack, or is readied or equipped as the pawn's rules say;
   - a weapon may be switched to, by the auto-switch rule;
   - the pickup sound plays;
   - the definition's `PickedUpStatID` statistic is incremented;
   - when the inventory ends up readied, its equip hook runs, and its equipped statistic is incremented and
     evaluated by the mission tracker.

The detailed take rules (afford, ready-after-pickup, the swap, bulk, announce, `AddInventory`'s readiness and
auto-switch) are NATIVE_INVENTORY_EQUIP.md's "Finding and taking a pickup" and `AddInventory` sections. This section
summarises them.

**`PickupQuery`.** No Willow game info replaces it, so the base `GameInfo` rule applies. A game info with a base
mutator asks the mutator's `OverridePickupQuery` first, and its answer stands when it overrides. Then a pawn with no
inventory manager is refused. Otherwise the inventory manager's `HandlePickupQuery` answers:
- no room: refused;
- the pickup's inventory's own query (`WillowInventory`: yes);
- any held inventory that denies it refuses it. `WillowWeapon` and `WillowItem` never deny, so for the slice's guns
  this passes;
- otherwise yes.

**The supported slice dispatch:** the base `GameInfo.PickupQuery` with no base mutator, then
`WillowInventoryManager.HandlePickupQuery`, with a `WillowWeapon` as the pickup's inventory and only `WillowWeapon`s and
`WillowItem`s held.

## Off contract

Every branch this note leaves unresolved answers a named `NotYet` in Off, never a guess. Named here:
- a touch whose validity cannot be settled, including what blocks `TouchPickupTrace` inside the native (section 3);
- the per-tick targeting's own named `NotYet`s, which NATIVE_PICKUP_TARGETING.md section 10 lists (the HUD, the
  camera, the pickup's bounds, collision for the trace, and the rest); eligibility and the ranking are values there,
  not `NotYet`s;
- a `PickupQuery` target other than the supported slice dispatch above: a game info with a base mutator, another
  game-info class, another inventory manager class, or a pickup's or a held inventory whose class replaces the query
  (for example `WillowUsableItem`);
- `GetInventoryPawn` on a pawn whose vehicle test is unresolved (open question 4): only a plain player pawn, which
  answers as no vehicle, is supported;
- the co-op clone branch (`ShouldUseCoopRange`, `CloneAndGiveToCoopPawns`).

An observed empty touched slot is not one of these: the key takes nothing.

## 6. The pickup's destroy

`WillowPickup.Destroyed`:
- tells its item that the pickup is gone;
- removes it from the globals' pickup list. That removal fires the `OnPickupDetached` global event, and
  `OnAllPickupsDetached` when no other pickup of the same holder is left (read with medium confidence). Read further
  in NATIVE_LINK6_AFTER_TAKE.md section 2 (2026-11-26): the events are not global. They go to the pickup's `Base` as
  a behavior consumer, the holder's list is the base's `Attached`, and with no `Base` nothing is raised;
- runs the base `DroppedPickup.Destroyed`;
- removes it from each local player's HUD post-render list and from the population master.
- The headless answers for this path (the population master's fixture value, a non-None master, and when
  `OnAllPickupsDetached` fires: only when the pickup was the last one on its `Base`) are in NATIVE_WILLOW_GLOBALS.md,
  section 6.3 (decision 2026-11-19-85).

## 7. For an implementer, in the port's terms

- The spawn side is NATIVE_LOOT_SPAWN.md's. The pickup actor is a `WillowPickup` spawned by `DropFrom`.
- The take is script from end to end: the touch state, `GetCurrentPickupable`, `PickupSomething`,
  `PickupPickupable`, `GiveTo` and `AddInventory`. The natives it reaches are already indexed (the pickup accessors,
  the afford and pay pair, the inventory counts and limits, `TouchPickupTrace`), except those in the table below.
- Link 7 needs the native targeting of section 4, as NATIVE_PICKUP_TARGETING.md gives it. Pickups are not chosen by the
  port's use-target choice (`ow_mission::interactive::choose_usable`): NATIVE_USE_INTERACTION.md says they have their
  own walk, which is section 4.

| Native | Rule | Confidence |
|---|---|---|
| `WillowPlayerController.GetInventoryPawn` | the controller's pawn; when that pawn answers as a vehicle, the vehicle's `Driver` instead. This is the pawn that will own the item, and it does not conflict with `AddInventory`'s own "not a vehicle" readiness test (NATIVE_INVENTORY_EQUIP.md) | medium |
| `WillowGlobals.RemovePickup(Pickup)` | removes the pickup from the globals' list, firing the two events of section 6 (not global: they go to the pickup's `Base` as a behavior consumer, NATIVE_LINK6_AFTER_TAKE.md section 2) | medium |
| `DroppedPickup.AddToNavigation` | does nothing in this build | high |
| the per-tick pickup targeting (the HUD's, no script name; it raises `SawPickupable` and `ClearSeenPickupable`) | NATIVE_PICKUP_TARGETING.md | high |

## Open questions

1. Answered by NATIVE_PICKUP_TARGETING.md (decision 2026-11-19-74): the capture radius, the segment and which
   pickups count (`bPickupable`). Its own open questions and live checks carry what stays UNVERIFIED.
2. **UNVERIFIED:** whether a pickup thrown by the player (the full-backpack swap of section 5) can be touched again
   on its way down, as the touch rule of section 3 suggests.
3. Settled by the script: no Willow game info replaces `PickupQuery`, so the base rule of section 5 applies.
   **UNVERIFIED:** whether the slice's game info has a base mutator (a named `NotYet` in Off if it does).
4. **UNVERIFIED:** what "answers as a vehicle" means for `GetInventoryPawn` (which pawn query it uses).
5. **UNVERIFIED:** whether a manual-pickup gun's collision touches Maya at link 6's 150 uu.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Kill a bullymong that drops a gun. Hook `DropFrom`, `WillowPickup.Landed` and `PickupAtRest`, and record the pickup's
  class, location and velocity at spawn and at rest.
- Walk over the gun without aiming at it. Hook `DroppedPickup.Touch`, `ValidTouch` and
  `WillowPlayerController.TouchedPickupable`, and read `CurrentTouchedPickupable`.
- Drop two guns a short way apart. Stand nearer one and aim at the other, then hook `SawPickupable` and
  `ClearSeenPickupable` and read `CurrentSeenPickupable`. Press the key and record which `GiveTo` runs (link 7).
- Sweep the aim slowly across the edge of a pickup at a fixed distance, and record when `SawPickupable` fires
  (the 0.9 cone, and the capture radius of open question 1). Move closer and further to find where the pickup
  radius cuts in.
- Stand 150 uu from a dropped gun without touching it, aim at it and press the key: record `CurrentTouchedPickupable`,
  `CurrentSeenPickupable` and whether anything is taken (open question 5 and the touch prerequisite).
- Pick up with a full backpack, and record the thrown weapon, the failed message and the full-backpack statistic.
- After a take, confirm the pickup's `Destroyed` ran, the inventory is in the backpack (or merged into a stack), and
  `WillowGlobals`' pickup list no longer holds it.
