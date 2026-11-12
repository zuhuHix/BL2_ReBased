## 2026-10-07: script swap 8, the Fire mission's event order follows the live golden trace

AI-assisted (Claude), implementer lane I6, with lane L2's live trace (`FIRE_MISSION_GOLDEN_TRACE.md`) as the evidence. Against the live trace,
`--slice-run` goes from 38 matched / 4 out of order / 5 missing / 4 extra to 43 / 2 / 2 / 4. Changes: the mission weapon is granted inside
`ActivateMission` (into the backpack, not readied; the player equips it) and removed inside `CompleteMission`, as observed. The dummy's `AIHold`
now runs, because the boundary handler for `Behavior_SpecialMove` selects output 0, as the installed script does on its first run. Turn-in now
runs `PlayMissionTurnedInDialog` (`VO_NPC_MissionTurnedIn`): the use path emulates the mission screen's open (`OnPlayerOpenedMissionUI`,
`BeginUse`); the turned-in line is silent for this mission because the act's output is unlinked (UNVERIFIED). Explained, not changed: the dummy's
1 s `RegisterTargetable` delay was already in the VM (a data link with `ActivateDelay` 1.0), and the dialog event "one wake late" is the
behavior's own first-run wait. Remaining: the 2 out-of-order items are those dialog markers; the 2 missing items are host UI (the screen close at
accept, the reward confirmation); the 4 extra are VM dialog lines the live run never selected (cause not isolated). New `tools/verify_fire_event_order.py`
checks 18 event-order rules (18 of 18 hold). Quest checks: `mission_weapon_removed_after_objective` became `mission_weapon_kept_after_objective`;
added `mission_weapon_granted_on_accept`, `lent_pistol_readied_from_backpack_by_player`, `mission_weapon_removed_on_turn_in` and
`turn_in_raises_mission_turned_in_dialog_tag`. Newly reached stubs from `BeginUse` (`AIComponent.ActivateEvent`, `Actor.ClearTimer`, one evaluator
context) change no event. Checks: ctest 11/11, verify_packages 9/9, UE build succeeded, quest 102/102 + 11/11, door 16/16, inventory 49/0.

