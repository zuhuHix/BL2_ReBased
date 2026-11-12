## 2026-10-07: script swap 9, the natural level-up's timed weapon-damage scale

AI-assisted (Claude), implementer lane I7 (work queue item 2). Observed in game (lanes L1 and L3): a natural level-up raised the held weapon's
damage for 30 s (scale sum +1.0; the amounts are in the maintainer's private record).

What changed:
- **VM.** A natural level-up runs `PlayerClass.OnLevelUpNaturally` through the existing labelled `RunBehaviors` stand-in. Its WeaponDamage
  effect (weapon context, Scale) is not a VM pool, so it is handed to the host as the new event `weapon_damage_scale`. The value (1.0) and the
  duration (30 s, `DurationType` Timed) are read from the installed skill. Whether a level-up is natural is decided by the installed script
  (`ExpLevelUp(bCheated)` calls `OnExpLevelChange(true, not bCheated)`).
- **CLI.** The new test-only `--slice-run` step `cheatlevel` runs `ExpLevelUp(true)`. That level-up, and a natural one inside the script's 1 s
  guard, give no event.
- **Tools and host.** `tools/weapon_stats.py` now writes each card's WeaponDamage stack terms (`damage_stack`); `damage` is unchanged. The host
  keeps each scale with its end time in game time and shoots with (base + PreAdd) x (1 + up + s) / (1 + down) + PostAdd, the observed
  (S + s) / S. A card without the stack falls back to card damage x (1 + s), a labelled UNVERIFIED stand-in. For the slice pistol the factor is
  1.735.
- **Local slice cards.** Only `damage_stack` was injected, by re-evaluating the five existing recipes. Re-evaluated cards also differ from the
  stored ones in rarity rating and colour, and the shotgun in reload, magazine and display. That is drift since the cards were generated; it is
  left alone and recorded as open.

UNVERIFIED: the VM stand-in, the host's application of the stack rule (the rule was read from native code), the skill's 0.5 s context re-gather
(not modelled: the scale follows the weapon held at the shot), and per-activation stacking. Not done here: the shield recharge boost (no shield
pool) and the cooldown reset (confirmed live by L3, next swap).

Tests: synthetic scenario S9 in `tests/mission_script_test.py`, two new `tests/weapon_stats_test.py` cases (30 OK), and the quest check
`natural_level_up_scales_weapon_damage_for_duration`.

Checks (rerun by the orchestrator): ctest 11/11, verify_packages 9/9, event order self-test and 18/18, UE build succeeded, quest 103/103 + 11/11,
mover 16/16, inventory 49 PASS / 0 FAIL.

