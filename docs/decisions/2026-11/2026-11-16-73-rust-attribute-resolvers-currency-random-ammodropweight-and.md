## 2026-11-16: Rust attribute resolvers: Currency, Random, AmmoDropWeight and Constraint as the C++ stand-ins, their natives noted

AI-assisted (Claude Code). Lane FIX-ATTR-RESOLVERS of the Rust port (docs/rust-port/SLICE_RUN_PLAN.md, step 3 of the
lane list; section 3.3 lane 1). Number 73 was reserved for this lane by the hub. There was no real-game session:
every rule below is the C++ reference's or read from the executable, and is **UNVERIFIED** in the running game. No
parsing behaviour changes.

- **What changed.**
  - `ow-attr`: four value resolver modules in `crates/ow-attr/src/value_resolvers/` (`currency.rs`, `random.rs`,
    `ammo_drop_weight.rs`, `constraint.rs`), their registration in `value_resolvers/mod.rs`, their tests
    (`attr_resolver_tests.rs`), and the four classes plus a `local_pools` answer in the shared test world
    (`testworld.rs`). `eval::random_between` is now `pub(crate)`, so the Random resolver uses the evaluator's one
    mapping of a draw.
  - Additive seams: `EngineQuery::LocalPlayerPool { resource }` (`ow-attr/src/api.rs`) and
    `QueryKind::LocalPlayerPool` (`ow-world/src/query.rs`). `ow-combat` registers its handler in Reference
    (`loot::register_local_player_pool`, called where `run.rs` makes the run's world info), with two tests in
    `loot/tests.rs`.
  - The analyst note `docs/verification/NATIVE_ATTR_RESOLVERS.md`.
  - No dependency, no `Cargo.toml` change.
- **Which resolvers.** The plan names Currency, Random and AmmoDropWeight. The installed bullymong loot run showed a
  fourth stop once those passed: `ConstraintAttributeValueResolver`, which the emergency ammo pool reaches. It is
  handled the same way. After it, no attribute resolver stops the plan's loot or fee steps in Reference.
- **Reference mode** reproduces the C++ stand-ins of `src/progression.cpp` (architecture R3):
  - **Currency:** the resolved context, or its `PlayerReplicationInfo` when it holds no `Currency` table; then
    `Currency[FormOfCurrency].CurrentAmount`.
  - **Random:** the two bounds, then one draw on `CrtStream::AttributeVariance`, mapped as `randomBetween` maps it.
    The draw replaces the running value, and nothing is saved.
  - **AmmoDropWeight:** the local player's health pool, through the new query (the first replication info in
    `GRI.PRIArray` owned by a `WillowPlayerController`, then its pawn's `HealthPool`, kept only for its own
    resource). Then the share of the pool's maximum against the threshold, with the stand-in's interpolation, or the
    no-pool weight.
  - **Constraint:** the running value raised to the enabled minimum, then lowered to the enabled maximum.
  - What the C++ refuses (a context with no currency table, a missing slot) is `NotYet { NotRead }` naming the C++.
- **Off mode:** `NotYet { NotRead }` for all four, naming the analyst note. The note
  (`docs/verification/NATIVE_ATTR_RESOLVERS.md`) reads the four natives in our own words (outcome tables), and its
  section 6 lists where the stand-ins differ from them:
  - Currency does not move from a controller to its replication info.
  - Random's real draw divides by 32767, its integer range truncates, and it keeps saved values by context name for
    a lifetime.
  - AmmoDropWeight walks every controller's pool and sums the below-threshold weights against the pool's minimum,
    skipping pools never modified.
  - Constraint's native rule is the stand-in's.
  - By the clean-room rule this lane implements none of the native rules; they go to a fresh implementer.
- **Writes** (add, remove, set-base) are `NotYet` for the four classes in both modes.
- **The installed game** (local runs of `ow-package --combat-run` over `Sanctuary_Dynamic.upk`; step lists and outputs
  under ignored `local/p2/fix-attr-resolvers/`; the C++ binary built from this tree):
  - **W6-LOOT's bullymong loot run** (COMBAT_LOOT.md's step list: `seed:20261009 maya:8 spawn:adult:7:6`, `rollpool`
    on its nine pool and stage pairs at 200 rolls, `lootbatch` adult and baby, then the low-health baby batch; the
    batches at 100 kills each). The C++ prints 197 rows.
    - Reference before: 29 rows, 24 equal to the C++ (each batch kill stopped at AmmoDropWeight, the ammo
      `rollpool`s at Random and AmmoDropWeight).
    - Reference after: **197 rows, 197 equal to the C++**.
    - The 300 errors left are the controller and player case of `Actor.Destroy`, raised after each kill's rows are
      written (lane FIX-ACTOR-DESTROY), so they change no row.
    - Off: 0 rows before and after. The run stops at the bridge's health pool, the unbound
      `Actor.LocalPlayerControllers` and the spawn's template archetype, before any resolver.
  - **S-f, the down and the respawn fee** (`maya:8 players:1 spawn:adult:7 cash:3000`, four melee swings,
    `bleed:50`). The C++ prints 17 rows.
    - Reference before: 16 rows, 16 equal, the fee row `down.fee.1` missing (stopped at Currency).
    - Reference after: **17 rows, 17 equal, no error**.
    - Off: 0 rows before and after, for the same reasons as the loot run.
  - Row agreement with the C++ says the port reproduces the stand-ins, not that the game does this.
- **The bl-sdk reads that would confirm the native rules** are in the note's section 6.
