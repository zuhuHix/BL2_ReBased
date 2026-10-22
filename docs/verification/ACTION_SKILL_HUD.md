# Maya's action-skill indicator on the HUD (Phaselock)

Lane C, 2026-10-22. AI-assisted (Claude Opus 5.5; capture by an Opus 5.5 sub-agent after a Codex sub-agent's sandbox
could not start the game). The host drives BL2's own HUD movie (converted to SWF, run in Ruffle over UE5,
`-owflashhud`); nothing here is redrawn natively. All frames, samples and converted movies stay under ignored `local/`.

## What the game shows

Two clips of the HUD movie (`UI_HUD.HUD`) make up the indicator. The script functions that drive them
(`WillowHUDGFxMovie.UpdateActionSkill`, `UpdateActionSkillPortrait`, `UpdateCharacter`, `EnableActionSkill`) are native
(function flags 0x20401), so the rules below were read from the running movie, not from code:

* **The Siren emblem** (`p1.activeAbility`): the magenta/cyan eye above the XP bar. It is the "sigil under the target"
  that the Phaselock effect record flagged as HUD (`PHASELOCK_STOCK_DATA.md`, "Not an effect of the skill"), and the
  "purple thing that runs out" in the maintainer's description. It sits on frame 1 (`none`) and goes to frame 3
  (`siren_action`) while the target is locked; its child `bar` (100 frames, 1 full, 100 empty) empties from right to
  left, leaving a dark teal outline.
* **The action-skill icon** (`p1.character.cooldown`, bottom left above the shield bar): frame 100 is the ready icon
  (green Siren with the black hand badge), frame 2 the running skill (gold, with flames, dark-red badge), frames 3-5 a
  gold-orange-red change and the later frames a dark-red veil over a grey icon that recedes upwards. The picture inside
  is the class's own icon movie `UI_HUD_AAIcons.AA-Siren`, loaded into `cooldown.icon.placeholderTexture` (the
  movie's built-in stand-in is a green crosshair). `p1.character` sits on frame 6 for Maya (the label `siren` is
  frame 5; both carry the same children).

## Real-game capture (2026-10-22)

`tools/real_game/scripts/action_skill_hud.py` samples, every rendered frame, the HUD movie
(`PlayerController.myHUD.HUDMovie`, `GetVariableNumber("p1.<clip>._currentframe")` and `_visible`/`_alpha`), the lift
skill's state and timing, and the cooldown pool (the player's `ResourcePool` whose definition is
`D_Resourcepools.PlayerPools.ActiveSkillCooldownPool_Siren`). `tools/real_game/action_skill_hud_capture.ps1` drives one
hit cast and one miss and takes screen frames. Maya level 8, Ice_P, one adult bullymong 650 uu away, about 55 fps,
saving blocked, save folder compared afterwards (no change). One target, one save, one PC.

Observed (t = seconds after the key press):

| Phase | t | Pool (s left of 13) | Emblem / bar | Icon frame |
|---|---|---|---|---|
| Ready | < 0 | 0 | off | 100 |
| Cast, lift | 0.02-0.69 | set to 13 on the press frame | off | 2 from 0.04 |
| Locked | 0.71-5.71 | 13, no drain | on; `bar = floor(1 + 99 (t - 0.70) / 5.0)` | 2 |
| Skill end | 5.714 | starts draining | off from 5.730 | 3 at 5.85 |
| Cooldown | 5.71-18.73 | drains at 1.000 /s | off | `floor(2 + 98 (1 - pool / 13))` |
| Ready again | 18.73 | 0 | off | 100 |

Both formulas match every sample when the previous rendered frame's value is used (one frame of lag); with the same
frame's value they are off by one frame on part of the samples. The emblem's bar does not pause during the release.
A miss (no pawn in view): the pool is set to 13 and drains at once; about 1.01 s later it snaps to 0 and the icon is
ready again; the emblem never shows. `p1.character.flash` stayed on its first frame in both casts. About 0.5-1.5 s
after ready a lighter, highlighted icon appears; which clip draws it was not found (UNVERIFIED, not driven by the host).
`_currentlabel` reads empty in the game's player, so frame labels are bench readings.

## HUD layout: the clusters' 3D turn (real game, 2026-10-22)

The first comparison (critic 7/10) showed the whole host HUD smaller and the icon further right than the game's. A second
live read (`tools/real_game/scripts/hud_layout.py`, `tools/real_game/hud_layout_capture.ps1`, Maya in Ice_P at 1280x720
and 1920x1080, settings and saves restored) found:

* No clip is moved or scaled in 2D: `Stage` 1280x720 `exactFit`, root on the `16_9` frame, `p1` at (216.05, 30.45), and
  every child of `p1` at its authored `_x`, `_y`, `_xscale`, `_yscale`. At 1920x1080 the frame is the 720p HUD
  stretched 1.5 times.
* The side clusters carry the movie's own `_yrotation`: +25 on health, shield, the action-skill icon (`character`),
  flaskBuff, badassToken, challenges, training and contextualPrompt; +15 on coop1-3; -25 on bullets, grenades,
  outerequip, slotmachine and MissionNotifications; -20 on minimapEcho. `_z`, `_xrotation`, `_perspfov` are 0. Ruffle
  keeps these values as plain properties (read back in the bench) but does not draw the turn.
* The movie definition `UI_HUD.HUD_Definition` has `View3DFOV` 70 (`bApplyHUDScale` true). A model fitted to 13 points
  measured on the live frame (rms 3.4 px with the focal length fixed by that FOV, 914 px): turn each clip about its
  origin, then project about the stage centre, so the unturned plane appears 1.095 times larger. The 1.095 is fitted,
  not read (UNVERIFIED); a plain 2D scale of the whole movie fits much worse (rms 14.8 px).

`tools/hud_overlay/hud3d.js` applies a 2D stand-in once the HUD has settled: `p1` scaled 1.095 about the stage centre,
and each turned child given the scale and position that meet the projection at its bounds' left and right edges. The
trapezoid of a turned clip (near edge taller) is not drawn. Applying it on the boot frame read wrong bounds in UE (the
icon was pushed off the left edge), so the page waits one second after the movie is ready.

## Host implementation

* `tools/hud_overlay/action_skill.js` maps the state to the movie calls above (emblem on/off and bar frame, icon frame,
  icon movie loaded once); `index.html` applies it after `character`. `tools/prepare_hud_icons.py` converts
  `UI_HUD_AAIcons.AA-Siren` (its one image record names no export, so the texture `AA-Siren_I1` is decoded first).
* `AOpenWillowMayaHUD::ActionSkillHudState` reads the host's Phaselock state through `AOpenWillowWalker`'s existing
  const accessors (no Phaselock code changed): the emblem's remaining fraction from the lock (`LockedAt`) to the skill's
  end (`SkillDuration`) of the last cast's timeline while the skill is active, and the icon's from the cooldown pool
  (`PhaselockRemaining() / PhaselockCooldown()`). The host's pool model already matched the capture: refilled at the
  cast, held through lift, lock and release, then 13 s at rate 1; a miss resets it after `ReleaseBufferTime` (1 s).
* `-owskillhudshots` (with `-owflashhud` and `-owcombattest`): aims at the dummy, casts, captures the screen with the
  HUD every 0.5 s from 1 s before to 22 s after the cast, quits. Each change of the inputs is logged as
  `OWSKILLHUD t=... active=... cooldown=...`.

Not reproduced: the one-frame lag; the unexplained post-ready highlight; anything the game does for other classes,
Gunzerker dual wield or vehicles.

## Checks

* `node tests/hud_action_skill_test.js`: the mapping over a synthetic cast against the formulas above (within one
  frame) and the call economy.
* `python tests/check_action_skill_hud_test.py`: the log checker on synthetic logs and generated PNGs, including its
  failure paths (no page read-back, the icon not following the state, the bar two frames late, a read-back state the host
  never sent, missing, empty or wrong-size captures, blank captures, the indicator in the wrong state, a run that never
  finished its shots, no `--shots`).
* `tools/test_action_skill_hud.ps1`: the in-engine run and `tools/check_action_skill_hud.py` on its log, in three groups.
  Host state (`OWSKILLHUD` lines, quantized to 1/100, the clips' resolution, so they change only when a drawn frame can):
  cast logged, ready before the cast, pool full at the cast and held while the emblem shows, emblem on at 0.70 s and off
  at 5.70 s within 0.15 s, emblem draining, drain rate 1 pool-second per second within 3 %, ready again at 18.73 s
  within 0.3 s. Movie (`OWSKILLPAGE` lines: after each change the page logs the state it applied and `_currentframe` read
  back from the emblem, its bar and the icon clip; the host logs the page console): every read-back's frames equal the
  frames that state maps to, every echoed state is one the host logged (so no lag allowance is needed), at least 20
  read-backs, the emblem bar from full (5 or less) to nearly empty (90 or more: a capture stall near the end can skip the last values), icon frames 2, 50-99 and a final 100.
  Captures (`--shots`, required for a PASS; without it the result is PARTIAL, exit 2): all 47 scheduled `OWSkillHud_*.png`
  of this run exist as 1280x720 PNG files, `OWSKILLHUDSHOTS done` is logged, and three of them, decoded with zlib only,
  show the indicator by colour counts in fixed screen boxes: green icon and no magenta emblem before the cast, gold
  icon and magenta emblem at +2.5 s, red icon and no emblem at +12 s. A run that has not exited within the timeout fails
  without running the checker. How close the captures look to the game is judged by the critic, not by this check.

## Results (2026-10-22, this PC)

* In-engine (`tools/test_action_skill_hud.ps1`, three runs): 9/9 checks each time. Last run: emblem on 0.713 s and off
  5.702 s after the cast, pool held through the lock, drain 1.000 pool-second per second, ready again at 18.698 s
  (game 18.73 s).
* With the movie read-back and capture checks (review round 1): 15/15. 175 read-backs, all within a frame of the host
  state; emblem bar seen from frame 1 to 99, icon frames 2 to 100; 47 of 47 captures written.
* Review round 2 (state quantized to 1/100, read-backs paired with their own state, captures decoded): 19/19. 169
  read-backs, all equal to the frames of their state; 47 of 47 valid 1280x720 captures; colour counts before the cast
  (icon green 970), at +2.5 s (icon gold 868, emblem magenta 1786) and at +12 s (icon red 215, no emblem). Quantizing
  cut the run's state lines from 1178 to 172 (page pushes alike); the drain rate read 1.003 and ready 18.700 s.
* Visual (independent critic, GPT-6-Luna, host and real frames at eight moments plus a drain strip; frames under ignored
  `local/laneC/pairs_r1`, `pairs_r2`): round 1 **7/10** (icon about 30 px right and 12 px high, emblem about 10 %
  narrower: the missing 3D layout); round 2, with `hud3d.js`, **9/10**. Remaining, ranked: the emblem's outer haze and
  streaks slightly tighter and fainter (5-10 px), the ready icon's green 5-10 % brighter. Both look like Ruffle's
  rendering of the movie's own art (filters and blending), not the state mapping; not pursued.
