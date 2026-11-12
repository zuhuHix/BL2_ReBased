## 2026-10-05: script swap 3: mission-linked sequence conditions and the controller helpers (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, from NATIVE_BEHAVIOR_POPULATION.md section 1 and NATIVE_CONTROLLER_HELPERS.md
only.

- The dummy's `BehaviorSequenceEnableByMission` conditions replace the earlier UNVERIFIED rule in `src/slice.*`:
  `MissionSystem` raises the tracker's observer notifications (status change after the script hook and before `Default`,
  active-set switch, objective progress, objective complete), and every notification recomputes each condition (objective
  state with mission-status gating, objective-only bit for objective-specific conditions, `ObjectiveSetRestrictions`,
  mission-level bits) and applies Enable/Disable, with events only on a real transition. The provider registers as a
  consumer at `spawnDummy` (bEnabledOnSpawn pass, then the immediate level-load verdict, then `OnSpawned`); nothing observes
  the mission before the spawn. `bSequenceEnabledMutex` and the enabled/disabled event order follow the note (no dummy
  sequence sets the flag; synthetic test only).
- Real data: the 22 host events are identical and in the same order, the final enabled sequences are the same. New:
  `Idle`'s enabled event now fires at registration and reaches `Behavior_SpecialMove` (an animation request), listed as a
  host-boundary call that the host does not run yet.
- Helpers: `IsDataValid` reports the tracker's `bDataValidated` flag, set by running the script `ClientValidateMissionData`
  once when the VM graph is built (a slice shortcut: the standalone trigger is not known); `GetCurrentPlaythrough`,
  `NativeGetMissionIndex` (range-checked), `IsPrimaryPlayer`, `GetHUDMovie` (None), `IsMenuLevel`, the globals getters and
  `GetGlobalsDefinition`; `UpdateLcdMissionStatus` and `PlayUIAkEvent` as documented no-ops. Turn-in stubs 14 → 12 (7 gone,
  5 newly reached further down the script: `Localize`, `AllExpansionSideMissionsComplete`, `IsLocalPlayerController`, a
  `SpawnPlayerMovie` call on a data-object stand-in, an engine iterator).
- Not modelled: per-instance objectives, `bInstanced`, the Kismet twin `SequenceEventEnableByMission`, the
  RequiredObjectivesComplete / Failed statuses, `ObjectiveCleared`.
- Checks: CTest 11/11 (new synthetic slice scenario on invented packages), packages 9/9, UE build Succeeded, quest suite
  PASS 81/81 and resume PASS 11/11, door suite PASS 16/16. No sensitive file touched.

