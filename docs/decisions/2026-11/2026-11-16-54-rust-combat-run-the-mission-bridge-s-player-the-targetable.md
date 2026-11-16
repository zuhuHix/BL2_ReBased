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
  recorded as the bridge's guard records it (`<step>: mission script: <text>`) and the run goes on, as the C++ guard
  does (review r1 of PR #356): a failed hit carries on with an empty report (its `hit.<id>.N.*` rows 0, the hit counted,
  the kill rows and the down state processed), a failed swing with an empty swing (`arrival.*` 0, no ratio). A
  `NotYet` is not a script error: it stops the step in a hit, a swing and a bleed frame alike, with no made-up rows
  (review r2). Test:
  `a_bridged_hit_or_swing_that_fails_keeps_its_rows_with_zeros_as_the_cxx_guard`. One ordering is not the C++'s: when
  the step then fails outside the guard (its player report), the C++ lists the guard's error under the next step,
  this port under the same one.
- **The first `maya` step sets the player count** (scenario setup, as the first `player:` step since decision
  2026-11-16-49): the bridge's player is the run's one local player, so the game's `EffectiveNumPlayers` becomes 1
  while it holds its class default 0. Without it every installed kill credit is `NotYet` (`KillCredit::players` holds 1
  to 4, decision 35's contract item). The C++ reads 0 there; on the installed table the S-d credit below is the same.
  The count is set before the pool's maximum is evaluated, as the C++ context always counts 1 (review r1).
- **Stand-in, `ParityMode::Reference` only (R3):** the player's health pool. The C++ bridge builds one by hand
  (`ensureHealthPool` / `seedHealthPool`: a manager on the controller, the class's `HealthPoolDefinition`, the maximum
  from `BaseMaxValue` with the controller as the source, the pawn's `HealthPool` reference to slot 0), a stand-in for
  the class-defaults routine that the Rust bridge does not build. `run/player.rs` reproduces it in Reference; Off
  answers `NotYet { NotPorted }` at `maya` and `mayahealth`. As the C++ (review r1): a build error other than a
  `NotYet` gives no pool, with the manager and the pool cleared, and the pawn's `HealthPool` reference is relinked on
  every fetch of the pawn (`pawnFor`). Tests:
  `maya_sets_the_bridge_player_and_builds_the_cxx_health_pool_only_in_reference_mode`,
  `mayahealth_seeds_the_stand_in_pool_in_reference_and_is_not_yet_in_off`,
  `the_maya_step_counts_the_player_before_the_pool_and_relinks_the_pawn_on_each_fetch`,
  `the_health_pool_build_swallows_a_data_error_and_clears_as_the_cxx_catch`. The Fire slice holds its own copy
  of the same stand-in (`ow-mission/src/slice.rs`); the two are not shared (different crates), an open item.
- **The bleed-out's injured path** (review r1) is tested on a toy pawn in a package of its own (`ToyInjured`, outside the
  packages the C++ digests were taken over), whose `SwitchToSidearm`, `Tick`, `InjuredDeathAnimDone`,
  `CalcInjuredDeadCamera` and `ShowRespawnDialog` are overridden in the test: the three callbacks at 2.28, 0.84 and
  11.8 s, the camera's end, the `bleed_over`, `bleed_wall`, `camera`, `respawn_health` and fee rows, both modes
  (`the_bleed_step_plays_the_injured_state_at_the_cxx_delays`).
- **The bridge's objects start from their class defaults** (contract item 1 of the first round, granted by the hub,
  review r1): `ow-mission/src/bridge.rs` `new_object` now uses `ow_vm::interp::instantiate_from_defaults`, as the C++
  `Runtime::instantiate` copies the class default object. Before, the controller, the replication info and the pawns
  started from zero, so the controller's `Instigated*` and `Received*DamageModifier` read 0 and every bridged hit and
  swing on the installed data applied 0. Test: `bridge::tests::the_bridge_objects_start_from_their_class_defaults`
  (an invented class default 1.0 on the toy controller). The `ow-mission` and `ow-cli` suites (`--mission-run`,
  `--slice-run`) pass unchanged.
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

**The order tripwire** (decision 35's named difference; reviews r1 and r2 of PR #356). A death sequence `B1 -> B2`
whose B1 broadcasts `Custom`: the C++ re-enters the running provider and prints `B1, C, K, B2` in the hit's `events`
(its inner raise lists the provider's lines at the moment of the raise); this port defers `Custom` and prints `B1, B2,
C, K`, and a later `CompareBool` in B2 could branch differently. The port does not reproduce the re-entry (`Provider`
methods take `&mut self`), so each event deferred onto a running provider records the provider's trace length when
the raising behavior started (a script behavior of the run marks it; a handler that marks nothing is judged
conservatively: more than one behavior ran) and the length of the events list at the deferral. The broadcast's class
share gets the same mark whenever the class provider is running (review r2). When the provider is back, the C++ list
would differ, and the tripwire trips, if after the deferral: its own run went on past the raising behavior; anything
was listed before the provider's own final report of lines (a note, another provider's lines, a second broadcast, the
next context of a loop); or a second event was deferred onto the same held provider. `Off` answers `NotYet {
NotPorted }` (the deferred events are dropped); `Reference` logs `note: combat-run (Rust): an event raised on a running
behavior provider was deferred past what the run did after the raise; ...` and runs them as before. A raise by the
last behavior that runs, with nothing listed after it, trips nothing. A raise made while a provider is being made
(its consumer registration) is not checked. Tests, each in both modes:
`a_broadcast_in_the_middle_of_a_death_sequence_is_not_yet_in_off_and_a_note_in_reference`,
`a_raise_by_the_last_behavior_is_not_tripped_and_an_unmarked_one_is_judged_conservatively`,
`events_deferred_by_a_deferred_event_trip_on_the_second_deferral_onto_one_provider` (the full events list) and
`a_broadcasts_class_share_onto_the_running_class_provider_is_checked`; `marked_leaves_no_stale_entry`. The 16 pinned
C++ lists of `ow-cli` trip nothing.

**Not done in this lane.**

- **The pawn hit and the AI hold in the Fire slice** (`ow-mission/src/slice.rs`, granted for the smallest seam). The C++
  slice uses both (`hitFromPlayer` on the dummy after `adopt`, and `Behavior_AIHold` through the dummy's AI component).
  Wiring them needs the combat pack and the combat state's hooks in the slice's world (built by `cmd/slice.rs`), the
  dummy's adoption and the hit through the bridge's controller. Left `NotYet` as it is; an open item for a follow-up
  (the hub's OK, review r1).
- **The kill's destroy.** `Actor.Destroy` of the victim's mind (a `Controller`) is `ow-world`'s rule, which answers
  `NotYet` for the controller and player case (NATIVE_ENGINE_CORE.md, not read); the C++ binds its own stand-in (it only
  marks the actor). In the C++ order the kill credit comes before that destroy, so the credit is made and then the hit
  fails. Not ported here: the native is not this lane's, and the VM offers no way to replace a bound native.

**Verified (automated, local, pc-hummer, on `origin/main` 7f739404, after FIX-KERNEL-NATIVES; review r1).** `cargo build
--workspace --locked` clean; `cargo test --workspace --locked` 45 binaries, 1,613 passed, 0 failed, 21 ignored (1,606
before review r1); `cargo build --release -p ow-cli --locked` clean; `tests/crate_edges_test.py` OK; `decisions_index.py
--check` OK, 1 added. After review r2: 1,615 passed, 0 failed, 21 ignored; `ow-combat` 111. `ow-combat` 109 tests (review r1 added the order tripwire's two tests, the guard's, the injured
path's, `mayahealth`'s, the relink's and the pool build's; `ow-mission` one, the class defaults). First round: 6 bridge
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
  DOWN_RESPAWN.md's first injury). The C++ prints 17 rows. Before: the run is refused before any step (exit 3). First
  round: 10 rows, 6 equal, 4 differ (the swings' `ratio.*` were 0: the bridge's zero-filled controller), 7 missing.
  After review r1 (the bridge's objects from their class defaults): 16 rows, all 16 equal (health, arrival, ratio, and
  `down.total`, `down.successive`, `down.bleed_over`, `down.bleed_wall`, `down.camera`, `down.respawn_health`), the
  fee row missing (open item 2; since review r2 its `NotYet` stops the `bleed` step after every other row was written).
  No tripwire note. Off: 0 rows (the pool is `NotYet` at `maya`, the bridge's script reaches the unbound
  `Actor.LocalPlayerControllers`, and the spawn stops as above).
- **S-d, the kill credit** (`maya:8`, a baby L6 spawned without its pawn balance, one lethal shotgun hit, `tick:0`).
  The C++ prints 9 rows: award 50, credit 35 (the live record's 50 / 35, COMBAT_KILL_XP.md), the hit, the loot. Before:
  refused (exit 3). After review r2: 2 rows, both equal (the baby's health and maximum); the hit makes its credit (the
  next tick's gain and the pool read 35, equal to the C++), then stops at `Actor.Destroy` of the mind (open item 3),
  whose `NotYet` now stops the step, so the hit, kill and loot rows (7) are missing rather than printed as zeros (review
  r1 had printed 9 rows, 2 of them zeros against 9999). Off: 0 rows.
- **Replay across processes:** the clean reruns of the Reference runs above gave the same rows as the first runs; the 15
  W6-COMBAT steps give the same 20 rows after review r1.

**Open items for the hub** (items 2 to 4 stay open, review r1).

1. Settled in review r1: the bridge's objects now start from their class defaults (above).
2. `WillowGame.CurrencyAttributeValueResolver` (`ow-attr`): the respawn fee's attribute. The C++ reads a stand-in (the
   amount on hand of `FormOfCurrency` from the replication info, `src/progression.cpp`); the port answers `NotYet`, so
   the `down.fee.<n>` row stays missing.
3. `Actor.Destroy`'s controller and player case (`ow-world`): every installed kill stops there after its credit.
4. A per-provider replacement hook, or a Reference-mode `Actor.Destroy` stand-in, if the hub wants the C++ kill rows
   before the record is read.
5. The Fire slice's pawn hit and AI hold (above), and the two health pool stand-ins (combat run, slice) not shared.
