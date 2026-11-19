## 2026-11-19: Read RunBehaviors as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane RUN-BEHAVIORS, analyst only. `BehaviorBase.RunBehaviors` is where 8 Reference checks of
`tests/mission_script_test.py` stop during the level-up. The special move of slice link 9 runs behavior lists too. The
new own-words note `docs/verification/NATIVE_RUN_BEHAVIORS.md` and five `NATIVE_INDEX.md` rows record, as behaviour
only:

- `RunBehaviors`: one shared kernel-info record with no thread, only its initial-run flag set; the list in index
  order, empty entries skipped, its length re-read each step; each behavior's script `ApplyBehavior`; nothing
  returned or checked; no consumer, provider or process. Output links and waits of behaviors run this way lead
  nowhere.
- The default `ApplyBehavior` (every resolved context, then `ApplyBehaviorToContext` per context) and
  `Behavior_RunBehaviorCollection`: one context, the collection found through
  `InstanceDataHelper.GetFirstInstanceDataObject`, run with the original arguments. That is the level-up's path.
- The special-move callers: `SpecialMoveComponent.RunBehaviors` behind its `PlayServer` / `PlayClient` / `PlayLocal`
  test, with the owner as self; and `GearboxAnimDefinition.OnStart` / `OnServerStart`, kernel event activations.

This extends the paragraph on `RunBehaviors` in NATIVE_DOWN_TIMERS.md without contradicting it. Raw output stays under
ignored `local/analysis/runbeh/`; the Ghidra runs were read-only and took the machine lock. Nothing is implemented:
everything named stays unbound or `NotYet` in Off until a fresh lane codes it from the note. Every rule is UNVERIFIED
in the running game; the note lists the bl-sdk checks and three open questions.
