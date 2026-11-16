## 2026-11-15: Rust item pools: the oracle's CrtRand path and the native pool roll in Off mode

AI-assisted (Claude Code). Lane W4-POOLS of the Rust port fills `crates/ow-items/src/pools.rs` and the `--item-pool`
command (`crates/ow-cli/src/cmd/items_pools.rs`), architecture section 3.7. No parsing behaviour changes. The number 25
was reserved for this lane by the hub. This file records the choices a later lane (W6-LOOT first) should be able to
find.

- **Edits outside the lane's own files** (granted in the lane brief): `tests/item_pools_rust_test.py`,
  `tests/rust_parity.d/pools.txt` and the `item-pools-rust` entry in `tests/CMakeLists.txt`. The ctest entry sets
  `OW_PARITY=reference`, as the manifest line's `mode=reference` marker does. ARCHITECTURE.md is not edited; the
  shipped shapes below differ from its section 3.7 line for `roll_pool` (see "Contract").
- **Two modes, the mode a value in `RollCx`.**
  - `ParityMode::Reference` reproduces `tools/loot_pools.py`'s CrtRand path (`roll_crt` / `roll_pool_crt`), called as
    `tools/weapon_gen_parity.py` `pool_drops` calls them: a list or pawn balance through `table` and `roll_crt`,
    anything else through `expand` and `roll_pool_crt`. It reads values as the tool's `Values` does, from one package's
    tag streams with the tool's schema lines (`PoolData::one_file`, which keeps the package's export order, so the
    resolver subobjects are scanned in the tool's order). The schema has one line the tool lacks,
    `ItemPoolIncludedLists=ObjectProperty`, for Off mode; it types only that array, which the tool never reads.
    Numbers keep Python's int and float kinds, because a pool total of int and float weights depends on how `sum()`
    adds an int into a float total. The `random.Random` path (`roll` / `roll_pool`) is not ported: it is not an
    oracle.
  - **The oracle's interpreter.** CPython 3.12 adds an int into a float total plainly; 3.14 compensates it as it does
    a float (`sum([0.1, 2, 0.2, 5])` is `0x1.d333333333334p+2` on 3.12.10 and `0x1.d333333333333p+2` on 3.14.7; on
    20,000 random mixed lists each interpreter matched its own rule every time and the other's 380 times less).
    `PySum` names the rule; `--python-sum 3.12|3.14` (default 3.14) passes it. The match holds only for ints within
    a C `long`: CPython adds the leading ints in a `long`, which is 32 bits on Windows, and beyond it switches to big
    ints and a generic loop that adds later floats with no compensation (review r2 found 520 misses in 50,000 such
    lists on 3.14.6). That path is not emulated: an int item, or the running total of the leading ints, outside the
    `i32` range is refused (`BadData`). Every weight on the installed game is far inside it. The tracked test reads its own
    interpreter's rule, checks it with that probe and fails, saying so, when the interpreter follows neither. Its
    fixture has a pool whose int and float weights sum differently under the two rules; on that fixture and on the
    installed game no drop depends on the rule (a last-bit change in a share rarely moves a pick). ctest on pc-hummer
    runs Python 3.14.7; the CI workflow (`.github/workflows`) pins `python-version: "3.12"`.
  - The tool's rules, reproduced only under Reference and UNVERIFIED as the game's: a listed pool fires when its
    probability is at least 1 or `rand()/32767` is below it; a pool draws `round_half_even(Quantity)` times (a zero or
    missing quantity counts as 1, a negative one as 0); every listed pool of a source fires, with no `bAutoReadyItems`
    split; no manufacturer is drawn and no item is rolled.
  - `ParityMode::Off` is `ItemPool.SpawnBalancedInventoryFromPool` as NATIVE_LOOT.md reads it from native code,
    UNVERIFIED in game: the gate with the truncated stage requirements and the cap, the reachable-entry test, one flat
    candidate list with W4-ROLL's pool-level manufacturer expansion (`candidates::expand`) and the weight floor, the
    caller's variance and its clamp to the pool window, the truncated quantity, the single-precision draw and the
    drop flag carried down. A source's pool list follows `SetupPawnItemPoolList` and `AddToItemPoolList`, and the
    death roll keeps the pools whose `bAutoReadyItems` is false and fires one when its chance is above 0 and
    `FRand() <= chance` (single precision, drawn only for a positive chance). Every value goes through the `ow-attr`
    evaluator with no context source.
  - Off mode returns each drop's pool-level candidate (manufacturer, definition, stage, level, grade-or-level word,
    weight). Rolling the item is the caller's: it instantiates the item and calls `roll::roll_candidate`
    (architecture section 3.7).
- **Off mode reads an untagged field along the class-default chain.** `Quantity`, the stage requirements, the flags
  and the lists come from the first default object along the class chain that tags them:
  `Default__KeyedItemPoolDefinition`, then `Default__ItemPoolDefinition` (the keyed class's superclass, read from its
  class export in `WillowGame.upk`), and `Default__ItemPoolListDefinition` and `Default__AIPawnBalanceDefinition` for
  their own classes.
  - The installed game's `Default__ItemPoolDefinition` tags `Quantity` as the constant 1, and `bAutoReadyItems` and
    `bSupportsGameStageVariance` as true, as NATIVE_LOOT.md states. `Default__KeyedItemPoolDefinition` tags nothing.
  - When no default object of the chain tags the field: no object, `false`, an empty array, and `NotYet` for a struct
    (its zero value is not read). A chain step whose default object is not in the scan is `NotYet`.
  - A value that is tagged but was not decoded is `BadData`, never read as untagged (the registry's reading gives
    `None` for it, which for anything but an object reference can only mean "not decoded").
  - The registry's census reads no `Default__` path, so `PoolData::one_file` adds these objects under an alias key
    that no package exports.
  - An untagged entry field (`BalancedInventoryData.bDropOnDeath`) keeps its struct default, which is not read, so the
    drop's flag is `None` then.
  - Review r1 of PR #324 found the chain stopping one level short: a keyed pool read no `Quantity`, so 0, so nothing
    spawned. On the installed game all 83 `KeyedItemPoolDefinition`s in `Startup.upk` leave `Quantity` untagged. They
    also tag no `BalancedItems` (only `Key` and `bAutoReadyItems`), so they now answer `NotYet` (below) instead of
    spawning nothing.
- **Off-mode order and guards.** `Quantity` is evaluated before the gate (NATIVE_LOOT.md step 1), so a random
  quantity draws even for a pool the gate then closes. `gate`, the spawn and `AddToItemPoolList` keep the path of
  pools or lists above them; a pool or list already on it is `BadData` (the native has no cycle guard). A truncated
  `Quantity` above 10,000 is `BadData` (our bound, not the game's).
- **Off-mode gaps that answer `NotYet` instead of a value.**
  - The global class mod pools (`GD_Itempools.ClassModPools.`), empty in the packages and filled at run time
    (decision 2026-11-13-04: the trigger and the class identifier list are open).
  - The uncommon chest multiplier (`GetUncommonChestItemPoolWeightMultiplier`'s values; no pool in `Startup.upk`
    sets the flag).
  - The per-player ammo multiplier from playthrough index 2 (`GetAmmoDropsPerPlayerMultiplier`'s `BalanceModifiers`
    values). Below index 2, NATIVE_LOOT.md reads it as 1.0, which Off mode uses (UNVERIFIED).
  - A `KeyedItemPoolDefinition` with no entries of its own: how its `Key` yields items is not read.
  - An entry with no `Probability`, or a listed pool the death roll keeps with no `PoolProbability`: the struct
    defaults of `BalancedInventoryData` and `ItemPoolInfo` are not read (review r2; before, such a weight read as 0).
    On the installed game no entry has none: 0 of 19,396 pool entries and 0 of 6,868 list entries, over the 306
    packages that hold pools (2 of the 2,010 packages fail to list their exports and were not read).
  - The readied roll (`AddPoolItems`): whether it tests the pool probability is not read.
  - A single-precision draw that rounding leaves past every interval: what the native does then is not read.
  - Value resolver classes `ow-attr` does not register (on the installed game: `RandomAttributeValueResolver` and
    `AmmoDropWeightAttributeValueResolver`), as their own `NotYet`.
- **Off-mode gap that is not checked.** A balance entry whose balance the registry does not hold is `BadData`, not a
  skipped entry; none occurs on the installed game's parity pools.
- **Draw order (hub decision, review r1).** `roll_pool` keeps returning the drops as a `Vec`. The native spawns each
  item inside the pick loop: `InitializeInventory` runs the item's own roll before the next pick, so the game
  interleaves pool draws and item draws. Through this shape a caller rolls the items after the pool, so one generator
  cannot match the native's order. The game's sequence is not reproduced in either case (UNVERIFIED). The hub amends
  architecture section 3.7.
- **More than one package.** `PoolData::from_files` reads several files (owner the first holder in the given order)
  with the same default objects, for W6-LOOT beyond `Startup.upk`. It is not an oracle reading: the tool reads one
  package. On the installed game no `Startup.upk` list tags `ItemPoolIncludedLists`, so the included-list path
  (`StandardEnemyGunsAndGear`) is met only beyond it.
- **Contract.** As shipped: `pools::roll_pool(pools: &PoolData, reg: Option<&Registry>, source: &ObjectPath,
  request: &PoolRequest, cx: &mut RollCx) -> Result<Vec<PoolDrop>, ItemError>`, where `PoolRequest` names the listing
  (`Every`, Reference only; `OnDeath`, Off mode's default; `Readied`, `NotYet`) and an optional variance formula (Off
  only; Reference answers `ItemError::WrongMode`). Section 3.7's line returned `Vec<DefinitionData>`; a pool drop has
  no definition data until its item is instantiated, which is W4-STATS's `instantiate_stats` and the caller's
  `roll_candidate`.
- **CLI.** `--item-pool <pool path> --cooked <install> --stage <n> --seed <n> [--count <n>] [--playthrough <index>]
  [--op-level <n>] [--package <file>] [--python-sum 3.12|3.14]`, and the batch `--item-pool --parity <file>
  --cooked <install> [--seeds <n>]` for the parity requests. `--package` (default
  `WillowGame/CookedPCConsole/Startup.upk`, the oracle's package), `--parity` and `--python-sum` are modifiers this
  lane added; section 2.3's row lists the others. `--parity` refuses a file outside the workspace's `local/` (the
  folder with `Cargo.toml` above the binary), as the Python tools refuse such an `--output`. `--overlay` is `NotYet`
  (exit 3), as for `--item-roll`.
- **Verified on the installed game** (pc-hummer, 2026-11-15; outputs and the comparison script under ignored
  `local/p2/w4-pools/`; the script imports the tools and edits none; the oracle read the package with the Rust
  `ow-package`, the tools' default reader since decision 2026-11-15-17):
  - `OW_PARITY=reference --item-pool --parity` (16 s): 7,680 rolls, the 64 pools and lists below `.WeaponPools.` or of
    class `ItemPoolListDefinition` in `Startup.upk`, at stages 1, 8, 15, 30, 50 and 80, seeds 1 to 20.
  - They equal `loot_pools.roll_crt` / `roll_pool_crt` on 7,680 of 7,680 rolls (11,298 drops), 0 Python errors. The
    first run used Python 3.12.10. After review r1 the run was repeated once with Python 3.12.10 (`--python-sum
    3.12`) and once with 3.14.7 (`--python-sum 3.14`): 7,680 of 7,680 each, and the two Rust outputs are
    byte-identical.
  - Off mode on the same requests (no oracle): 6,360 rolls complete with 5,100 drops (4,980 with the drop flag true,
    120 false). 1,320 answer `NotYet`: 720 for the run-time class mod pools, 480 for
    `AmmoDropWeightAttributeValueResolver`, 120 for `RandomAttributeValueResolver`. The counts are the same after
    review r1: none of these 64 pools reaches a keyed pool or an included list.
- **Verified on an invented install.** `tests/item_pools_rust_test.py` is the tracked gate: its own invented install
  (13 parity pools and lists, a pawn balance and a pool outside `.WeaponPools.`), the Rust `--item-pool` in
  Reference mode compared with `loot_pools`' CrtRand path on the C++ reader, roll for roll. The manifest line
  `pools.txt` and ctest `item-pools-rust` run it; it passes under Python 3.12.10 and 3.14.7. Rust unit tests in
  `pools.rs` pin the Reference rules on the cases of `tests/loot_pools_test.py`, both sum rules on the measured probe,
  and each Off-mode rule above on synthetic data. Review r1 added tests for included lists, custom playthrough lists,
  a max window, the class-default chain, undecoded values, cycles, the quantity bound, the multipliers, the 1e-8
  floor, the in-place replace and a draw past every interval. Review r2 added tests for the `long` refusal (the reviewer's
  case), the Quantity drawn before a closed gate (exactly one draw), a diamond of shared pools and a list two lists
  include (both roll), and the untagged probabilities.
- **Not verified.**
  - Every rule, in the running game. The 7,680 rolls prove that the Rust roll keeps to the oracle's rules, not that
    the rules are the game's: the native truncates the quantity, tests `FRand() <= chance` and draws manufacturers at
    the pool level, which Off mode follows and the oracle does not.
  - The seeding is the tools' (one `CrtRand(seed)` per roll); the game's random sequence is not reproduced.
