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
| `WillowGlobals.GetGlobalsDefinition()` | the globals' `TheGlobalsDefinition` when it is a `GlobalsDefinition`; otherwise loads the configured one first (section 4) | on the first load only: `TheGlobalsDefinition`, and a running index written into each numbered challenge of the definition's `ChallengeList` (section 4, step 3) | high | UNVERIFIED |
| `GlobalsDefinition.GetDropLifeSpanType(int)` | the drop life-span kind of the first rarity band that holds the given rarity level; 3 when none does | none | high | UNVERIFIED |
| `GearboxGlobals.GetPopulationMaster()` | the globals' `ThePopulationMaster`, as it is | none | high | UNVERIFIED |
| `WillowGlobals.RemovePickup(Pickup)` | dispatches `OnPickupDetached`, and `OnAllPickupsDetached` when the pickup was the last on its `Base`, then removes every entry of the pickup from `PickupList` (section 6.3, step 7) | `PickupList` | medium for the events, high for the removal | UNVERIFIED |

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
skip `AddPickup` silently. That is why section 6 does not answer None for either getter.

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
  `ShrinkPickupBeforeDestruction` with the delay `LifeSpan - PickupShrinkDuration` (section 6.3, step 4 of the
  sequence; a delay of 0 or below is UNVERIFIED).

## 6. For an implementer: the Off answers, step by step

These follow the hub's rulings for the pickup drivers:
- preflight the cheap and certain before any write;
- after the run's first write, a refusal keeps its typed outcome. A `NotYet` stays `VmError::NotYet` with its kind and
  record, its text gains the "stopped after partial writes" prefix, and every write already made stays in place (no
  Off rollback). Any other error passes through unchanged;
- the gate grades by the `VmError` variant, never by text.

The pickup's spawn comes after the run's first write (the gun's creation). So everything the route needs that can be
known before that write belongs in the preflight, and the call-time guards below are only for what cannot.

**The typed outcomes used below.**
- *Layer absent:* `VmError::NotYet`, kind `LayerAbsent`, record this note.
- *Not read:* `VmError::NotYet`, kind `NotRead`, record this note.
- *Data error:* `VmError::Script`, naming the native, for a declared layout of the wrong shape. This is the same
  variant the inventory definition natives use for a malformed declaration.

### 6.0 What `TheGlobalsDefinition` is

It is an ordinary script field, not a native-only member. It is not declared on `WillowGlobals` itself, which is why a
look at `WillowGlobals`' own fields finds only `DefaultGlobalsDefinitionName`.
- **Where it is declared:** on `GearboxFramework.GearboxGlobals`, the parent class of `WillowGame.WillowGlobals`.
  `WillowGlobals` inherits it. Read from the installed `GearboxFramework.upk` export table: an `ObjectProperty` named
  `TheGlobalsDefinition` whose outer is the `GearboxGlobals` class. Beside it, `ThePopulationMaster` is another
  `ObjectProperty` on the same class.
- **Its declared type:** an object reference to `GearboxFramework.GearboxGlobalsDefinition`. The installed
  `WillowGame.GlobalsDefinition` class has `GearboxGlobalsDefinition` as its parent, so a `GlobalsDefinition` fits the
  field. The getter's own test is stricter than the type: section 4's class check asks for `GlobalsDefinition`.
- **`DefaultGlobalsDefinitionName`** (a string, declared on `WillowGlobals`) is not the holder. It is the configured
  object name that section 4's first load reads, and Off never runs that load.
- **The native getter reads and writes this field.** That is section 4's reading, and the live checks below confirm
  it: the field read through the SDK must be the object the getter returns. Until then it is UNVERIFIED.

**How Off fills it:** look the field up on the instance's class through its parent chain, not on `WillowGlobals`' own
declarations. Then write the object reference of the installed `GD_Globals.General.Globals` into it at world set-up
(6.1, step 2), as a normal property write on the instance. A port whose class lookup stops at the most-derived class
will miss the field. That is a lookup bug, not a missing field, and must not be answered by storing the definition
anywhere else (a side table, bridge state, or `DefaultGlobalsDefinitionName`).

### 6.1 World set-up (before the run)

The world the pickup driver runs in must, at set-up:
1. make one `WillowGame.WillowGlobals` instance and record it as the world's globals object (the port's
   `WorldQuery::set_globals` holds such a slot);
2. **write** the installed `GD_Globals.General.Globals` into that object's `TheGlobalsDefinition` field, the field
   inherited from `GearboxGlobals` (section 6.0).
   - The mission bridge's loader (`ow-mission`'s `globals_definition`) finds the same object but caches it in bridge
     state only. Reusing it alone leaves `TheGlobalsDefinition` unset, so `GetGlobalsDefinition` would meet the first
     load of section 4.
   - The loader may be reused to find the object, but the field must be written.
   - **Preloading serves the pickup route's reads only.** The challenge numbering of the first load (section 4,
     step 3) stays unimplemented. In the game it runs at that first load; in the port it does not run at all, and no
     answer here may stand in for it.
3. leave `ThePopulationMaster` (declared on `GearboxGlobals`) at None. This is the supported headless fixture's
   value (section 6.3, step 6).

### 6.2 The drivers' preflight (before the run's first write)

Each item is checked before the gun is made. Each failure is returned as it is, with no prefix, because nothing has
been written yet.
1. **The globals instance:** the world records a globals object. None recorded: *layer absent*, "the world's globals
   object".
2. **Its class:** the object is a `WillowGame.WillowGlobals`, or a subclass. Otherwise: *data error*.
3. **The definition:** `TheGlobalsDefinition` holds an object whose class is `GlobalsDefinition` or a subclass. None:
   *not read*, "the globals definition's first load, which numbers the challenge list". The world set-up of 6.1 did
   not fill it, and Off does not run that load. Another class: the same *not read*, because the game would reload in
   that case too.
4. **The schemas the route reads,** on the declared classes:
   - `PickupList` on the globals' class: declared, and a dynamic array whose element is an object reference to
     `WillowGame.WillowPickup` (section 3; the installed declaration's element type). Any other element type,
     another object class included: *data error*;
   - `RarityLevelColors` on the definition's class: declared, and a dynamic array of the `RarityLevelColor` struct.
     That struct must declare `MinLevel` and `MaxLevel` as ints and `DropLifeSpanType` as a byte. Otherwise: *data
     error*;
   - `ThePopulationMaster` on the globals' class: declared, and an object reference. Otherwise: *data error*.
   - `TheGlobalsDefinition` on the globals' class: declared, through the parent chain (section 6.0), and an object
     reference to `GearboxFramework.GearboxGlobalsDefinition`, its declared type. Any other type, another object
     class included: *data error*.

   Every schema here is checked before any field value is read, step 3's included (steps 1 and 2 read only the
   world's slot and the instance's class). A declaration of the wrong shape or reference target is a `VmError::Script`
   before the run's first write, so it cannot pass the preflight and then fail after writes.

### 6.3 The natives, at call time

Only what cannot be known before the run's first write is checked here.
1. **`GetWillowGlobals` and `GetGearboxGlobals`:** each answers the world's globals object, the same on every call,
   and writes nothing. **With no world globals object** (a world the preflight did not check), each answers *layer
   absent*, "the world's globals object". Neither answers None, because None would let `PostBeginPlay` skip
   `AddPickup` silently. On the pickup route this comes after the first write, so it carries the prefix.
2. **`AddPickup(P)`:** appends `P` as given, None included, at the end of `PickupList`. It does no duplicate check
   and fires no event. The schema was checked in the preflight. A world that skipped the preflight and fails the
   schema gets the same *data error*, raised before the append.
3. **`GetGlobalsDefinition`:** answers `TheGlobalsDefinition` when it holds a `GlobalsDefinition`. Otherwise it
   answers 6.2 step 3's *not read*, with the prefix after the first write. Off never runs the first load.
4. **`GetDropLifeSpanType(L)`:** section 5's rule, a pure read of the entries as they stand. The schema was checked
   in the preflight.
5. **The HUD loops** (`AddPostRenderedActor`, `RemovePostRenderedActor`): they run only for a local player
   controller whose `myHUD` is a `WillowHUD`. The headless world has none, so they call nothing.
6. **`GetPopulationMaster`:** answers `ThePopulationMaster` as it stands.
   - **The supported fixture's value is None.** `Destroyed` then skips `RemoveSpawnedActor`, as the game does with
     None.
   - **A non-None master** (a world that set one) makes `Destroyed` call `RemoveSpawnedActor(Self, false)` on it.
     That is a native of `PopulationMaster` (and of `WillowPopulationMaster`) not read here, so it answers *not read*,
     "PopulationMaster.RemoveSpawnedActor". On the destroy path that comes after the first write, so it carries the
     prefix.
7. **`RemovePickup(P)`.** In the game, the native:
   1. dispatches `OnPickupDetached`;
   2. checks the pickup's `Base` (the actor it is based on). When there is a `Base`, it walks a list held by that
      actor, each entry mapped to its pickup by an engine helper that was not read. It dispatches
      `OnAllPickupsDetached` unless one of those entries is a pickup other than this one. So that event fires when
      this was the last pickup on that holder. With no `Base`, it does not fire. Medium confidence: the list's name
      and the helper were not read;
   3. removes every entry equal to `P` from `PickupList`, keeping the others in order.

   The dispatch was read further in NATIVE_LINK6_AFTER_TAKE.md section 2 (2026-11-26). The events go to the
   pickup's `Base` as a behavior consumer, the list is the base's `Attached`, and the helper is a `WillowPickup` class
   test. That note's Off answer replaces the one below.

   **Off answer:** the event dispatch is not ported, so the native answers *not read*, "the pickup events of
   RemovePickup". It does so **before any dispatch and before any change to `PickupList`**, so the list still holds the
   pickup. On the destroy path that comes after the first write, so it carries the prefix. A dropped pickup in the
   slice has no `Base`, so once dispatch is ported it fires `OnPickupDetached` only.

**The sequence of `make`, once the two getters answer** (section 1's script order):
1. `AddPickup`;
2. `Actor.LocalPlayerControllers`, bound in the port; no HUD, so nothing is called;
3. the pickup's `InitializeFromInventory`, with `GetManufacturer` and `GetInventoryDefinition` (both bound). The
   `MissionTracker` reads happen only for a mission item;
4. `InitializeDropLifespan` (script):
   - the kind: a mission item (`bIsMissionItem`) takes kind 3 and calls neither getter; any other pickup takes
     `GetGlobalsDefinition` then `GetDropLifeSpanType` of its `InventoryRarityLevel`;
   - the span: the inventory definition's `PickupLifeSpan` when the pickup is not a mission item and that value is
     above 0; otherwise the kind's (section 5);
   - the actor's `LifeSpan` is set to the span;
   - **only when `LifeSpan` is above 0** (strictly), `SetTimer` is called with the delay `LifeSpan -
     PickupShrinkDuration`, not looping, for `ShrinkPickupBeforeDestruction`. With a span of 0 there is no timer.
     What a delay of 0 or below does (a span no longer than `PickupShrinkDuration`) is the timer native's rule. It
     is **UNVERIFIED** here: this note does not say it is clamped, cancelled or fired at once;
5. `InventoryAssociated` (the mesh and physics set-up, not read here);
6. `SetPickupability`, with `TryToSetInteractParticles` reading `GlobalsDefinition.PartPickup`.

What follows in `touch` and `key` is NATIVE_PICKUP.md's.

## Open questions

1. Where and when the engine creates the globals instance and stores its pointer (not found, as before).
2. What the first load of the globals definition writes into the challenge list (which fields, and what the two kinds
   mean), and whether script reads it. The port does not implement it (section 6.1).
3. Whether a later slice world should hold a population master. For now the supported fixture holds none
   (section 6.3, step 6).
4. The event dispatch inside `RemovePickup`: what listens to `OnPickupDetached` and `OnAllPickupsDetached`, and the
   list and helper behind the same-holder test (section 6.3, step 7).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- **The definition's field.** On the globals object, read the property `TheGlobalsDefinition` by name through the
  SDK. Expect it found on `GearboxGlobals` (not on `WillowGlobals`), and expect its value to be the same object
  (identity) that `GetGlobalsDefinition()` returns (section 6.0).
- **One object.** Call `GetWillowGlobals()` and `GetGearboxGlobals()` from two different classes, on the title screen
  and in a map. Expect the same `WillowGlobals` object every time.
- **The definition's load, cold, then before, after and repeated.**
  - From a cold start (the game just launched, before the title screen's first globals-definition read, if a hook can
    run that early), read `TheGlobalsDefinition`. Expect None, or the definition if an earlier engine step loaded it.
  - Hook `GetGlobalsDefinition`. On its first call, read `TheGlobalsDefinition` and the challenge list's indices
    before and after.
  - **Pass condition for the first call**, all three required:
    1. `TheGlobalsDefinition` is None when the call starts;
    2. after the first call that returns, `TheGlobalsDefinition` holds the loaded object, and the call returned that
       same object (identity);
    3. the challenge writes of section 4, step 3, are seen on that call and not before it. This part can be judged
       only once the written fields are named (open question 2).
  - Call it again, and expect the same object and no further change to the indices.
  - If the hook cannot run before the first load, or the challenge fields cannot be named and observed, the cold
    capture is an open verification gap, and the challenge numbering stays **UNVERIFIED**.
- **The pickup list on add.** Read `PickupList` before and after dropping a gun: expect one more entry, at the end,
  that is the new pickup. Call `AddPickup` twice on one scratch pickup: expect two entries. Call `AddPickup(None)`:
  expect one more entry, holding None.
- **The pickup list on removal, with the events sampled.** Hook `OnPickupDetached` and `OnAllPickupsDetached` where
  the SDK exposes them (they are global events; otherwise hook the callers it can see). In each hook, read
  `PickupList`.
  - **A pickup with no `Base`** (a dropped gun): expect `OnPickupDetached` only, with the pickup still in the list
    when it fires, and gone after `RemovePickup` returns.
  - **Two pickups on one holder** (two items in one opened container, if they share a `Base`): take one, and expect
    `OnPickupDetached` only. Take the other, and expect both events.
  - Expect every entry of the removed pickup gone, and the others in order. Where no shared-holder case can be set up,
    that case is an open verification gap.
- **The rarity bands.** Read `RarityLevelColors`. Then call `GetDropLifeSpanType` for:
  - a level inside each band;
  - each band's `MinLevel` and `MaxLevel` (both ends are included);
  - a level between bands, if any, and one above the last band;
  - a level held by two overlapping bands, if any exist (expect the earlier band's kind).

  Expect the band's kind, and 3 outside every band. On a dropped gun, compare `LifeSpanType`, `LifeSpan` and the timer
  with section 6.3's sequence. If no bands overlap in the installed data, the overlap case is open.
- **The population master.** Read the `ThePopulationMaster` field and `GetPopulationMaster()`, and expect the same
  object (identity). Do it in a map with a population (non-None expected), and on the title screen (None, if the game
  holds none there). Hook `RemoveSpawnedActor` while a pickup is taken in the map. The None case on the destroy path
  may not be reachable live; it then stays open.
- **Accessed none.** Not checkable live without breaking the globals, which the game never lacks; it stays a script
  rule.
