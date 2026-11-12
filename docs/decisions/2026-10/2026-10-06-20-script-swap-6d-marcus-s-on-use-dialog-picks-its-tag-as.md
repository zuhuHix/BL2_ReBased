## 2026-10-06: script swap 6d, Marcus's on-use dialog picks its tag as script (UNVERIFIED natives; line not played)

AI-assisted (Claude), implementer lane I1, from `docs/verification/NATIVE_BEHAVIOR_CONTEXT.md`. `BehaviorBase.GetBehaviorContext` is bound as
the pure resolver (selectors 0, 1, 2 and 4; anything else None; the instance-data path is not implemented). The behavior thread runner fills a
`BehaviorContextData` Input property (first linked object, selector 4) before a behavior runs and clears its object afterwards; a Context link
that resolves to nothing skips the behavior and follows only a supported default link. The VM Marcus's `ConsumerHandle` starts at -1 and is
assigned once at consumer registration; his mind (a bare `WillowMind` with his archetype's AI class) and dialog component come from his stock
data, so `WillowAIPawn.PlayOnUseDialog` runs its own guards and choice: MissionsAvailable while Fire is NotStarted, AllMissionsInProgress while
Active, MissionComplete at turn-in, NoMissions after. `GearboxDialogComponent.TriggerEvent` is reported as a host event, not played: the
two-step dispatch (generic group no-match, Trigger act, Marcus's own group) is not implemented in DialogSystem. Also bound:
`WillowDialogGlobalsDefinition.Get`. Checks: ctest 11/11, verify_packages 9/9 with class bodies exact, UE build succeeded, quest suite 92/92 +
resume 11/11 (was 90 + 11; rerun by the orchestrator), door suite 16/16. The `GetBehaviorContext` stub is gone; no new stubs. Verified:
automated suites only.

