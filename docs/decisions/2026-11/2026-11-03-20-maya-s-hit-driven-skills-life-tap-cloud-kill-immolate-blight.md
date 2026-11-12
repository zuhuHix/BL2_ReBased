## 2026-11-03: Maya's hit-driven skills: Life Tap, Cloud Kill, Immolate, Blight Phoenix (Skills lane PR 4)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILL_BEHAVIOURS.md` section 7. Behaviour change, from the
behaviour graphs and effect data; UNVERIFIED in game.

- `UOpenWillowSkills` listens to `OpenWillowHit::OnHit()` (PR #75's hit events) for Maya's gun hits, so no other lane's
  file changes. Life Tap heals 0.012 x grade of the applied damage while the kill-skill timer runs; Cloud Kill leaves a
  5 s corrosive cloud (0.25 s ticks, radius 220, 0.25 x PlayerSkillDamage) with a 5.5 s cooldown; Immolate adds
  0.1 x grade x the hit as fire damage while Maya is down; Blight Phoenix pulses corrosive and fire around Maya every
  0.5 s while the kill-skill timer runs (radius 512, 0.02 x grade x PlayerMeleeDamage, melee buff taken as 0).
- Skill hits go through `ApplyShotDamage` with weapon id `skill:<name>`; the listener ignores them and status ticks.

