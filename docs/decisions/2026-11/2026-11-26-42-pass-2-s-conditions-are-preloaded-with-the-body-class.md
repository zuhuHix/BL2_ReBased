## 2026-11-26: Pass 2's conditions are preloaded with the body class

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-PASS2-CONDITION-PRELOAD, on main 785c2a4. Clean room: this lane did not
write NATIVE_BEHAVIOR_KERNEL_PASS2.md and did not read the executable. No session was run. UNVERIFIED in the running
game.

- **What was decided:**
  - **the preload** (`ow-cli` `combat::body_class_preload`, the maya step's Off harness, #779's walk): after each
    provider definition it loads, it now also loads that definition's `BehaviorSequences` entries'
    `CustomEnableCondition` objects. They go in sequence order, each once, then each condition's `LinkedObjective` when
    it is set. That is exactly what pass 2 needs resident (NATIVE_BEHAVIOR_KERNEL_PASS2.md section 4, steps 1 and 7);
  - **what is not loaded:**
    - the `LinkedMission`, which pass 2 identifies by path and reads only when it is Complete (only the bridge's own
      mission can be);
    - anything the condition or the objective references in turn;
  - **the log:** each load is logged as #779's are, "harness input: set-up preload: <path> (UNVERIFIED: streamed with
    the character)", then a count line of its own. A resident object is not logged. A stand-in is skipped and logged
    (#779's rule, shared);
  - **the refusal's text:** pass 2's not-loaded refusals, for the condition (preflight and re-evaluation) and the
    `LinkedObjective`, now name the object: "<text>: <store path>". With no path (a run-time object) the text is as
    before. The kind (`NotRead`) and the record are unchanged.
- **Reference:** unchanged.
- **The slice gate** (installed, Off, strict, release build, under the machine lock; main 785c2a4 and this change): 6
  ok / 8 `NotYet`, 51.2% on both.
  - **7 pass-2 objects** were preloaded: `PlayerEvents_Siren`'s four `BehaviorSequenceEnableByMission` conditions, and
    three linked objectives in the Episode 11 and 17 mission packages.
  - **Links 1, 3, 4, 5, 13 and 14** pass `InitializeBehaviorProviders`' pass 2. They stop at
    **`WillowPlayerPawn.SetDefaultStance`: "the class's own implementation is not resolved"** (`NotRead`,
    NATIVE_GO_DOWN_PATH.md), NATIVE_MAYA9_AND_AFTER.md section 2's row 7.
  - Before, they stopped at the unloaded condition's `NotRead`.
  - Links 2 and 6 to 12 are unchanged.
- **Tests:**
  - `body_class_preload_tests.rs`, an invented package: the exact load list, with the toy player event definition's
    sequences. That is two conditions, one met twice, a sequence with none, and the first condition's objective, with
    their own count line. The first condition's `LinkedMission` and another reference stay unloaded. A second run
    still loads nothing;
  - `ow-behavior`: the conditioned-player-event test pins the named refusal.
- **Gates:** `cargo test -p ow-cli -p ow-behavior` all pass (218 and 155 in the two library suites); `crate_edges_test.py`
  OK.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game.
- **Sensitive files:** none.
