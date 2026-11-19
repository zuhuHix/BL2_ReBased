## 2026-11-19: ActionSkill.GetBehaviorProviderDefinition, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-GET-BEHAVIOR-PROVIDER-DEFINITION, slice link 9. Coded from
docs/verification/NATIVE_GET_BEHAVIOR_PROVIDER_DEFINITION.md alone (#553 at ff49fd67), with the hub's ruling to take
its proposed Off answer and the hub's notes on #553's r1 review. Clean room: lane ANALYST-GET-BEHAVIOR-PROVIDER-
DEFINITION (another session) read the executable; this lane did not open it or that lane's analysis output. Synthetic
fixtures only.

- **What changes (`ow-skills` `action_skill::provider_definition`, registered by the action-skill pack, Off only,
  first-wins):** `WillowGame.ActionSkill.GetBehaviorProviderDefinition` answers the receiver's own
  `BehaviorProviderDefinition` field as stored, None included:
  - no archetype lookup, no class default and no load;
  - the field is read straight from the heap, never through a loading read;
  - the definition it names stays unloaded, and the event that receives it loads or refuses it.
- **UNVERIFIED (low: a fitted step):** that the C++ interface method the native reaches is the one that reads this
  field. The note ties it by a structural fit, not by name (its open question 1). The rule is bound, labelled, as
  the hub ruled.
  - It agrees with the consumer registration (the skill registering with the archetype's
    `BehaviorProviderDefinition`, NATIVE_ACTION_SKILL.md section 6) only if a spawned skill's field still names the
    archetype's subobject. That is the note's open question 2, not read.
  - On link 9 the harness runs the script on the archetype itself, so the two agree there.
- **Refusals** (none is marked, since nothing is written):
  - a receiver whose class does not declare `BehaviorProviderDefinition` as an object reference is the port's data
    error, `VmError::Script`. It can come only from broken data, so its tests build such classes by hand;
  - an unloaded receiver is `NotYet { NotRead }`, "GetBehaviorProviderDefinition: an unloaded receiver", because
    reading its field would need a load.
- **Binding:** only `ActionSkill`'s declaration. In the installed packages neither `LiftActionSkill` nor
  `ExecuteActionSkill` declares its own copy, so their calls reach this one. The other 21 declaring classes stay
  unbound. Reference binds nothing.
- **Tests** (`ow-skills` `provider_definition_tests.rs`, 6 after review r1 of #560):
  - a subclass receiver's field, and None;
  - a lazy definition returned unloaded, with nothing written;
  - the data errors on hand-built classes;
  - the unloaded receiver refused unmarked, with nothing written and the handle still unloaded. Main has no #518
    counters yet, so the heap's `Debug` text stands in for them;
  - Off binds and Reference does not;
  - a slotless stand-in of an `ActionSkill` class refused the same way, unmarked, nothing written, and the subclass's
    method lookup finding the declaration (review r1 of #560).
- **Checks:** `cargo test --workspace -j 2` 2,449 passed, 0 failed, 46 ignored; clippy shows nothing new; crate edges
  OK.
- **Installed `--slice-gate`, Off, strict, under the machine lock,** on a throwaway local merge with #523 (d2227301),
  #535 (7f8fc7d7), #548 (22b29bfb) and #555 (0e9092a8), never pushed. The merge took #518's counter files, renamed
  #523's two counter calls locally, and joined the action-skill pack registration. Result: 1 ok / 3 not yet / 10
  error.
  - Link 9 passes `spawn` (1 of 2) and gets past this native in `cast`.
  - It now stops at the unbound `Object.GetFuncName()`, the next argument of the same `ActivateBehaviorEventFromScript`
    call. So the Parameters rule is still not reached.
- **Review r1 of #560:** a stand-in with no slots whose class has some (which `is_unloaded` does not report) is now
  the same `NotRead` refusal instead of a heap error; the fitted step is labelled low, as the accepted note (ced46479)
  has it; the consumer registration is cited from NATIVE_ACTION_SKILL.md section 6.
- **UNVERIFIED.** Nothing here was checked in the running game. The note's live checks would confirm it.
