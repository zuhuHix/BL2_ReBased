## 2026-11-06: Converge's damage from one live read; a session is complete only if each step got what it came for (Skills lane)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILL_BEHAVIOURS.md`, "Fourth run" and "What a useful fifth run needs".

- **Behaviour change.** A Converge blast does 1.16 x PlayerSkillDamage (the data reading gave 1 x), fitted to one live read on a level-8 Maya, UNVERIFIED
  beyond it. The reason is not known, as with Helios's 0.29. Checked in `OpenWillow.Skills` as a formula.
- **Tool rule.** The fourth run exited 0 and reported itself complete although no kill happened and no heal could be seen: the rule only asked whether
  each step ran without an error. Each step now names the evidence it came for (kills happened, target took damage, a heal could be observed), and a
  session is complete, and exits 0, only with all of it. `tests/skills_live_probe_test.py` fails on the old rule. A step reporting its own evidence can
  still be wrong about what it means; this only stops "ran" being read as "read".
- **Not read, and still UNVERIFIED:** the kill-skill timer (no kill happened; what stopped the gun is not known), Sweet Release's heal (the immunity hook
  may have blocked it), Converge's pull and radius, Sub-Sequence's re-lock and `OnKilledTarget` count. `DeactivateSkill` is not shown to work.

