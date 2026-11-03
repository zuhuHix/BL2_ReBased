# Menus parity: coverage and the mission log (2026-11-03)

AI-assisted (Claude). Branch `feature/menus-integrity`. Covers the menus Maya uses in the Phase 4 slice (Sanctuary + Maya:
spawn, fight, loot a gun, equip, Phaselock, one mission, die, respawn). Raw script listings, AS2 listings and the
presentation data stay under ignored `local/menus/` and `local/ui/run/`; this page records rules in our own words, names of
game objects, constants and counts.

**Evidence levels.** "Read" = read from the installed packages (UnrealScript with `research/script_disasm.py`, the movie's AS2
with a local helper, data with `ow-package --object-dump`); "seen" = compared with the running real game. Nothing on this page
was seen in the real game in this round: bl-sdk is not installed in this machine's Borderlands 2 (no `sdk_mods`, no loader
DLL), so the real-game probe (`tools/real_game/scripts/menus_probe.py`) could not run. Everything below is `UNVERIFIED` against
the game unless a row says otherwise.

## Coverage

| Screen | In the slice? | Host page | Driven from | State | Checks |
|---|---|---|---|---|---|
| StatusMenu: Inventory | yes (loot, equip) | `inventory.html` | host inventory, VM navigation | round 21 of the earlier parity loop (critic 7.85, 14 matched pairs, 2026-10-22); open items below | `test_inventory_actions.ps1`, `inventory_navigation_test.js` |
| StatusMenu: Skills | yes (Phaselock, points) | `skills.html` | host skills | round 21; tree navigation read from script | `SkillsTest`, `skill_navigation_test.js` |
| StatusMenu: Missions (mission log) | yes (one mission) | `missions.html` (new) | `UOpenWillowQuest::MissionLogJson` (VM mission state) | new this round: list, summary, rewards, level panel, hints, header tabs | `mission_log_test.js` (synthetic), `test_mission_log.ps1` (in engine) |
| StatusMenu: Map | marginal (no fast travel in the slice) | none | - | not started: `StatusMenuMap` loads a per-level map movie through `extStatusMenuMapPostMapMovieLoad`; the path given to `setMapMovie` was not observed | - |
| StatusMenu: Badass Rank / Challenges | no | none (tab disabled) | - | not started | - |
| Inspect (inventory) | yes | inside `inventory.html` | host preview actor | stand-in backdrop; no 3D turn on the card/hint, no emblem | action suite (inspect steps) |
| Pause menu | yes (any session) | none | - | not started; the host has no pause screen | - |
| Mission accept / turn-in screens (QuestAccept movie) | yes | none | - | not started: the host accepts and turns in with the use key near Marcus (`UOpenWillowQuest::TryUse`), no briefing screen | quest suite (host flow only) |
| Respawn (death) screen | yes | gameplay HUD's | HUD worker | outside this lane | - |
| Vendor screen | no (no shop in the slice) | none | - | not started | - |

Header tabs: Missions, Inventory and Skills switch pages (`/__ow_tab_missions|inventory|skills`, intercepted by the UE
browser). Map and Badass Rank stay disabled.

## Mission log (Missions tab)

The page runs the installed StatusMenu movie in Ruffle on its `missionlog` frame and fills it the way
`StatusMenuExGFxMovie` does (read). Presentation data comes from the installed game through
`tools/prepare_mission_log.py` into ignored `local/ui/run/missionlog.json`:

- `UI_StatusMenu.StatusMenu_Definition` (Startup): the list row's icon frame and text colour per status. Values: not started
  `undiscovered` #97B8BD, active `inprogress` #FF8000, active plot-critical `inprogressMission` #FF8000, ready to turn in
  `turnin` #00FF00, complete `complete`, failed `failed` and ineligible `none` all #808080.
- `UI_StatusMenu.GFxTextList_Missions`: 17 rows in container `selections`, normal colour #3D82A4, highlight #FFFFFF, category
  rows wrapped in the definition's prefix/postfix (bold italic #51AAD6) with the `divider` icon.
- `GD_Globals.General.Globals`: LevelDifference thresholds impossible 5, hard 3, tough 1, normal -3.
- Strings from `WillowGame.int` / `WillowMenu.int` (header, level, XP, story/optional, dividers, hints); key tokens resolved to
  PC text from `DefaultGame.ini` (`[Enter]`, `[Escape]`, `[Q]`, `[Page Up] / [Page Down]`).

Rules read from script (implemented in `tools/hud_overlay/mission_log.js`, own words):

- **Screen setup** (`extSetupMissionLogScreen`): no missions -> the movie's `missionlog_none` frame. The script then writes
  `missions.title_questname.text`, a field this movie does not contain, so the frame's own "No Current Missions" stays (the
  host does the same). Otherwise the list header is the upper-cased "My Missions" plus `<font size="22">(All)</font>` for a
  single player (primary host, default sort).
- **List rows** (`UpdateListOfMissionTitles`): statuses 1 and 2 use the active presentation, the plot one when
  `bPlotCritical`; the row is added to the native `GFxTextListContainer`, which calls the movie's
  `SetTextEntry(container, index, text, type)`. The container (row order, category rows, row markup, scrolling) and
  `UpdateMissionTextList` are native and were not read: the host groups TURN IN, IN PROGRESS, COMPLETED MISSIONS, FAILED, wraps
  the name in the status colour (white when highlighted) and skips category rows when moving. **UNVERIFIED.**
- **Summary** (`UpdateMissionDetails`): header frame `new` (0), `inprogress` / `inprogressMission` (1, 2), `turnin` (3),
  `complete` (4), `none` (5); mission name in `header.description`; `OptionalRequired` frame and text from `bPlotCritical`;
  credits through `SetRewardsTotalCredits` (0 hides the panel; the Fire mission's credit multiplier is 0); XP through
  `SetRewardsXP("%d XP")`; level text "LEVEL: %d" and the difficulty frame from mission level minus player level against the
  thresholds above (`>=`, in order, else `trivial`); the item reward clip hidden when there are no reward presentations (the Fire
  mission has none).
- **Description** (`WillowPlayerController.GetMissionDescriptionForUI`, UnrealScript): summary, blank line; unless failed,
  complete or ready to turn in, one line per current objective (four spaces, ProgressMessage, then ": p/n" when the count is
  above 1 - with spaces around the slash for Japanese, Korean and Chinese - ": Complete" for a done single-count objective);
  optional objectives in #B0F5FF; "    TURN IN!" when ready to turn in; blank line and the description in #8B8B8B; for
  complete and ready-to-turn-in missions a blank line and "Turn In: <location>". Which strings the native
  `MissionTracker.GetMissionDescription` returns as summary and description was not read: the host sends MissionSummary and
  MissionDescription (TurnInDescription when ready to turn in). **UNVERIFIED.**
- **Level panel** (`UpdateGeneralDetails`): "Level N"; XP text "xp / next" below the maximum level; bar frame
  `(xp - start) * 100 / (next - start) + 1` in integer arithmetic, 100 at the maximum level. The movie's
  `SetPlayerTotalCredits` writes `mission.credits` (not `missions.`), so no credits show on this frame; the host calls it the
  same way.
- **Hints** (`SetTooltipsText`): each hint is prepended, five spaces apart: [Scroll] Track Sort Close. Track while the
  highlighted mission can be tracked (`CanTrackHighlightedMission` not read; host: active or ready, not already tracked);
  Scroll while the description scrolls. Ignore/Unignore (`CanToggleHighlightedMissionFilter`) is left out.
- **Input**: the movie's own list events (`extLogListRollover`, `extLogListTrack`, `extLogUp`, `extLogDown`) select and
  track; keys Up/Down, Enter (track: `OWMISSION {"track":id}`, nothing to change with one mission), Page Up/Down scroll the
  description, Escape / Tab / L close, I and K switch tabs. The PC key for the log itself and the Sort action were not observed.
- The movie turns the missions clip 5 degrees about Y at 92 % scale (root frame script); the page applies the turn with
  `menu3d.js`, as the other two pages do.

### Checks

- `node tests/mission_log_test.js`: synthetic rules (statuses, header frames, difficulty, XP bar, rows, selection window,
  description, hints, routes).
- `tools/test_mission_log.ps1` (in engine, `-owquest`): open the page, nothing listed before Accept, the mission under IN
  PROGRESS with the `inprogress` header after Accept, Down stays on the only mission, header tabs to Inventory and back, Escape
  closes. 2026-11-03 on pc-hummer: **7 PASS / 0 FAIL** (host behaviour only). The first run with the reset fix missing showed
  the no-missions frame after Accept (see "Movie quirks" in DECISIONS 2026-11-03); the check now also requires frame 1.

### Captures (host only, `local/menus/missionlog/`)

`OWMission_1_NotStarted.png` (the movie's "No Current Missions" panel) and `OWMission_2_Active.png` (the slice mission under
IN PROGRESS: name, summary "Test Marcus's fire weaponry.", objective "Go to firing range", the grey description, LEVEL: 8,
DIFFICULTY NORMAL, OPTIONAL MISSION, 395 XP, the level bar). The list row's embedded font subset dropped `, : !` from the
mission name in the first capture; rows now use the movie's `$WillowBody` face like the other fields. No real-game frame
exists to score against, so no critic round was run for this page.

Known artefact: in a headless replay, a column of list scroll ticks shows after a no-missions -> missions transition (not
seen in the engine capture).

## Open

1. Real-game frames and live strings: the htmlText of the description per state, the list order and category rows, the
   PC keys of the mission log, the Map movie path. Needs bl-sdk on this machine (a maintainer decision) or the other PC.
2. Map tab; accept / turn-in briefing screens; pause menu.
3. Inventory round-21 open list (see `INVENTORY_MOVIE_PROTOTYPE.md`): VALUE thumbnails, BRANDS order, card frame, Inspect turn
   and emblem, zoom lines and flavour wrap, number keys.
