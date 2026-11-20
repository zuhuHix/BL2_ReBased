## 2026-11-19: What blocks a pickup's touch (DroppedPickup.TouchPickupTrace), links 6 and 7's next stop

AI-assisted (Claude Code)

Opus 5.5. Lane ANALYST-LINKS-6-7-PAST-TOUCH, an analyst lane. It reads native code; another lane implements.

- **The stop.** The probe was a throwaway merge, never pushed: main b0d9ca08, #595 (9de51c43), and #609's own two
  commits plus #617's (7fd8c2ba), cherry-picked because main already holds rebased copies of #609's earlier commits.
  It ran the installed Off `--slice-gate`, strict, under the machine lock: 3 ok / 9 NotYet / 2 errors. Links 6 and 7
  each run both of the driver's 2 steps. `make` passes, and `touch` stops at #595's own `LayerAbsent` refusal,
  "Actor.GetBoundingCylinder: the colliding components' bounds" (a live pickup; the port tracks neither attachment
  nor component bounds). #595 already ports that native, so a probe-only stand-in for the refusal (an invented radius
  40 and half height 20) ran the probe again. Both links then stop, still in `touch`, at
  `DroppedPickup.TouchPickupTrace`, the port's Off stub ("what blocks the touch trace inside the native was not read").
  No open PR ports it.
- **The note.** docs/verification/NATIVE_TOUCH_PICKUP_TRACE.md. The native is one point line check with the pickup as
  source actor, flags 134226566 (the one-result bit added), and a true answer only when the hit names no actor.
  - It runs from the pickup's touch point to the toucher, the reverse of the parameter names. That corrects
    NATIVE_PICKUP.md section 3's "direction resolved", which now points to the new note.
  - Walls, world-geometry actors, terrain and movers block it. Pawns and the pickup itself do not.
  - One game class's filter rejects every trace carrying bit 134217728; its class was not identified.
  - The proposed Off answer reuses the spawn's collision service and actor walk, with ordered write-free and load-free
    refusals. It has a test list and a NATIVE_INDEX row.
- **Verified:** nothing in the running game. No bl-sdk session was granted. The note gives the live check.
- **UNVERIFIED:** every rule in the note. The raw analysis stays under ignored `local/analysis/touch_pickup_trace/`.
