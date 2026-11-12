## 2026-10-22: Gameplay HUD parity: the movie by default, true perspective for the turned clusters, real values

AI-assisted (Claude Opus 5.5 orchestrating, Opus and Haiku sub-agents; critic GPT-6-Luna). Record:
`docs/verification/HUD_PARITY.md`. No parser behaviour changed.

- **Default HUD.** BL2's HUD movie (Ruffle overlay) is the gameplay HUD whenever its page reports ready; the host opens
  `http://127.0.0.1:8767/` without `-owflashhud`. The canvas HUD is a fallback (not ready, load error, `-owcanvashud`)
  and no longer draws the temporary Phaselock ring and orb. The movie counts only when the page's ready line echoes the fresh
  id the host put in its URL (`?owid=`); an older page from another checkout's server gives the canvas. The quest, mover, Phaselock-shots and character-capture runs pass `-owcanvashud` so their outcome does not
  depend on a server being up.
- **Perspective.** `tools/hud_overlay/hud_warp.js` draws the turned clusters through a per-clip homography from the
  fitted projection (F 914 derived from the definition's View3DFOV 70, read live; scale 1.095 fitted, UNVERIFIED), reading
  Ruffle's WebGL canvas (forced `preserveDrawingBuffer`) at 2x into an overlay canvas. The 2D stand-in (`hud3d.js`) stays
  for the minimap and as the fallback. Residuals against a real frame: 1-4 px, side clusters slightly nearer the centre.
- **Values.** Health and its rounded text, the equipped shield's card capacity (bar full: no shield model), magazine /
  reserve and grenade current / max with the game's no-break spaces, zero-padded level. Crosshair style, idle spread only (no
  widening while firing: not done) and ammo icon per weapon type from a live read (pistol 3/9/3, shotgun 7/17/5); other types UNVERIFIED and left alone.
  Ruffle hands non-ASCII ExternalInterface strings to AVM1 as UTF-8 bytes, so such text is set through `htmlText`
  entities (bench-verified).
- **Checks.** `tests/hud_warp_test.js`, `tests/hud_fields_test.js`, `tests/check_hud_shots_test.py` (36 synthetic cases); in-engine
  `tools/run_hud_shots.ps1` 17/17, `tools/test_action_skill_hud.ps1` 19/19, `tools/test_inventory_actions.ps1` 49/49
  (bounded recipe set). Critic 8.0, then 7.7 measured (crosshair spread per gun and 1-4 px side placement remain); blind
  A/B warp 8 vs stand-in 6, warp preferred in 6 of 6.

