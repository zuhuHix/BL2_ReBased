## 2026-10-06: script swap 5: GoToRange through the stock waypoint script (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, finished by the orchestrator after the subagent hit a usage limit; both worked from
NATIVE_OBJECTIVE_TRIGGERS.md only. The placed `WillowWaypoint_9` runs its own script on the VM (observer registration, `Touch`, the
set-changed re-check over `Touching`); new natives `Actor.IsPlayerOwned`, `MissionTracker.IsMissionObjectiveActive` /
`IsMissionObjectiveComplete` / `IsObjectiveSetActive` / `UpdateObjective` / `RegisterMissionObserver`, and the controller's
`UpdateMissionObjective` after each applied update. The host only reports overlap begin/end of the player and Marcus. VM: interface
casts succeed when the class defines every function of the interface (structural stand-in; the implemented-interface table is not
decoded, UNVERIFIED). Not yet swapped: the dummy's `Behavior_UpdateMissionObjective`. Checks: CTest 11/11 (new scenario F), packages
9/9, UE build Succeeded, quest PASS 83/83 and resume PASS 11/11, door PASS 16/16. No sensitive file touched.

