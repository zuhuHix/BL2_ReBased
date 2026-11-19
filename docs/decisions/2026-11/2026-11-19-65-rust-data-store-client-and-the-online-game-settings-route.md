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
