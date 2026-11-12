## 2026-09-27: Host-owned skill points for Maya (level, XP, spending)

Maya's level, experience, skill points and grades now live in the UE host
(`UOpenWillowSkills`, a component on `AOpenWillowWalker`). The Skills page only
reports clicks and presents what the host sends. Skill gameplay effects are out
of scope and not applied.

Rules, and what each rests on:
- Earned points = level - 4, i.e. one per level from level 5. The real-game
  trace under ignored `local/ui/traces` (the respec run) shows a level 45
  character (`SetPlayerLevel "Level 45"`, XP between the level-45 and level-46
  thresholds, next level at 2,715,586, in both traces) with 41 unspent points and no grades, which is 45 - 4. That checks
  one level only; other levels and any DLC or level-cap rules are UNVERIFIED.
- XP threshold for level L: floor(60 * L^2.8 - 60). The formula is a
  community-known curve, not read from the install. It reproduces the traced
  next-level value exactly (2,715,586 for level 46) and nothing else has been
  checked, so it is UNVERIFIED. No level cap is modelled and the host awards no
  XP yet, so `-owlevel=<N>` sets the starting level (default 1, no points).
- The action skill costs one point per grade and is spent first. The trace's
  first spend was Phaselock through `extCellClicked(-1, -1, -1)`, going from
  grade 0 to 1 and from 41 to 40 points. The three trees also require it,
  because the installed root branch's first tier has PointsToUnlockNextTier 1
  (`actionSkillPoints` from `tools/prepare_skill_tree.py`). That gate is read
  from data. The trace spent the action point first, so what the game does
  with a tree click before it is not observed (UNVERIFIED).
- A tier opens once its branch holds the sum of `pointsToUnlockNext` of every
  lower tier; a skill cannot go past its max grade; every spend needs a point.

Channel: the page writes `console.log('OWSKILL {"branch":B,"tier":T,"cell":C}')`
from its own cell hit targets. It also forwards the movie's `extCellClicked` in
case the movie sees a release itself (it did not in the checks below, because
the page's targets cover the cells). The HUD's existing `OnConsoleMessage`
handler parses the line, queues the spend, applies it in `DrawHUD`, and pushes
`owSkills({points, actionGrade, grades})` back. No new binding API was needed.
The page now shows a tier as enabled only when the host rule would accept a
spend there. It also adds the action-skill hit target before the cells: the
`ActiveAbility` clip's bounds reach y=200 on the stage (probably an invisible
child), so it had covered the first tier and taken its clicks and hovers.

Automated checks:
- `OpenWillow.Skills` (new UE automation test, synthetic tree, no game data):
  points by level, action-skill gate, tier lock, max grade, no points, bad
  cells, the owSkills JSON and level-up at an XP threshold. It passed in a
  `-game -nullrhi` run.
- Replay against the real game (one-off, not committed, because it reads the
  local trace): the trace's 55 `extCellClicked` calls, replayed through
  `TrySpend` on the local Siren tree at level 45, gave the game's outcome for
  all 55. That is 41 accepted, plus 4 refusals for locked tiers, 8 for max
  grade and 2 for no points (the game spent on the action skill before any
  tree click).
- ctest 6/6, `tools/verify_packages.py` (9 packages match) and
  `tests/gfx_to_swf_test.py` (8 tests) pass. Windows Application Control
  blocked the freshly built unsigned `ow-package.exe` on this machine. So the
  ctest scripts and verify_packages ran against the main checkout's
  `ow-package.exe`, built 2026-09-22; `src/` and `CMakeLists.txt` have no commits
  since then and this change does not touch them. The UE 5.8 editor module
  build succeeded.

In-game check (UE 5.8, Sanctuary `-game` window at 1280x720, `-owlevel=45`).
Input was real OS keyboard and mouse events sent to the UE window by a local
helper script, not a person's hand, and no enemy XP was involved:
- K opened Skills, and the host logged 41 points. Ten clicks gave, in order:
  Mind's Eye refused (action skill not unlocked); Phaselock accepted (40);
  Sweet Release accepted five times (35), then refused at max grade;
  Restoration accepted (34); Elated refused (6 of 10 points in branch). There
  was one page message per click.
- The captured frame shows 34 skill points, Phaselock 1/1, Sweet Release 5/5
  in the maxed frame, Restoration 1/5, and tier 3 dimmed. After K to close
  (gameplay HUD back) and K to reopen, the menu showed the same grades, and
  another Sweet Release click was refused at max grade.
- The scripted `-owcombattest -owcombatshots -owskillshots` run still completes
  at level 1 with 0 points.

Not done or not verified: grades are lost when the session ends (no save).
Phaselock can still be cast at action grade 0, because gating it is a gameplay
effect left for the next step. The HUD shows the XP fraction but no level
number (the page does not wire `levelText`). Nothing here is compared with the
real game's menu beyond the trace replay and the by-eye capture.

