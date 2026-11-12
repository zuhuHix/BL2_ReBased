## 2026-11-04: skill hits carry Source = Skill; Cloud Kill and Immolate on bullet or rocket hits (Skills lane, after PR #88)

AI-assisted (Claude). With the hit-event `Source` field on main (PR #88), every skill hit (blasts, Recompense,
Immolate's extra hit) sets `Source = Skill`, the hit listener ignores Skill and Status sources, and Cloud Kill and
Immolate proc on `Source` Bullet or Rocket, the game's two damage sources for them (a rocket's explosion now counts; a
bullet's impact explosion and a Tediore throw do not). Record: `NATIVE_SKILL_BEHAVIOURS.md` section 7. UNVERIFIED in game.

