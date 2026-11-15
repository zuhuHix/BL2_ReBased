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
    resolver subobjects are scanned in the tool's order). Numbers keep Python's int and float kinds: CPython 3.12's
    `sum()` adds an int into a float total without compensation, and a pool total of int and float weights depends
    on it. The `random.Random` path (`roll` / `roll_pool`) is not ported: it is not an oracle.
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
- **Off mode reads an untagged pool field from the class default object.** `Quantity`, the stage requirements and
  the flags come from `Default__ItemPoolDefinition` (and `Default__KeyedItemPoolDefinition`,
  `Default__ItemPoolListDefinition`) when the pool does not tag them. The installed game's default object
  (`WillowGame.upk`) tags `Quantity` as the constant 1 and `bAutoReadyItems` and `bSupportsGameStageVariance` as true,
  which agrees with the class defaults NATIVE_LOOT.md states. The registry's census reads no `Default__` path, so
  `PoolData::one_file` adds these objects under an alias key that no package exports. With no such object in the scan
  the field is `NotYet`. An untagged entry field (`BalancedInventoryData.bDropOnDeath`) keeps its struct default,
  which is not read, so the drop's flag is `None` then.
- **Off-mode gaps that answer `NotYet` instead of a value.**
  - The global class mod pools (`GD_Itempools.ClassModPools.`), empty in the packages and filled at run time
    (decision 2026-11-13-04: the trigger and the class identifier list are open).
  - The uncommon chest multiplier (`GetUncommonChestItemPoolWeightMultiplier`'s values; no pool in `Startup.upk`
    sets the flag) and the per-player ammo multiplier (`GetAmmoDropsPerPlayerMultiplier`).
  - The readied roll (`AddPoolItems`): whether it tests the pool probability is not read.
  - A single-precision draw that rounding leaves past every interval: what the native does then is not read.
  - Value resolver classes `ow-attr` does not register (on the installed game: `RandomAttributeValueResolver` and
    `AmmoDropWeightAttributeValueResolver`), as their own `NotYet`.
- **Off-mode gap that is not checked.** A balance entry whose balance the registry does not hold is `BadData`, not a
  skipped entry; none occurs on the installed game's parity pools.
- **Contract.** As shipped: `pools::roll_pool(pools: &PoolData, reg: Option<&Registry>, source: &ObjectPath,
  request: &PoolRequest, cx: &mut RollCx) -> Result<Vec<PoolDrop>, ItemError>`, where `PoolRequest` names the listing
  (`Every`, Reference only; `OnDeath`, Off mode's default; `Readied`, `NotYet`) and an optional variance formula (Off
  only; Reference answers `ItemError::WrongMode`). Section 3.7's line returned `Vec<DefinitionData>`; a pool drop has
  no definition data until its item is instantiated, which is W4-STATS's `instantiate_stats` and the caller's
  `roll_candidate`.
- **CLI.** `--item-pool <pool path> --cooked <install> --stage <n> --seed <n> [--count <n>] [--playthrough <index>]
  [--op-level <n>] [--package <file>]`, and the batch `--item-pool --parity <file> --cooked <install> [--seeds <n>]`
  for the parity requests. `--package` (default `WillowGame/CookedPCConsole/Startup.upk`, the oracle's package) and
  `--parity` are modifiers this lane added; section 2.3's row lists the others. `--overlay` is `NotYet` (exit 3), as
  for `--item-roll`.
- **Verified on the installed game** (pc-hummer, 2026-11-15; outputs and the comparison script under ignored
  `local/p2/w4-pools/`; the script imports the tools and edits none; the oracle read the package with the Rust
  `ow-package`, the tools' default reader since decision 2026-11-15-17):
  - `OW_PARITY=reference --item-pool --parity` (16 s): 7,680 rolls, the 64 pools and lists below `.WeaponPools.` or of
    class `ItemPoolListDefinition` in `Startup.upk`, at stages 1, 8, 15, 30, 50 and 80, seeds 1 to 20.
  - They equal `loot_pools.roll_crt` / `roll_pool_crt` on 7,680 of 7,680 rolls (11,298 drops), 0 Python errors.
  - Off mode on the same requests (no oracle): 6,360 rolls complete with 5,100 drops (4,980 with the drop flag true,
    120 false). 1,320 answer `NotYet`: 720 for the run-time class mod pools, 480 for
    `AmmoDropWeightAttributeValueResolver`, 120 for `RandomAttributeValueResolver`.
- **Verified on an invented install.** `tests/item_pools_rust_test.py` is the tracked gate: its own invented install
  (12 parity pools and lists, a pawn balance and a pool outside `.WeaponPools.`), the Rust `--item-pool` in
  Reference mode compared with `loot_pools`' CrtRand path on the C++ reader, roll for roll. The manifest line
  `pools.txt` and ctest `item-pools-rust` run it. Rust unit tests in `pools.rs` pin the Reference rules on the cases
  of `tests/loot_pools_test.py` and each Off-mode rule above on synthetic data.
- **Not verified.**
  - Every rule, in the running game. The 7,680 rolls prove that the Rust roll keeps to the oracle's rules, not that
    the rules are the game's: the native truncates the quantity, tests `FRand() <= chance` and draws manufacturers at
    the pool level, which Off mode follows and the oracle does not.
  - The seeding is the tools' (one `CrtRand(seed)` per roll); the game's random sequence is not reproduced.
