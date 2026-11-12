## 2026-10-05: native notes on how the Fire mission's objectives are progressed (UNVERIFIED)

AI-assisted (Claude), analyst lane G9, Ghidra; own-words note `docs/verification/NATIVE_OBJECTIVE_TRIGGERS.md`, no listing
text. GoToRange is completed by a placed `WillowWaypoint` (its cylinder `Touch` by a player-owned pawn, gated by
`IsMissionObjectiveActive`, then `UpdateObjective` without a player argument); the waypoint also observes the mission and
re-checks actors already inside when the active set changes (a host that only checks on entry misses a player standing in
the range). The tracker fans out `UpdateMissionObjective` to every local controller. Fire: `Pawn.TakeDamage` →
`NotifyTakeHit` → the AI controller fires `OnTakeDamage` on the AI definition's then the AI class's provider, with
`DamageType` = the pipeline's damage type definition and a threshold filter; `Behavior_UpdateMissionObjective` is script.
`AdvanceObjectiveSet`, `MissionRemoteEvent`, `ClearObjective` act only with the tracker as consumer; mission remote events
match by name and mission. Corrections to C1/G2 notes and two slice records are listed in the note. All UNVERIFIED in game.

