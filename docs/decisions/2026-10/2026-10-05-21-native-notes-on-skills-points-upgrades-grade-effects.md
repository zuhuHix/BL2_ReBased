## 2026-10-05: native notes on skills: points, upgrades, grade effects, cooldown (UNVERIFIED)

AI-assisted (Claude), analyst lane G7, Ghidra; own-words note `docs/verification/NATIVE_SKILLS.md`, no listing text. Skill
points are awarded by the script `ExpLevelUp` (`GeneralSkillPoints += PointsPerLevelUp` after the level rises; the
confirmed `max(0, L − 4)` total follows) and announced through `FireSkillPointsChangedDelegates`. Spending: UI
`RequestSkillUpgrade` → `CanUpgradeSkill` → `ServerUpgradeSkill` → native `PlayerSkillTree.UpgradeSkill` (refusals, tier
unlock by summed `PointsToUnlockNextTier`, child branches; the level-5 gate is UI-only). Grade to effect:
`Base + PerGradeUpgrade × ((g − start) div max(interval, 1)) + bonus` (bonus = the single best `BonusUpgradeList` entry),
modifiers applied through the attribute stack and refreshed on the next tick after a grade change. Cooldown pool refill and
drain (the Phaselock pause reading is consistent). `tools/skill_stats.py` lacks the bonus rule and defaults
`GradeToStartApplyingEffect` differently (not changed yet). All UNVERIFIED in game.

