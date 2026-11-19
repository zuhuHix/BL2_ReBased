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

Each rolled inventory is dropped at the dead pawn's location with the pawn's item toss velocity. `DropFrom` spawns
the inventory's `DroppedPickupClass` (a `WillowPickup`), gives it falling physics and the toss velocity (and torque by
the drop's or the definition's rule), links the inventory to it, and initialises it from the inventory with pickup
enabled. A mission that denies the pickup, an inventory its owner may not drop, a missing pickup class or a failed
spawn destroys the inventory instead. NATIVE_LOOT_SPAWN.md has the full rule and its own checks.

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
   - a toucher that can pick up inventory (`bCanPickupInventory`) and that is controlled or drives a vehicle;
   - not the pickup's own instigator while the pickup is still falling and moving upward (a pawn cannot catch what
     it just tossed on the way up);
   - a clear trace from the toucher to the pickup's touch trace point (`TouchPickupTrace`, indexed). If the trace
     fails, the touch is checked again after 0.5 seconds;
   - the game info's `PickupQuery` agreeing.
3. **The controller's side** (`WillowPlayerController.TouchedPickupable`): with the pickup enabled, it becomes the
   controller's `CurrentTouchedPickupable`. When the definition says this player picks it up automatically
   (`ShouldPlayerAutomaticallyPickup`), the controller takes it at once, as the key would (section 5).
4. **The touch ends** (`UnTouch`): the controller forgets it if it was the touched one.

## 4. The look: which pickup the player sees (link 7)

The player controller's per-tick targeting, a native, chooses at most one **seen** pickup. Read with medium
confidence, as a rule and not as its structure:
- **Candidates** are the live pickups the game's globals keep a list of (`WillowGlobals`; a destroyed pickup leaves
  the list, section 6), skipping disabled ones.
- **Within the pickup radius** (`GlobalsDefinition.PickupRadius`), a pickup must lie inside a narrow cone around the
  view direction (the cosine of the angle is above 0.9).
- **Each candidate gets a score** divided by its distance. The pickup already seen gets 0.25 added to its score
  before the division, so the choice holds steady. The score itself was not read (UNVERIFIED; open question 1). The
  best ratio wins.
- **A line check** from just in front of the winner (25 units back toward the viewer) to the view point must not hit
  anything but the pickup or its base. Otherwise there is no seen pickup this tick.
- **The events.** When the winner is within the pickup radius and differs from the last seen one, the controller's
  `SawPickupable` event is raised with it. When there is no winner and one was seen, `ClearSeenPickupable` is
  raised. The same pass also drives the interaction highlight and icon, which are presentation.

On the controller, `SawPickupable` records the pickup as `CurrentSeenPickupable` (on a client, after asking the
server, which records it only when it has an inventory and a definition).

**What the key takes** (`GetCurrentPickupable`, NATIVE_USE_INTERACTION.md): the touched pickup when its definition is
picked up automatically; otherwise the seen one when there is one; otherwise the touched one. So a pickup the player
is aiming at wins over a nearer one they merely touch. That is link 7's rule, as read.

## 5. The take

1. **The key** (`PickupSomething`) uses the current pickup (section 4). Nothing happens without one, when the
   pickup refuses the attempt (`DenyPickupAttempt`), or when the controller has no inventory pawn
   (`GetInventoryPawn`). It also needs that the player can afford it (`CanAffordToPickUpPickupable`, indexed;
   loot is free).
   - **With room** (`HasRoomInInventoryFor`, through the inventory manager): the server take runs (step 2).
   - **Without room**, for a weapon the pawn can use, when the held weapon can be thrown (or none is held): the held
     weapon is thrown, then the take runs. An equippable item does the same for the item at its equipment location.
   - **Otherwise** the "pickup failed" message shows, and the full-backpack statistic is updated.
   - On success the price is paid (`PayForPickupable`, indexed).
2. **The server take** (`PickupPickupable`) takes the current pickup, plus nearby bulk pickups when its definition
   allows bulk pickup:
   - a bulk pickup is another live, enabled `WillowPickup` whose definition also allows it, within the pickup
     radius of the first;
   - for each pickup taken, while there is room and `PickupQuery` agrees, the pickup is given to the pawn: `GiveTo`,
     or a clone for each co-op pawn when the co-op range rule applies;
   - a weapon taken with ready-after-pickup is marked for the pawn to grab;
   - after each give, the pickup is no longer the touched or the seen one, and every player controller is told
     `UnTouchedPickupable`;
   - no room, or a refused query: the failed message, and the pickup's `FailedPickup`.
3. **`DroppedPickup.GiveTo`:** with an inventory, the inventory announces the pickup, the inventory's `GiveTo` gives
   it to the pawn, the pickup forgets the inventory, and `PickedUpBy` **destroys the pickup**.
4. **`Inventory.GiveTo`:** with a pawn that has an inventory manager, the manager's pickup statistics are updated and
   its `AddInventory` runs.
5. **`WillowInventoryManager.AddInventory`** (script, NATIVE_INVENTORY_EQUIP.md has its natives):
   - an item merges into a stack of the same definition already held (the quantity adds up and the new one is
     destroyed);
   - otherwise it goes to the backpack, or is readied or equipped as the pawn's rules say;
   - a weapon may be switched to, by the auto-switch rule;
   - the pickup sound plays;
   - the definition's `PickedUpStatID` statistic is incremented;
   - when the inventory ends up readied, its equip hook runs, and its equipped statistic is incremented and
     evaluated by the mission tracker.

## 6. The pickup's destroy

`WillowPickup.Destroyed`:
- tells its item that the pickup is gone;
- removes it from the globals' pickup list. That removal fires the `OnPickupDetached` global event, and
  `OnAllPickupsDetached` when no other pickup of the same holder is left (read with medium confidence);
- runs the base `DroppedPickup.Destroyed`;
- removes it from each local player's HUD post-render list and from the population master.

## 7. For an implementer, in the port's terms

- The spawn side is NATIVE_LOOT_SPAWN.md's. The pickup actor is a `WillowPickup` spawned by `DropFrom`.
- The take is script from end to end: the touch state, `GetCurrentPickupable`, `PickupSomething`,
  `PickupPickupable`, `GiveTo` and `AddInventory`. The natives it reaches are already indexed (the pickup accessors,
  the afford and pay pair, the inventory counts and limits, `TouchPickupTrace`), except those in the table below.
- Link 7 needs the native targeting of section 4, or a recorded stand-in for it. The port has the use-target choice
  along the view ray (`ow_mission::interactive::choose_usable`), and pickups are not chosen there:
  NATIVE_USE_INTERACTION.md says they have their own walk, which is section 4.

| Native | Rule | Confidence |
|---|---|---|
| `WillowPlayerController.GetInventoryPawn` | the controller's pawn; when that pawn answers as a vehicle, the vehicle's `Driver` instead | medium |
| `WillowGlobals.RemovePickup(Pickup)` | removes the pickup from the globals' list, firing the two global events of section 6 | medium |
| `DroppedPickup.AddToNavigation` | does nothing in this build | high |
| the per-tick pickup targeting (no script name; it raises `SawPickupable` and `ClearSeenPickupable`) | section 4 | medium |

## Open questions

1. **UNVERIFIED:** the per-candidate score of section 4 (not read), and which pickups count as disabled there.
2. **UNVERIFIED:** whether a pickup thrown by the player (the full-backpack swap of section 5) can be touched again
   on its way down, as the touch rule of section 3 suggests.
3. **UNVERIFIED:** which `PickupQuery` the slice's game info answers (base `GameInfo` or a Willow version).
4. **UNVERIFIED:** what "answers as a vehicle" means for `GetInventoryPawn` (which pawn query it uses).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Kill a bullymong that drops a gun. Hook `DropFrom`, `WillowPickup.Landed` and `PickupAtRest`, and record the pickup's
  class, location and velocity at spawn and at rest.
- Walk over the gun without aiming at it. Hook `DroppedPickup.Touch`, `ValidTouch` and
  `WillowPlayerController.TouchedPickupable`, and read `CurrentTouchedPickupable`.
- Drop two guns a short way apart. Stand nearer one and aim at the other, then hook `SawPickupable` and
  `ClearSeenPickupable` and read `CurrentSeenPickupable`. Press the key and record which `GiveTo` runs (link 7).
- Sweep the aim slowly across the edge of a pickup at a fixed distance, and record when `SawPickupable` fires
  (the 0.9 cone). Move closer and further to find where the radius cuts in.
- Pick up with a full backpack, and record the thrown weapon, the failed message and the full-backpack statistic.
- After a take, confirm the pickup's `Destroyed` ran, the inventory is in the backpack (or merged into a stack), and
  `WillowGlobals`' pickup list no longer holds it.
