## 2026-11-16: Rust Fire mission slice, --slice-run and the dialog seam as shipped

AI-assisted (Claude Code). Lane W6-FIRE of the Rust port fills `crates/ow-mission/src/slice.rs` (the port of
`src/slice.cpp`, `vm::FireMissionSlice`) and `crates/ow-cli/src/cmd/slice.rs` (`--slice-run`, the port of its branch in
`src/cli.cpp`). It builds on decisions 2026-11-15-34 (the tracker and the bridge), 2026-11-15-32 (dialog and movers),
2026-11-15-30 (the behavior kernel), 2026-11-15-29 (pools and the level-up rebase) and 2026-11-15-27 (states). No
parsing behaviour changes. There was no real-game session in this lane: every rule keeps the label its record gives it
(NATIVE_BEHAVIOR_POPULATION.md sections B and C, FIRE_MISSION_GOLDEN_TRACE.md, NATIVE_OBJECTIVE_TRIGGERS.md,
NATIVE_PROGRESSION.md, NATIVE_LEVEL_UP_ATTRIBUTES.md), UNVERIFIED, except what those records say was observed live.

- **Shipped shapes.** `slice::FireMissionSlice` (a cheap handle): `new(world, mission_path, map_package,
  provider_path)`, the host's calls of the C++ (`accept`, `use_marcus`, `screen`, `close_screen`, `confirm_reward`,
  `touch_waypoint`, `enter_range`, `spawn_dummy`, `damage_dummy`, `hit_dummy`, `turn_in`, `cheat_level_up`,
  `set_region_game_stage`, `set_player_experience`, `set_player_health`, `tick`, `save_state` / `load_state`), `drain`,
  `errors`, `not_yet`, and the dummy's report (`dummy_boundary`, `dummy_boundary_calls`, `dummy_enabled_sequences`,
  `dummy_trace`). `HostKind` (the C++ `HostEvent::Kind`, with the `--slice-run` JSON names) and `HostEvent` are
  `#[non_exhaustive]`. The mission is read from `Startup` and the dummy, the waypoint and the director from the given
  map package, as the C++.
- **Re-entrancy.** The dummy's provider lives in a slot of the slice, taken out while it runs. Its
  `Behavior_UpdateMissionObjective` handler reaches the tracker through the bridge, whose notifications re-apply the
  dummy's conditions on the same provider: the handler lends it back with `tracker::lend` (the second use of `lend`
  that decision 2026-11-15-34 waited for; it needed no public hook on the kernel, because the handler is the slice's
  own). The tracker's notify hook returns nothing, so an error it meets is kept and returned by the public call that
  caused it, after that call's own work (the C++ exception would unwind the tracker's routine at once).
- **The dialog** is the mission's own (`MissionSystem::dialog`, lane W6-MISSION's wiring, PR #337): the slice uses
  that `Rc` (the C++ `mission_.dialog()`) for the dummy pawn's talker and the CLI's `lines:`, `timedlines:` and
  `talker:` steps, through `MissionSystem::with_dialog`. Before #337 landed this lane carried its own attach seam; it
  was dropped at the rebase.
- **Seam edits in other files** (granted by the hub as the smallest seam edits; each needed to wire the slice, or asked
  for by the review of PR #337):
  - `bridge.rs`: `apply_behavior` takes `Option<ObjRef>` (the C++ passes a null context for the dummy's objective
    behavior; it had no other caller). The mission screen's stand-in writes the director and the UI manager in the
    property's declared kind (`set_reference`): the installed movie class declares some of those properties as
    interfaces, which the invented packages did not, so on the installed game the screen was "not hosted" before.
    `TriggerEvent` hands the dialog the store's object for the director's body class (`store_object`, below), and
    `store_object`, `instantiate_copy` and `error_text` are `pub(crate)` so the slice reuses them (one place each). The
    `MarcusUse::on_use_tag` comment no longer calls the dialog another lane's.
  - `tracker.rs`: `MissionSystem::with_dialog` (the borrow and its "in use" refusal in one place); the documentation
    of `dialog` states its two rules (never replace `DialogSystem::on_line`, where the tracker's sink lives; never
    `dialog::register` a mission's dialog on the VM); `EffectKind::Dialog`'s comment no longer says "never emitted".
  - `tracker/tests.rs`: the doc comment of `the_dialog_behavior_runs_on_the_tracker_s_own_handler` now says what the
    test asserts.
  - The director's body class (in `TriggerEvent`) and the dummy pawn's (at the spawn) are references of copies, that
    is stand-ins of the archetype's package; the dialog loader resolves a stand-in only through the mission package,
    so both are handed over as the store's object for that export (`store_object`), as the C++ loads a stand-in from
    its own package. Still needed after FIX-DEFS-PREFIX (#336): the store now loads actor archetypes at 4, but the
    bridge still makes the director as a copy (`instantiate_copy`). Checked on the installed run: without
    `store_object` 10 of its 22 step records differ (every line an echo talker, the on-use, dismissal and turned-in
    tags lost).
  - `tests/mission_script_test.py` (granted by the hub, fixture only, invented values, no assertion changed, each with a
    one-line comment): the toy shapes now match the installed ones the Rust readers depend on. The toy
    `ResourceDefinition.DefaultResourcePoolClass` names a metaclass; the toy `PlayerController` derives from an
    `Engine.Controller`; the toy `ExpLevel` carries the plain-attribute flag; the toy comparison enum lists the
    installed order (`LessThan` before `GreaterThan`). A fifth of the same kind (approved by the hub): the map toy's
    `PersistentLevel` is an `Engine.Level` (a declared class, added last to the toy Engine
    package), because since #336 the store reads a placed actor's 26-byte prefix only under a `Level`, and the toy
    waypoint otherwise does not load. The C++ binary passes the whole file.
  - The `ow-cli` scaffold (granted by the hub): `--slice-run` was the last unported command, so `cmd::not_ported` and
    `args::Family::owner` (used only by it) are removed, the doc comments of `main.rs`, `args.rs` and `NOT_PORTED`
    (exit code 3 now means a `NotYet` met during a command) say so, and `crates/ow-cli/tests/not_ported.rs` keeps the
    two cases that still mean something (no arguments; the parity variable's value, whose valid form now reaches the
    command's own package error), its header saying every command is ported.
- **Choices, each a C++-parity choice unless said otherwise.**
  - `advance_time` adds the tick's seconds to `WorldInfo.TimeSeconds` (a double sum kept as a float, the C++), not
    `World::tick`, which ticks actors; the C++ slice ticks none.
  - The dummy's pawn is a copy of `GD_TargetDummy.Character.Pawn_TargetDummy` with the ordinary 4-byte prefix (the
    C++ `instantiateExport`), made at the first spawn, registered as a talker under its body class's name tag and
    bound to the named Kismet variable `TargetDummy` after the spawn behaviors.
  - `--slice-run` reruns on a store whose load set is `Startup` and the given file (`IdentityPolicy::Loaded`) when the
    store refuses a file's copy of a path another file holds (the invented root holds two copies of the mission
    package), as `--mission-run` does with its one file.
- **Stand-ins of the C++ (architecture R3): Reference reproduces, Off answers.**

  | Stand-in | Reference (the C++) | Off | Pinned by |
  |---|---|---|---|
  | the player's health pool after a level-up (`max_health`, `health`; the `health:` input) | at `player:` / `health:` a pool built by hand over the class's `HealthPoolDefinition` (the resource's `DefaultResourcePoolClass`, in a `ResourcePoolManager` on the controller, the controller as provider) and seeded (full, or the given current); the player pawn's `HealthPool` reference set to it (the C++ `linkPlayerHealth`: the manager, slot 0, the pool's GUID byte, the pool); after a level-up its maximum and current reported after the skill points, the C++ order Level, SkillPoints, MaxHealth, Health | `NotYet { NotRead }` at `health:` and at a level-up of a class that names a health pool (the class-defaults routine that builds pools is not read) | unit tests `the_health_stand_in_reproduces_the_cxx_events` (an invented class, pool and resource: the four events in order and the pawn's reference in Reference, `NotYet { NotRead }` at `cheatlevel` in Off) and `the_health_stand_in_by_mode`; `mission_script_test.py` S7 / S7b (local, below) |
  | `Behavior_AIHold` on the dummy before it has a mind | a recorded no-op | `NotYet { NotPorted }`: the mind comes with the combat chain, which `ow-combat` has (W6-COMBAT) but the slice does not wire yet (follow-up W6-COMBAT-RUN2) | unit test `the_ai_hold_stand_in_by_mode`; the installed run (the Idle sequence's hold at the registration) |
  | 18 of the 20 natives the C++ logs as zero-result stubs on the installed Fire route (`REFERENCE_STUBS` in `cmd/slice.rs`, each entry with its reason: `Actor.LocalPlayerControllers`, `MissionTracker.GetObjectivesProgress`, `Object.Localize`, the GFx and Leviathan calls, ...) | allowed as logged zero-result stubs (`StubPolicy::Refuse { allow }`, the logged texts are the C++ ones) | refused, `VmError::UnboundNative` (architecture 3.8) | CLI unit test `the_reference_allow_list_is_reference_only`; the installed run |

  The allow-list was approved by the hub (R3: Reference only). It **deliberately differs from `--mission-run`**, whose
  VM refuses these natives in both modes: decision 2026-11-15-34's refusal of `Actor.LocalPlayerControllers` and
  `MissionTracker.GetObjectivesProgress` still holds for the tracker and the bridge until their rules are read; the
  slice only lets the C++ bytes be reproduced under `OW_PARITY=reference`. The other two natives the C++ stubs on that
  route, `Actor.SetTimer` and `Actor.ClearTimer`, are not listed: the Rust world binds them (lane W5-WORLD), so the Rust
  stub list lacks their lines. The slice never runs the world tick, so no timer fires and nothing else differs.

  Not stand-ins, `NotYet { NotPorted }` in both modes: the player's hit through the stock pawn chain (`pawnhit:` once
  the dummy has a pawn; the combat chain is not wired into the slice yet, follow-up W6-COMBAT-RUN2; unit test
  `the_pawn_hit_chain_is_not_yet`), and the level-up's skill behaviors (the bridge's `RunBehaviors` inside the
  level-up, decision 2026-11-15-34; lane W6-SKILLS). Read but UNVERIFIED in both modes: Toggle as a flip of the
  current state.
- **Verified (automated, local, pc-hummer, on origin/main 34117d3).**
  - 20 slice unit tests on invented packages (`slice/tests.rs`: the registration order, the verdicts against the
    tracker, restrictions, another mission's state, objective-specific conditions, the unsupported condition class,
    the dummy's and the mission's sequence changes, the range wrapper, the direct hit, each stand-in by mode and what
    the health pool reproduces, the game time, the mission's dialog, the C++ event names, the missing provider's text,
    and replay determinism: one scripted run on two fresh worlds in one process gives identical events, errors, trace
    and report); 3 CLI unit tests. `cargo test --workspace`: every test passes, `cargo build` gives no warning.
  - ctest, all 173 entries registered and passing. The 7 that run the Rust binary (`rust-parity`, `item-registry-rust`,
    `item-roll-rust`, `item-stats-rust`, `item-pools-rust`, `mover-rust`, `def-dump-synthetic`) register only when
    `target/release/ow-package.exe` exists at configure time; an earlier count of 166 was a configure from before the
    release binary was built, and so had run without them. `rust-parity` runs `tests/rust_parity.d/slice.txt` (4
    lines, all passing; 48 of 48 lines in 23 files).
  - `tests/mission_script_test.py`'s 20 `--slice-run` commands, replayed on both binaries (a local harness): with the
    fixture changes above, 13 of 20 give the C++ stdout and exit code byte for byte under `OW_PARITY=reference` (all of
    D, S7 and F, and the inside-the-guard calls of S7b, S9 and S10); the other 7 reach the level-up's skill behaviors
    and exit 3. In Off mode 5 of 20 (D, F and the S7 call without a level-up); every other call is a level-up, and
    there the health pool is `NotYet`. The C++ binary passes the whole file.
  - `--mission-run` keeps PR #337's numbers on invented packages after these seams: its 34 commands of that file give
    34 of 34 identical under Reference and 33 of 34 under Off (the one difference is scenario W's lent weapon,
    `NotYet` by decision 2026-11-15-34).
  - The installed game, local only (output under `local/`): `tools/verify_fire_event_order.py` with the Rust reader,
    `OW_PARITY=reference`: **29 of 33 rules hold** (the C++: 33 of 33). Of the default run's 22 steps, 20 give the C++
    step record byte for byte; the turn-in and the reward confirmation differ (below). All 17 step records of the lines
    run match; it differs only in its error line (below) and so its exit code (1). In Off mode the run stops at the
    accept (refused natives) and at the spawn (the dummy's hold, `NotYet`, exit 3), so the tool reads no JSON.
  - **The hub accepted 29/33 for this PR; the remaining 4 rules move to FIX-PLAYTHROUGH (#341, 2 turn-in rules) and
    FIX-DIRECTOR-STORE (2 no-errors rules).**
- **Open items.**
  - `ow-attr`: `WillowGame.PlayThroughCountAttributeValueResolver` is `NotYet { NotRead }`; the installed turn-in's
    reward percentage reaches it, so the turn-in has no experience and no reward page (2 of the 4 failing rules, and
    the reward confirmation's).
  - The director is still a copy (the bridge's `instantiate_copy`; lane W6-MISSION), so its references are stand-ins
    even after FIX-DEFS-PREFIX (decision 36): `EvaluateInitializationData` on one of them (`StartLingerTimer` at the
    use key) is a heap error line ("no slot"), in both installed runs (the other 2 failing rules: "the run has no
    errors"). Loading the director through the store, which #336 now allows, would remove it and `store_object`.
  - W6-SKILLS (the level-up's skill behaviors: S7b's refill, S9, S10), and W6-COMBAT-RUN2 (wiring the landed combat
    chain, #335, into the slice's `pawnhit:` and the dummy's hold). The 77 installed `--mission-run` missions were not
    rerun after the screen seam.
  - The 18 Reference stubs: each is an unread rule; Off mode refuses them until it is read.
