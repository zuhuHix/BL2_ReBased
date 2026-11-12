## 2026-10-05: native notes for the mission script bridge: accept, complete, kickoff, rewards (UNVERIFIED)

AI-assisted (Claude), analyst lane C1, Ghidra; own-words note `docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md`, no
listing text. Accept and turn-in are script (`WillowPlayerController.AcceptMission` / `ServerCompleteMission`) driving the
natives `MissionTracker.ActivateMission` / `CompleteMission`, which share one status routine. That routine calls back into
script (`WillowPlayerController.UpdateMissionStatus` on each local controller) before observers and the `Default` event (id
6 + status). Rewards are granted by script: `UpdateMissionStatus` → `ServerGrantMissionRewards` → credits, `ExpEarn`,
items, reward UI. The kickoff after acceptance is a pending record the tracker's tick consumes (`Default` id 12 one tick
after id 7), answering an open item of NATIVE_MISSION_DISPATCH. `ExpEarn` only raises the pool; the level-up comes from
the pool's update. Corrections to NATIVE_MISSION_DISPATCH B4 (hook order) and NATIVE_PROGRESSION (`ExpLevelUp(bCheated)`)
are recorded in the new note with pointers from both. Every rule UNVERIFIED in game except the 395 XP amount (2026-10-02).

