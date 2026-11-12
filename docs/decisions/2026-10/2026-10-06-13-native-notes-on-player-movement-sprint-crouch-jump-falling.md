## 2026-10-06: native notes on player movement, sprint, crouch, jump, falling and FOV (UNVERIFIED)

AI-assisted (Claude), analyst lane G16, Ghidra; own-words note `docs/verification/NATIVE_PLAYER_MOVEMENT.md`, no listing text. Maya's
cooked movement data (GroundSpeed 440 from the class default, AirSpeed 500, JumpZ 630, WalkingPct 0.3, CrouchedPct 0.5, MaxFallSpeed
1500, PlayerAirControl 0.11, cylinder 80 x 42, crouch 50, eye height 70, sprint FootSpeed +0.35 = 594), the script rules for sprint
entry and exit, crouch, jump and falling damage, and native behaviour in our own words: pawn speed fraction, velocity step and
braking, falling step, eye-height and bob update, WeaponBob, view location and the FOV helpers. Supersedes the Walker's estimates
(450 / 420 / 650 / 45 degrees). Correction to NATIVE_WEAPON_TRIGGER_AMMO: sprint does add to the accuracy pool. Open: the native that copies
class speeds onto the pawn, walking/ladder/step physics, and a measured jump (gravity -500 with JumpZ 630 predicts a high apex).
All UNVERIFIED in game.

