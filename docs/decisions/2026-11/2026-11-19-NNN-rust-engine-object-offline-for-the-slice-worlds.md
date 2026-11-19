## 2026-11-19: Rust engine object for the slice worlds: a GameEngine with an empty online subsystem slot, in Off

AI-assisted (Claude Code). Lane IMPL-ENGINE-OBJECT-OFFLINE (the file number is a placeholder until
`tools/decisions_index.py` takes three digits). It implements the hub's ruling of 2026-11-19 on the headless world's
online state; the hub marked the ruling reversible. Stacked on #479 (`NATIVE_LOGIN_STATUS.md`, decision
2026-11-19-95). No real-game session was run. No dependency was added and no bounds check was loosened.

- **The ruling.** The port has no online subsystem, so the slice worlds register a `GameEngine` object whose
  `OnlineSubsystem` slot is empty. **Port Off answer: the port has no online subsystem; UNVERIFIED against a
  logged-in game.** This is a fact of the port, not an invented game value, and it matches a real game state (playing
  offline). No Steamworks object is made and no login status is set.
- **What changed.**
  - `ow_world::world::online::register_offline_engine` makes the engine from `Engine.GameEngine`'s class defaults.
    It writes the `OnlineSubsystem` slot empty and registers the `EngineObject` VM service.
    - It refuses (`VmError::Registration`) in Reference, and in a world that has an engine object already.
    - A package set without `Engine.GameEngine` (a synthetic world) registers nothing and answers `Ok(None)`; its
      `GetOnlineSubsystem` keeps the no-engine `NotYet { LayerAbsent }`.
  - The combat-run world (`ow-cli`'s `combat::world`) calls it in Off. That is the world the slice gate's combat
    links use: 1, 3, 4, 5, 13 and 14. Reference runs are unchanged, and so is every other world.
- **What follows from the note.**
  - `GameEngine.GetOnlineSubsystem` answers None by its empty-slot rule.
  - `UIInteraction.GetLoginStatus` answers 0 (`LS_NotLoggedIn`) by its installed script.
  - `WillowPlayerReplicationInfo.IsInvalidName` takes the status-0 branch, `ServerSetPlayerNameAsCharName`, as
    installed script.
- **Tests.** In `world/online_tests.rs`, 2 new tests:
  - in Off, the engine is a `GameEngine`, its slot is empty, and `GetOnlineSubsystem` answers None;
  - a second registration is refused;
  - in Reference, registration is refused and no engine object is registered;
  - a package set without `Engine.GameEngine` registers no engine object and keeps the `LayerAbsent` refusal.
- **Installed slice gate (Off, strict), same base with and without the call:** links 1, 3, 4, 5, 13 and 14 move from
  `GameEngine.GetOnlineSubsystem` (the world's engine object, `LayerAbsent`) to
  `PlayerController.UpdateURL` (the URL option write of `SetName`), both "stopped after partial writes: " in
  `maya:8`. The other 8 rows are byte-identical. Numbers in `docs/verification/GATES.md`. This is what the host did,
  not parity.
- **Not changed:** the natives after the login status (`UpdateURL`, `SaveConfig`, `UpdateInputContext`). They stay
  named `NotYet { NotRead }` until 0cde's analyst notes land.
