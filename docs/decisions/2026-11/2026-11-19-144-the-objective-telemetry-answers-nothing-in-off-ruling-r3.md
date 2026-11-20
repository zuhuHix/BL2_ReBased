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
  - **Review r1 of #533, item 1 (high), settled by #519.** The VM on this base binds by the dispatch key, which
    names no package, so a same-named class of another package with the same signature would also get the no-op (the
    status sibling of #509 has the same pattern). #519's package-qualified dispatch settles it across the VM; #533 is
    rebased onto main after #519 lands. Until then the body checks the called function's resolved path and gives any
    other declaration the unbound behaviour: `UnboundNative` under the stub policy, or the legacy stub line where the
    policy allows the key. Test: `another_package_s_objective_telemetry_is_not_the_no_op`, which holds on this base
    and with #519. The status sibling is left to #519.
  - **Review r1 of #533, item 2.** The no-effect test now compares the whole heap (every object's slots, its load
    state and every allocation), the RNG streams, the VM log and the store's log before and after, and adds an omitted
    player (the ordinary argument error, with nothing written). #518's counters are not on this base: a TODO in the
    test names them.
- **Test** (synthetic, `mission_numbers_tests.rs`): `the_objective_telemetry_answers_nothing_for_every_player`. It
  checks:
  - the dispatch key the note names;
  - the answer for a None player and for a player;
  - the player's slots unchanged, no new object, and nothing logged;
  - the script error for a non-object player.
- **Not covered by R3:** the other `Record*` events. The note says each needs its own check that it ends in the
  shared submit routine.
