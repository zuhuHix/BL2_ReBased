## 2026-11-19: MissionTracker.AllExpansionSideMissionsComplete, read as analyst

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-LINK12-EXPANSION-SIDE-MISSIONS, slice link 12. Analyst note
only (clean room): docs/verification/NATIVE_ALL_EXPANSION_SIDE_MISSIONS_COMPLETE.md. Written in own words from the
installed script, the package layouts, one installed value and local, read-only Ghidra queries under the machine
lock. Raw output stays under ignored `local/analysis/aesmc/`. No live session was run.

- **Read (it re-checks NATIVE_MISSION_OBJECTIVES.md, which holds):**
  - the native reads the tracker's whole `MissionList` and the controller's current playthrough list, and writes
    nothing;
  - it is false for a None controller or an out-of-range playthrough index;
  - each side mission of the given expansion (not plot critical, the same `DlcExpansion`, no `MarketingUnlock`, not
    repeatable) must have a controller entry with status `MS_Complete`; a missing entry or another status is false;
  - it is true otherwise, including when none count.
- **Link 12's path:** `UpdateMissionStatus` writes the turned-in mission's status first, then, for a side mission,
  calls `CheckAllSideMissionsCompleteAchievement(Mission.DlcExpansion)`, the native's only script caller. The slice's
  Fire mission is a base-game side mission (installed flags), so the call is (controller, None).
- **For the implementer:** bind it in Off, write-free and loading nothing:
  - the playthrough index comes from NATIVE_PLAYTHROUGH_QUERY.md's settled rule;
  - while the port's tracker holds only the bridge's one record, refuse exactly as `GetAllMissions` does. A
    one-record walk would answer true wrongly;
  - an unloaded mission handle reached in the walk is `LayerAbsent`;
  - the status is compared by enum name.
- **UNVERIFIED.** Every rule is unconfirmed in the running game; the note lists the live checks.
