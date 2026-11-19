# Native: the globals object on the pickup path, WillowGlobals.GetWillowGlobals and what it serves (2026-11-19)

AI-assisted (Claude Code). Lane ANALYST-WILLOW-GLOBALS, analyst only (clean room, docs/LANE_BRIEF.md; decision
2026-11-19-85). The sources, in the order used:
- the records: NATIVE_CONTROLLER_HELPERS.md (the three globals getters), NATIVE_PAWN_DATA.md and NATIVE_PICKUP.md
  (section 6, the destroy);
- the script, through `research/script_disasm.py` (every `WillowPickup` function, the `WillowGlobals` and
  `GearboxGlobals` script, the controller's pickup functions);
- the installed packages, through `tools/ghidra/class_layout.py` (which field each native reads);
- `Borderlands2.exe` locally, through `tools/ghidra/run.ps1` read-only queries, one headless run at a time under the
  machine lock.

Raw output stays under ignored `local/analysis/globals/`; this note is in our own words. No bl-sdk session was granted
and no live evidence covers it, so **every rule here is UNVERIFIED** until the live checks at the end confirm it. The
lane that wrote this note does not implement it: a fresh implementer codes it from this note alone.

**Why it matters.** With #452 (the pickup drivers) and #462 (`GetInventoryDefinition`), slice links 6 and 7 stop in
`make` at the unbound `WillowGlobals.GetWillowGlobals()`. The call is in the spawned pickup's `PostBeginPlay`, so it
runs inside the pickup's spawn, after the run's first write.

## Summary

| Native | What it does | Writes | Confidence | Status |
|---|---|---|---|---|
| `WillowGlobals.GetWillowGlobals()` (static) | the engine's one globals object; None when the engine holds none | none | high | UNVERIFIED |
| `GearboxGlobals.GetGearboxGlobals()` (static) | the same object, by the same read | none | high | UNVERIFIED |
| `WillowGlobals.AddPickup(Pickup)` | appends the pickup to the globals' `PickupList` | `PickupList` | high | UNVERIFIED |
| `WillowGlobals.GetGlobalsDefinition()` | the globals' `TheGlobalsDefinition` when it is a `GlobalsDefinition`; otherwise loads the configured one first (section 4) | `TheGlobalsDefinition`, once | high | UNVERIFIED |
| `GlobalsDefinition.GetDropLifeSpanType(int)` | the drop life-span kind of the first rarity band that holds the given rarity level; 3 when none does | none | high | UNVERIFIED |
| `GearboxGlobals.GetPopulationMaster()` | the globals' `ThePopulationMaster`, as it is | none | high | UNVERIFIED |
| `WillowGlobals.RemovePickup(Pickup)` | removes every entry of the pickup from `PickupList`, after the two global events (NATIVE_PICKUP.md section 6) | `PickupList` | medium for the events, high for the removal | UNVERIFIED |

## 1. Where the globals are reached on the pickup path

The script calls the globals in these places on the way links 6 and 7 take. Each call is a static call through the
`WillowGlobals` class, and the result is used as a context object.

1. **The spawn (`make`).** `WillowPickup.PostBeginPlay` runs the base `Actor.PostBeginPlay`, then
   `GetWillowGlobals().AddPickup(Self)`. Then, for each local player controller whose `myHUD` is a `WillowHUD`, it
   calls that HUD's `AddPostRenderedActor(Self)`.
2. **The initialisation (`make`).** `WillowPickup.InitializeFromInventory` ends by calling `InitializeDropLifespan`,
   `InventoryAssociated` and `SetPickupability`. Each of these reaches the globals definition:
   - `InitializeDropLifespan`: for a pickup that is not a mission item, `LifeSpanType` becomes
     `GetWillowGlobals().GetGlobalsDefinition().GetDropLifeSpanType(InventoryRarityLevel)` (a mission item takes kind
     3). See section 5 for what the kind then sets.
   - `InventoryAssociated`, through `UpdateTouchRadiusForAutomaticallyPickedUpInventory`: only when the inventory is
     picked up automatically (a case links 6 and 7 do not take) and the pickup is not a mission item, it sizes the
     touch cylinder from `GlobalsDefinition.PlayerInteractionDistance`.
   - `SetPickupability`, through `TryToSetInteractParticles`: for a pickup that is not a mission director, turning
     particles on with none set yet and the pickup pickupable sets them to `GlobalsDefinition.PartPickup`.
     Turning them off clears them.
3. **The touch and the key.** No function on this path reads the globals. The controller's `PickupSomething`, the
   touch handlers and `GiveTo` hold no globals call. `DenyPickupAttempt` reads `GlobalsDefinition.MissionAcceptDefinition`,
   but only for a mission item with a directive.
4. **The destroy (after the take).** `WillowPickup.Destroyed` calls `GetWillowGlobals().RemovePickup(Self)` after its
   item's `PickupDisassociated`, then the base `DroppedPickup.Destroyed`. Then, for each local player's `WillowHUD`,
   it calls `RemovePostRenderedActor(Self)`. Last, when `GetGearboxGlobals().GetPopulationMaster()` is not None, it
   calls `RemoveSpawnedActor(Self, false)` on it.
5. **Mission-only users, outside links 6 and 7:** `RegisterMissionDirector`, `SetMissionMaterialParameter` and
   `PickupAtRest`. The first two read `GlobalsDefinition.MissionGlowColorParameter`, and `PickupAtRest` reaches the
   population master.

When a getter answers None, script does not stop. The context call is skipped with the engine's "accessed none"
warning, and a skipped call that would return a value gives that type's zero. The game never shows this on the
pickup path, because the globals object exists from start-up (section 2). A port that answers None would therefore
skip `AddPickup` silently. That is why section 6 does not answer None.

## 2. GetWillowGlobals and GetGearboxGlobals: how the object is found

- Both natives read the same engine-held pointer. When the running engine object exists, they answer the globals
  instance it holds; otherwise they answer None. They create nothing and check nothing. This agrees with
  NATIVE_CONTROLLER_HELPERS.md, which this read confirms at the level of the two exec functions.
- **The object's class** comes from configuration: `GlobalInstanceClassName=WillowGame.WillowGlobals` in the
  `[GearboxFramework.GearboxGlobals]` section of `WillowGame/Config/DefaultEngine.ini`. Its definition's name is
  `DefaultGlobalsDefinitionName=GD_Globals.General.Globals` in the `[WillowGame.WillowGlobals]` section.
- **When it is created:** not read here, as in the earlier records. It is in place before any map or pawn script runs,
  so every call on the pickup path sees the same one object for the whole session.

## 3. AddPickup: the globals' pickup list

- `AddPickup(Pickup)` takes its one object parameter and appends it at the end of `PickupList`, the globals' dynamic
  array of `WillowPickup` references. It always appends.
- The routine also scans the list for the same pickup first, but the result of that scan decides nothing. A pickup
  already in the list is appended again (high: the append does not depend on the scan).
- It fires no event, checks no class and reads nothing else. A None argument is appended as None (nothing refuses it).
- The list is what the per-tick pickup targeting walks (NATIVE_PICKUP.md section 4) and what `RemovePickup` edits.

## 4. GetGlobalsDefinition: the globals definition, loaded once

- If the globals' `TheGlobalsDefinition` (declared on `GearboxGlobals`) holds an object of class `GlobalsDefinition`
  or a subclass, that object is the answer.
- Otherwise:
  1. the native loads the object named by the globals' `DefaultGlobalsDefinitionName` (the configured
     `GD_Globals.General.Globals`), as a `GlobalsDefinition`;
  2. stores it in `TheGlobalsDefinition`;
  3. walks that definition's `ChallengeList` (a `PlayerChallengeListDefinition`) and numbers its challenges by kind,
     writing a running index into each challenge. One challenge, `Fyrestone_FindChest`, is skipped by name. The
     fields written and the meaning of the two kinds were not read further (open question 2);
  4. answers the loaded object.
- So the load happens at most once a session. After it, the getter is a field read.

## 5. GetDropLifeSpanType: the drop life-span kind

- `GetDropLifeSpanType(RarityLevel)` walks the definition's `RarityLevelColors` array in order. Each entry is a
  `RarityLevelColor`: `MinLevel`, `MaxLevel`, a colour, `DropLifeSpanType` and `RarityRating`.
- The first entry with `MinLevel <= RarityLevel <= MaxLevel`, both ends included, gives its `DropLifeSpanType`.
- When no entry holds the level, including an empty array, the answer is 3.
- Nothing is written.
- **What the script does with the kind** (`InitializeDropLifespan`): a non-mission pickup whose
  `GetInventoryDefinition()` has a `PickupLifeSpan` above 0 uses that span. Otherwise the kind picks it:
  - 0 picks `VeryShortLivedDropLifeSpan`;
  - 1 picks `ShortLivedDropLifeSpan`;
  - 2 picks `LongLivedDropLifeSpan`;
  - 3 and 4 give 0.
  The script then sets the actor's `LifeSpan` to that value. When it is above 0, it sets a one-shot timer named
  `ShrinkPickupBeforeDestruction`, `PickupShrinkDuration` before the end.

## 6. For an implementer: the Off answers, step by step

These follow the hub's rulings for the pickup drivers:
- preflight the cheap and certain before any write;
- a `NotYet` after the run's first write is terminal, with the "stopped after partial writes" prefix and its kind kept;
- there is no Off rollback;
- other errors pass through unchanged.

The pickup's spawn is after the run's first write (the gun's creation), so every `NotYet` below that the run reaches
is terminal. That is why the globals object belongs in the preflight.

1. **The world's globals object, before any write.** The world the driver runs in must hold one `WillowGlobals`
   instance, made at world set-up and kept for the run. Check that it exists in the drivers' preflight. Without one,
   `GetWillowGlobals` is a `NotYet` of kind `LayerAbsent` ("the world's globals object", the world set-up that makes
   it). It is not None: None would let `PostBeginPlay` skip `AddPickup` silently. The port already makes such an
   object in the mission bridge's world and records it through `WorldQuery::set_globals`. Whether the equip run's
   world gets the same one is the implementer's choice.
2. **`GetWillowGlobals` and `GetGearboxGlobals`:** answer that object, the same on every call. They write nothing.
3. **`AddPickup(P)`:** append `P` as given, None included, at the end of the object's `PickupList`, with no duplicate
   check and no event. If the class declares no `PickupList`, or declares it as something other than a dynamic array
   of object references, that is a data error, raised before the append.
4. **`GetGlobalsDefinition`:** answer `TheGlobalsDefinition` when it holds a `GlobalsDefinition`. The lazy branch
   (section 4, steps 1 to 3) writes the challenge list in a way not read here, so Off does not run it at call time.
   - Fill `TheGlobalsDefinition` at world set-up with the installed `GD_Globals.General.Globals`, so the call is a
     read.
   - If it is still not a `GlobalsDefinition` at call time, answer a `NotYet` of kind `NotRead` ("the globals
     definition's first load, which numbers the challenge list").
   - The port's mission bridge already answers this native with the installed object, so its loading can be reused.
5. **`GetDropLifeSpanType(L)`:** section 5's rule, a pure read. A `RarityLevelColors` that is not declared as a
   dynamic array of that struct is a data error.
6. **The HUD loops** (`AddPostRenderedActor`, `RemovePostRenderedActor`): they run only for a local player
   controller whose `myHUD` is a `WillowHUD`. The headless world has none, so the loops call nothing. No new answer
   is needed.
7. **`GetPopulationMaster`:** answer the globals' `ThePopulationMaster` as it is. A None answer makes `Destroyed`
   skip `RemoveSpawnedActor`, as in the game. Whether the headless world should hold a population master is not
   settled here (open question 3).
8. **`RemovePickup(P)`:** remove every entry equal to `P` from `PickupList`, keeping the others in order. The two
   global events of NATIVE_PICKUP.md section 6 come first. Their dispatch was read only at medium confidence, so until
   it is ported, the events are a `NotYet` of kind `NotRead` raised before the removal, so the native makes no write
   before it. That `NotYet` is terminal by the run's rule, because the destroy comes after the first write.

**Which native the run reaches next**, by the script order of section 1 once the two getters answer:
1. `AddPickup`;
2. `Actor.LocalPlayerControllers` (bound in the port);
3. the pickup's `InitializeFromInventory`, with `GetManufacturer` and `GetInventoryDefinition` (both bound);
4. `MissionTracker` reads only for a mission item;
5. `GetGlobalsDefinition` and `GetDropLifeSpanType`;
6. `SetTimer` with the drop's life span;
7. `InventoryAssociated` (the mesh and physics set-up, not read here);
8. `SetPickupability`.

What follows in `touch` and `key` is NATIVE_PICKUP.md's.

## Open questions

1. Where and when the engine creates the globals instance and stores its pointer (not found, as before).
2. What the first load of the globals definition writes into the challenge list (which fields, and what the two kinds
   mean), and whether script reads it.
3. Whether a headless slice world should hold a population master, so that `Destroyed`'s `RemoveSpawnedActor` runs.
4. The event dispatch inside `RemovePickup` (medium in NATIVE_PICKUP.md): what listens to `OnPickupDetached` and
   `OnAllPickupsDetached`.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- **One object.** Call `GetWillowGlobals()` and `GetGearboxGlobals()` from two different classes, on the title screen
  and in a map. Expect the same `WillowGlobals` object every time.
- **The pickup list.** Read `PickupList` before and after dropping a gun. Expect one more entry, at the end, that is
  the new pickup. Call `AddPickup` twice on one scratch pickup and expect two entries. Take the pickup and expect
  every entry of it gone, with the others in order.
- **The definition.** Read `TheGlobalsDefinition` at the title screen and in a map. Expect `GD_Globals.General.Globals`,
  and compare it with `GetGlobalsDefinition()`.
- **The life-span kind.** Read `RarityLevelColors`, then call `GetDropLifeSpanType` for a level inside each band,
  for one between bands if any, and for one above the last. Expect the band's kind, and 3 outside every band. On a
  dropped gun, compare `LifeSpanType` and `LifeSpan` with section 5.
- **The population master.** On a dropped pickup, read `GetGearboxGlobals().GetPopulationMaster()`, and hook
  `RemoveSpawnedActor` while the pickup is taken.
- **Accessed none.** Not checkable live without breaking the globals, which the game never lacks; it stays a script
  rule.
