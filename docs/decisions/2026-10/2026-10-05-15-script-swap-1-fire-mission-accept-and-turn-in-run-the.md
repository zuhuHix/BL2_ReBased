## 2026-10-05: script swap 1: Fire mission accept and turn-in run the installed controller script (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, written from `docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md` only (no
analysis output opened). Phase 2 step D, first stand-in replaced.

- `FireMissionSlice::accept` runs `WillowPlayerController.AcceptMission` and `turnIn` runs `ServerCompleteMission` on a VM
  controller (authority role, `WorldInfo.GRI.MissionTracker` → a VM `MissionTracker`) with the installed mission object.
  New `src/mission_script.*` binds, to the bridge's own objects only, `MissionTracker.ActivateMission`, `CompleteMission`,
  `PlayTurnIn`, `GetMissionStatus` per the note, driving `MissionSystem`, which stays the single owner of mission state.
  Each status change calls the script `UpdateMissionStatus` and `TriggerMissionStatusChangedDelegates` before observers and
  the `Default` event. The host stand-in "kickoff at once" is replaced by the note's pending record consumed on the next
  tick (the 22 host events on real data are unchanged; the three kickoff events move from accept to the next tick).
- Inferred without a note (labelled in code): `MissionTracker.IsDataValid` (true), `NativeGetMissionIndex` (list lookup).
  Stand-ins kept: `ExpEarn` only records its call (the host still grants XP); `GetExperienceReward` returns the host's
  amount. On real data the script reaches `ExpEarn(395, SideMissionAward)`, equal to the host's 395 by construction.
  Not modelled: `CompleteMission`'s chain, untracking, unlock queue and fast-forward prompt. The slice refuses a turn-in
  that is not ReadyToTurnIn (the real screen offers it only when `CanEndMission` holds).
- 19 natives remain logged stubs on this path (listed in `SANCTUARY_RPG_MISSION.md`, "Script swap 1"); their zero results
  match the Fire mission's data. Known VM gap: static-array fields inside a zero struct are not materialised (reward
  struct reads log out-of-bounds; harmless for this mission).
- Checks: CTest 11/11 (new `mission-script-synthetic`), packages 9/9, UE module build Succeeded, quest suite first run
  PASS 80/80 (79 before, one new check) and resume PASS 11/11. Sensitive file: `CMakeLists.txt` (one source and one test
  added). All native rules UNVERIFIED in game.

