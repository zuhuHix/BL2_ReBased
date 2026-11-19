## 2026-11-19: Read the current-playthrough query as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane LINK-10-MISSIONLIST, analyst only (the hub's steer: reading native code makes it an analyst lane). The hub gave it number 102. Decision files take a two-digit number, so it is filed
under 99, which the probe-only lane -99 left unused. Slice links 10 to 12 stop in `accept` at the mission bridge's `MissionTracker.GetAllMissions`
`NotYet` (layer absent) on `fix/rust-mission-numbers` (#480). The new own-words note
`docs/verification/NATIVE_PLAYTHROUGH_QUERY.md` and two `NATIVE_INDEX.md` rows record, as behaviour only:

- **The engine-wide condition** of `GetActiveMissionNumber` (NATIVE_MISSION_NUMBERS.md open question 1): the engine's
  current world is set. It is the object the next step reads its `WorldInfo` from, so it holds exactly when there is a
  world to read.
- **The controller's current-playthrough query** (open question 2) is the script native
  `WillowPlayerController.GetCurrentPlaythrough()`. It answers the world's `WorldInfo.GRI`'s `CurrentPlaythrough` when
  that is a `WillowGameReplicationInfo`, else 0. It writes nothing. Subclass overrides are named `NotYet`s.
  `AllExpansionSideMissionsComplete` uses the same query.
- **Why these alone do not move the slice:** the bridge never sets the tracker's `ActiveMission`, so the tracker branch
  still asks `GetAllMissions`. A local, reverted probe that answered the bridge's one record moved the stop to #480's
  current-playthrough `NotYet`. The tracked plot mission of the golden trace stores `MissionNumber` -1, so a tracked
  fixture would only take the fallback. The tracker's whole `MissionList` is what moves the slice, and how the game
  builds it was not found in this pass (open question 3).

Method: the script with `research/script_disasm.py`, the native list with `tools/ghidra/script_natives.py`, the
layouts with `tools/ghidra/class_layout.py`, the natives and the replication info's virtual with Ghidra 12.1.4
headless, read-only, under the machine lock. Raw output stays under ignored `local/analysis/link10/`. The note was
scanned with the brief's pattern. No bl-sdk session was granted and no recorded evidence covers these natives, so
every rule is UNVERIFIED. This lane implements nothing; another lane implements the note.

Open (the note's questions): subclass overrides of the two virtuals; the global's writers in detail; how the
tracker's `MissionList` is built; whether `MissionNumber` is assigned at run time.
