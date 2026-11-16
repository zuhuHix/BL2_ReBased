## 2026-11-15: Rust mission tracker, script bridge and --mission-run as shipped

AI-assisted (Claude Code). Lane W6-MISSION of the Rust port fills `crates/ow-mission/src/tracker.rs` (the port of
`src/mission.cpp`, `vm::MissionSystem`), `crates/ow-mission/src/bridge.rs` (the port of `src/mission_script.cpp`,
`vm::MissionScript`, for what `--mission-run` reaches plus the waypoint and observer pieces the slice needs) and
`crates/ow-cli/src/cmd/mission.rs` (`--mission-run`, the port of its branch in `src/cli.cpp`). It builds on decisions
2026-11-15-30 (the behavior kernel), 2026-11-15-29 (pools and the level-up rebase), 2026-11-15-26 (the world) and
2026-11-15-18 (the experience curve and the mission reward). No parsing behaviour changes. There was no real-game
session in this lane: every rule keeps the label its record gives it (NATIVE_MISSION_DISPATCH.md section B,
NATIVE_MISSION_SCRIPT_BRIDGE.md, NATIVE_PROGRESSION.md sections 2 and 3, NATIVE_CONTROLLER_HELPERS.md,
NATIVE_OBJECTIVE_TRIGGERS.md, NATIVE_USE_INTERACTION.md, NATIVE_MARCUS_USE_CHAIN.md, NATIVE_BEHAVIOR_CONTEXT.md),
UNVERIFIED, except the lent weapon's place in the status routine, which was observed (FIRE_MISSION_GOLDEN_TRACE.md,
lane L2).

- **Shipped shapes.** Public enums and structs that may grow are `#[non_exhaustive]`.
  - `tracker::MissionSystem` (a cheap handle; clones are the same mission): `new(vm, package, path)`, `accept`,
    `kickoff`, `play_turn_in`, `update_objective`, `update_objective_by_path`, `custom_event`, `turn_in_mission`,
    `tick`, `drain`, `errors`, `not_yet`, the queries (`status`, `active_set`, `available`, `can_start`, `can_end`,
    `completed_branch`, `objective_state`, `tracker_lines`, `lends_weapon`, ...), `bind_kismet_variable`, `note`,
    `save_state` / `load_state`, and four hooks (`set_status_hook`, `set_kickoff_hook`, `set_notify_hook`,
    `set_objective_hook`). `Status`, `EffectKind`, `Effect`, `TrackerLine`, `Notification`, `link::*` (B7),
    `find_object` and `lend`.
  - `bridge::MissionScript` (a cheap handle): `new(world, mission)`, `accept`, `turn_in`, `sync_restored`,
    `set_region_game_stage`, `set_player_experience`, `update_experience_pool`, `cheat_level_up`, `place_marcus`,
    `mission_lists`, `use_marcus`, `close_screen`, `confirm_reward`, `place_waypoint`, `touch`, `untouch`, `notify`,
    `apply_behavior`, `report_json`, and the readers the C++ has (`controller_status`, `exp_earned`, `stubs`, `notes`,
    `player_controller`, `player_pawn`, ...). `bridge::world(vm)` makes the `ow_world::World` the bridge needs.
- **Re-entrancy** (the tracker and the bridge are re-entrant in the C++: a behavior handler calls back into the
  tracker, which fires more events on the same provider; a script hook calls natives that read or change it). The
  state is behind a `RefCell` borrowed only for short scopes, and each provider lives in a slot it is taken out of
  while it runs. A handler lends the running provider back to its slot while it calls out (`tracker::lend`: the
  running provider is swapped for a copy of its data, the kernel's suspended call never sees the copy, and the
  provider comes back with what the nested calls did), so the nested calls are the C++ nested calls, depth-first.
  `unsafe` is forbidden in the workspace; this is the safe form. A call that finds the slot empty is refused
  (`tracker::PROVIDER_BUSY`, `NotYet { NotPorted }`); no path reaches it.
- **Choices, each a C++-parity choice unless said otherwise.**
  - The bridge places its `WorldInfo` as the world's and its `WillowGlobals` as the world's globals, so the world's
    `GetWorldInfo` natives and the evaluator's no-context resolver answer with them. It does not bind
    `BehaviorBase.GetWorldInfo`, `RecalculateAttributeInitializedState` or `ResourcePoolManager.RecalculateBaseValues`
    again (the world's packs bind them, decision 2026-11-15-29; the VM refuses a second binding).
  - The experience curve and `GetExperienceReward` are `ow_balance::curve::experience_required` and
    `ow_balance::xp::mission_xp`, with the percentage evaluated with the controller as the source (the record; the
    C++ used an empty context) and the playthrough from the replication info's `CurrentPlaythrough`. The second
    playthrough below level 50 therefore gets the record's multiplier 1, where the C++ refuses every playthrough but
    the first.
  - The director (`GD_Marcus.Character.Pawn_Marcus`), his directive table, AI class and dialog component are copies
    instantiated from their exports with the C++ prefix (`ow_object::cdo::instantiate_export` over a decode context
    of their own, the C++ `instantiateExport`), not loaded through the definition store: the store's prefix rule
    (decision 2026-11-15-16) gives an `Engine.Actor` export outside a level 26 bytes and refuses one whose tags do not
    parse from there, which the invented director's (4-byte) do not. Open for W3-DEFS (below).
  - `--mission-run` reruns on a store whose load set is the given file alone (`IdentityPolicy::Loaded`) when the
    store refuses a file's copy of a path another file holds, wherever the refusal shows (the command's error or an
    error line a script run recorded), as `cmd/behavior.rs` does for a provider.
  - The status-changed `Default` event, the kickoff record, the lent weapon and the objective queue are the C++ order
    (module documentation of `tracker.rs`).
- **`NotYet` where the C++ runs a stand-in or another lane's system.**
  - The dialog (lane W5-DIALOG, `dialog.rs` still a stub): `Behavior_TriggerDialogEvent` has no handler, so the kernel
    records its `unsupported behavior class` line and `NotYet { NotPorted }` and stops that branch; the dialog steps
    of `--mission-run` (`lines:`, `timedlines:`, `talker:`, `component:`, `pawn:`, `kismet:` with a talker) and the
    director's `GearboxDialogComponent.TriggerEvent` are `NotYet { LayerAbsent }`. `--mission-run` exits 3 when the dialog
    behavior was reached or a script run stopped on a `NotYet`; a behavior class neither binary handles gives the same
    error line in both and the C++ exit code.
  - The level-up's health pool (swap 7b; the C++ builds one by hand, the record's way is `CreatePool`, which Off mode
    refuses on the installed data, decision 2026-11-15-29): not built; `set_player_health` is `NotYet`.
  - `BehaviorBase.RunBehaviors` inside the level-up (the C++ stand-in applying the OnLevelUp skills' effects; skills
    are lane W6-SKILLS): `NotYet { NotPorted }`.
  - `GetExperienceReward` of a mission with optional objectives (the C++ logs them as not implemented and leaves them
    out): `NotYet { NotPorted }`.
  - The pawn health and hit chain (`CombatSystem`) is lane W6-COMBAT's and is not in the bridge.
  - `--mission-run` is a gameplay command, so its VM refuses an unbound native (`StubPolicy::Refuse`, architecture
    3.8): a native the C++ logs as a stub is an error line here.
- **Fixture-only changes (granted by the hub).** `tests/behavior_test.py` and `tests/mission_script_test.py`: a toy
  `Core.upk`; every object property names the toy `Core.Object` and every toy class derives from it; an array
  parameter or local gets an inner property; struct parameters, locals and results name a struct (toy
  `BehaviorKernelInfo`, `BehaviorParameters`, `BehaviorConsumerHandle`, `BehaviorContextData`,
  `NameBasedObjectPath`, `MissionDirectorData`); `ResourcePools[16]`; and declarations the Rust readers require and
  the C++ defaults (`AttributeInitializationDefinition.RandomVariance`, `RangeRestriction`, `RoundingMode`,
  `GlobalAttributeValueResolver.GlobalAttribute`, `SimpleMathValueResolver.Arg1Option` and `Arg1Attribute`, the
  controller's `CharacterClass` and `MyWillowPawn`), all unset. No assertion changed; both files still pass on the
  C++ binary.
- **Verified (automated, local, pc-hummer).**
  - `tests/behavior_test.py` passes on the Rust binary (a new line in `tests/rust_parity.d/behavior.txt`).
  - `tests/mission_script_test.py`, run with every `--mission-run` on the Rust binary and every other command on the
    C++ one (a local harness): every check outside the dialog scenario E passes. Of its 34 `--mission-run` commands,
    19 give the C++ stdout and exit code byte for byte; the 15 that differ are scenario E's (exit 3 here).
  - 19 tracker and 17 bridge unit tests on invented packages (`tracker/tests.rs`, `bridge/tests.rs`), one or more per
    rule, including the whole of `behavior_test.py` section 6; `tests/rust_parity.d/mission.txt` (3 lines).
  - The installed game, local only (output under `local/`): the 77 missions the Sanctuary packages reference (all
    exported in `Startup.upk`), each run by both binaries with `accept tick:0.5 tick:0.5 turnin` (the tracker) and
    with `stage:8 player:8:2000 script:accept tick:0.5 script:screen script:turnin pool` (the bridge).
    - Tracker: 49 of 77 give the C++ stdout and exit code byte for byte. The other 28 reach
      `Behavior_TriggerDialogEvent` (the dialog, lane W5-DIALOG) and exit 3; in 26 of them every other error line
      equals the C++ one.
    - Bridge: 0 of 77 equal. 44 stop on a native the C++ logs as a stub and the Rust gameplay command refuses
      (`Actor.LocalPlayerControllers` in all 44, `MissionTracker.GetObjectivesProgress` in 31); the other 33 reach the
      dialog behavior first.
- **Open items.**
  - W3-DEFS: the prefix of an actor archetype outside a level (the director): the C++ reads 4 bytes.
  - W5-DIALOG: the dialog handler, the dialog steps and the director's dialog; then the dialog scenario E and the
    dialog missions of the installed game.
  - The natives the installed controller script reaches that neither binary implements (the C++ stubs them):
    `Actor.LocalPlayerControllers` and `MissionTracker.GetObjectivesProgress` first. Each needs its rule read before
    the bridge runs the installed accept and turn-in.
  - W6-FIRE: `--slice-run` builds on `MissionScript` (waypoint, observers, `apply_behavior` are ported; the health
    pool, the level-up skills and the combat calls are not).
  - The architecture's `cmd/mission.rs` row lists `--mission` and `--tick`; both are `--kismet-run`'s modifiers in
    the C++ (`crates/README.md` already says so) and are lane W5-KISMET's.
