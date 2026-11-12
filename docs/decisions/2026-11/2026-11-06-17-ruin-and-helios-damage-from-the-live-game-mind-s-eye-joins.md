## 2026-11-06: Ruin and Helios damage from the live game; Mind's Eye joins the crit stack's Scale sum (Skills lane)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILL_BEHAVIOURS.md`, "Third run". Read from the running game with bl-sdk hooks on
one level-8 Maya: every formula here is checked at level 8 only. Raw reads stay under ignored `local/realgame/`.

- **Behaviour change, corrected from a contradicted data reading.** Ruin's pulses do 1 x PlayerSkillDamage (the note had 7.2 x
  PlayerSkillStatusEffectDamage). Helios does 0.29 x grade x PlayerSkillDamage, linear at grades 1 and 5 (the note had 0.2). The factor 0.29
  is fitted to the live read and its reason is not known. The timing of both was confirmed.
- **Behaviour change, Mind's Eye.** Live, the crit bonus is (base 2.0 + PreAdd) x (1 + Scale sum), and the skill adds Scale 0.05 per grade:
  this fits the reads at grades 1, 3 and 5 to 4 decimals. The hook now adds the skill's part (2.0 x 0.05 x grade) to the
  event's bonus instead of multiplying it. With no other modifiers in the host, the result is unchanged: 2.5 at grade 5. The stack's
  other modifiers are not identified or modelled.
- **Unchanged and UNVERIFIED: the 7 s kill-skill timer.** In two runs, two gun kills with Life Tap 5 left the game's timer at 0 and no
  `UpdateKillSkills` call was recorded; the targets had been wounded by a direct `TakeDamage` first, so this is no verdict.
- **Not modelled yet, seen live:** status damage ticks about every 0.33 s; the target's resistances and scales change the health lost.
- **Tools.** The capture takes `-Steps`, runs the timer first with unwounded, controlled targets, and lowers Maya's health for Sweet
  Release's heal; `python tests/skills_live_probe_test.py`.

