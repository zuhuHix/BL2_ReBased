## 2026-11-03: Maya's movement from the stock data in the host (maya lane, item 1)

AI-assisted (Claude). Record: `docs/verification/NATIVE_PLAYER_MOVEMENT.md` "Host implementation". Host behaviour change; no live reads.

- **Speeds and physics** move from estimates (450 run, 650 sprint, jump 420, 45-degree floors) to the cooked values: run 440, sprint 594,
  crouch 220, jump Z 630, air control 0.11, friction 8 with braking factor 2, floor Z 0.7. The predicted jump: a 318 uu apex and
  2.27 s in the air with the falling step's fluid friction 0.3 (396.9 and 2.52 s without it); logged at every landing, not yet
  compared with a live jump. In the air there is no horizontal speed cap (stock rule at air control 0.11).
- **Sprint** follows the script's rule (a 45-degree input cone, press-to-start, ended by crouch, firing, zoom or going down), with the
  sprint FOV (the flexible-FOV maths, eased at 10) and the sprint eye height. **Crouch** is new (hold, Left Ctrl or C).
- **Camera bob and eye height** follow the read eye-height and bob update; **falling damage** follows `TakeFallingDamage`.
- All UNVERIFIED in game. Synthetic automation test `OpenWillow.MayaMovement`.

