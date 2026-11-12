## 2026-11-03: the HUD shield bar follows Maya's shield pool (maya lane, item 4)

AI-assisted (Claude). Record: `docs/verification/HUD_PARITY.md`. Host behaviour change.

- The Flash HUD's `shield` fill and `shieldText`, and the canvas HUD's shield bar, now read `FOpenWillowMayaState` (drains on hits,
  recharges after the delay) instead of a full bar at the card's capacity. The text is the current pool rounded. The action-skill
  cooldown ring already read the Phaselock cooldown (`ActionSkillHudState`); unchanged.
- Test fixtures that meant "health to 0" or "half health" (`-owhuddownshots`, the quest's level-up fixture) add the shield, which now
  takes hits first.

