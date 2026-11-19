## 2026-11-19: Rust Willow globals natives in Off

AI-assisted (Claude Code). Lane IMPL-WILLOW-GLOBALS, implementer (slice links 6 and 7, the `make` step). Number 94 was
given by the hub.
- **The branch.** Stacked on `origin/fix/rust-pickup` 21e1a2b8 (#452), which has the pickup drivers and their
  preflight.
- **The source.** NATIVE_WILLOW_GLOBALS.md at `research/rust-willow-globals` 7876ae77 (#469; analyst lane, decision
  2026-11-19-85), the head on origin when this lane handed back. Its round-1 revision was not pushed yet. The rules
  were coded from that note alone.
- **Clean room.** This lane did not read the natives and opened no `local/analysis/`, Ghidra or REA output for them.
  It read the installed packages only for field names and declared types (`tools/ghidra/class_layout.py`,
  `tools/ghidra/script_natives.py`).
- **No dependency, no parsing change.** No real-game session; every rule is **UNVERIFIED** in the running game.
  Reference is unchanged: nothing is bound there.

**New module `ow_inventory::globals`** (Off, registered by the inventory pack; each native bound by its path and
skipped when its key is already bound, as the mission bridge binds the two getters in its own world):
- `WillowGlobals.GetWillowGlobals()` and `GearboxGlobals.GetGearboxGlobals()`: the world's one globals object
  (`WorldQuery::set_globals`). None held is the world's `NotYet { LayerAbsent }`, never None.
- `WillowGlobals.AddPickup(Pickup)`: appended to `PickupList` as given, None included, no duplicate check, no event.
  A `PickupList` not declared as a dynamic array of object references is a data error before the append.
- `WillowGlobals.GetGlobalsDefinition()`: `TheGlobalsDefinition` (declared on `GearboxGlobals`) when it holds a
  `GlobalsDefinition`. Otherwise `NotYet { NotRead }`: the first load numbers the challenge list, which was not read.
- `GlobalsDefinition.GetDropLifeSpanType(RarityLevel)`: the first `RarityLevelColors` band with `MinLevel <= level <=
  MaxLevel` gives its `DropLifeSpanType`, else 3. The declared schema (ints and a byte) is checked.
- `GearboxGlobals.GetPopulationMaster()`: `ThePopulationMaster`, as it is.
- `WillowGlobals.RemovePickup` keeps `ow_inventory::pickup`'s `NotYet { NotRead }` before any change, as the note's
  section 6.3 step 7 says.

**The pickup world and the preflight** (`ow-cli`):
- `equip_run::pickup_world` is the equip run's world with the globals object made at world set-up: a `WillowGlobals`
  from its class defaults, whose `TheGlobalsDefinition` is the installed `GD_Globals.General.Globals` (the configured
  `DefaultGlobalsDefinitionName`, the note's section 2), recorded with `WorldQuery::set_globals`. Only links 6 and 7
  use it; link 8's world is unchanged.
- `slice_pickup`'s preflight, before any write, now checks:
  - the world's globals object, a `WillowGlobals`;
  - its definition;
  - the declared schemas of `PickupList` and `RarityLevelColors`;
  - that the four natives on the route (`GetWillowGlobals`, `AddPickup`, `GetGlobalsDefinition`,
    `GetDropLifeSpanType`) are declared native and bound.

**Tests** (synthetic, `ow_package::testkit`):
- 7 in `globals_tests.rs`: the getters with and without a world object, appends with duplicates and None, the
  declared-schema errors, the definition read and its `NotYet`, the band rule with ends and gaps, the population
  master, and Reference binding nothing;
- 1 in `slice_pickup_tests.rs`: the preflight refuses a world without globals.

**Verified** (first revision, cf1d7e02's content at d15ff339). The host suites only, plus the installed Off
`--slice-gate`, strict, under the machine lock:
- 0 ok, 12 not yet, 2 error of 14, as before;
- links 6 and 7 still stop at `make` on the unbound `WillowInventory.GetInventoryDefinition()`, #452's preflight
  check, now after the globals checks pass on installed data.

With #462 merged locally (not pushed), links 6 and 7 pass the preflight and the globals calls in `make`, and stop at
the next unbound native, `DroppedPickup.GetPickupableInventoryDefinition()` (an `IPickupable` accessor,
NATIVE_INVENTORY_EQUIP.md).

**Open.**
- The note's round-1 revision (#469): done, below.
- `RemovePickup`'s two global events.
- The globals definition's first load.
- Whether the headless world holds a population master.
- The `IPickupable` accessors, the next stop.

**To the revised note** (#469 r1 at c5d21b49, plus the hub's notice of its last round):
- **The preflight's schemas** come first, before any value is read, and each mismatch is a data error
  (`VmError::Script`):
  - `TheGlobalsDefinition` is found through the parent chain and must be declared as a `GearboxGlobalsDefinition`;
  - `ThePopulationMaster` must be an object reference;
  - `PickupList` must be a dynamic array of `WillowPickup` references;
  - the definition's `RarityLevelColors` as before.
- **The note's texts and record:**
  - no world globals object is *layer absent* "the world's globals object";
  - the definition is *not read* "the globals definition's first load, which numbers the challenge list";
  - `RemovePickup` is *not read* "the pickup events of RemovePickup", before any dispatch or change to `PickupList`.
- **`PopulationMaster.RemoveSpawnedActor`** and the `WillowPopulationMaster` declaration are bound to *not read*,
  for a world that holds a master. The supported fixture holds none (section 6.3, step 6).
- **The `SetTimer` delay** for a span of zero or less is the script's and the timer service's; nothing here answers
  it.
- **Tests:** 9 globals tests (new: the declared reference types, and the getters' text without globals) and the
  `RemovePickup` record.

**To #469 r2** (f7291a3b): `RarityLevelColors` was checked on the declared base `GlobalsDefinition` class before
`TheGlobalsDefinition` is read, and the held object's concrete class only after. That order is corrected below (review
r1 of #476). The exact reference targets (`WillowPickup` elements, a `GearboxGlobalsDefinition` field) were already
checked. The `SetTimer` delay of zero or below is the script's and the timer native's: UNVERIFIED there, and nothing
here answers it. The installed gate at 71f80ca1 gave the same rows as above.

**Rebased** onto main (#452 landed): `git rebase --onto origin/main 21e1a2b8`.

**Review r1 of #476** (a fix commit on 71f80ca1):
- **`AddPickup`** checks the declared element class itself (exactly `WillowPickup`) before it reads the list, so a
  caller that skips the preflight cannot append to a `PickupList` of another class.
- **The rarity schema** wants the declared `GlobalsDefinition.RarityLevelColor` struct, and scalar `MinLevel`,
  `MaxLevel` and `DropLifeSpanType`.
- **The set-up prepares, and the preflight loads nothing.** `pickup_world` loads the weapon class's default object and
  the installed globals definition (the allowed preload), and hands the preflight a `PickupSetup` with the default
  object and the definition's concrete class.
  - The preflight first checks the declarations it will read: the globals' schemas, then `RarityLevelColors` on that
    concrete class.
  - Then it reads values: `DroppedPickupClass` from the loaded default, `TheGlobalsDefinition`, and a check that the
    held definition is of the set-up's class.
  - So the order is: the world's globals object and its class (the slot and the instance's class), then every
    declaration, then the values.
- **A lazy definition handle** is a named `NotYet { NotRead }` in `GetGlobalsDefinition` and `GetDropLifeSpanType`,
  checked with `DefStore::is_unloaded`, with no load (a lazy load is a write). No synthetic fixture makes a store
  handle unloaded cheaply here, so that guard has no test of its own.
- **Typed set-up errors.** `pickup_world` returns `VmError` (a typed `run_world_t`), and `run_pickup` grades its
  failure with `classify_error`, not by text.
- **`GetGlobalsDefinition`** reads the validated holder first: None is the first-load `NotYet`, and the
  `GlobalsDefinition` class is resolved only for a non-empty holder.
- **Tests:** globals 11 (new: `AddPickup` on a list of another element class, and a same-named struct that is not
  `RarityLevelColor`).

