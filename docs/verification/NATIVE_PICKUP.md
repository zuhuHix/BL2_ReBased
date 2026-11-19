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
   - a clear trace between the toucher's location and the pickup's touch trace point (`TouchPickupTrace`, indexed).
     NATIVE_INVENTORY_EQUIP.md gives its direction as from the pickup to the pawn. The script passes the pawn's
     location first, and the native's direction was not re-read here, so the direction stays UNVERIFIED (only
     "blocked or not" matters to the rule). If the trace fails, the touch is checked again after 0.5 seconds;
   - the game info's `PickupQuery` agreeing (section 5, "PickupQuery").
3. **The controller's side** (`WillowPlayerController.TouchedPickupable`): with the pickup enabled, it becomes the
   controller's `CurrentTouchedPickupable`. When the definition's `ShouldPlayerAutomaticallyPickup(controller)` answers
   true, the controller takes it at once, as the key would (section 5). That query defaults to the definition's
   `bAutomaticallyPickup` flag, but a mission item with a directive answers true on its own rule
   (NATIVE_INVENTORY_EQUIP.md). It is not the same test as the flag `GetCurrentPickupable` reads (section 4). A gun
   has neither.
4. **The touch ends** (`UnTouch`): the controller forgets it if it was the touched one.

## 4. The look: which pickup the player sees (link 7)

The player controller's per-tick targeting, a native, chooses at most one **seen** pickup. Read with medium
confidence, as a rule and not as its structure:
- **Candidates** are the live pickups the game's globals keep a list of (`WillowGlobals`; a destroyed pickup leaves
  the list, section 6), skipping disabled ones.
- **Within the pickup radius** (`GlobalsDefinition.PickupRadius`), a pickup must lie inside a narrow cone around the
  view direction (the cosine of the angle is above 0.9).
- **The cone is horizontal:** only the yaw of the view and of the direction to the pickup count, not the pitch.
- **Each candidate gets a score** divided by its distance. The score measures how close the view ray passes to a
  short vertical segment through the pickup's centre: 1 when the ray passes through the axis, falling to 0 at a
  capture radius, and 0 beyond it. The pickup already seen gets a wider radius and a taller segment, and 0.25 added to
  its score before the division, so the choice holds steady. A candidate whose score is not positive is not chosen.
  The best ratio wins. The capture radius and the segment's base height come from values passed in that were not
  resolved (UNVERIFIED; open question 1).
- **A line check** from just in front of the winner (25 units back toward the viewer) to the view point must not hit
  anything but the pickup or its base. Otherwise there is no seen pickup this tick.
- **The events.** When the winner is within the pickup radius and differs from the last seen one, the controller's
  `SawPickupable` event is raised with it. When there is no winner and one was seen, `ClearSeenPickupable` is
  raised. The same pass also drives the interaction highlight and icon, which are presentation.

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

**The slice fixtures** (SLICE_GATE.md, the pickup links):
- **Link 6** (one gun dropped 150 uu ahead of Maya, aimed at): the only pickup, so it alone can supply the touch.
  Whether its manual-pickup collision (mesh-sized, NATIVE_INVENTORY_EQUIP.md) reaches Maya at that distance was not
  read (UNVERIFIED; open question 5). Off: with no touch, the key takes nothing, which is the game's answer and not a
  `NotYet`.
- **Link 7** (the aimed gun, plus a nearer decoy 45 uu to Maya's side, about 90 degrees off her aim): the touch can
  come from either gun, whichever overlaps Maya. The decoy fails the 0.9 cone, so the aimed gun is the only eligible
  candidate. **Bounded case:** when Maya aims through the aimed gun's axis, its score is 1 whatever the capture
  radius, so it is chosen if it passes the line check and a touch holds. Off may answer that case.
- **Every other case is a named `NotYet` in Off:** a single candidate whose axis the ray does not pass through (its
  eligibility needs the capture radius), and any ranking among several eligible candidates (open question 1).

## 5. The take

1. **The key** (`PickupSomething`) uses the current pickup (section 4). Nothing happens without one, when the
   pickup refuses the attempt (`DenyPickupAttempt`), or when the controller has no inventory pawn
   (`GetInventoryPawn`). It also needs that the player can afford it (`CanAffordToPickUpPickupable`, indexed;
   loot is free).
   - **With room** (`HasRoomInInventoryFor`, through the inventory manager): the server take runs (step 2).
   - **Without room**, only when ready-after-pickup is set and the pawn can use the item (the readiness gate of
     NATIVE_INVENTORY_EQUIP.md, step 5): a weapon swaps with the held one when that can be thrown (or none is held),
     and an equippable item with the item at its equipment location; then the take runs.
   - **Otherwise** the "pickup failed" message shows, and the full-backpack statistic is updated.
   - On success the price is paid (`PayForPickupable`, indexed).
2. **The server take** (`PickupPickupable`) takes the current pickup, plus nearby bulk pickups when its definition
   allows bulk pickup:
   - a bulk pickup is another live, enabled `WillowPickup` whose definition also allows it, within the pickup
     radius of the first;
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

**`PickupQuery`** (the slice's game info has no override, so the base `GameInfo` rule applies): a game info with a base
mutator asks the mutator's `OverridePickupQuery` first, and its answer stands when it overrides. Then a pawn with no
inventory manager is refused. Otherwise the inventory manager's `HandlePickupQuery` answers:
- no room: refused;
- the pickup's inventory's own query (`WillowInventory`: yes);
- any held inventory that denies it refuses it. `WillowWeapon` and `WillowItem` never deny, so for the slice's guns
  this passes;
- otherwise yes.

**Named `NotYet`s in Off** on this path, until read or ruled:
- a base mutator on the game info (its `OverridePickupQuery`);
- a pickup's inventory or a held inventory whose class replaces the query (for example `WillowUsableItem`);
- the co-op clone branch (`ShouldUseCoopRange`, `CloneAndGiveToCoopPawns`);
- the per-tick targeting outside the bounded case of section 4.

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
- Link 7 needs the native targeting of section 4, within the bounded case it states. Pickups are not chosen by the
  port's use-target choice (`ow_mission::interactive::choose_usable`): NATIVE_USE_INTERACTION.md says they have their
  own walk, which is section 4.

| Native | Rule | Confidence |
|---|---|---|
| `WillowPlayerController.GetInventoryPawn` | the controller's pawn; when that pawn answers as a vehicle, the vehicle's `Driver` instead. This is the pawn that will own the item, and it does not conflict with `AddInventory`'s own "not a vehicle" readiness test (NATIVE_INVENTORY_EQUIP.md) | medium |
| `WillowGlobals.RemovePickup(Pickup)` | removes the pickup from the globals' list, firing the two global events of section 6 | medium |
| `DroppedPickup.AddToNavigation` | does nothing in this build | high |
| the per-tick pickup targeting (no script name; it raises `SawPickupable` and `ClearSeenPickupable`) | section 4 | medium |

## Open questions

1. **UNVERIFIED:** the capture radius and the segment height of section 4's score, and which pickups count as
   disabled there.
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
