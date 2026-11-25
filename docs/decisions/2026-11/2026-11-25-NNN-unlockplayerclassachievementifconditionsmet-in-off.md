## 2026-11-25: UnlockPlayerClassAchievementIfConditionsMet in Off, for link 9's lift

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-UNLOCKPLAYERCLASSACHIEVEMENT, slice link 9. Coded from
docs/verification/NATIVE_UNLOCK_CLASS_ACHIEVEMENT.md section 4 alone (#662, decision 2026-11-25-01). Clean room: the
analyst lane read the native, this lane did not open the executable or the analysis output. Synthetic fixtures only.

**What changes** (Off only; Reference keeps the stub policy):
- `ow_inventory::globals::class_achievement` implements
  `WillowPlayerController.UnlockPlayerClassAchievementIfConditionsMet(byte,int,bool,bool)` as the note's preflight
  steps 1 to 8 describe:
  - the receiver resident;
  - the four arguments, an omitted one or `Value::None` read as its zero;
  - `PlayerClass` an object field. None, or a class other than `WillowGame.PlayerClassDefinition` or a child of it,
    answers false; an unloaded handle of that class is `NotRead`;
  - `PlayerClassAchievementData` checked against its declaration;
  - the selection; false when nothing is selected;
  - otherwise `ClientUnlockAchievement`, looked up with `Vm::find_loaded_method` before each call (code not loaded
    is `NotRead`; resident code without it is a `VmError::Script`), once per selected achievement, then true;
  - #518's counters at entry decide the "stopped after partial writes" mark.
- **Binding:** one keep-existing `register`, Off only, and only for the function declared under the note's key. The
  inventory's globals registration and the action-skill pack (link 9's world registers no inventory pack) both call
  it, so whichever runs first binds the one function.

**Choices the note leaves open** (UNVERIFIED):
- The class test comes before the load test. The cast needs only the handle's class, so an unloaded handle of
  another class answers false without a refusal.
- Each stored entry must be of the declared struct, not only the declaration (the lesson of review r1 of #664). The
  note names no path for the entry struct, so the declaration is checked by its three fields, as the note says.
- The achievement byte is passed to the event as an int, the sibling port's reading of `AchievementId`.
- The event is looked up again before each call. The first lookup comes before any event, so both refusals are
  write-free there. A later one would be marked as after a partial write if the counters moved.

**Tests** (the note's 1 to 11, synthetic; each preflight refusal and false answer asserts #518's three counters, the
name table, the heap and the event list unchanged): `class_achievement_tests.rs` in `ow-inventory` (11 tests, 1 to 10
and test 11's keep-existing and Reference half). `action_skill/class_achievement_tests.rs` in `ow-skills` covers test
11's either-order binding beside the inventory registration (2 tests).

**Not verified:**
- anything in the running game;
- the field name `PlayerClass`;
- the cast's class;
- the event name, shared with the sibling native;
- the port's impact count at link 9's call;
- where link 9 stops next (the installed slice gate was not run: there is no install on this machine yet).
