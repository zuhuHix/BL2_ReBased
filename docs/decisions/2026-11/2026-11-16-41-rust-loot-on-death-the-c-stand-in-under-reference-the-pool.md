## 2026-11-16: Rust loot on death: the C++ stand-in under Reference, the pool native NotYet in Off

AI-assisted (Claude Code). Lane W6-LOOT of the Rust port fills `crates/ow-combat/src/loot.rs` (with `loot/tests.rs`
and the invented packages' loot part in `loot/fixture.rs`) and the S-g steps of `--combat-run`, architecture section 8,
Wave 6. No parsing behaviour changes. The C++ reference is `src/combat_loot.cpp` and the S-g steps of
`src/combat_run.cpp`; the records are NATIVE_LOOT.md and COMBAT_LOOT.md. There was no real-game session in this lane.
**Every rule is UNVERIFIED in the running game** unless COMBAT_LOOT.md names the live row it reproduces; the C++ is
the oracle for every Reference text and number, which says nothing about the game.

- **Shape.** `loot::enable(vm)` (called by `run::Run::new` right after `death::enable_deaths`, as the C++ binds its
  loot natives inside `enableDeaths`; `loot::Pack` does the same for a host) adds the `LootState` service (the death
  rolls, `LootDrop` / `LootPool` / `LootItem`, the C++ record shapes), binds `ItemPool.SpawnBalancedInventoryFromPool`
  and `AIPawnBalanceDefinition.SetupPawnItemPoolList` where the package set declares them native (`hit::bind`),
  overrides `WillowPawn.DropLootOnDeath` in Reference mode only, and registers the loot-spawn behavior handlers.
  `loot::setup_loot` is the spawn's loot setup; `loot_json`, `loot_rows`, `Batch` and `inventories_json` are the
  report shapes the run prints.
- **Both modes.**
  - The spawn's loot setup (`PopulationFactoryBalancedAIPawn.SetupBalancedPopulationActor`, script, read by lane Sg):
    the item game stage is `ExpLevel` when `BaseItemGameStage` is 0, else `GameStage`, plus the truncated
    `ItemGameStageOffset` evaluated with the pawn as the source; then `SetupPawnItemPoolList` through the VM.
  - `SetupPawnItemPoolList`: custom lists replace the defaults when not empty, included lists first and depth first,
    then the own pools; false and nothing written for a balance with no playthrough entry. `GetPlayThroughIndex`:
    Reference takes entry 0 (the C++ models the first playthrough only); Off takes the world's playthrough index, at
    most the last entry (NATIVE_LOOT.md), from the world's balance state, `NotYet { LayerAbsent }` without one. A list
    that includes itself: Reference stops at the C++ depth bound of 16 with its text; Off refuses it at once.
- **Reference only: the C++ stand-ins (architecture R3, 6.1).** Each has an Off test in `loot/tests.rs`.

  | Stand-in | Reference | Off | Off test |
  |---|---|---|---|
  | One `rand()` for the whole loot path, seed 20261009, `seed:<n>` restarts it | the VM's `CrtStream::AttributeVariance`, which the evaluator's variance also draws from (as the C++ `evaluator_.setRandom(rand15)` shares it) | `seed:<n>` restarts `CrtStream::LootPool` (the pool roll's stream), a harness control | `the_seed_step_restarts_the_loot_stream_of_each_mode` |
  | 21 stand-in inventories from 7 `Pool_Coins` calls (6.1) | an instance of the item's class with `bDropOnDeath`, no `InitializeInventory` | no inventory: the pool native is `NotYet` before any draw (below) | `off_mode_makes_no_stand_in_inventory_the_pool_native_is_not_yet_before_any_draw` |
  | `DropLootOnDeath` replaced by the script's loop, items "dropped" as a record | the C++ loop | the installed script runs (no override); the override's body is `NotYet` if called | `off_mode_runs_the_installed_drop_loot_script_and_records_no_roll`, `the_drop_loot_override_body_is_not_yet_in_off_mode` |
  | A caller's `GameStageVarianceFormula` | the C++ note `UNIMPLEMENTED ... (not implemented)` | `NotYet { NotPorted }` | `a_variance_formula_is_the_cxx_note_in_reference_and_not_yet_in_off` |
  | `GetPlayThroughIndex` always 0 | entry 0 | the world's playthrough, at most the last entry | `off_mode_reads_the_world_playthrough_at_most_the_last_entry_and_reference_reads_the_first` |
  | The loot batch's sums of the override's records | the C++ batch | `NotYet` before any kill (no records to sum) | `off_mode_loot_batch_is_not_yet_before_any_kill` |

- **W5-EQUIP's contract 2, settled item by item** (`SpawnBalancedInventoryFromPool` against
  `ow_items::pools::roll_pool`, decision 2026-11-15-25). The pool rule is `roll_pool`'s (R2: one implementation), so
  the Off native must go through it; it cannot yet:
  1. *The caller's stage.* The native takes `GameStage`; `roll_pool` reads the snapshot's. Not a blocker: a copy of
     the world's snapshot with the call's stage gives it.
  2. *The variance.* The native gets the caller's `GameStageVarianceFormula` object; `roll_pool` takes registry
     data (`PoolRequest::with_variance`). Off: `NotYet { NotPorted }`; the death roll passes none.
  3. *The context.* The native evaluates every value with `ContextSource`; `roll_pool` evaluates with no source. A
     context resolver given no source answers "no context" (not `NotYet`, `ow-attr` context resolvers), so a value
     that needs the source would read silently wrong. **Blocker**: the death roll and the `rollpool` step always pass
     the pawn.
  4. *The draw order.* The native spawns each item inside its pick loop; `roll_pool` returns all drops first. Not a
     blocker: the pool draws use `CrtStream::LootPool` and an item's own roll its own stream, so spawning after the
     picks draws the same numbers. Whether the game shares one generator is unread either way (UNVERIFIED).
  5. *The return value* (found here). The native answers false when the gate fails or any pick finds no candidate;
     `roll_pool` returns only the drops, so a closed gate, an empty list and a quantity of 0 look alike. **Blocker.**

  So in Off mode the native is `NotYet { NotPorted }` naming 2, 3 and 5, checked before anything is drawn or spawned
  (no half-written state). Once `roll_pool` takes a source and reports its outcome, the native is `roll_pool` on the
  pool with the snapshot copy of item 1, then `ow_inventory::spawn::resolve` and `spawn_one` for each drop (real
  inventories with `InitializeInventory`, the replacement of the stand-in), with `bDropOnDeath` = the drop's flag AND
  the caller's (`None`, an untagged entry flag whose struct default is not read, then `NotYet`).
- **The decision-12 stand-in** (decision 2026-11-15-12: `src/progression.cpp` reads `GameInfoAttributeContextResolver`
  and `DesignerAttributeContextResolver` as keeping the object the chain held before them; the loot's artifact weights
  read a designer attribute that way). It is **not reproduced** under Reference here: `AttributeSystem` refuses a
  second resolver for a class (`with_context_resolver`), so no crate above `ow-attr` can swap the two for Reference
  only. Off is as `ow-attr` reads section 5: the game info for `GameInfo` (a reading, not a stand-in) and
  `NotYet { LayerAbsent("ow-vm") }` for `Designer`. The invented packages reach neither; on the installed game a
  Reference loot row that reaches them differs from the C++. Contract item below.
- **The loot-spawn behaviors** (`Behavior_SpawnItems`, `Behavior_SpawnLoot`, `Behavior_SpawnLootAroundPoint`,
  `Behavior_SpawnLootAtPoints`, `Behavior_DropItems`) are registered into the kernel's registry. They are script
  classes (`research/native_by_class.json` lists no native for them) whose rules NATIVE_LOOT.md has not read, and
  their `ApplyBehaviorToContext` needs the consumer (`SelfObject`), which `registry::HandlerFn` does not receive; each
  handler answers `NotYet { NotPorted }` naming that input. A provider's own handlers come first, so the combat run's
  death providers still list `Behavior_SpawnItems` at their boundary as the C++ does.
- **Where the Rust differs from the C++.** A step the C++ crashes on (a `rollpool` without a stage on a pawn that has
  no `GameStageForSpawnedInventory`: a null dereference, exit `0xC0000005`) is an error here. A balance chain deeper
  than 64 (`BaseDefinition`) is an error (a Rust-only bound; the C++ guards the walk by the balances in it). The
  C++ evaluator draws every variance from the loot generator once deaths are enabled; the Rust Reference evaluator
  draws from the same stream, so this holds, but the generator is the VM's attribute stream rather than a private one.
- **Seam edits (granted: the smallest, to add the S-g steps and register the handlers).**
  - `crates/ow-combat/src/run.rs`: `loot::enable` after `enable_deaths`; the `seed`, `rollpool` and `lootbatch`
    steps (`roll_pool_steps`, `loot_batch`); the spawn's loot setup before its rows and the pawn list (the C++ order:
    a balance the package lacks fails the spawn before the pawn is listed); a kill's `loot` report (`loot_after`,
    `drops_seen`); `rollpool` removed from `NO_STATE` (the C++ reports the pawn's state after it; it had been listed
    while the step was a placeholder); the `LOOT_LANE` placeholder constant removed; the module documentation's loot
    bullet.
  - `crates/ow-combat/src/run/fixture.rs`: a `#[path]` module declaration of `loot/fixture.rs` and one call in each
    of `engine`, `willow_game` and `toy_pawns` (the file had left the loot part to this lane). W6-COMBAT's 16 pinned
    digests are unchanged on the new bytes.
  - `crates/ow-combat/src/run/tests.rs`: the two tests that pinned the placeholder (`NotYet { LayerAbsent("W6-LOOT")
    }`) now pin the landed behaviour, their maya and spawn checks kept.
  - `crates/ow-cli/src/cmd/combat.rs`: not touched.
- **Fixture.** `loot/fixture.rs` ports `tests/combat_test.py`'s loot builders value for value, and adds what the
  Rust run needs and the Python toy lacks: the definition fields the Rust evaluator reads on every initialization
  definition (`ValueFormula`, `ConditionalInitialization`, `RangeRestriction`, `RoundingMode`; without them the
  Rust evaluator refuses `Pool_Rand`'s quantity, which Reference catches as the C++ catches its errors), the batch's
  damage source `WillowDmgSource_Shotgun` and stock damage type `GD_Impact.DamageType.DmgType_Normal`, `Balance_Two`
  (two playthrough entries) and `List_Loop` / `Balance_Loop` (a list that includes itself).
- **Verified (automated, local, pc-hummer).**
  - 27 loot unit tests in `loot/tests.rs` (89 `ow-combat` tests pass with W6-COMBAT's, 2 ignored: the two fixture
    writers), one per rule above.
  - The S-g steps against the C++ binary on the same package bytes: twelve step lists (kills on each pawn balance,
    seeds, the cap, `rollpool` on every toy pool, `lootbatch`, the refusals, the two new balances) are byte-identical
    in Reference mode after W6-COMBAT's normalization (timing fields, `SetHealth` lines), pinned as digests of the
    C++ output (`the_sg_step_lists_equal_the_cxx_binary_in_reference_mode`). The C++ binary was built in this lane's
    worktree on main 34117d3 (a build of the earlier base d9679ce, with the same `src/`, gave the same digests).
  - `tests/combat_loot_standin_test.py` passes on the Rust binary under `OW_PARITY=reference` (manifest line
    `tests/rust_parity.d/loot.txt`) and on the C++ binary.
  - `tests/combat_test.py` on the Rust binary under `OW_PARITY=reference`: 3 checks fail, W6-COMBAT's two
    `SetHealth`-event checks and one of the loot section's, the random quantity of `Pool_Rand`, whose Python toy
    definition lacks the four fields above (so the Rust evaluation fails and the pool is "not evaluated"); every other
    loot check passes. In Off mode the loot section fails by design (the toy `DropLootOnDeath` is an empty script). So the
    file is not a manifest line.
- **The installed game (local, outputs under `local/p2/w6-loot/installed/`).** One run over `Sanctuary_Dynamic.upk`:
  seed 20261009, `player:8` (the `maya` step needs the mission bridge, not wired), the four bullymong kinds
  (`adult`, `baby`, `regular`, `ranged`) spawned and each killed once, `rollpool` on nine pools of their list, and two
  batches of 100 kills (`adult`, `baby`). The C++ binary: exit 0 in 54 s, 180 rows (156 of them loot rows: 16 from the
  four kills, 27 from `rollpool`, the rest the batches), no error, 45 stubbed natives; its counts lie where
  COMBAT_LOOT.md's do (the adult's item stage 4 to 7 on 19 / 25 / 25 / 31 kills, the baby's 6 on all 100; the
  money pool tried 23 and 18 times and always true; both ammo pools tried and never true). The Rust binary: 0 rows in
  both modes, because **every spawn stops before the loot setup**: in Reference at
  `GearboxFramework.NounAttributeValueResolver` (`NotYet` in `ow-attr`, lane FIX-NOUN), in Off at the archetype's
  reference `Default__WillowAIPawn.SpecialMoveComponent0` (W6-COMBAT's relink refuses a reference into a class default
  object in Off mode). So no loot rule has run on installed data and no installed death drop is compared yet. Open
  item, not worked around (lane brief); the run is to be repeated once FIX-NOUN lands.
- **Contract items for the hub.**
  - `ow_items::pools::roll_pool` (W4-POOLS): a context source for its evaluations and the native's outcome (gate
    passed, every pick found a candidate), so the Off native can go through it (items 3 and 5 above); the variance as
    an object (item 2) if a caller other than the death roll needs it.
  - `ow-attr` (W3-ATTRCTX): a way to reproduce decision 12's two context resolvers under Reference only (a replace
    API, or a mode-aware resolver), or a hub decision that Reference loot rows may differ there.
  - `ow_inventory::spawn::register` (W5-EQUIP) binds `SpawnBalancedInventoryFromPool` as `NotYet` under the same key;
    a host that registers `ow-inventory`'s natives before `ow-combat`'s keeps that stub (`hit::bind` skips a bound
    key). The combat run registers no `ow-inventory` natives, so it is not affected; the stub should go once the Off
    native lands.
  - `registry::HandlerFn` gets no consumer (decision 2026-11-15-30): the loot-spawn behaviors' scripts cannot run
    from the global registry until it does.
- **Open items.** The Off pool native (the contract items above); the loot-spawn behaviors' rules (NATIVE_LOOT.md,
  "Not read yet"); the pickup and the item toss (host side); `Pool_Money_1or2`'s live quantity 3 (COMBAT_LOOT.md,
  OPEN); the ammo and health drop weights (`AmmoDropWeightAttributeValueResolver` is not in `ow-attr`, so on the
  installed game those pools are `NotYet` in both modes); the live 3-item money drop (architecture section 12,
  question 20).
