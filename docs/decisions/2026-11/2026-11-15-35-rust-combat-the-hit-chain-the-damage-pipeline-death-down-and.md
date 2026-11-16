## 2026-11-15: Rust combat: the hit chain, the damage pipeline, death, down and kill XP glue, and --combat-run

AI-assisted (Claude Code). Lane W6-COMBAT of the Rust port fills `crates/ow-combat/src/{hit,damage,death,down,xp,run}.rs`
(with `hit/access.rs`, the test files under each module's folder and the invented packages in `run/fixture.rs`) and the
`--combat-run` command (`crates/ow-cli/src/cmd/combat.rs`, its parity test in `cmd/combat/tests.rs`), architecture
section 8, Wave 6. No parsing behaviour changes. The C++ references are `src/combat.cpp` (its pawn half; W5-POOLS took
the pool half), `combat_hit.cpp`, `combat_death.cpp`, `combat_down.cpp`, `combat_xp.cpp` and `combat_run.cpp`; the
records are NATIVE_DAMAGE_DEATH.md, NATIVE_COMBAT_GROUNDWORK.md, NATIVE_DOWN_TIMERS.md, NATIVE_CONTROLLER_HELPERS.md
and the COMBAT_* records. There was no real-game session in this lane. **Every rule is UNVERIFIED in the running game**
unless a line says otherwise; the C++ is the oracle for every text and number, which says nothing about the game.

- **Shape.** `hit::Pack` (a `NativePack`) adds the `CombatState` service and binds the natives of the hit chain,
  `damage`, `death`, `down` and `xp`. As the C++ `bind`, a native is bound under the key the loader derives when the
  package set declares that function native, and skipped otherwise; a key a lower pack bound first stays that pack's
  (`ResourcePool.GetCurrentValue`: W5-POOLS; `Actor.Destroy`: W5-WORLD, whose rule removes the actor where the C++ only
  sets `bDeleteMe`). `run::Run` is the scripted run of `combat_run.cpp` over an `ow_world::World`; `run::combat_run`
  returns the report and whether a step failed. The owner's raises (`OnTakeDamage`, the death-chain events) are
  callbacks in `CombatState`, which the run sets to its providers.
- **The C++ rules, ported as written** (each UNVERIFIED as a reading of its native): the pipeline pool (a fresh object
  from the class defaults per acquire, the first free entry reused); `PawnCheckTakeDamagePreconditions`;
  `ActorTakeDamageInner` (stores `MostRecentDamageTaken`); `RecordRecentDamage`; the two damage type modifier natives;
  `NotifyTakeHit` (raises only for a pawn whose AI class has an AI definition); the four-step opinion rule; the hit
  region by bone, else the default region; the demigod holds per reason; `AdjustDamage` stages 1 to 4, 6, 7, the
  intrinsic armor and the damage surface, with the same "not implemented" notes for the stages the C++ leaves out; the
  melee and thrown-rock deliveries and the always-connecting swing; the world a kill needs, the death preparation
  (inventory manager, own component copies, death definition, consumer handles from 100, walking physics), the
  tracker's `NotifyPawnDied`, the death definition's events, the expression evaluators, the output-link list; the down
  state's natives and the currency rules; the experience multiplier from the AI class's starting values.
- **The kill experience is `ow-balance`'s** (decision 2026-11-15-18, architecture R2): `CalculateExperiencePointsForKill`
  calls `kill_xp_award` and `AwardCombatExperienceToParty` calls `kill_xp` per player; this module supplies the party
  (the game's replication info), the count, the levels and the world's playthrough. Where `ow-balance` refuses (a
  vehicle victim or killer, a critical kill without `ExpScaleForCriticalHitKills`, a non-empty
  `DamageTypeExpModifiers`) the result is its `NotYet` in both modes, where the C++ logs and goes on. None is reached by
  the slice's kills. `PlayerCount` holds 1 to 4, so a game count outside the weight table (the `WillowGameInfo`
  default 0) is passed as a count past the table's rows, which reads as bonus 0 and weight 1, the record's rule; a
  table of four rows or more makes that `NotYet` (contract item below). **The installed table has four rows**
  (`GD_Globals.General.Globals.ExpAwardWeights`, counted locally through `--def-dump`; `ExpScaleByLevelDifference` has
  six), so on the installed game every credit of a run whose game keeps the default `EffectiveNumPlayers` of 0 is
  `NotYet` until the contract item is settled; a `players:<n>` step of 1 to 4 sets the count.
- **Where the Rust differs from the C++, and why.**
  - Each spawn is instantiated from its package export with the C++ 4-byte prefix (`ow_object::cdo::instantiate_export`,
    as W5-KISMET does), and its resource references are then relinked to the definition store's objects by path. The
    store itself refuses every pawn archetype of the installed game: it derives a 26-byte prefix for an `Actor`
    subclass export outside a level, and their tags do not parse there (checked locally on `Pawn_TargetDummy` and
    `Pawn_PrimalBeast`). The swing's definitions are read the same way, as the C++ reads them. A reference the store
    resolves to no definition, and a value nested deeper than the relink bound, are kept as the C++ keeps its resource
    stand-ins in `Reference` mode and refused in `Off` mode (review r1 of PR #335).
  - `hit::stock_object` (the damage type of a `hit` step, the globals definition) takes the store's first holder of
    the path as one shared object; the C++ makes a copy, looked up in the victim's package first and then in Startup.
    The two read the same values unless cooked copies of the path differ between files.
  - The pool manager is the world's spawned actor (W5-POOLS), so a run places the level's `WorldInfo` in the actor list.
  - **`SetHealth` events.** The C++ pool half logs `SetHealth <value>` into the event list. The Rust `SetHealth` is
    W5-POOLS' native and logs nothing; no VM API lets this lane observe it. So those lines are missing from a hit's
    `events` (the `order` list still shows the calls). Contract item below.
  - A behavior event raised on a provider that is running (a death definition's `Behavior_CustomEvent` broadcasting
    to its own provider) is deferred and raised once the outermost raise's own event has returned, in order; the C++
    re-enters (`Provider` methods take `&mut self`). A provider is marked running from before it is made until it is
    back, so a re-entrant raise never makes a second provider over the same definition; only the outermost raise
    drains, `OnTakeDamage`'s raise included, at most 10,000 deferred events (a runaway is an error). Review r1 of PR
    #335 found the first version hung on this path; the test
    `run::tests::an_event_raised_on_a_running_provider_runs_after_it_once_and_on_the_same_provider` pins the order.
  - The mission bridge (W6-MISSION, on main since e45490a) is not wired into the run yet: a follow-up lane wires the
    `maya`, `tick`, `bleed` and `cash` steps against it for S-d and S-f (hub decision, review r1 of PR #335). Until then
    a run with a `maya:` step is `NotYet { NotPorted }` before any step (exit 3), and `tick`, `cash`, `bleed` and
    `mayahealth` answer the C++ texts of a run without the bridge. Loot (W6-LOOT): `seed`,
    `rollpool`, `lootbatch` and the loot setup of a spawn with a pawn balance are `NotYet { LayerAbsent }` (the spawn is
    made first, so the pawn and its rows exist), and a kill reports no `loot`.
  - None of the C++'s per-provider behavior handlers (`Behavior_AIHold`, `Behavior_Conditional`, `Behavior_CustomEvent`,
    `Behavior_CompareBool` and the boundary lists) is registered in the VM's global registry: each needs the consumer
    pawn, which `registry::HandlerFn` does not receive. They are set per provider (`Provider::handle`), as the C++ does.
    The C++ has no handler that deals damage itself, so none is written; `Behavior_CauseDamage` stays unregistered.
- **Parity modes and the stand-ins** (architecture R3 and 6.1).
  - The 6.1 row "a property reading 0 with base 0 is the identity 1" (`GetPawnDamageTypeModifier`): `Reference` only;
    in `Off` mode such a value is `NotYet { NotRead }` naming the start-up values (`ApplyAttributeStartingValues`, not
    read), and a value the pawn holds is read as it is. The critical-hit bonus: `Off` reads the instigator's value, and
    0 with base 0 is `NotYet { NeedsRealGame(8) }` (question 8); `Reference` reads it as the C++. Replacement tests:
    `damage::tests::a_resistance_of_zero_with_a_base_of_zero_is_one_only_in_reference_mode` and
    `the_critical_bonus_of_zero_with_base_zero_needs_the_real_game_in_off_mode`. No tracked C++ test pins this row
    (`tests/combat_test.py` never reaches `AdjustDamage`), so no manifest line is `mode=reference` for it.
  - An attribute the VM object's class does not declare: `Reference` reads the stage's identity and notes it once, as the
    C++; `Off` is `NotYet { NotRead }`. A C++ "not implemented" site: `Reference` logs `UNIMPLEMENTED <what> (not
    implemented)` and goes on; `Off` is `NotYet { NotPorted }`.
  - The stages `AdjustDamage` leaves out (review r1 of PR #335): `Reference` keeps the C++ `note:` texts and goes on;
    `Off` is `NotYet` where each would apply: the AI weapon scales of a bullet or rocket and the shield stage of a
    victim with a valid shield pool (`NotPorted`; the shield matters for the slice, S-e's bullymong hits Maya's shield),
    a globals definition without `GlobalAIDamageScale` (`NotRead`), a region hit whose pipeline carries an impact
    definition (its per-material modifier, `NotPorted`; the C++ reads the region's `DefaultImpactResponse` whatever the
    pipeline carries), a hit with a damage type whose region names no surface (`NotRead`: the C++ reads flesh), the
    hit-region cap, averted death and the protection timer (`NotPorted`). None of the 16 pinned step lists reaches them.
  - The stub policy of `--combat-run`: `Off` refuses every unbound native (`Refuse`, nothing allowed); `Reference` is
    `LogAndZero`, the C++ run's handling of every unbound native.
  - Where NATIVE_DOWN_TIMERS.md (read after the C++) differs from the C++, `Off` takes the record: `SetSecondWindReason`
    stores the reason only for a pawn that is injured and has none yet (the C++ stores it always), and `IsInjured` of
    an AI pawn also answers its own `bInjured` byte.
- **Fixture fix (granted).** `tests/combat_test.py`'s invented packages now load in the Rust reflection too: every toy
  class derives from a toy root `Engine.Object` that every object and class property names, `ResourcePools` is a
  static array of 16, the toy `Actor` and `WorldInfo` declare the fields the world's actor spawn reads, and the
  manager's class default object holds `Role` = 3. No assertion changed; the file passes on the C++ binary as before.
- **Verified (automated, local, pc-hummer).**
  - 58 `ow-combat` unit tests (after review r1 of PR #335) on the invented packages of `run/fixture.rs` (a Rust port of `combat_test.py`'s builders
    without their loot part, plus invented allegiances, hit regions, a resisting damage type, expression trees,
    currency and consumer handles), one per rule above; the re-entry tests run on the `ow-behavior` fixture's provider
    packages with one more invented package.
  - `--combat-run` against the C++ binary on the same package bytes: sixteen step lists (the scenarios of
    `combat_test.py` without loot, plus three that run `AdjustDamage` on an armored toy pawn). After removing the two
    timing fields and the `SetHealth` lines, the Rust report is byte-identical to the C++ one: all 16 in `Reference`
    mode, 13 in `Off` mode. The other 3 are `NotYet` in `Off` mode on purpose (undeclared attributes). Pinned as
    digests of the C++ output in `cmd/combat/tests.rs`. A seventeenth list of refusals equals the C++ except the
    `seed` step (W6-LOOT).
  - `tests/combat_test.py` on the Rust binary: every check before its loot section passes except the two that read
    `SetHealth` event lines; the loot section fails (W6-LOOT). So it is not a manifest line yet;
    `tests/rust_parity.d/combat.txt` holds three refusal lines that pass.
- **The installed game (local, outputs under `local/`).** One run of the S-a, S-b, S-c and S-e steps that need no
  bridge (five spawns, three bases, a player, two hits on the dummy, a kill, two swings). The C++ printed 29 rows; the
  Rust (`Reference` mode) stopped every spawn at `GearboxFramework.NounAttributeValueResolver`, which `ow-attr` answers
  `NotYet` in both modes (the C++ evaluator reads a fitted stand-in for it), so it printed 0 rows. No installed-game
  number is compared yet.
- **Contract items for the hub.**
  - An observer seam in `ow_world::pools::set_health` (or leave to ow-combat its own `SetHealth`) so the run can log
    `SetHealth <value>` as the C++ does; then `tests/combat_test.py` can be a manifest line once W6-LOOT lands.
  - The store's prefix rule for an `Actor` archetype outside a level (W3-DEFS): every installed pawn archetype is
    refused; this lane reads them through `instantiate_export` at the C++ prefix instead.
  - `NounAttributeValueResolver` (W3-ATTREVAL): every installed enemy health pool reaches it; a read of its native or a
    `Reference` stand-in is needed before any installed-game combat row can be compared.
  - `registry::HandlerFn` does not receive the consumer (decision 2026-11-15-30 names the additive route); a handler that
    needs it can only be set per provider.
  - `ow_balance::xp::KillCredit::players` cannot hold a count outside the weight table.
- **Open items.** The mission bridge's steps (S-d credit into Maya's pool, S-f's bleed-out and respawn: a follow-up
  lane wires them against W6-MISSION's bridge);
  loot (W6-LOOT); the stages `AdjustDamage` leaves out; every native body the C++ reads by role (the down natives, the
  holds, the expression evaluators); the installed-game comparison after the three items above.
