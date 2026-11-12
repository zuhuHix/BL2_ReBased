## 2026-10-07: Fire mission golden trace recorded in the real game and compared with the VM (lane L2)

AI-assisted (Claude), real-game lane L2, method as lane L1 (one game process under the run lock, saves blocked from the first frame, memory
only, driver removed; all 33 save and config files byte-identical afterwards). The level-8 Maya accepted the Fire mission at Marcus, touched the
range waypoint, burned the target dummy with the lent pistol and turned in; hooks recorded the mission tracker, behavior, dialog, XP and reward
calls in order (`docs/verification/FIRE_MISSION_GOLDEN_TRACE.md`, `REALGAME_GROUND_TRUTH.md` lane L2; raw trace under ignored `local/realgame/L2/`).
Observed in game: the status routine order for Active, ReadyToTurnIn and Complete; the lent weapon goes into the backpack inside
`ActivateMission` and is removed inside `CompleteMission`; turn-in `ExpEarn(395)` with an empty item reward (+395 XP, level 8 kept);
behavior-link delays 0.505, 2.002 and 3.002 s. Against `--slice-run`: 38 events match, 4 out of order, 5 missing in the VM, 4 extra. Differences to
fix: the mission weapon grant and removal positions; the dummy's `RegisterTargetable` running about 1 s after spawn together with the mission's
`ChangeRemoteBehaviorSequenceState` (absent in the VM); `AIHold` on the dummy's spawn (absent in the VM); `PlayMissionTurnedInDialog`
(`VO_NPC_MissionTurnedIn`) not run. Host-side: the mission screen's close and reward confirmation, and the dialog event one wake late. The VM run
used a binary from before swap 7b. Dialog lines were shortened by an installed dialog-skipper mod, so line-length delays are not measured.

