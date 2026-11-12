## 2026-11-03: Skill effects in the host: Maya's skill modifiers from the stock SkillDefinitions (Skills lane PR 2)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILLS.md` section 4.8 (coverage table per stat and skill).

- **Data.** New `tools/prepare_skill_effects.py` writes ignored `local/ui/run/skilleffects_siren.json` (31 skills, 50
  effect rows, 34 constraints on the 2026-11-03 install); `UOpenWillowSkills::LoadTree` loads it beside the tree.
  Synthetic test: `python tests/skill_effects_test.py`.
- **Rules (behaviour change; native reads, UNVERIFIED in game).** `FOpenWillowSkillEffects` turns a trained skill into
  modifiers with the grade formula of NATIVE_SKILLS 4.3 (start grade, integer-divided interval, best bonus entry) and
  combines them with the stack formula of NATIVE_WEAPON_RULES 1. Passive skills need a point; ActionAugment skills count
  while Phaselock runs; kill skills not yet. Constraints are evaluated as data (attribute comparisons, And/Or, on foot,
  healthy); an attribute the host cannot answer makes its comparison false, as the native expression does.
- **API.** `OpenWillowSkillEffects::Stack / Apply / EffectValue / IsLoaded` (`OpenWillowSkillEffects.h`). One-line hooks
  in `OpenWillowWalker.cpp`: shot damage (Accelerate, Wreck), fire interval (Wreck), reload time (Foresight), Phaselock
  lock time (Suspension, Thoughtlock) and cooldown drain rate (Quicken). Shields, crits, status chance, clip size and
  max health are not hooked (no host stat, or another lane's code); listed in the record.
- **Checks.** New UE automation test `OpenWillow.SkillEffects` (invented manifest): grade formula, interval and bonus,
  scale-up/scale-down combination, conditions, augments, kill skills off, respec clears modifiers.


