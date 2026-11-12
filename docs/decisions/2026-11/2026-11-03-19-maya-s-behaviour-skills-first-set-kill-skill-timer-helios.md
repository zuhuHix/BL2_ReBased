## 2026-11-03: Maya's behaviour skills, first set: kill-skill timer, Helios, Converge, Ruin, Sweet Release, regeneration (Skills lane PR 3)

AI-assisted (Claude; analyst sub-agent read the behaviour graphs and script, own-words note). Record:
`docs/verification/NATIVE_SKILL_BEHAVIOURS.md` (rules with confidence per skill; section 7 is the host status).
Nothing here was seen in the running game; native bodies were not read.

- **Kill skills (behaviour change).** A kill restarts a 7 s timer (KillSkillDurationsPerPlayers, one player); Kill skills'
  modifiers count while it runs. Refresh rather than add is a guess. Hook: one line in `NotifyKilledEnemy`.
- **Phaselock variants (behaviour change).** Helios, Converge and Ruin blasts on the lifted target at the graph's times,
  radii and damage formulas (PlayerSkillDamage = 8 x 1.13^level, PlayerSkillStatusEffectDamage = 1.13^level); Sweet
  Release heals 15 % of missing health per orb when the held target dies. Status effects, Converge's pull, the orb
  actors and the blocked-target chain are not done. The Phaselock code is read, not hooked.
- **Regeneration.** `tools/prepare_skill_effects.py` now writes balance formulas (Sustenance: 0.004 x grade x missing
  health) and Allies-with-self rows (Elated); the host heals Maya by `HealthPassiveRegenerationRate` per second. New
  `AOpenWillowWalker::Heal` (one header line).
- **Corrections found** (recorded in the note, section 5): the blocked-target damage is 10 x PlayerMeleeDamage (5 x for a
  player); `ActionSkill_Phaselock` sets `bAllowAutomaticActiveAbilityActivation`; `Att_EnemyIsPhaseLocked` comes from the
  cooldown-manager skill (selection to release). Read from data, UNVERIFIED in game.

