## 2026-10-06: native notes on weapon firing, ammo, accuracy, recoil and reload (UNVERIFIED)

AI-assisted (Claude), analyst lane G13, Ghidra; own-words note `docs/verification/NATIVE_WEAPON_TRIGGER_AMMO.md` (named NATIVE_WEAPON_FIRING.md until the 2026-11-02 merge), no listing text;
builds on NATIVE_WEAPON_RULES.md. A press fires once in `WeaponFiring`, then on a timer re-armed per shot with
`GetFireInterval`; auto-reload one interval after the last round. The slice pistols fire Bullet (a light projectile with
travel time), not hit-scan; range `WeaponRange` 16,384. Clip and pool are separate counters (clip = max(min(clip, ClipSize,
pool) - ShotCost, 0)). Accuracy: a player pool cone (max 12, min 2, impulse per shot, decay 8/s after 0.2 s) scaled by
`1 - ZoomEffect` plus a per-pellet `Spread` cone that zoom does not remove. Recoil kick = impulse × 182.0444 scaled by zoom
and FOV ratio, decaying over `WeaponKickRecoveryTime`. Reload refills the clip at `ReloadTime × ReloadCompletePercent`;
melee, grenade, swap and action-skill start stop it (the Phaselock abort matches the 2026-10-02 observation). Open:
held-trigger burst restart, interrupted-reload refill, pass-through. All UNVERIFIED in game except the cited observation.

