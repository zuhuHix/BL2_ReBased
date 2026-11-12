## 2026-10-01: slice player side on stock data

AI-assisted. Host work; no package parsing change; nothing compared against the original game.

- The lent mission weapon is the recipe matching the mission's own `MissionWeapon` (Maliwan fire pistol), given
  at the Fire objective and shown with the imported Maliwan sample mesh. Each shot hands the held item's
  damage type path to the dummy's `OnTakeDamage` check; the host fire-damage class is removed. A normal-damage
  pistol takes the wrong-element path and has its own check.
- Hand play (`tools/run_quest.ps1`) loads `local/items/slice` with Maya at the slice gear level (8, an
  UNVERIFIED slice choice). First-person arms stay hidden until a weapon is drawn; whether the original shows
  arms when unarmed is UNVERIFIED.
- Turn-in adds the candidate XP amount (0.05 x the experience span at mission level 8 = 396; the native rule
  is UNVERIFIED) to the skills component.
- Phaselock reads `action_skill_siren.json`: lift, lock length, fade, cooldown paused while a target is held,
  miss reset, re-lock penalty, Suspension's bonus. Targeting, the cast gate and target state stay host
  stand-ins; the cooldown model and curve shapes are UNVERIFIED.
- Stock data drops no item for this mission (the dummy has no pools; the reward is XP only). A labelled
  turn-in loot stand-in (first fallback-pool seed that drops a weapon) exercises the pickup path; it is not
  stock behaviour.
- `tools/prepare_weapon_paint.py` accepts material chains without a pattern texture; the pistol decal and the
  shader channel reading are UNVERIFIED.
- Checks: quest suite 57/57 and resume 7/7, door suite 16/16, inventory suite 47 PASS / 2 KNOWN_DIVERGENCE,
  CTest 10/10, packages 9/9. Two earlier failing quest runs are kept in the record. Known gaps: Phaselock can
  lift the target into ceiling beams, XP and skill grades are not saved, health is not recalculated on
  level-up, slice guns have no inventory 3D preview. Record: `docs/verification/SANCTUARY_RPG_MISSION.md`,
  "Player side with stock data".

