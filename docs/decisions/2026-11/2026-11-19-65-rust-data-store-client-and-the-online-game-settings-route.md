## 2026-11-19: Rust data store client and the online game settings route in Off, from the analyst note

AI-assisted (Claude Code). Lane IMPL-ONLINE-GAME-SETTINGS, the fresh implementer of `NATIVE_ONLINE_GAME_SETTINGS.md`
(analyst lane ANALYST-NEXT-STOP, decision 2026-11-19-61; read on `research/rust-next-stop` at 40c0a894). Its section 5
is the Off contract by the hub's ruling of 2026-11-19. This lane coded from the note alone and opened no analysis
output. There was no real-game session, so **every rule is UNVERIFIED** in the running game. No dependency was added
and no bounds check was loosened. The `NotYet` this lane replaces is decision 2026-11-19-19's layer-absent
`GetDataStoreClient`, which the note's contract replaces with the modelled client.

- **What was added** (`crates/ow-world/src/world/datastore.rs`, bound by the world in Off only; Reference keeps the
  VM's stub policy, R3):
  - **`UIInteraction.GetDataStoreClient`** answers the world's one engine client. A `DataStores` VM service holds it,
    made on the first call from the `Engine.DataStoreClient` class defaults together with its global stores. The
    stores are made first and the client last; any object made before a failure is freed again. The note's
    class-default fallback is out of scope for Off (section 5).
  - **The global stores.** The port reads no configuration file, so the list names the one store the note read from
    the engine configuration that this path needs: `WillowGame.WillowOnlineGameSettings_DataStore`. It is made from
    its class defaults (its `Tag` and its settings list).
  - **`DataStoreClient.FindDataStore`** (section 3):
    - a `None` tag answers None;
    - a call with a player is `NotYet { NotPorted }`: the port models no player data stores;
    - otherwise the global stores are searched by `Tag`, compared without case;
    - a tag no registered store has is `NotYet { NotRead }`, because the configuration's other global stores are not
      registered, so a None could be wrong;
    - a call on another client is `NotYet { NotPorted }`.
  - **The settings entry is not assumed empty** (section 5). A store that declares `GameSettingsCfgList` is answered
    only when its first entry's `GameSettings` is set. Otherwise the lookup is `NotYet { NotRead }` naming open
    question 2: what fills that entry in a world with no online subsystem. The script reads the entry as a plain field
    right after the lookup, so the lookup is the last native boundary before that read.
  - **Branch 1, the online subsystem,** is bypassed, not refused: the host has none, so `GetGameSettings` is never
    called and nothing is bound for it.
  - **The natives after the settings are found** (section 5, steps 1 to 5) are bound in Off as `NotYet { NotRead }`
    naming the note: `GetActiveMissionNumber`, `GetActivePlotCriticalMissionNumber`, `GetCurrPlaythrough`,
    `GameEngine.GetDLCManager`, `HasCompatibilityData`, `StaticGetDlcContentIdForCurrentMap` and
    `Settings.SetIntProperty`. A path the package set does not declare as a native binds nothing. None of them is
    reached while the settings entry is a refusal.
- **Tests** (`world/datastore_tests.rs`, 4, invented `Engine.upk` and `WillowGame.upk`):
  - one client per world, with the store and its tag from the class defaults;
  - `FindDataStore`'s cases, including the settings entry: no entry, an entry with no `GameSettings`, a filled one;
  - the unread natives;
  - a client that cannot be made, which leaves no instance behind.
  - The world test of decision -19 now checks the binding in each mode and that a failing call makes nothing.
- **Installed `--slice-gate`** (pc-hummer; first on main b985dcbf plus the note, then again after the rebase onto
  `research/rust-next-stop` 4a9974ea, with the same rows; before and after; rows under ignored `local/`):
  - Strict 0/14/0 and past-errors 0/14/0, before and after. Only links 10 to 12 change, the same way in both:
  - before: stopped at the accept step on `UIInteraction.GetDataStoreClient` (decision -19's layer-absent `NotYet`);
  - after: stopped at the accept step on `DataStoreClient.FindDataStore`'s settings-entry `NotYet` (open question 2).
  - An earlier draft that took the entry as empty went past it and stopped at `BehaviorBase.RunBehaviors` outside
    the level-up (the bridge's Off refusal on this main). The hub's ruling removed that guess.
- **Not changed:** the note and its index row (on `research/rust-next-stop`) are left to their lane.
- **Review r2 of #448.**
  - **Item 1, `FindDataStore` follows the note.** A player's own stores are searched first. The port's players hold
    none, so the global stores follow on that miss. A tag no store has answers None. The NotYet stays only for a call
    on another client.
  - **Item 2, the settings entry.** The open-question-2 `NotYet` moved off the lookup to the settings-entry read.
    - In Off, the controller's script `GetOnlineGameSettingsViaDatastore` is overridden. Of the note's two route
      functions it is the one that answers the settings, by the installed declarations; its sibling
      `GetOnlineGameSettingsDatastore` has a `gameSettingsName` local and answers the store.
    - The override runs the store lookup as script. With no store it answers None (the note's "refresh does
      nothing"). Otherwise it is the named `NotYet`, whatever the entry holds.
    - A caller that only wants the store gets it.
  - **Item 3, preflight.** The getter takes the service's mutable borrow and finds every class it makes before it
    allocates anything. A held borrow or a missing class leaves nothing behind.
  - **Item 4, terminal failures.** A failure after the first object is made is terminal. A `NotYet` gets the
    "stopped after partial writes" mark, with its kind kept; other errors pass unchanged; nothing is rolled back.
    Reference is as it was.
  - **Item 5, tests (6).**
    - All seven downstream bindings are `NotYet`, and branch 1's `GetGameSettings` binds nothing.
    - A refresh run as script on invented packages answers None with no store, and stops at the named settings-entry
      read with one.
    - The player-then-global search; the held borrow and the missing class; the partial-writes mark.
  - **Installed `--slice-gate`:** strict 0/14/0 and past-errors 0/14/0. Links 10 to 12 stop at the accept step on
    `WillowPlayerController.GetOnlineGameSettingsViaDatastore`'s settings-entry `NotYet` (open question 2). Before this
    branch they stopped on `UIInteraction.GetDataStoreClient`. Only those three rows differ.
  - **Follow-up, outside this PR's files:** whether the slice accept path keeps going to the event pump after a
    script failure (`ow-mission`'s slice and bridge) is left as it is.
