## 2026-11-03: Skills menu: respec in the host, the page's spend rule as a tested module (Skills lane PR 1)

AI-assisted (Claude). Records: `docs/verification/NATIVE_SKILLS.md` section 3.1 "Host", `SKILL_TREE_NAVIGATION.md`
"Spend rule on the page".

- **Respec (behaviour change).** `UOpenWillowSkills::ResetTree` returns every point, the action skill's included, and keeps
  level and experience; `PurchaseReset` charges `ResetCost(level)` = 100 x 1.12^level credits (truncated), the formula of
  `GD_Globals.Skills.CostToResetSkillPoints` read from the install. The rule follows the native note and agrees with the
  local respec trace at level 45 (41 free points, no grades); the cost is UNVERIFIED in game. No caller yet: the Quick
  Change station is not built.
- **Page rule.** The tier/action-skill/max-grade/no-points rule the Skills page uses to show open tiers and the
  "Spend Point" hint is now `tools/hud_overlay/skill_rules.js`, tested by `node tests/skill_rules_test.js` (synthetic).
  Same rule as before; the host still decides each spend. Already in place before this change and unchanged: spending
  per tier with the 5-point gate, ranks, and progression in the quest save (`ProgressionJson` / `RestoreProgression`).
- **Checks.** `OpenWillow.Skills` gains respec and cost checks (UE automation; run by the hub's test runner).

