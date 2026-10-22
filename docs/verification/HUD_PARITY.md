# Gameplay HUD parity (BL2's HUD movie over UE5)

HUD lane, 2026-10-22. AI-assisted (Claude Opus 5.5 orchestrating; the perspective warp designed and written by an Opus
5.5 sub-agent, the host changes, the real-game capture and the log checker by Claude Haiku 5.5 sub-agents; visual
critic Codex GPT-6-Luna). The host drives BL2's own HUD movie (`UI_HUD.HUD`, converted to SWF, run in Ruffle in a
transparent page over UE5); nothing below is redrawn natively. Converted movies, real-game frames, live reads and host
captures stay under ignored `local/`.

## What changed

* **The movie is the default gameplay HUD.** `AOpenWillowMayaHUD` opens the overlay page at `http://127.0.0.1:8767/`
  (the port `tools/run_ue_flash_hud.ps1` and `tools/hud_overlay/serve.py` use) when `-owflashhud=<url>` is not given.
  The movie counts only once the page logs `OpenWillow HUD movie ready`; until then, or after a load error (the overlay
  is then removed), the canvas fallback draws. `-owcanvashud` forces the fallback. The log line
  `OpenWillow HUD path: movie|canvas (<reason>)` says which path is in use.
* **Only this checkout's pages.** Another worktree's `serve.py` can answer on the same port with its own, possibly
  older, pages. The page's ready line names the pages directory its server serves (`serve.py`'s `X-OpenWillow-Pages`
  header, `OpenWillow HUD movie ready pages=<dir>`); the host shows the movie only when that is its own
  `tools/hud_overlay` and otherwise falls back to the canvas. Checked in UE against another checkout's older server
  (its ready line names no directory): `OpenWillow HUD path: canvas (the page is served from '', not this checkout's
  tools/hud_overlay)`.
* **Test and capture runs keep the canvas HUD.** `tools/test_quest.ps1`, `tools/test_mover.ps1`,
  `tools/run_phaselock_shots.ps1` and `tools/run_ue_character.ps1` pass `-owcanvashud`, so their outcome cannot depend
  on whether an overlay server happens to be listening (the Maya HUD exists only with `-owwalk -owmaya`; the ambient,
  UE-viewer and probe runs have no Maya HUD). The interactive `tools/run_quest.ps1` keeps the default.
* **The temporary skill indicator is gone.** The canvas fallback no longer draws the violet ring, orb and countdown it
  used for Phaselock; it keeps a crosshair, the shield and health bars, the XP bar with the level and the weapon panel,
  with no skill indicator at all. Maya's action-skill indicator is the movie's own (`ACTION_SKILL_HUD.md`).
* **True perspective for the turned clusters** (`tools/hud_overlay/hud_warp.js`, below), replacing the 2D stand-in
  for every turned clip except the minimap (`minimapEcho` keeps the stand-in; another lane owns it).
* **Real values** pushed by `PushFlashHudState` (below) and the crosshair style, its idle (at-rest) spread and the ammo
  icon per weapon type (`tools/hud_overlay/hud_fields.js`). Not done: the crosshair does not widen while firing (the host
  has no live spread or bloom state to drive it from; the game's widening rule was not read).

## Real-game reads (2026-10-22)

`tools/real_game/hud_parity_capture.ps1` with `tools/real_game/scripts/hud_parity.py`: Maya in Sanctuary, 1280x720
windowed, saving blocked, save folder and `WillowEngine.ini` compared afterwards (changed files restored from the
backup). One character, one PC; only a pistol and a shotgun were available, so only those two weapon types were read.
What a save holds (its items, counts and pool values) is not recorded here; the rules below are what the HUD showed.

| Read | Pistol | Shotgun |
|---|---|---|
| `p1.crosshairs.outer` frame (crosshair style) | 3 | 7 |
| `p1.crosshairs.outer.weapon` frame (spread) at rest | 9 | 17 (it widened while firing and settled one frame wider) |
| `p1.bullets.bullet_icon` frame (ammo icon) | 3 | 5 |

* The ammo text is `<magazine> / <reserve>` and the grenade text `<count> / <max>`; the spaces around `/` are no-break
  spaces (U+00A0), not ASCII spaces. Example with invented numbers: `12 / 340`.
* Health and shield texts are the pools rounded to the nearest integer (an invented pool of 250.4 would show `250`, 250.6
  `251`). The level text is zero-padded to two digits (`07`); the class-mod text was empty.
* Firing three shots lowered the magazine by three and left the reserve unchanged in the text.

## Perspective warp (`tools/hud_overlay/hud_warp.js`)

The game turns the side clusters about the vertical axis (`_yrotation` +25 on health, shield, the action-skill icon and
four more; -25 on ammo, grenades and three more; `ACTION_SKILL_HUD.md`, "HUD layout"). Ruffle draws only 2D transforms,
so the earlier stand-in (`hud3d.js`) gave each turned clip one scale and position: a rectangle, not the trapezoid.

* **Model** (fitted earlier on a live frame, unchanged): turn a clip about its origin (`x1 = ox + u cos a`,
  `z = u sin a`, `u = x - ox`), then project about the stage centre with focal length F = 914 px (the movie definition's
  View3DFOV 70 over the 1280 px stage) and the unturned plane magnified 1.095 (fitted, UNVERIFIED). Both screen
  coordinates and the depth term are linear in (u, y, 1), so each clip's mapping is exactly a 3x3 homography.
* **Drawing.** `hud_warp.js` makes Ruffle's WebGL context keep its drawing buffer (it wraps `getContext` before
  `ruffle.js` loads), hides the player (opacity 0; it keeps drawing) and copies its canvas into an overlay WebGL canvas
  every animation frame. Source pixels no turned clip owns are drawn in place (the unturned clips are already where the
  game shows them, since `hud3d.js` still scales `p1` by 1.095); each turned clip's pixels are drawn through its own
  homography (inverse mapping per output pixel, premultiplied alpha). Each source pixel has one owner: a clip's own
  bounds first, then its bounds plus a 12 px glow margin, smaller clips first. The action-skill icon turns about a
  point 6 px from the health bar's, which would put it up to 1.7 px off under the bar's mapping, so every clip keeps its own.
* **Supersampling.** The hidden player renders at twice the page size and the overlay canvas keeps that size, so the
  browser's downscale averages the resampled pixels. One bilinear tap per output pixel at 1:1 left the turned bars and
  their digits visibly softer than the game's on a host frame; at 2x the digits' outlines match the game's sharpness by
  eye.
* **Fallback.** No WebGL for the overlay, a Ruffle context that cannot be read (WebGPU), `?hud3d=affine` in the page
  URL, or a server that does not serve `hud_warp.js`: the page keeps the 2D stand-in for every turned clip. It logs
  `OpenWillow HUD 3D path warp|affine: <why>`.
* **Bounds.** `getBounds` includes hidden children (the badassToken and challenges panels span a third of the screen),
  so the regions are the union of the visible children, re-read when the applied state changes (at most every 250 ms)
  or every 5 s, and split again when Ruffle resizes its canvas.

Measured against the real 1280x720 frame (bench, Edge headless): the health bar's left edge 22.7 px tall and its right
tip 20.8 px (real left edge about 23 px; the stand-in drew 21.9 px everywhere); 9 hand-measured points rms 3.0 px, max
5.6 px. Registering the warped bench against the real frame over many edge pixels leaves the left cluster about 1.5 px,
the right cluster about 3-4 px and the XP bar about 1 px off: both side clusters sit slightly further out in the game
and everything about 1 px higher. A stronger perspective (F 860-890) or a scale of 1.097 fits the hand points no better
(rms 2.9-3.1 px), so F stays at the FOV-derived 914 and the scale at 1.095. In UE (CEF, 1280x720) the log reports the
warp path and the in-engine checks below pass. Cost in the bench: the warp's JavaScript 0.13 ms per frame, frame
interval unchanged; reading the regions about 30 ms each time.

Not established: where the 1.095 comes from. An attempt to find Scaleform's projection rule in the executable with the
REA tools timed out on the 27 MB binary and produced nothing; a live read of the game's projected points (Scaleform's
own 3D matrices were not exposed through `GetDisplayMatrix`) would settle it.

## Values the host pushes (`PushFlashHudState`)

| Field | Source | Faked or missing |
|---|---|---|
| health bar, `healthText` | `AOpenWillowWalker::GetHealth()/GetMaxHealth()`, text rounded | none |
| shield bar, `shieldText` | the equipped shield's `Capacity` card stat; with no shield, the movie's `none` frame | the bar stays full: the host has no shield damage or recharge model |
| ammo bar, `ammoText` | magazine left / size; `magazine / reserve` from the weapon's ammo pool, no-break spaces | a no-cost weapon (Infinity) shows the magazine only; the game's display for it is not observed (UNVERIFIED) |
| grenade bar, `grenadeText` | the grenade ammo pool `current / max`, hidden when its max is 0 | the grenade icon's type (`grenades.protean.payload`) is not driven |
| `levelText`, XP bar | `UOpenWillowSkills`, level zero-padded | the class-mod title (`comm`) is not sent |
| `weaponType` | the held weapon's ammo type key | only pistol and shotgun frames are known; other types keep the movie's own frames (UNVERIFIED); the spread does not widen while firing |

Non-ASCII text reaches the movie through `htmlText` as numeric entities: Ruffle's ExternalInterface hands non-ASCII
strings to the movie as their UTF-8 bytes, which drew the no-break space as an A-circumflex and a space; `htmlText`
keeps the field's own format (seen in the bench).

## Checks

* `node tests/hud_warp_test.js`: the homography against the written-out formula (0.01 px, six turns, seven origins, a
  grid of points), inverses at three canvas sizes, the trapezoid in both directions (+25: near edge taller and the bar
  longer; -25 mirrored), every source pixel drawn once and inside its region's quad, reserved regions kept in place, the
  path choice and fallbacks, the context patch on a fake canvas, the visible-bounds rules on a fake harness.
* `node tests/hud_fields_test.js`: ASCII text as `text`, no-break spaces and markup characters as `htmlText` entities, a
  non-BMP character as one entity; the pistol and shotgun frames; no frames for unobserved types.
* `node tests/hud_action_skill_test.js` (unchanged contract, still green).
* `python tests/check_hud_shots_test.py` (16 synthetic cases) for `tools/check_hud_shots.py`, the checker of
  `tools/run_hud_shots.ps1`: one `-game` run with the slice weapons and `-owhudshots` (the held weapon at rest, three
  shots, quit; `-Shield` equips the first gear-manifest shield the level allows, `-Level` sets Maya's level, `-Affine`
  selects the stand-in for A/B captures). Checks: the movie path, the warp path, the state line format (level padding,
  ` ` separators), the shield shown, each shot one round lower with the reserve unchanged, the run finished, both
  captures valid 1280x720 PNGs, not blank, with the health bar's red and the ammo bar's blue in fixed boxes. Movie
  read-backs: after each change the page logs `OWHUDPAGE`, the state it applied (written as the host's `OWHUDSTATE`)
  and what the movie shows for it (the health, shield, ammo and grenade bar frames, the five texts, the crosshair style,
  spread and ammo-icon frames; `hud_fields.js`, `pageLine`); the checker requires every echoed state to be one the host
  logged, every frame and text to equal what that state maps to, and the last page ammo text to be the host's last.
* In-engine, this PC: `tools/run_hud_shots.ps1 -Level 36 -Shield` 15/15 with the page read-backs (11/11 before them) (magazine 7, 6, 5, 4; reserve 200; shield
  the first equippable gear-manifest shield); `tools/test_action_skill_hud.ps1` 19/19 (three runs during the lane: one 18/19, see below, then 19/19 twice).
  The 18/19 run pushed the state before the scripted cast in the same frame, so the full pool appeared one frame late
  (0.152 s against 0.15 s); the cast now runs first.
* `tools/test_inventory_actions.ps1 -Items <bounded recipe set>` (the set the 2026-10-22 post-merge verification used):
  49/49. Run with the full local demo library instead it gave 24 pass, 3 fail, 22 not run: that library fills the 39-slot
  backpack, so unequipping into it is refused (`CanAddToBackpack`); local data, not this change.

## Visual review

Independent critic (GPT-6-Luna), host captures against real frames at matched moments (Maya, pistol at rest; after
three shots), numbers and other state differences excluded. Frames under ignored `local/hudp/pairs_r1`, `pairs_r2`,
`ab3`.

* Round 1 (warp at 1:1): **8.0/10**. Its points (crosshair curved, glow 6-10 px narrower, left cluster 6-8 px right,
  trapezoids 2 degrees flatter) were checked with measurements in round 2 and mostly dropped.
* Round 2 (warp with 2x supersampling): **7.7/10**, measured on the full-resolution frames: the pistol crosshair's ticks
  span 49 px against 44 px; the side clusters about 3 px nearer the centre (as the registration above); the "LV" label
  2 px lower; bar spans confounded by the different digits over them. The crosshair difference does not follow the 1.095
  scale (the shotgun's brackets differ by about 7 %, the pistol's by about 20 %), so the spread probably depends on the
  gun or animates; not pursued.
* Round 3, blind A/B in UE (`tools/run_hud_shots.ps1` with and without `-Affine`, the same state; REAL, A and B stacked
  per region, the assignment shuffled per moment and kept away from the critic): the warp was preferred in all six
  comparisons (left, right and full frame, at rest and after firing; confidence 5, 4, 5, 5, 4, 5). Deciding measurements:
  the real health bar's fill spans y 369-440 at one column and 333-400 at another; the warp 377-439 and 341-401 (the
  same taper, offset); the stand-in 371-433 at both (no taper); the ammo bar's left tip x 283 real, 282-286 warp,
  305-312 stand-in; the stand-in also clips the action-skill icon at the screen edge. Whole-HUD scores: warp **8/10**,
  stand-in **6/10**, at both moments.

Remaining, ranked: crosshair spread per gun (not modelled); the side clusters 1-4 px nearer the centre than in the game
(projection constants fitted, UNVERIFIED); the grenade icon's type; Ruffle's rendering of the movie's glow filters.
