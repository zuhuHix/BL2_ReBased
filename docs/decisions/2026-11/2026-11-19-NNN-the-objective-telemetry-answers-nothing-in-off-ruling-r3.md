## 2026-11-19: The objective telemetry answers nothing in Off (ruling R3)

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-R3-MISSION-OBJECTIVE, implementer, for slice link 11. The number NNN is a
placeholder; the hub gives it.

- **Clean room.** Coded from NATIVE_RECORD_MISSION_OBJECTIVE_CHANGED.md alone, section 3 (analyst lane ANALYST-LINK12,
  #531 at `research/rust-link12-next` 35cf2fd), and the hub's ruling R3. This lane did not read the native, and it
  opened no analysis output.
- **No real-game session.** The rule is a port answer, **UNVERIFIED** in the running game.
- **No dependency and no parsing change.** Reference is unchanged: the VM's stub policy still applies there.
- **The branch.** It is stacked on #509 (`fix/rust-link10-next`), where ruling R3's sibling, the status telemetry, is
  bound in the same module.
- **What is bound** (`ow_world::world::mission_numbers`, bound with the module's other natives in Off only):
  `WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer(object,int,int,int,name)`.
  - **The declared parameters**, in order, are `ForPlayer`, `ObjectiveIndex`, `ObjectiveProgress`,
    `PreviousObjectiveProgress` and `MissionDefinition`, with no return value. This follows the hub's reading on the
    review of #531; the note's draft named them by fit, and its analyst corrects it. All three script calls pass the
    old value third and the new value fourth. Nothing reads the integers or the name, and their meaning stays
    UNVERIFIED.
  - It answers nothing, for every player, a None one included. There is no write, no load and no log line. R3 adds no
    telemetry-specific refusal or preflight.
  - The ordinary argument errors still apply, as for the status event: a player argument that is not an object is a
    `VmError::Script`.
  - It is bound by its dispatch key, the same known limit as the status event's (#509). Package-qualified dispatch
    comes with #519.
- **Test** (synthetic, `mission_numbers_tests.rs`): `the_objective_telemetry_answers_nothing_for_every_player`. It
  checks:
  - the dispatch key the note names;
  - the answer for a None player and for a player;
  - the player's slots unchanged, no new object, and nothing logged;
  - the script error for a non-object player.
- **Not covered by R3:** the other `Record*` events. The note says each needs its own check that it ends in the
  shared submit routine.
