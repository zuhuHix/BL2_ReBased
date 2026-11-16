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
  (`tracker::PROVIDER_BUSY`, `NotYet { NotPorted }`). A lend whose call takes the provider and does not give it back
  marks the mission lost: every later call on it is refused (`tracker::PROVIDER_LOST`), rather than the placeholder
  copy running (review r1 of PR #334). The director's provider in the bridge has the same lost state, and every
  reader and caller of it refuses with `PROVIDER_LOST` once it is set (review r2 of PR #334): the registration check
  behind `IsBehaviorSequenceEnabled` and `ActivateBehaviorEventFromScript`, the path branch of
  `ResolveBehaviorProviderDefinitionReference`, and every call that takes the provider out of its slot (the use key's
  `OnUsed`, the consumer registration). Unit tests: `a_lost_provider_makes_the_mission_unusable` (the mission),
  `a_lost_director_provider_refuses_its_readers` (the director).
  - **Wave-6 contract limit (review r1 of PR #334).** Only the tracker's and the bridge's own handlers can lend. A
    behavior handler of another lane, registered in the kernel's registry, that reaches a tracker native which fires
    an event (`ActivateMission`, `CompleteMission`, `PlayTurnIn`, `UpdateObjective`, a custom event) finds the slot
    empty and gets `PROVIDER_BUSY`. A public lending hook (on the kernel's `RunInfo` or the provider) would be an
    additive follow-up; the hub keeps `lend` until W6-FIRE shows a second use.
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
    parse from there, which the invented director's (4-byte) do not. Open for W3-DEFS (below). **Untested on installed
    data:** no installed run here reaches a placed director (the missions are run from `Startup.upk`, which holds no
    `GD_Marcus` pawn), so the 4-byte decode of the real archetype is checked only on invented packages.
  - **One bridge per fresh world, not transactional.** `MissionScript::new` refuses a world that already has actors
    (it places its own `WorldInfo` as the level's first actor). After an error part way through, the objects it made
    stay in the world's VM, its `WorldInfo` may already be placed and some natives may be bound: the world cannot be
    used after a failed `new`. The CLI builds the bridge once and then fails as a whole; W6-FIRE and the host must drop
    the world too.
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
  - The pawn health and hit chain (`CombatSystem`) is lane W6-COMBAT's and is not in the bridge.
  - `--mission-run` is a gameplay command, so its VM refuses an unbound native (`StubPolicy::Refuse`, architecture
    3.8): a native the C++ logs as a stub is a script error line here, and the run's `NotYet` count decides exit 3.
    The bridge's own stubs follow the same rule (review r1 of PR #334; the first draft logged and returned zero in
    both modes, which got around the policy).
- **The bridge's C++ stubs: Reference-only stand-ins (architecture R3 and section 6.1; review r1 of PR #334).** In
  `ParityMode::Reference` each keeps the C++ bytes (its `UNIMPLEMENTED ...` stub line, and the C++ result); in Off
  mode each answers honestly:

  | Stand-in | C++ (Reference) | Off | Pinned by |
  |---|---|---|---|
  | a bound native called on another object (`outside_binding`) | stub line, zero result | the stub policy decides: `VmError::UnboundNative` under `Refuse`, the stub under `LogAndZero` | no tracked test reaches it (the toys call every native on its own object); unit test `a_call_outside_the_binding_is_the_unbound_behaviour` |
  | `GetCurrencyReward` with a non-zero `CreditRewardMultiplier` | stub line, 0 credits | `NotYet { NotRead }` (the credit formula's values) | unit test `credits_with_a_multiplier_by_mode` (no tracked test: the toys' multiplier is 0, the Fire mission's too) |
  | `GetCurrencyReward` of a mission with optional objectives | stub line, their rewards left out | `NotYet { NotPorted }` | unit test `optional_objective_currency_by_mode` |
  | `GetItemRewardsForPlayer` with reward items or pools | stub line, no items | `NotYet { LayerAbsent("ow-combat loot (lane W6-LOOT)") }` | unit test `reward_items_by_mode` |
  | `GetBehaviorContext` with an `InstancedDataContextName` | stub line, None | `NotYet { NotPorted }` | unit test `instance_data_context_by_mode` (scenario H's probes have no name) |
  | `RunBehaviors` outside the level-up | stub line, nothing | `NotYet { NotPorted }` | unit test `not_implemented_is_the_cxx_line_in_reference_and_not_yet_in_off` |
  | `GetExperienceReward` of a mission with optional objectives | stub line, their terms left out | `NotYet { NotPorted }` | unit test `optional_objective_experience_by_mode` |

  `RunBehaviors` inside the level-up (the C++ skill-effect stand-in) is not ported in either mode (`NotYet`, W6-SKILLS).
- **Rules neither side models (review r1 of PR #334).** The C++ goes on silently; here Off mode answers `NotYet`
  where the data shows the feature, decided before anything changes, and Reference keeps the C++:

  | Rule | Off | Reference (the C++) | Pinned by |
  |---|---|---|---|
  | an objective set of another class than `MissionObjectiveSetDefinition` (collection, branching) | `NotYet { NotPorted }` at `MissionSystem::new` | the error line `unsupported objective set class ...` when the set is reached | none in the tracked tests; installed `M_KillYourself` |
  | the lent weapon's region-stage condition (NATIVE_MISSION_SCRIPT_BRIDGE.md) | `NotYet { NotRead }` in the Active and Complete status routine of a mission that lends its weapon | granted inside Active, removed inside Complete, the condition not applied | `mission_script_test.py` scenario W (so W runs in Reference mode only) |
  | `bRepeatable` on a Complete mission (`CanStartMission`, the mode read from the VM) | `NotYet { NotRead }` | false | unit test `a_repeatable_complete_mission_is_not_yet` |
  | a plot-critical mission whose activation finds a pending kickoff record (the overwrite) | `NotYet { NotRead }` | the pending record kept, so the tick plays `PlayKickoffDialogOnly` (13), not the activation's `PlayKickoff` (12) | unit test `a_plot_critical_overwrite_is_not_yet`, which sets the record (a one-mission tracker cannot reach the state) |
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
    C++ one (a local harness). With `OW_PARITY=reference`: every check outside the dialog scenario E passes, and of its
    34 `--mission-run` commands 19 give the C++ stdout and exit code byte for byte, the 15 that differ being scenario
    E's (exit 3 here). In Off mode scenario W stops on the lent weapon's unread condition (4 checks; 18 of 34 equal).
  - `tests/behavior_test.py` passes on the Rust binary in both modes.
  - 23 tracker and 25 bridge unit tests on invented packages (`tracker/tests.rs`, `bridge/tests.rs`), one or more per
    rule and, for each stand-in, both modes; they include the whole of `behavior_test.py` section 6.
    `tests/rust_parity.d/mission.txt` (3 lines).
  - The installed game, local only (output under `local/`): the 77 missions the Sanctuary packages reference (all
    exported in `Startup.upk`), each run by both binaries with `accept tick:0.5 tick:0.5 turnin` (the tracker) and
    with `stage:8 player:8:2000 script:accept tick:0.5 script:screen script:turnin pool` (the bridge).
    Counted in both parity modes (review r1 of PR #334; the C++ binary is unchanged):
    - Reference: tracker 49 of 77 give the C++ stdout and exit code byte for byte; the other 28 reach
      `Behavior_TriggerDialogEvent` (the dialog, lane W5-DIALOG) and exit 3, and in 26 of them every other error line
      equals the C++ one. Bridge 0 of 77: 44 stop on a native the C++ logs as a stub and the Rust gameplay command
      refuses (`Actor.LocalPlayerControllers` in all 44, `MissionTracker.GetObjectivesProgress` in 31), the other 33
      reach the dialog behavior first.
    - Off: tracker 40 of 77 equal; 19 reach the dialog behavior, 13 are refused at construction for a collection or
      branching objective set, 5 in the status routine for the lent weapon's unread region-stage condition. Bridge 0 of
      77: 40 refused natives (`LocalPlayerControllers` 40, `GetObjectivesProgress` 25), 24 dialog, 13 set class.
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

## Addendum (2026-11-16): the dialog wired

AI-assisted (Claude Code). W5-DIALOG landed as 413eafd (PR #333, decision 2026-11-15-32), and lane W6-MISSION wires it
(branch `fix/rust-mission-dialog-wiring`; this addendum is named by decision 2026-11-16-39). No parsing behaviour
changes; every dialog rule keeps W5-DIALOG's labels.

- **One dialog per mission.** `MissionSystem` owns one `Rc<RefCell<DialogSystem>>` over the mission's package
  (`MissionSystem::dialog`), as the C++ tracker owns its `DialogSystem`:
  - `dialog::handler(rc)` runs `Behavior_TriggerDialogEvent` on the mission's own provider (`Provider::handle`), not
    through the VM's registry;
  - the provider's time hook advances the dialog clock to the kernel's time (the frame's without a provider);
  - the status routine sets the tracked mission's priority floor (`MissionDialogGroup`, `bPlotCritical`) while
    Active or ReadyToTurnIn;
  - every line the dialog chooses is a `Dialog` effect with the C++ detail text.
- **The bridge shares that `Rc`.** The director's `GearboxDialogComponent.TriggerEvent` does what the C++ does:
  `pawn_dialog` of his body class, then `trigger_on_component` on `MissionSystem::dialog`. It records the tag as the
  turned-in, dismissal or on-use dialog. There is no second dialog system and no second binding.
- **`--mission-run`.** The dialog steps (`lines:`, `timedlines:`, `talker:`, `kismet:`, `component:`, `pawn:`) run as
  in the C++. The exit-3 rule for the dialog behavior is gone: an unhandled behavior class gives the C++ line and
  the C++ exit code.
- **Tests (invented packages).**
  - `a_bridge_dialog_event_is_seen_by_the_tracker_in_reference_mode` and `..._in_off_mode`: the bridge's `TriggerEvent`
    starts a line that is the tracker's `Dialog` effect. The tracker's own dialog behavior has no talker before it;
    after it, it finds the talker the bridge registered and is held back by that line, which is still live. It starts
    once that line has ended on the tracker's clock.
  - `the_dialog_behavior_runs_on_the_tracker_s_own_handler`, in both modes.
- **Verified (automated, local, pc-hummer).**
  - `tests/mission_script_test.py` with `--mission-run` on the Rust binary:
    - Reference mode: all 34 `--mission-run` commands give the C++ stdout and exit code byte for byte (19 before),
      and the whole file passes. Scenario E passes.
    - Off mode: 33 of 34; scenario W, the lent weapon's unread condition, is the one left by design, and scenario E
      passes. This needed the fixture to declare the toy Engine class `AkEvent`, the class of the dialog scenario's
      `Ak_*` exports (a fixture-only change granted by the hub; decision 2026-11-16-39). Without it the Off-mode
      definition store decodes a `TalkAkEvent` naming them as None (Reference keeps the reference), and 13 scenario E
      runs chose no line (20 of 34).
  - The installed game, local only (the 77 Sanctuary missions, both step lists of the first count):
    - Reference: tracker 73 of 77 identical (49 before); the 4 others differ only in an objective name's case, the
      first spelling the name table keeps (`ow-object`), which the dialog difference hid before. Bridge 1 of 77 (0
      before); 76 stop on the refused natives (`LocalPlayerControllers` 76, `GetObjectivesProgress` 59).
    - Off: tracker 58 of 77 (40 before): 13 objective-set-class refusals, 5 lent-weapon refusals, 1 name case. Bridge
      1 of 77 (0 before): 63 refused natives, 13 set class.
- **Open.**
  - The name table's first-spelling rule against the package's own spelling (`ow-object`).
  - The two refused natives (hub decision: they stay refused).
