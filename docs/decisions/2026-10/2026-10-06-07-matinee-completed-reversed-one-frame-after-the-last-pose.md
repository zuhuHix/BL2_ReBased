## 2026-10-06: Matinee Completed / Reversed one frame after the last pose (UNVERIFIED rule)

AI-assisted (Claude), orchestrator from NATIVE_KISMET_MATINEE.md ("Deactivated": a finished `SeqAct_Interp` is noticed on the next
update). The host mover (`OpenWillowMover.cpp`) now applies the end pose, then on the next frame runs `InterpolationFinished` and
fires Completed / Reversed, for both the door and the target track; a Play / Reverse arriving in between keeps the action running
and suppresses that output, as the note's update order implies. `DoorRunning` / `TrackRunning` stay true until the deactivation.
Checks: UE build Succeeded, door PASS 16/16, quest PASS 83/83 and resume 11/11 (CTest and packages unaffected: host only). The
note's other correction (the move-track pose composition differs when keys rotate) is not applied yet.

