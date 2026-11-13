## 2026-11-13: Ammo back on respawn: five pools by the injured definition's share

AI-assisted (Claude Code). Lane A of the hub run of 2026-11-13. The UE host gave no ammo back on a respawn. The game's injured
definition names five refills for that moment, and the live respawns fit them, so the host now applies the same rule.

- **The rule, read from installed script and package data:** `PlayerInjuredDefinition.ResurrectedBehaviors` holds five
  `Behavior_RefillResourcePool`, for the combat rifle, combat shotgun, patrol SMG, repeater pistol and sniper rifle ammo. Rockets and
  grenades are not in it. Each share is `Att_PercentAmmoRestoredOnDeath`, a constant 0.32; `MaxPercentage` is the class default, 1.0.
  The apply step is script (`ResourcePool.RefillPercentage`): a pool below `MaxPercentage` of its range is set to the smaller of
  current + 0.32 x maximum and the maximum, so it only rises. `SetCurrentValue` truncates toward zero for these integer-only
  resources (NATIVE_LEVEL_UP_ATTRIBUTES.md, section 4). No executable was read for this.
- **The moment:** the respawn start (the game's `ResurrectPlayer`), as COMBAT_GOLDEN_TRACE.md step 15 saw it live.
- **Host:** `OpenWillowRespawnAmmoRefill.h` (a plain helper), called from `AOpenWillowWalker::Respawn`; one log line per respawn,
  `OpenWillow Maya respawn ammo refill: N of 5 pools raised`. Synthetic cases extend `OpenWillow.MayaState.RespawnWaitSplit`, so
  the automation count stays 13. The script VM is unchanged: it still logs the injured definition's lists as not implemented.
- **Verified:** the data and script reading above, and the host's arithmetic by the synthetic test. The live downs so far had the
  five pools full, or one of them below its cap and at its cap after the respawn, with the launcher pool unchanged
  (COMBAT_DOWN_RESPAWN.md, "Ammo"). That fits the rule but cannot show its size.
- **UNVERIFIED:** the 0.32 size and the truncation in the running game, the pool minimum of 0, the ammo pool's own "current value
  changed" hook and truncation remainder (not read), and that nothing in the slice modifies the attribute. The host's pools carry
  the base maximum without ammo upgrades.
- **Slice gate:** the `ammo reserve refilled` fixture and `tools/check_slice_gate.py` are unchanged; the checker grades no ammo
  amount. After a respawn the fixture may fire less often in later links, because the reserve is no longer left empty.
