## 2026-11-15: Rust resource pools, pawn health and the level-up rebase as shipped

AI-assisted (Claude Code). Lane W5-POOLS of the Rust port fills `crates/ow-world/src/{pools,levelup}.rs` and adds
their tests in `crates/ow-world/src/pools/tests.rs` (architecture section 8, Wave 5). No parsing behaviour changes.
The rules come from NATIVE_COMBAT_GROUNDWORK.md sections 1 and 2, NATIVE_LEVEL_UP_ATTRIBUTES.md sections 1, 2 and 4,
and NATIVE_ATTRIBUTES.md sections 5 and 8, all read from the executable. The C++ references are `src/combat.cpp` and
`src/mission_script.cpp`. There was no real-game session in this lane.

- **Seam edit outside the lane's files** (granted: register the query kind and the natives). `crates/ow-world/src/world.rs`:
  `World::new` now registers `pools::Pack` and `levelup::Pack` right after the world's own `Pack`. That is two lines.
  `query.rs` is unchanged: `QueryKind::ResourcePool` already existed, and `pools::Pack` registers its handler through
  `WorldQuery::register`.
- **Shipped shapes.** Public structs that may grow are `#[non_exhaustive]`.
  - `pools::Pack` registers 26 natives:
    - the pool natives `GetMaxValue`, `GetCurrentValue`, `SetCurrentValue`, `UpdateLastValues`, `PoolIsNowFull`,
      `ResetIdleTimer` and `ClearAttributeModifierStacks`;
    - the manager natives `CreatePool`, `CreatePoolAtIndex`, `InternalCreatePoolAtIndex`, `DeletePoolByReference`,
      `DeletePool`, `DeletePoolAtIndex`, `DeleteAllPools`, `HasResourcePoolForResourceDefinition`,
      `GetResourcePoolForResourceDefinition`, `CreateResourcePoolManagerIfNecessary` and `RecalculateBaseValues`;
    - `Controller.GetResourcePoolForResourceDefinition`;
    - `Actor.IsResourcePoolValid` and `Actor.GetResourcePoolCurrentValueOrDefault`;
    - the pawn natives `SetHealthResourcePoolDefinition`, `GetHealth`, `SetHealth`, `GetMaxHealth` and `SetMaxHealth`.

    It also adds the `PoolMaps` service and the `QueryKind::ResourcePool` handler.
  - `levelup::Pack` registers `Controller.` and `WillowPlayerController.RecalculateAttributeInitializedState()`.
  - Free functions for the lanes above: `create_pool`, `create_pool_at_index`, `delete_pool_at_index`,
    `delete_all_pools`, `pool_for_resource`, `controller_pool`, `ensure_manager`, `recalculate_base_values`,
    `set_current`, `update_last_values`, `pool_is_now_full`, `clear_modifier_stacks`, `resolve`, `read_ref`,
    `ref_value`, `slot`, `pools_of`, `health_pool`, `get_health`, `set_health`, `get_max_health`,
    `set_health_pool_definition`, and `levelup::recalculate_attribute_initialized_state`. They use the types
    `PoolRef`, `Created` and `Recalculated`.
  - Each of the 28 keys equals the key the Rust loader derives from the installed `Engine.upk` and `WillowGame.upk`.
    This was checked with `--run` under `LogAndZero`, which logs the key of an unbound native, and the output stayed
    under `local/`. That checks the binding, not the behaviour.
- **The pool's script runs as installed**: `InitializeAuthoritative`, `CalculateBaseValues`, `ApplyUpgrades`,
  `UpdateCurrentValueOnExtremaChange`, `Reinitialize` and `ShutDown` (architecture R2). The level-up's "base and
  maximum rewritten, current untouched" is that script on top of the natives; no Rust copy of it exists.
- **Choices, each UNVERIFIED unless said otherwise.**
  - The manager's `ResourcePoolMap` (resource to slot) is a VM service, `PoolMaps`, because the object model holds a
    map property as `Value::None` (decision 2026-11-15-06). Creation records an entry and `DeletePoolAtIndex` removes
    it. The lookups and `DeletePoolByReference` read it, as the record says. The C++ keeps no map:
    `DeletePoolByReference` deletes the reference's own slot, and the lookup scans the slots. The two agree unless a
    slot and the map disagree.
  - Same object means same `ObjRef`. The definition store gives one object per export. The C++ compares loaded copies
    by path.
  - A manager is made by spawning a `ResourcePoolManager` actor owned by the owner (`actors::spawn`), only with an
    authoritative owner and a world that has a `WorldInfo`. The C++ instantiates one without the spawn rules.
  - `ResourcePools` must be declared with 16 elements. Any other count is a typed error, where the C++ pads a
    one-element synthetic array.
  - `Controller.GetResourcePoolForResourceDefinition` takes an omitted `bCreatePoolIfOneDoesntExist` as true, the
    record's default.
  - The engine query reads only one resource owner resolver, the controller resolver with a controller source (the
    record: a controller resolves to itself). It then returns the slot the owner's manager map gives. Every other
    resolver class or source is `NotYet { NotRead }`. A None resolver, a None manager or no entry gives `Ok(None)`.
- **`NotYet` where the record is silent, and the Reference stand-ins.** In `ParityMode::Off` the following are
  `NotYet { NotRead }`. Each refusal is decided before anything changes.
  - `ResetIdleTimer` has no note (NATIVE_INDEX.md: "none (guess)").
  - The "current value changed" hook of any pool subclass. Only the base class's hook is read, and it is empty.
  - The class-level pawn attributes and the ten status-effect resistances of `RecalculateAttributeInitializedState`,
    whenever their conditions hold. The record does not name the properties they land in.

  `ParityMode::Reference` keeps the C++ reading for all three: it stamps the world time, skips the hook, and skips both
  class-level parts (`src/mission_script.cpp`: "not modelled"; for Maya's class the data are 0 and 1). No tracked test
  pins them. **For the architecture 6.1 table (the hub's edit):** three Reference-only rows owned by W5-POOLS, each
  replaced by its Off-mode `NotYet` test in `pools/tests.rs`.

  These are always `NotYet`:
  - `CreatePoolAtIndex` with no manager;
  - a vehicle's pool provider in the controller lookup;
  - `DeletePool` whose manager differs from the reference's;
  - the `WillowMind` override of the recalculation.

  The `WillowVehicle` thunk is its own native and is not registered.
- **Where the output differs from the C++, and why.** The C++ binary reaches its pool natives only inside
  `--combat-run`, `--mission-run` and `--slice-run` (W6 lanes). Its `--run` leaves them unbound, as the Rust `--run`
  does. So `tests/rust_parity.d/worldpools.txt` holds no line, and says why.
- **Verified (automated, local, pc-hummer).**
  - 29 `ow-world` unit tests on an invented `Engine.upk`. Its natives are declared with the record's signatures, so
    every call goes through the derived key, and its pool script bodies are invented. They cover every rule above: the
    clamp order and the authority gate, integer truncation, the idle stamp, `UpdateLastValues`' 0.0001,
    `PoolIsNowFull`, the reference rule, each `CreatePool` outcome (slot 0 with GUID 0, the refusal, the reinit quirk,
    the lowest free slot, the 17th resource), deletion, the lookups, the controller's lazy creation, the pawn's health
    natives and `SetHealthResourcePoolDefinition`, the engine query, the rebase order and the level-up refusals.
  - **The installed game, local only.** An ignored test, `installed_game_maya_health_pool_levels_1_to_80`
    (`OW_COOKED`, Reference mode), runs on the installed game. It creates `D_Resourcepools.PlayerPools.HealthPool`
    through `CreatePool`, which runs the installed `InitializeAuthoritative`. Then, for levels 1 to 80, it sets the
    replication info's `ExpLevel` and runs `RecalculateAttributeInitializedState`, which runs the installed
    `CalculateBaseValues(true)` through the real evaluator. Results:
    - the base and effective maximum equal `max(20, 80 x 1.13^L)` bit for bit at 80 of 80 levels;
    - levels 8, 9 and 10 match the live values of NATIVE_LEVEL_UP_ATTRIBUTES.md section 7;
    - the follow rule held at 80 of 80 levels: from half health current was untouched, and a full pool followed the
      new maximum.

    Output: `local/p2/w5-pools/health_1_80.txt`. This shows that the VM, the installed script and the evaluator agree
    with the recorded formula. It is not a new real-game observation.
- **Open items.**
  - The three Reference stand-ins above: read `ResetIdleTimer`, the subclass hooks, and the pawn and status-component
    properties of the class-level parts (Ghidra).
  - The `WillowMind` and `WillowVehicle` versions; the vehicle pool provider; resource owner resolvers other than the
    controller one, and pawn sources.
  - Whether `CreatePoolAtIndex` makes a manager.
  - `DeletePool`'s two managers.
  - The network-update mark of `DeletePoolAtIndex` (no replication here).
  - The slot order the class produces (NATIVE_COMBAT_GROUNDWORK.md 1.10; a cheap live read).
- **UNVERIFIED.** Every rule above as read from the executable. The exception is the observed one: the health pool's
  base and maximum are rewritten by the player controller's recalculation and current health is untouched
  (2026-10-07, lane L1).
