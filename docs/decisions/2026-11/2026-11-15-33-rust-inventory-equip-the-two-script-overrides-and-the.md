## 2026-11-15: Rust inventory: equip, the two script overrides and the manager natives

AI-assisted (Claude Code). Lane W5-EQUIP of the Rust port fills `crates/ow-inventory/src/{manager,equip,spawn}.rs`
(with their test files under `manager/`, `equip/` and `spawn/`) and the `--inventory-move` command
(`crates/ow-cli/src/cmd/inventory.rs`), architecture section 8, Wave 5. No parsing behaviour changes. It builds on
decisions 2026-11-15-13 (attribute effects), 2026-11-15-24 (item stats), 2026-11-15-26 (the world) and 2026-11-15-21
(the roll). Nothing here was checked in the running game: every rule is **UNVERIFIED** there.

- **Edits outside the lane's files** (each granted by the hub): `tests/vm_test.py`'s inventory fixture gives its
  `CachedObjects` array an inner property (an `ObjectProperty` of the fixture's `Object` class, the next export), the
  same fixture gap W4-INTERP2 fixed for the other arrays; no assertion changed, and the C++ binary passes the file as
  before. `tests/rust_parity.d/inventory.txt` is new. No seam edit in `ow-world` was needed: W5-WORLD's query service
  already keys the two inventory kinds (`QueryKind::EquippedInventory`, `QueryKind::Inventory`).
- **`--inventory-move`** is the C++ adapter (`src/inventory_navigation.cpp`) on the Rust VM: the installed
  `InventoryListPanelGFxObject.MoveDelta` on an item-only list, with the getter `GetEntryKindAtIndex` bound to answer
  the position of `EAK_Source` in its return enum. Every C++ check and text is kept. Differences, none of which a
  tracked test or the comparison below meets:
  - The getter's enum is read from its export as the C++ reads it, before the getter's signature is checked. The Rust
    reflection types a function's parameters with the enum, so a bad enum leaves the getter declared without
    parameters and the signature check would name the wrong cause. A package that breaks both reports the enum.
  - `MoveDelta`'s bytecode is decoded at the first call, not when the adapter opens.
  - The stub policy is `Refuse` (architecture 3.8): an unbound native stops a move with the VM's text where the C++
    logs a stub and reports the log line. Both fail the move.
- **The two overrides** (`equip::bind_overrides`, architecture rule R2):
  - `Engine.AttributeEffect.ApplyAttributeEffects` is `ow-attr`'s `apply_effects`. The installed script appends its
    records to `OutModifiers` and never empties it, so the override appends too.
  - `WillowGame.WillowWeapon.CalculateWeaponBaseValues` is `ow_items::stats::weapon_base_values` on the weapon and
    its `DefinitionData.WeaponTypeDefinition`. The script writes each base with the attribute let (opcode 0x5F), which
    this VM recomputes in `CombineForm::Vm5F`, so the override passes `Vm5F` too. A weapon without a type is refused
    (`BadData`); the script would read every field through a None context and write zeros.
  - Each function is loaded before it is bound (`Vm::override_script` takes only a declared script function).
- **Equip and unequip** (read from the installed script with `research/script_disasm.py`, 2026-11-16; listings under
  ignored `local/analysis/`):
  - An item's external effects: the source is the instigator's `Controller`, or, when that is None, the owning player
    controller of the instigator's `WillowInventoryManager` (its script function, run). The override is the item.
    - Nothing happens while `ExternalAttributeModifiers` holds records.
    - Otherwise the definition's `ExternalAttributeEffects` go into `ExternalAttributeModifiers`. When
      `PartExternalAttributeModifiers` is empty, the parts' go into it.
    - Then the external attribute slots.
  - A weapon's: the same guard; the type's and every part's effects into `ExternalAttributeModifiers`, with
    `Instigator.Controller` itself as the source (the weapon's script does not use the owning-controller lookup for
    its effects).
  - Unequip removes each record's modifier from its context, then empties the arrays. The records live in the item's
    own arrays as `AppliedAttributeEffect` structs, where the script keeps them, so a script and a Rust remove see the
    same records.
  - The parts are the definition data's part fields in declared order, the material included and the name parts not.
    The script walks them with the native iterator `AllParts`, whose order and `bool` argument are not read.
  - The external slots (`WillowInventory.ApplyExternalSlotEffectModifiers`, NATIVE_ATTRIBUTES.md section 9) read the
    item's slot list and each slot's computed value, made by `InitializeAttributeSlots`, which is not read. A
    definition that lists an external slot not run as a skill is `NotYet { NotRead }`; a definition that lists none
    gives the native nothing to apply, and it does nothing (this reading is UNVERIFIED).
- **Inventory manager natives** (NATIVE_INVENTORY_EQUIP.md; the fields were checked against the installed classes
  through the Rust reflection):
  - `CountUnreadiedInventory` answers the stored `BackpackInventoryCount`, and `GetUnreadiedInventoryMaxSize` the
    stored `InventorySlotMax_Misc`.
  - `SetInventoryMaxSize` stores 12 for a smaller size unless `bOverrideDefaultMin`.
  - `CountReadiedWeapons` counts the readied `WillowWeapon` entries along `InventoryChain`. A chain that comes back to
    an entry it passed is -1; the record says "loops back to itself", and any revisit is taken here.
  - `GetWeaponReadyMax` answers the `WeaponReadyMax` value, its flag ignored.
  - Not registered: `FindLeastValuable*` (the eligibility virtual is not read), `ItemActors`, the holster natives.
- **Engine queries.** `EquippedInventory { location }`: a controller's or pawn's pawn must hold a
  `WillowInventoryManager`, and the answer is the pawn's `EquippedItems[location]` (0 to 3; 4 answers None). The
  record says "the item the source's inventory manager holds at the location"; that this lands on the array
  `WillowPawn.EquipItem` fills is this reading, UNVERIFIED. Another source is `NotYet { NotRead }`. `Inventory` is
  `NotYet { NotRead }`: the record names a lookup through the source's interface and nothing more.
- **Spawn natives** (NATIVE_LOOT.md). `SpawnBalancedInventoryFromInventoryBalanceDefinition` expands the balance at
  `min(stage, cap)` (`ow_items::candidates::expand`), picks `Quantity` times on `CrtStream::LootPool`
  (`pick_candidate`), and spawns one inventory per pick. One spawn:
  - the definition's `InventoryClass` is spawned and must be a `WillowInventory`;
  - `SourceDefinitionName` is the balance's name;
  - then `SetGameStage`, `SetAwesomeLevel`, the `InitializeInventory` event, and `bDropOnDeath` true.
  - Two choices where the record is silent: the actor is spawned by the level's `WorldInfo`, and
    `SourceResponsibleName` is not written (the record does not say which field of the context source it is taken
    from; it is telemetry only).
  - The registry and roll data are a VM service the host installs (`spawn::install`).
  - `SpawnBalancedInventoryFromPool` is `NotYet { NotPorted }`. W4-POOLS' `roll_pool` (decision 2026-11-15-25)
    takes the snapshot's stage, registry variance data and no context source, and returns every drop before any spawn;
    the native needs the caller's stage, its variance definition and context source, and a spawn inside the pick
    loop. A contract item for the hub.
  - Store objects are keyed by the store's naming rule (`ow_defs::identity::full_path`, crate-private there, restated
    in `spawn::object_path`).
- **The differential test** (Rust-only form of the command, `<package> --inventory-move --script-diff <weapons file>
  --cooked <dir> --output <file>`). Two worlds over the installed game: on one the installed script runs, on the
  other `ow_inventory::manager::Pack` binds the two overrides. Each weapon is a `WillowWeapon` from its class defaults
  with its definition data and level. Both worlds run `CalculateWeaponBaseValues`, then `ApplyAttributeEffects` with
  the weapon as the source once per holder (the type, then each part) on the holder's `WeaponAttributeEffects`.
  Compared: every numeric and array property of the weapon (floats bit for bit, a modifier by its type and value),
  each call's records and errors. Both worlds bind the three `AttributeDefinitionBase` natives the installed script
  calls (`ResolveContext`, `AddAttributeModifier`, `RemoveAttributeModifier`) to `ow-attr`, in the harness only.
- **Verified on the installed game** (pc-hummer, 2026-11-16; outputs under ignored `local/p2/W5-EQUIP/`):
  - The differential test on 50 weapons: 50 balances spread over the 249 weapon balances, stages 1 to 50, rolled
    with this branch's Reference `--item-card --requests`; 31 weapon types. **50 of 50 identical**, 19,610 values
    compared, 369 records made on each side, every weapon with records. The script side ran at least 208 bytecode
    steps per weapon, the override 1. Two deliberate breaks of the Rust side were each caught on all 50 weapons
    (bases not written; the last effect of each call dropped).
  - `--inventory-move`, C++ against Rust on the installed `WillowGame.upk`: 126 requests (counts 1 to 12, 40 and
    2048, both deltas, starts at both ends and the middle, and six refused requests), all identical.
  - The native keys the pack registers equal the keys the Rust loader derives from the installed packages.
- **Verified on invented data.**
  - `--inventory-move`, C++ against Rust, byte for byte (stdout, stderr, exit code): 546 runs on `tests/vm_test.py`'s
    package and seven broken variants of its enum (count 100000, 0 and -1, no source entry, two source entries, the
    source first, a trailing byte), all identical.
  - `tests/vm_test.py` on the Rust binary: its 11 `--inventory-move` sites pass. The file still fails 3 other sites
    (`MakePair`, a W3-DEFS follow-up; two `--mover-probe` sites, lane W5-DIALOG).
  - 27 `ow-inventory` unit tests on a VM built by hand:
    - the overrides through declared script functions (records appended, the override source first, a VM without
      the functions refused, a typeless weapon refused);
    - item and weapon equip and unequip, the guards, the part guard and the external-slot `NotYet`;
    - the equip controller;
    - the five manager natives, the readied-weapon loop, both queries;
    - the spawn natives' parts that need no store (the registry value reader, the cap, the missing layers, the pool
      `NotYet`, a None balance).
  - 3 `ow-cli` tests of the harness's input and comparison.
- **Open.**
  - A tracked parity line for `--inventory-move` on an invented package needs a tracked test file that runs only that
    command (none may be added in this lane); `tests/rust_parity.d/inventory.txt` holds the two failures before any
    package is read.
  - No lane binds the `AttributeDefinitionBase` natives (`ResolveContext`, `ResolveContexts`, `AddAttributeModifier`,
    `RemoveAttributeModifier`, `SetAttributeBaseValue`, the value getters) in a pack; script that calls them stops at
    `UnboundNative` in a gameplay world.
  - `ow-defs`' object naming rule (`identity::full_path`) is crate-private; `spawn::object_path` restates it.
  - The spawn natives on the installed game need a level load (its `WorldInfo`) and the natives `InitializeInventory`
    reaches. Only unit tests cover them.
  - Binding `SpawnBalancedInventoryFromPool` over `ow_items::pools` (the contract item above).
  - `AllParts`' order and argument, `InitializeAttributeSlots` and the external slots, `FindLeastValuable*`,
    `ItemActors`, where the spawn native places an actor, the field `SourceResponsibleName` is taken from.
