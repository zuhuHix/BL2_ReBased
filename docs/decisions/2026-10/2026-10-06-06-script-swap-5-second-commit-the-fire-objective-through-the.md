## 2026-10-06: script swap 5, second commit: the Fire objective through the stock behavior script (UNVERIFIED rules)

AI-assisted (Claude), orchestrator acting as implementer from NATIVE_OBJECTIVE_TRIGGERS.md and NATIVE_ENGINE_CORE.md only. The dummy's
`Behavior_UpdateMissionObjective` runs its own `ApplyBehaviorToContext` on the VM instead of the slice's direct update; new binding
`BehaviorBase.GetWorldInfo` (the bridge's world), and the VM world's `Role` is authority. The context object is None (no VM dummy
pawn), which yields bit 0 as a pawn context does. Checks: CTest 11/11 (toy WorldInfo is now an Actor, as the stock class is),
packages 9/9, UE build Succeeded, quest PASS 83/83 and resume 11/11, door PASS 16/16. No toy coverage of the script behavior yet.

