## 2026-10-22: Maya's action-skill indicator drives BL2's own HUD clips; the HUD clusters' 3D turn gets a 2D stand-in

The maintainer asked for the purple indicator that runs out while Phaselock holds its target. It is two clips of the HUD
movie, driven in the game by native code (`WillowHUDGFxMovie.UpdateActionSkill` and its neighbours are native), so the
rules were read from the live movie during real casts (AI-assisted capture; record
`docs/verification/ACTION_SKILL_HUD.md`):
- `p1.activeAbility` shows the Siren emblem (`siren_action`) from the lock (0.70 s) to the skill's end (5.70 s); its bar
  is `floor(1 + 99 * elapsed / lock time)`. This emblem is also the "sigil under the target" that the Phaselock effect
  record had flagged as HUD.
- `p1.character.cooldown` follows the cooldown pool: `floor(2 + 98 * (1 - pool / 13))`, 2 while the skill runs (the pool
  is held at 13 through lift, lock and release), 100 when ready. The icon picture is `UI_HUD_AAIcons.AA-Siren`, loaded
  into `cooldown.icon.placeholderTexture`; `p1.character` sits on frame 6. A miss refills the pool, which drains and
  snaps to 0 after about 1 s. The host's existing pool model already matched; no Phaselock code changed.
- The whole HUD looked smaller in the host because the side clusters carry `_yrotation` (+-25, 15, -20) that Ruffle
  ignores, projected with the movie's 70-degree FOV; the unturned plane appears 1.095 times larger about the centre
  (fitted, UNVERIFIED). `tools/hud_overlay/hud3d.js` approximates it with per-clip 2D transforms (no trapezoid).
Implemented in `tools/hud_overlay/action_skill.js`, `hud3d.js`, `index.html` and `AOpenWillowMayaHUD`
(`ActionSkillHudState`, `-owskillhudshots`), with `tools/prepare_hud_icons.py` for the icon movie. Checked: synthetic
mapping and checker tests, the in-engine run 9/9 against the real timings, critic 7/10 then 9/10. Not reproduced: the
one-frame lag of the game's HUD, a post-ready highlight whose clip was not found, other classes.

