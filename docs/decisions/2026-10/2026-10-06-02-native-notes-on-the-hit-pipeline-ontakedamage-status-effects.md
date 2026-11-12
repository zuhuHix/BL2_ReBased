## 2026-10-06: native notes on the hit pipeline, OnTakeDamage, status effects, death and respawn (UNVERIFIED)

AI-assisted (Claude), analyst lane G11, Ghidra; own-words note `docs/verification/NATIVE_DAMAGE_DEATH.md`, no listing text.
Order of one hit: `TakeDamage` chain → precondition → native `AdjustDamage` pipeline (per-stage factors recorded in
`DamageEventSummary`: AI scale, source, element, level difference, backstab, hit region and crit, shield, protection timer,
caps) → Kismet damage events → health → `NotifyTakeHit` (raises `OnTakeDamage` after the health change, outputs Instigator,
Damage, ShieldDamage, DamageSource = the shot's source class, DamageType = the pipeline's definition; the AI-class raise
filters on `Damage + ShieldDamage < DamageThreshold`) → status-effect roll → depleted or hit presentation. Incendiary status
ticks report the Status damage type, so only the Impact hit matches the dummy's Fire chain. AI death order (tracker
`NotifyPawnDied` first, loot drop, kill XP), player down state (bleed-out 12/8/4/1 s, second wind floors), respawn at full
health and shield, death fee 0.07 × credits rounded half up. Corrects the slice record on `DamageSource` and G2's open item on
the event filter. A local class_layout copy sized `MapProperty` as 60 bytes (assumption, checked only by field names landing).
All UNVERIFIED in game.

