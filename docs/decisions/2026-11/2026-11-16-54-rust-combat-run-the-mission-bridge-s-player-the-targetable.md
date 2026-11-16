## 2026-11-16: Rust combat run: the mission bridge's player, the Targetable resolver, the provider check and the sequence tripwire

AI-assisted (Claude Code). Lane W6-COMBAT-RUN2 finishes the `--combat-run` steps decisions 2026-11-15-35, 2026-11-16-41
and 2026-11-16-49 left open, in `crates/ow-combat/src/run.rs` (with the new `run/player.rs` and the tests in
`run/tests.rs`), `crates/ow-cli/src/cmd/combat.rs` and `crates/ow-attr/src/value_resolvers/` (the Targetable resolver,
its tests and its registration). No parsing behaviour changes. There was no real-game session in this lane and no
native code was read. **Every rule here is UNVERIFIED in the running game**; the C++ (`src/combat_run.cpp`,
`src/mission_script.cpp`, `src/progression.cpp`) is the oracle for every text and number, which says nothing about the
game.

**The mission bridge's player (S-d and S-f).**

- A run with a `maya` step is built over W6-MISSION's bridge, as the C++ `combatRun`: `ow-cli` builds
  `MissionSystem` over `Startup`'s `GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire` and `MissionScript` on the run's
  world (the combat pack already in it), and hands the run the bridge's player through the new trait
  `ow_combat::run::PlayerBridge` (`ow-mission` depends on `ow-combat`, so the run cannot name the bridge).
  `player_controller` and `player_pawn` are the bridge's own objects; there is no private copy. The run uses the
  `WorldInfo` the bridge placed instead of placing its own.
- Ported from the C++ step by step: `maya` (level, experience, `short<n>`, the pawn's preparation), `tick` (time, the
  pool's update, the gains), `cash`, `mayahealth`, `bleed` (frames of 1/60 s by default, the injured state's `Tick` and
  the three timed callbacks at the C++ delays 2.28 / 0.84 / 11.8 s, the death camera's per-frame call, the second wind
  and respawn rows, the fee rows), the bridged hit (the controller's `Pawn` set to the player's while the chain runs)
  and swing (the player's pawn as the victim), the kill rows' `credit` (what the bridge's pool gained), the `down`
  object on a hit and a swing, and the report's `down.fee.<n>` rows. A bridged chain or bleed frame that fails is
  recorded as the bridge's guard records it (`<step>: mission script: <text>`) and the run goes on; the step's result is
  then the error, where the C++ prints the partial report the chain had built.
- **The first `maya` step sets the player count** (scenario setup, as the first `player:` step since decision
  2026-11-16-49): the bridge's player is the run's one local player, so the game's `EffectiveNumPlayers` becomes 1
  while it holds its class default 0. Without it every installed kill credit is `NotYet` (`KillCredit::players` holds 1
  to 4, decision 35's contract item). The C++ reads 0 there; on the installed table the S-d credit below is the same.
- **Stand-in, `ParityMode::Reference` only (R3):** the player's health pool. The C++ bridge builds one by hand
  (`ensureHealthPool` / `seedHealthPool`: a manager on the controller, the class's `HealthPoolDefinition`, the maximum
  from `BaseMaxValue` with the controller as the source, the pawn's `HealthPool` reference to slot 0), a stand-in for
  the class-defaults routine that the Rust bridge does not build. `run/player.rs` reproduces it in Reference; Off
  answers `NotYet { NotPorted }` at `maya` and `mayahealth`. Test:
  `maya_sets_the_bridge_player_and_builds_the_cxx_health_pool_only_in_reference_mode`. The Fire slice holds its own copy
  of the same stand-in (`ow-mission/src/slice.rs`); the two are not shared (different crates), an open item.
- The globals references the C++ `maya` step loads by hand (the death camera's end time) are not loaded here: the
  definition store loads a definition's references when they are read.

**The Targetable resolver** (`WillowGame.TargetableAttributeValueResolver`, new `targetable.rs`). The class has no
script functions (its export table lists only `ValueToResolve` and its enum), so its get is the inherited native, which
no record has read (NATIVE_ATTRIBUTES.md, "Not read yet"). The installed data has one such resolver, a subobject and the
only value resolver of `D_Attributes.Targetable.Targetable_Is_Champion` (counted locally in `Startup.upk`'s export
table). Reference: the C++ stand-in, "not a champion" (0.0) for a resolver whose outer attribute is named
`Targetable_Is_Champion` and lists it in its chain; any other attribute, or an outer that is not such an attribute, is
`NotYet { NotRead }` (the C++ refuses other condition attributes). Off: `NotYet { NotRead }` for get and get-base.
Writes: `NotYet` in both modes. Tests: the four in `targetable_tests.rs`, each mode.

**The provider check** (`run.rs`, the drain and `raise_behavior_event`): a broadcast's class share now skips a pawn by
class, `AIClass` -> `BehaviorProviderDefinition` (`has_class_provider`), instead of matching the "has no behavior
provider" text of an error. Same outcomes on every pinned list.

**The sequence-step tripwire.** When the `sequence` step's own events raise an event on the same pawn's class provider,
that event is deferred past the step (the cost of deferring, decision 35), so it runs on the changed sequence where the
C++ runs it inside the change. Off now answers `NotYet { NotPorted }` (the change itself has happened; the deferred
event is dropped); Reference logs the labelled note `note: combat-run (Rust): the sequence step deferred events on its
own class provider; ...` and runs it as before. Tests: `the_sequence_step_is_one_raise_so_its_events_reach_the_provider_after_it`
(Reference) and `off_mode_refuses_a_sequence_step_whose_events_raise_on_its_own_class_provider`.

**The order test** (decision 35's named difference, now pinned):
`a_broadcast_in_the_middle_of_a_death_sequence_runs_after_the_sequence_unlike_the_cxx`. A death sequence `B1 -> B2`
whose B1 broadcasts `Custom`: the C++ prints `B1, C, K, B2`, this port `B1, B2, C, K`, in both modes. The port does not
match the C++ (re-entering a running provider needs `Provider` methods that do not take `&mut self`); the test asserts
the port's order and that the same behaviors run.

**Not done in this lane.**

- **The pawn hit and the AI hold in the Fire slice** (`ow-mission/src/slice.rs`, granted for the smallest seam). The C++
  slice uses both (`hitFromPlayer` on the dummy after `adopt`, and `Behavior_AIHold` through the dummy's AI component).
  Wiring them needs the combat pack and the combat state's hooks in the slice's world (built by `cmd/slice.rs`), the
  dummy's adoption and the hit through the bridge's controller; and on the installed data that hit would apply 0 until
  the bridge contract item below is settled (the controller's damage modifiers read 0). Left `NotYet` as it is.
- **The kill's destroy.** `Actor.Destroy` of the victim's mind (a `Controller`) is `ow-world`'s rule, which answers
  `NotYet` for the controller and player case (NATIVE_ENGINE_CORE.md, not read); the C++ binds its own stand-in (it only
  marks the actor). In the C++ order the kill credit comes before that destroy, so the credit is made and then the hit
  fails. Not ported here: the native is not this lane's, and the VM offers no way to replace a bound native.

**Verified (automated, local, pc-hummer, on `origin/main` 7f739404, after FIX-KERNEL-NATIVES).** `cargo build --workspace --locked` clean;
`cargo test --workspace --locked` 45 binaries, 1,606 passed, 0 failed, 21 ignored; `cargo build --release -p ow-cli
--locked` clean; `tests/crate_edges_test.py` 19 tests OK; `decisions_index.py --check` OK, 1 added. `ow-combat` 103 tests (6 new bridge
tests on a fake bridge over the toy classes, each in both modes: the C++ texts without a bridge, the health pool, the
kill credit, cash and the bleed frames, the gains, and two runs of the same steps in one process giving the same
report), plus the tripwire's Off test and the order test; `ow-attr` 197 tests (4 new); the `ow-cli` comparison with
the C++ binary on the invented packages unchanged (16 lists in Reference, 13 in Off, equal after the timing fields and
the `SetHealth` lines), and a new `ow-cli` test running each of the 16 lists twice in one process, in each mode, with
the same output. `tests/combat_test.py` on the Rust binary fails the same 3 checks in Reference before and after (the
two `SetHealth` event lines, decision 35's item, and one loot quantity check) and stops in its loot section in Off, as
decision 41 says; `tests/rust_parity.d/combat.txt`: 3 of 3 lines pass. No parity line is added: the invented packages
are not on disk in CI, and `combat_test.py` is not yet whole on the Rust binary.

**The installed game (local, outputs under `local/p2/w6-combat-run2/`, no game data here).**

"Before" is the Rust binary of `origin/main` cf78bdc9, "after" this branch; the C++ binary is the reference. A row is
"equal" when its printed value is the C++'s.

- **W6-COMBAT's 5 spawns** (its 15 steps). Reference: before 16 rows, all equal, 13 of the C++'s 29 missing; after 20
  rows, all equal, 9 missing. The 4 new rows are the two swings' `arrival.*` and `ratio.*` (0.715 for both, as the
  C++), past the Targetable resolver. The 9 missing: the adult's hit and kill (3 + 1), now stopping in its death roll
  at `AmmoDropWeightAttributeValueResolver` (W6-LOOT's open item), and its 5 loot rows. Off: 0 rows before and after;
  every spawn stops at the archetype reference `Default__WillowAIPawn.SpecialMoveComponent0` (decisions 37 and 40).
- **W6-LOOT's bullymong run** (decision 41's step list: seed 20261009, `player:8`, the four kinds spawned and each
  killed once, nine `rollpool` calls of 200, two batches of 100). The C++ prints 180 rows. Reference before: 0 rows,
  every stage-6 spawn stopped at the Targetable resolver. After: 30 rows, 26 equal, 4 differ, 150 missing. Of the 4,
  the two `loot.batch.*.n` are 0 (each batch kill stops at `AmmoDropWeightAttributeValueResolver`), and the two
  `Pool_Money_1or2` item counts (247 / 254 against 252 / 257) follow from the random stream: the C++'s four kills
  rolled loot and drew from it, this port's did not. A run with no kill before the two money rolls gives 8 of 8 rows
  equal. The missing rows are the kills, their loot, the batches' sums and the `rollpool` calls that stop at
  `RandomAttributeValueResolver` or `DesignerAttributeContextResolver` (W6-LOOT's open items). Off: 0 rows before and
  after (the spawns, as above; `lootbatch` is `NotYet` by design).
- **S-f, the down and respawn** (`maya:8 players:1 spawn:adult:7 cash:3000`, four melee swings, `bleed:50`; COMBAT_
  DOWN_RESPAWN.md's first injury). The C++ prints 17 rows. Before: the run is refused before any step (exit 3).
  Reference after: 10 rows, 6 equal, 4 differ (the swings' `ratio.*` are 0: contract item 1), 7 missing (the
  `down.*` rows and the fee: no injury happens). With contract item 1's one-line change made locally (not committed):
  16 rows, all 16 equal (health, arrival, ratio, and `down.total`, `down.successive`, `down.bleed_over`,
  `down.bleed_wall`, `down.camera`, `down.respawn_health`), the fee row missing (contract item 2). Off: 0 rows (the
  pool is `NotYet` at `maya`, the bridge's script reaches the unbound `Actor.LocalPlayerControllers`, and the spawn
  stops as above).
- **S-d, the kill credit** (`maya:8`, a baby L6 spawned without its pawn balance, one lethal shotgun hit, `tick:0`).
  The C++: award 50, credit 35, and the next tick shows 35 in the pool (the live record's 50 / 35, COMBAT_KILL_XP.md).
  Before: refused (exit 3). Reference after: the hit applies 0 (contract item 1). With item 1's change made locally:
  the credit goes into the pool (the tick's gain and the pool read 35, equal to the C++), and the hit then stops at
  `Actor.Destroy` of the mind (contract item 3), so the hit and kill rows stay missing.
- **Replay across processes:** the clean reruns of the three Reference runs above gave the same rows as the first runs.

**Contract items for the hub.**

1. **The bridge's objects start from zero, not from their class defaults** (`ow-mission/src/bridge.rs`, `new_object`
   uses `Vm::instantiate`; the C++ `Runtime::instantiate` copies the class default object). The controller's
   `Instigated*` and `Received*DamageModifier` read 0, so every bridged hit and swing on the installed data applies 0.
   A local experiment with `ow_vm::interp::instantiate_from_defaults` there (not committed) gave the S-f run 16 of the
   C++'s 17 rows equal and the S-d credit 35 equal to the C++ (numbers above). A change for W6-MISSION's owner.
2. `WillowGame.CurrencyAttributeValueResolver` (`ow-attr`): the respawn fee's attribute. The C++ reads a stand-in (the
   amount on hand of `FormOfCurrency` from the replication info, `src/progression.cpp`); the port answers `NotYet`, so
   the `down.fee.<n>` row stays missing.
3. `Actor.Destroy`'s controller and player case (`ow-world`): every installed kill stops there after its credit.
4. A per-provider replacement hook, or a Reference-mode `Actor.Destroy` stand-in, if the hub wants the C++ kill rows
   before the record is read.
