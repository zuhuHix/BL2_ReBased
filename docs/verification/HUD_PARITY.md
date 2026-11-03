# Gameplay HUD parity (BL2's HUD movie over UE5)

HUD lane, 2026-10-22. AI-assisted (Claude Opus 5.5 orchestrating; the perspective warp designed and written by an Opus
5.5 sub-agent, the host changes, the real-game capture and the log checker by Claude Haiku 5.5 sub-agents; visual
critic Codex GPT-6-Luna). The host drives BL2's own HUD movie (`UI_HUD.HUD`, converted to SWF, run in Ruffle in a
transparent page over UE5); nothing below is redrawn natively. Converted movies, real-game frames, live reads and host
captures stay under ignored `local/`.

## Element inventory (the slice loop)

HUD lane, 2026-11-03. The rows are the HUD movie's own named clips under `p1` (read from the converted `UI_HUD.HUD`
with a local SWF tag listing; the listing stays under ignored `local/analysis/hud/`), plus the two effects the movie
has no clip for. "Slice" says whether BL2 shows the element in the Phase 4 loop (spawn, fight, loot, equip, Phaselock,
one mission, die, respawn). "Driven" means the host pushes real state to that clip and a check reads back what the
movie shows. Scores are the independent critic's, and only where a matched real-game frame existed.

| Element (clip) | Slice | Host driver today | Check | Parity |
|---|---|---|---|---|
| Health bar and text (`health`) | yes | driven: `AOpenWillowWalker` health | `check_hud_shots.py` | 8/10 (2026-10-22, with the clusters) |
| Shield bar and text (`shield`) | yes | partly: the equipped shield's capacity; the bar stays full | `check_hud_shots.py` | as health; no damage or recharge model |
| Shield break (`shielddown`) | yes, when the shield breaks | not driven: the host has no shield damage | none | missing |
| Ammo bar, text, icon (`bullets`) | yes | driven: magazine, reserve and ammo type | `check_hud_shots.py` | 8/10 (with the clusters) |
| Reload / empty prompt (`reload`: `reload`, `depleted`, `empty`) | yes | not driven: the movie leaves its `desc` text and the choice of state to native code (`UpdateAmmo` is native), neither read | none | missing |
| Grenade count (`grenades`) | yes | driven: count and max. The icon's type (`protean.payload`: `none`, `standard`, `transfusion`, `mirv`, `bouncing`, `area`, `singularity`) is not: the host's grenade mod carries no delivery type, and guessing it from the name was not done | `check_hud_shots.py` | type missing |
| Crosshair (`crosshairs`) | yes | style and idle spread for pistol and shotgun; no widening while firing: the host's spread is the card value, with no bloom state to drive it | `check_hud_shots.py` | 7.7-8/10; spread per gun open |
| XP bar, level, class-mod text (`xpbar`, `levelClassMod`) | yes | driven: XP and level; the class-mod title is not sent | `check_hud_shots.py` | with the clusters |
| Level-up banner (`levelup`, label `go`) | yes, on the mission reward | **driven** (HUD lane 2): any rise of Maya's level plays `go`; the movie writes its own text and flashes the XP bar | `check_hud_down.py` (feedback) | UNVERIFIED: no real frame |
| Action skill (`character`, `activeAbility`) | yes | driven (ACTION_SKILL_HUD.md) | `test_action_skill_hud.ps1` | see ACTION_SKILL_HUD.md |
| Minimap and compass (`minimapEcho`) | yes | driven (HUD_MINIMAP.md) | `test_hud_minimap.ps1` | see HUD_MINIMAP.md |
| Fight For Your Life (`dying1`: `fight` label, 100-frame bleed-out bar, title and hint) | yes | **driven** (this lane): the Walker's down state | `check_hud_down.py` (down) | UNVERIFIED: no real frame on this PC |
| Second wind banner (`secondwind`, from `go`) | yes | **driven** (this lane) | `check_hud_down.py` (down) | UNVERIFIED: no real frame |
| Respawn fee (`moneyRemoval`, labels `in`, `out`, `ganked`) | yes, after the respawn | not driven: the host has no currency or fee | none | missing |
| Mission tracker (`MissionNotifications.Module1-6`) | yes | **driven** (this lane): mission name, objectives with their progress text, turn-in | `check_hud_down.py` (tracker) | UNVERIFIED: no real frame on this PC |
| Mission banners (`MissionFeedbackController`) | yes (accepted, complete) | not driven | none | missing |
| Interaction prompt (`tools`, `toolsSecondary`: `talk`, `use`, `guns`, ...; `desc` text) | yes (Marcus, the gun pickup) | not driven: the host shows a debug text line. The label and text come from the native `UpdateInteractionIcon` (the HUD's handler is native too), not read | none | missing |
| Contextual prompt (`contextualPrompt`) | unknown | not driven | none | unknown whether the loop shows it |
| Pickup item card (`outerequip`, `equipcard` with `stat1-7`, `manufacturer`) | yes (looking at a gun) | not driven | none | missing |
| Damage direction (`dmg_u`, `dmg_d`, `dmg_l`, `dmg_r`) | yes, when Maya is hit | **driven** (HUD lane 2) from any hit with a known source; but nothing in the UE host attacks Maya yet (enemy attacks run in the C++ VM only, PR #59) | `check_hud_down.py` (feedback) | UNVERIFIED: no real frame; side split and inner label UNVERIFIED |
| Area name (`WorldDiscovery`) | yes, entering Sanctuary | not driven | none | missing |
| Damage numbers | yes | the host's canvas draws them over the movie; the movie has no clip for them | none | not compared |
| Hit marker | no in stock BL2 (no clip in the movie; a mod adds one, UNVERIFIED) | none | none | n/a |
| Not in the slice loop | | `AssassinView`, `badassToken`, `bossModule`, `boundry`, `challenges`, `coop1-3`, `crosshairsDualWield`, `crouch`, `eridiumCounter`, `flaskBuff`, `instinct`, `instinctbar`, `radial`, `ring`, `ShieldAbsorb`, `slotmachine`, `tradingTips`, `training`, `triggeredSkills` | | |

## What changed

* **The movie is the default gameplay HUD.** `AOpenWillowMayaHUD` opens the overlay page at `http://127.0.0.1:8767/`
  (the port `tools/run_ue_flash_hud.ps1` and `tools/hud_overlay/serve.py` use) when `-owflashhud=<url>` is not given.
  The movie counts only once the page logs `OpenWillow HUD movie ready`; until then, or after a load error (the overlay
  is then removed), the canvas fallback draws, and the overlay stays hidden (it loads and runs, as the preloaded menu
  pages do); it is shown in the same place the canvas stops, so the two never show together. A menu that hides it
  restores it only if the page was accepted. `-owcanvashud` forces the fallback. The log line
  `OpenWillow HUD path: movie|canvas (<reason>)` says which path is in use.
* **Only a page that answers this run.** Another worktree's `serve.py` can answer on the same port with its own, older
  pages. The host appends a fresh id to the page URL (`?owid=<guid>`) and the page echoes it in its ready line
  (`OpenWillow HUD movie ready id=<id> pages=<dir>`, `hud_fields.js` `readyLine`); the host parses exactly that form
  (the first token after `id=`, followed by ` pages=`) and the movie counts only on an exact match, else the canvas
  draws. The id is set before any `#fragment` and replaces an `owid` already in the URL (`WithPageId`; not unit-tested:
  the module has no test harness for it, checked by reading and by the runs below). No path is compared, so a checkout opened through a junction or alias is not refused
  (a first version compared the served directory with the host's own and would have been). `pages=` (serve.py's
  `X-OpenWillow-Pages` header) is logged for diagnosis only, so a server from another checkout whose pages already
  speak this protocol is accepted; the launchers (`tools/run_ue_flash_hud.ps1`) still refuse such a server by that
  header. Checked in UE against another checkout's older server: `OpenWillow HUD path: canvas (the page did not echo
  this run's id: another server or older pages)`.
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

## Fight For Your Life, second wind and respawn (HUD lane, 2026-11-03)

AI-assisted (Claude Opus 5.5). Health 0 no longer respawns Maya at once. `AOpenWillowWalker` now has a down state, with
the live numbers of [COMBAT_DOWN_RESPAWN.md](COMBAT_DOWN_RESPAWN.md):

* Health 0 puts her down (Injured). The bleed-out timer starts 2.28 s later and runs 12 s. Successive injuries cut it by
  12 / 3 each, to a 1 s minimum; an injury less than 30 s after the previous one counts as successive.
* The end of the bleed-out is death (Dead), and the respawn follows 5.76 s later. The respawn resets the successive count.
* A kill while Injured is a second wind: health goes back to 25 % of the maximum. `AOpenWillowCombatTarget` reports the
  kill when Maya is the damage causer.
* Damage while down is ignored, and the Phaselock gate sees the injured state.
* A health refill while Injured (a level-up) ends the down state, so "down" always means health 0. The game is widely
  reported to revive a player who levels up in Fight For Your Life; that is not observed here (UNVERIFIED).
* **The weapon while down** (host rule, UNVERIFIED; to confirm live with bl-sdk). It follows the script read in
  NATIVE_DAMAGE_DEATH.md 10.1 and COMBAT_DOWN_RESPAWN.md: the injured state's special move puts the weapon away
  (`InjuredWeaponPutDownTime` 0.1 s), and its end, `SwitchToSidearm` (2.28 s live), brings one back
  (`InjuredWeaponEquipTime` 0.2 s).
  * Going down puts the weapon away: it releases the trigger, drops the shots a burst still owes, and cancels a running
    reload. A cancel is the same as for a weapon change: a refill already past its point is kept.
  * Until 2.28 s after the injury (`AOpenWillowWalker::DownWeaponReady`), fire and reload presses do nothing.
  * After that, the held weapon fires and reloads as usual; Fight For Your Life is fought with the gun.
  * While Dead nothing fires.
  * Clearing the trigger on entry is part of the put-away, so it does not contradict firing later: a new press is
    needed, as after any put-away.
  * Not modelled: the switch to a sidearm (the held weapon comes back), and the 0.1 s / 0.2 s put-down and equip times
    inside the 2.28 s.
* Not modelled: crawling, the injured bonus damage, the shield half, Defense Boost, a partner revive and the respawn fee
  (`moneyRemoval`).

The host pushes `down` (`null`, or the phase and the bleed-out left, quantised to the bar's 100 frames) and `secondWind`
(a count; the page plays `p1.secondwind` from `go` once per increase). The page puts `p1.dying1` on its `fight` label
while injured, with the bleed-out bar on frame `1 + round((1 - left) * 99)`, and off otherwise. The title and hint come
from `hud_strings.json` (`tools/prepare_hud_strings.py`: the install's own localization, written under ignored
`local/`). Which of those fields the native fills, and with which string, was not observed (UNVERIFIED).

**Nothing in the UE host damages Maya yet.** The enemy attacks of PR #59 run in the C++ VM only, so in the host only the
capture sequence and the quest suite reach the down state.

**Mission tracker.** `MissionSystem::trackerLines()` (`src/mission.*`) lists the active objective set, each objective
with its `ProgressMessage` from the package; `--mission-run` output ends with a `"tracker"` array. The quest exposes it
read-only (`UOpenWillowQuest::HudTracker`). The page fills `p1.MissionNotifications.Module1-6`: the mission name, one
module per objective (`Complete`, else `Optional` for an optional objective, else `NotComplete`), or `TurnIn` when ready
to turn in, and every module is off without a mission. The host's `OWHUDTRACK` line marks each objective `[x]`, `[o]` or
`[ ]` the same way, and the checker requires the matching inner frame. In the Fire mission run it shows the first objective, then the second, then the turn-in, then nothing.
How long the game keeps a completed objective on screen was not observed, so the host replaces it at the next change.
The page and the host's log share the tracker data, so they could agree on a wrong objective set. The set selection
itself is therefore checked independently in `tests/mission_script_test.py` (scenario T, below). There the mission model
refuses objective updates once a mission is ready to turn in, so an optional objective left open then stays open;
whether the game still counts it is UNVERIFIED (a mission-model question, not a HUD one).

## Hit arrows and the level-up banner (HUD lane 2, 2026-11-03)

AI-assisted (Claude Opus 5.5). What the movie does by itself, read from its own frame scripts in the converted movie (the
listing stays under ignored `local/analysis/hud/`):

* **Damage arrows.** Each of `dmg_u`, `dmg_r`, `dmg_d` and `dmg_l` stops on frame 1 with its inner clip hidden. From frame 2
  it shows the inner clip and animates by itself over 36 frames, with random offsets. The inner clip's labels name what
  took the hit: `Health`, or a shield type (`Original`, `Generic`, `Nova`, `Absorb`, ...).
* **Level-up banner.** `levelup`'s `go` frame writes the banner text itself, starts the inner animation and flashes the
  XP bar.

What the host sends:

* **Hits.** `AOpenWillowWalker::TakeDamage` counts every hit that took health and has a known source: a point hit's shot
  direction reversed, else the causer's or the instigator's position. The HUD turns the last source's direction into
  a quadrant of the view yaw: within 45 degrees ahead is `u`, within 45 degrees of behind is `d`, otherwise `r` or `l`.
  On each new hit the page plays that side's arrow from frame 2 on its `Health` frame. The host has no shield damage,
  so every hit is `Health`. How the game splits the circle and picks the label is native (UNVERIFIED).
* **Level-ups.** The HUD counts any rise of Maya's level after the first one it reads, and the page plays `levelup` from
  `go`. `levelDisp.text` is left alone, because what the native puts there was not observed.

Not done, because there is nothing real to drive them:

* the reload prompt: its state and its `desc` text are native (`WillowHUDGFxMovie.UpdateAmmo`);
* crosshair widening: the host has no bloom state;
* the grenade icon type: the grenade mod item has no delivery type.

## Checks

* `node tests/hud_warp_test.js`: the homography against the written-out formula (0.01 px, six turns, seven origins, a
  grid of points), inverses at three canvas sizes, the trapezoid in both directions (+25: near edge taller and the bar
  longer; -25 mirrored), every source pixel drawn once and inside its region's quad, reserved regions kept in place, the
  path choice and fallbacks, the context patch on a fake canvas, the visible-bounds rules on a fake harness.
* `node tests/hud_fields_test.js`: ASCII text as `text`, no-break spaces and markup characters as `htmlText` entities, a
  non-BMP character as one entity; the pistol and shotgun frames; no frames for unobserved types; the `OWHUDPAGE`
  line (`pageLine`); the ready line echoing the URL id wherever it sits in the query, `id=-` without one (`readyLine`).
* `node tests/hud_action_skill_test.js` (unchanged contract, still green).
* `python tests/check_hud_shots_test.py` (36 synthetic cases) for `tools/check_hud_shots.py`, the checker of
  `tools/run_hud_shots.ps1`: one `-game` run with the slice weapons and `-owhudshots` (the held weapon at rest, three
  shots, quit; `-Shield` equips the first gear-manifest shield the level allows, `-Level` sets Maya's level, `-Affine`
  selects the stand-in for A/B captures). Checks: the movie path, the warp path, the state line format (level padding,
  ` ` separators), the shield shown, each shot one round lower with the reserve unchanged, the run finished, both
  captures valid 1280x720 PNGs, not blank, with the health bar's red and the ammo bar's blue in fixed boxes. Movie
  read-backs: after each change the page logs `OWHUDPAGE`, the state it applied (written as the host's `OWHUDSTATE`)
  and what the movie shows for it (the health, shield, ammo and grenade bar frames, the five texts, the crosshair style,
  spread and ammo-icon frames; `hud_fields.js`, `pageLine`); the checker pairs each read-back with the latest host state logged before it and requires
  its echo to equal that state, an acknowledgement for every host state from the first one the page saw, every frame
  and text to equal what the state maps to, and the last page ammo text to be the host's last. `page_identity`: the
  ready line, in the host's HUD-console envelope and the page's exact form (look-alike lines from another console or
  inside another message do not count), echoes the id in the overlay URL.
* In-engine, this PC: `tools/run_hud_shots.ps1 -Level 36 -Shield` 17/17 with the page read-backs and the page id (11/11 before them);
  the same checker on a run against another checkout's older server fails `movie_path` and `page_identity`, as intended (magazine 7, 6, 5, 4; reserve 200; shield
  the first equippable gear-manifest shield); `tools/test_action_skill_hud.ps1` 19/19 (three runs during the lane: one 18/19, see below, then 19/19 twice).
  The 18/19 run pushed the state before the scripted cast in the same frame, so the full pool appeared one frame late
  (0.152 s against 0.15 s); the cast now runs first.
* `python tests/check_hud_down_test.py` (20 synthetic cases) for `tools/check_hud_down.py`, the checker of
  `tools/run_hud_shots.ps1 -Down` and `-Tracker`.
  * Down: a matching run passes. A late bleed-out, a late respawn, a wrong second-wind health, a missing page echo, a
    wrong bar frame, no strings file, an unreadable strings file, a title other than the strings file's, no second wind
    and the canvas path each fail the check meant to catch them.
  * `down_page_texts` compares every injured title the page shows with the `injured` value of the generated strings
    file (`--strings`); it reports only match or mismatch, never the text.
  * `down_shots` checks more than file validity: the bleed-out bar's red must fill a fixed box (400-900 x 380-580) in
    down_start and down_mid, be absent from the other four captures, and end at least 40 px further left at down_mid.
    The box and colour were measured on this lane's captures (about 3,500 and 2,300 pixels; 0 elsewhere). Synthetic
    PNGs cover: no panel, a panel in the `up` capture, a bar that does not drain, and a missing capture.
  * Tracker: a matching run with an optional objective passes; a wrong objective text, an optional objective shown as
    `NotComplete` and a run without the turn-in fail. All fixture texts are invented.
* `down_weapon` (the same `-Down` run): the first injury lands in the tick of a 3-shot burst's first shot. That burst
  comes from a test fixture that sets the held weapon's burst count to 3, then restores it; no slice gun has a burst
  count above 1. The check requires:
  * the two owed shots dropped;
  * fire and reload presses during the put-away doing nothing;
  * a press after the put-away firing, and a reload starting;
  * the second injury, landing mid-reload, cancelling it.

  `tests/check_hud_down_test.py` adds 6 synthetic cases for it (26 in all).
* `python tests/prepare_hud_strings_test.py` (4 cases): `--output` must resolve inside this repository's `local/`. An
  outside path, a repository path outside `local/`, and a path that climbs out with `..` are refused with exit 2 and
  nothing written. A path under `local/` gets the invented strings.
* `tests/mission_script_test.py` scenario T (ctest `mission-script-synthetic`): a toy mission whose first set
  auto-enables a second set (one objective with count 2, one optional), plus an unused set. `trackerLines()` gives:
  * nothing before acceptance, and only the active set's objectives in their order with their invented
    `ProgressMessage`;
  * the optional flag, and completion only when the count is reached;
  * after the required objective, a ready-to-turn-in status with the optional objective still open (the current rule
    above);
  * nothing from the unused set.
* In-engine, pc-hummer, 2026-11-03 (main 95e48fb plus this lane):
  * `run_hud_shots.ps1 -Down`: 10/10, twice. The first injury's total was 12.00 s; bled out 14.283 s after the injury
    (expected 14.28); respawn 5.760 s after that; the second wind left 100 of 400; 110 of 110 down states echoed by the page.
  * `-Tracker`: 4/4 (7 of 7 tracker states read back, 5 with a mission, the turn-in shown).
  * `run_hud_shots.ps1` (default): 16/16.
  * `-Level 36 -Shield`: 16/17. `shield_shown` fails only because this PC's local items have no shield Maya can equip at
    level 36 (local data).
  * `test_action_skill_hud.ps1` 19/19; `test_hud_minimap.ps1` 15/15.
  * `test_quest.ps1` first run 111/112. The new `player_death_goes_down_first` and the respawn checks after the bleed-out
    pass. The failure is `dialog_hook_finds_every_line_and_plays_nothing` (a missing dialog audio line, the Fixes lane's;
    #105 before this lane added a check). The resume run did not run, because the suite stops after a failed first run.
* After review round 1, rebased on main 490661e (PR #61), pc-hummer:
  * ctest 21/21; verify_packages 9/9 exact.
  * `test_quest.ps1` first run **112/112** and resume **11/11**, after regenerating the local audio manifest with PR #61's
    `tools/audio_slice_chain.py` (47 entries). With the old manifest the same build gave 111/112 (3 dialog misses).
  * `run_hud_shots.ps1 -Down` 10/10 with the new title and pixel checks (bleed-bar pixels: up 0, down_start 3,540,
    down_mid 2,281, the other three 0).
  * `-Tracker` 4/4.
* Rebased on main 542d0c6 (PR #62, momentum and sweep), pc-hummer:
  * ctest 20/20 (PR #62 retired one test); verify_packages 9/9 exact; UE module build OK.
  * Every shot, projectile, splash and status tick still reaches the dummy through `AOpenWillowWalker::ApplyShotDamage`
    with Maya as the causer, so `NotifyKilledEnemy` (second wind) sees all of them. Checked by reading, not by a run.
  * `test_quest.ps1` 112/112 and resume 11/11; `run_hud_shots.ps1 -Down` 10/10; `-Tracker` 4/4.
  * `test_weapon_firing.ps1` 14/14, after regenerating the local slice recipes' `firing` block with
    `tools/weapon_firing.py`. The first run with the older local recipes gave 11/14: the slice pistol, shotgun and SMG had
    no `firing` block, so the expected momentum fell back to the default (local data, not this change).
* Review round 3 (the weapon while down; the strings output guard), main 542d0c6, pc-hummer:
  * ctest 20/20; verify_packages 9/9; checker 26/26; `prepare_hud_strings_test` 4/4; UE module build OK.
  * `run_hud_shots.ps1 -Down` 11/11. Exactly three shots in the run (the burst's first, the press after the put-away, the
    shot after the respawn), and the magazine stays at 7 through the put-away.
    The first try failed `down_weapon` because of the test sequence, not the rule: a press and release in the same
    tick never starts a burst, and the reload begun while down had refilled the magazine before the second injury.
    The sequence now holds the trigger until the first shot and fires once after the respawn.
  * `test_quest.ps1` 112/112 + resume 11/11; `test_weapon_firing.ps1` 14/14.

* HUD lane 2 (after PR #63, on main 8b7b7f4): `tools/run_hud_shots.ps1 -Feedback` (`-owhudfxshots`): four 5-point hits whose sources sit
  ahead, right, behind and left of the view, then a level-up, with captures. Checked by `check_hud_down.py --mode feedback`:
  the sides u r d l in order, each arrow playing on its `Health` frame, the banner on `go` with a text, and 5 complete
  captures. `tests/check_hud_down_test.py` now has 30 cases (4 for this mode), and `tests/hud_fields_test.js` covers
  `feedbackCalls` / `feedbackLine`. In UE on pc-hummer: 6/6. In the captures each arrow is on its own screen edge, and
  the banner shows over LV 09.
  On main 8b7b7f4, after the rebase over PR #63 (the down sequence stays editor-only; the feedback sequence and
  `FeedbackHudField` stay outside the `WITH_EDITOR` block, since they inject no input):
  * ctest 21/21; verify_packages 9/9; checker 30/30; Node test green; UE build OK;
  * `-Feedback` 6/6, with the same captures (each arrow on its own edge, the banner over LV 09);
  * `-Down` 11/11, because `TakeDamage` now also counts hits.
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
