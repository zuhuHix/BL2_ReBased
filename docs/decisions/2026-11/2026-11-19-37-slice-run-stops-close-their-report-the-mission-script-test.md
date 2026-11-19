## 2026-11-19: Slice-run stops close their report; the mission script test grades a NotYet as not yet

AI-assisted (Claude Code). Lane FIX-MISSION-SCRIPT-TEST. `tests/mission_script_test.py` failed on main with the Rust
binary in both modes; this lane makes it grade what the Rust port does, without loosening any check. No dependency was
added, no bounds check was loosened and no `NotYet` guard was relaxed. No game rule changed.

- **The cause in Off: a cut-off report.** `--slice-run` wrote its JSON as it went and returned mid-report when a step
  raised a `NotYet`, so `json.loads` failed. The test crashed in `level_events('stage:4', 'player:4:30', ...)` (S7),
  and `tools/verify_fire_event_order.py` could not read a stopped installed run.
- **The fix in `ow-cli` (`cmd/slice.rs`).** A step that meets a `NotYet` now ends the run with a well-formed report:
  - that step's record has `"ok":false`, its events so far and `"not_yet":<text>`;
  - no later step runs, and the report's usual tail follows;
  - then `"stopped":{"step","not_yet"}`;
  - a run that met any `NotYet` (a stop, or one a script run or the dummy's provider recorded and went on past) also
    lists their texts in `"not_yet":[..]`;
  - the exit code stays 3, and the stop's text still goes to stderr.

  The C++ has no `NotYet`, so both keys appear only on runs that cannot match it, and a run that compares with the C++
  keeps its bytes: `tools/rust_parity.py` holds 49 of 49 lines, the 4 `--slice-run` lines among them. Any other step
  failure (usage, a number parse) keeps the C++'s cut-off shape.
- **The test (`tests/mission_script_test.py`).**
  - After a `--slice-run` that met a `NotYet`, a check that fails is listed as a named not-yet with the run's `NotYet`
    text. It is never a pass. A check that holds still holds.
  - Exit 1 when any check failed, 3 when none failed but some are not yet, 0 when all pass.
  - The `--mission-run` scenarios are graded as before.
- **Scenario W's fixture.** The toy `WeaponMission` now also has a `GameStageRegion` (a toy `ToyRegionDefinition`).
  - The check was written to the C++ rule, which grants the lent weapon on the objective test alone. Off's rule
    (NATIVE_MISSION_SCRIPT_BRIDGE.md, decision 2026-11-16-81) also needs a region, so Off failed the check against a
    fixture neither rule's author meant to differ on.
  - With the region both rules grant, and the check stays as strict as it was.
  - The C++ binary still passes the whole file.
- **`tools/verify_fire_event_order.py`** grades a stopped run:
  - rules on steps the run finished are graded as before;
  - a rule that needs the stop step or a later one, or the whole run (the dummy's order), is `NOTYET`, never a pass;
  - a short run without a stop is still refused;
  - exit 1 on a failure, 3 when only not-yets remain;
  - its self-test gained a stopped-run case.
- **The Reference failures: a real gap, not lost expected-failure rows.** The 8 failing Reference checks are S7b's
  refill (2), S9 (4) and S10 (2). They all reach `BehaviorBase.RunBehaviors` during the level-up.
  - In Reference that is the C++ stand-in, `src/mission_script.cpp` `runBehaviors` / `applySkillEffects`: the OnLevelUp
    collection's skill effects applied by their data (NATIVE_LEVEL_UP_ATTRIBUTES.md section 7). About 160 lines, never
    ported. The bridge answers `NotYet { NotPorted }` (decision 2026-11-16-38 listed it as W6-SKILLS' open item).
  - The test never marked these as expected failures: the C++ binary passes them all, and the Rust binary never did
    (decision 38: "the other 7 reach the level-up's skill behaviors and exit 3").
  - Closing it means porting that stand-in, which uses the NATIVE_LEVEL_UP_ATTRIBUTES and NATIVE_ATTRIBUTES records
    rather than a NATIVE_BEHAVIOR_* one. That is more than this lane's few lines, so it is reported, not papered over:
    they are now named not-yets, and the test exits 3 in Reference.
- **Results** (pc-hummer; outputs under ignored `local/p2/fix-mission-script-test/`):
  - **Off:** 0 failures, 17 not yet, exit 3 (before: a crash). The 17 are S7, S7b, S9 and S10 checks that stop at the
    player's health pool, which is `NotYet { NotRead }` in Off by design (decision 2026-11-16-38).
  - **Reference:** 0 failures, 8 not yet (the `RunBehaviors` gap above), exit 3 (before: 8 failures, exit 1).
  - **The C++ binary:** passes, exit 0 (unchanged).
  - **Installed Fire, `verify_fire_event_order.py`:**
    - Off: 13 of 33 rules hold, 3 fail, 17 not yet (before: an exception on the cut-off report). The run stops at
      `hit:fire` (the Pawn IAIInterface routing). The 3 failures are real: the accept's screen close comes in another
      order in Off, and both runs have error lines (`TriggerDetachSprites`, `AIComponent.ActivateEvent`,
      `UIInteraction.GetDataStoreClient`).
    - Reference: 33 of 33, as before.
- **ctest.** `tests/CMakeLists.txt` registers this test (`mission-script-synthetic`) with the C++ binary only. Running
  it on the Rust binary as well would need a new registration there. That file was not edited, because CMakeLists files
  are sensitive; the maintainer can add it.
- **Open:**
  - the level-up `RunBehaviors` stand-in port (Reference);
  - the Off health pool (`NotYet` by design);
  - a Rust registration of this test in ctest.
