## 2026-10-07: real-game lane L3, a real fight recorded; a level-up resets the action-skill cooldown

AI-assisted (Claude), real-game lane L3 (work queue items 1 and 2). Method as lanes L1 and L2. The level-8 Maya travelled to Three Horns Divide
(`Ice_P`) with the game's own `InitiateTravel` and fought bullymongs with real mouse shots. The new probe is
`tools/real_game/scripts/combat_probe.py` (463 hooks plus a per-frame sampler), and the record is `docs/verification/COMBAT_GOLDEN_TRACE.md`.

Confirmed in game:
- **Hit chain:** each pellet runs `TakeDamage`, `ReduceDamage`, `AdjustDamage`, `SetHealth`, `NotifyTakeHit`, `TookDamageFromEnemy` and
  `CheckInjured`, in that order.
- **AI death:** `NotifyPawnDied`, `Died` with `DropLootOnDeath` first, `OnKilledBy`, `Killed`, `CalculateExperiencePointsForKill`,
  `AwardCombatExperience`, in that order.
- **Kill experience:** awards of 112.5 and 50.0, credited to the pool as +45 and +7.
- **Drops and pickups:** drops came from the pools the script calls (`Pool_Money_1or2`: 3 items for an adult, 1 for a baby; `Pool_GunsAndGear`).
  Money is picked up by touch and a weapon by the use key. A full backpack refuses.
- **Being hit:** the shield takes the hit first (31.616 of a 44.22 grenade) and the overflow goes to health. The shield recharges after 1.61 s at
  25.9 per second.
- **Down, death, respawn:** down at health 0, the targeted state 2.28 s later, bleed-out 12.0 s, death camera 5.76 s, then
  `ResurrectPlayer(IDied)` with full health and shield. The fee is 0.07 x cash, rounded half up, charged 11.8 s after arrival.
- **Second wind:** a kill while down restored health to 25 % of max in the kill frame and shield to 90 % on the next frame.
- **Part B:** a level-up (`ExpLevelUp(false)`) sets the action-skill cooldown to 0 in the same call, both at its 13.0 maximum during a real
  Phaselock hold and at 7.99 during the drain. The 4.0 s shield boost (90.29 per second) and the 30 s weapon-damage scale (41.4 to 61.9) were
  observed again.

Contradicted: NATIVE_DAMAGE_DEATH's 2.0 s fee timing (11.8 s observed).

UNVERIFIED or not shown: the shortened bleed-out of successive injuries; the pipeline stage factors behind 0.715 (grenade) and 0.741 (melee);
the experience level factor; `Pool_Money_1or2`'s quantity rule; the respawn station choice.

Mods: Drop Chance Multiplier was not active at runtime. Loot Collector and Be Gone Out Of Bounds Loot were switched off in memory with no
settings write.

Script stand-ins in the session: camera aim, player placement, and the misc slot limit raised in memory for one pickup.

Save safety: 60 files hashed before and after. The 33 save and config files and the SDK settings folder are byte-identical. Sanity Saver's own
`Save0008.json.gz`, which that mod rewrites at load, was restored from the backup, and after that 0 of 60 differ.

Checks: ctest 11/11, verify_packages 9/9 (docs and a game-side probe only).

