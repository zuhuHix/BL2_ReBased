## 2026-10-07: combat groundwork native notes (pool references, pawn health, pipeline objects, kill notification, kill experience)

AI-assisted (Claude), analyst lane G24. The notes are in `docs/verification/NATIVE_COMBAT_GROUNDWORK.md` (20 index rows). They were read from
`Borderlands2.exe` with Ghidra and from the installed script and data, and are written in our own words. The forbidden-pattern scan is clean.
Nothing here changes code or parsing. **Every rule is UNVERIFIED in the running game.**

Summary:
- **Pool references.** A pool reference is a manager, a slot (0 to 15), a one-byte generation and a cached pool. It is valid only if its slot is
  non-empty and the generation byte matches. `CreatePool` takes the lowest free slot and allows one pool per resource. The pool's owner is the
  manager's owner (the controller). The player's slot order is read but not observed: Maya's seven ammo pools first, then accuracy, experience,
  action-skill cooldown, melee cooldown, health and shield.
- **Pawn health natives.** `GetHealth`, `SetHealth` (values in [0, 1) become 0), `GetMaxHealth` and `SetMaxHealth`.
- **Damage pipeline objects.** The globals object keeps a growing pool of pipeline objects for reuse.
- **NATIVE_DAMAGE_DEATH section 1's open items:**
  - two stages are in the other order (the final damage is unchanged);
  - `IntrinsicArmorReduction` divides by 1 + 0.01 x armor (0 armor for Maya and the dummy);
  - `RecipientDamageTypeReduction` is never written;
  - the crit base is 2.0 (the earlier 0 was the property decoder's empty value for attribute defaults);
  - the AI attacker's extra level factor only applies from playthrough index 2 (UVHM).
- **Kills.** `NotifyActorDied` and `NotifyPawnDied` handle defend targets and kill objectives, with no effect on the Fire mission. Kill
  experience is max(10, multiplier x 10 x (level + 4)), split by player count, scaled by a level-difference table, truncated, and passed to
  `ExpEarn(amount, 1, 1)`.

Open contrast: lane L3's two observed pool credits (45 of 112.5, 7 of 50) are smaller than the note's table gives for a single player killing a
higher-level enemy. Those kills may have been partial credit, since AIs on `Ice_P` killed each other. A kill-experience lane must settle this live
before implementing.

