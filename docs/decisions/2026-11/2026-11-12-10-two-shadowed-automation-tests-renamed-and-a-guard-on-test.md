## 2026-11-12: Two shadowed automation tests renamed, and a guard on test paths and the suite count

AI-assisted (Claude Code). Unreal's automation framework never runs a test whose path is also the folder of other tests.
Two of the host's tests had that shape, and the `ue_automation` suite did not notice.

- **What was shadowed.** The plain `OpenWillow.WeaponAssembly` test (`WeaponAssemblyTest.cpp`, the library planning rules on a
  synthetic library) since its first sub-test, `OpenWillow.WeaponAssembly.DropBeamColor`, came in on 2026-11-06; the hub found it
  on 2026-11-11 and recorded it in `tools/suites.txt`. The plain `OpenWillow.MayaState` test (`OpenWillowMayaStateTest.cpp`, the
  shield, health, down state, stand-in rule and fee checks) since PR #247 added `OpenWillow.MayaState.RespawnWaitSplit` on
  2026-11-12. Evidence, the engine logs of two suite runs on 2026-11-12 on the second machine: the run before #247 completed 11
  tests with `OpenWillow.MayaState` among them; the run on 1442f17 completed 11 with `OpenWillow.MayaState.RespawnWaitSplit` in
  its place and no line for the plain path, and its log says it found 11 automation tests for the `+`-joined names. The suite's pass line stayed `success=11 ... expected=11` because one test
  was lost and one added, so the checks PR #253 added to the MayaState test have never run.
- **Why, from the engine source.** Read in the UE 5.8 source on the zuhu desktop, not seen in a run: the controller's report
  tree keys each node by its full test path, so a test and a folder with the same path share one node, and a node with children
  runs only its children. Each `+` part of `Automation RunTests` is a substring match on the full path (a leading `^` or a
  trailing `$` anchors it), so the suite's names `OpenWillow.MayaState` and `OpenWillow.WeaponAssembly` still select the renamed
  tests.
- **The rename.** `OpenWillow.MayaState.Core` and `OpenWillow.WeaponAssembly.Core`; the C++ class names and the checks are
  unchanged. The suite now expects 13: the eight names of the row match 13 test paths, counted by
  `tests/automation_paths_test.py` with the engine's substring rule.
- **The guard.** `tests/automation_paths_test.py` (ctest `py-automation_paths`) reads every `IMPLEMENT_SIMPLE` /
  `IMPLEMENT_COMPLEX` automation test path under `host/ue5/OpenWillow/Source` and fails, naming both tests, when one path is a
  dot-prefix of another or a path is used twice. It also fails when the `ue_automation` pass line's `expected=`, the count the
  row prints and the count it checks disagree, or differ from the number of test paths the row's names match. A complex test
  under those names is refused, since its count depends on its parameters. Against the base commit it reports both shadowed
  tests and `expects 11 tests but its names match 13`. Synthetic cases use invented paths.
- **UNVERIFIED** until the hub's suite run: that the framework now finds 13 tests for these names, and every result of the two
  renamed tests. By reading the code, every check of `OpenWillow.WeaponAssembly.Core` should pass (its body and the planning code
  are unchanged since the PR #58 records of 2026-10-23 say it passed); the checks of `OpenWillow.MayaState.Core` read against the current code
  should pass too, but none of them has run since #247.
