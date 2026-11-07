# Menus parity: coverage and the mission log (2026-11-03)

AI-assisted (Claude). Branch `feature/menus-integrity`. Covers the menus Maya uses in the Phase 4 slice (Sanctuary + Maya:
spawn, fight, loot a gun, equip, Phaselock, one mission, die, respawn). Raw script listings, AS2 listings and every extracted
value (colours, thresholds, markup constants, localized and mission text) stay under ignored `local/menus/` and
`local/ui/run/missionlog.json`; this page records rules in our own words and names of game objects and movie clips only.

**Evidence levels.** "Read" = read from the installed packages (UnrealScript with `research/script_disasm.py`, the movie's AS2
with a local helper, data with `ow-package --object-dump`); "seen" = compared with the running real game. Nothing on this page
was seen in the real game in this round: bl-sdk is not installed in this machine's Borderlands 2 (no `sdk_mods`, no loader
DLL), so the real-game probe (`tools/real_game/scripts/menus_probe.py`) could not run. Everything below is `UNVERIFIED` against
the game unless a row says otherwise.

## Coverage

| Screen | In the slice? | Host page | Driven from | State | Checks |
|---|---|---|---|---|---|
| StatusMenu: Inventory | yes (loot, equip) | `inventory.html` | host inventory, VM navigation | round 23 (2026-11-03): critic 7.99 against 7.84 for round 21 in one blind A/B round (14 matched pairs); open items below | `test_inventory_actions.ps1`, `inventory_navigation_test.js` |
| StatusMenu: Skills | yes (Phaselock, points) | `skills.html` | host skills | round 21; tree navigation read from script | `SkillsTest`, `skill_navigation_test.js` |
| StatusMenu: Missions (mission log) | yes (one mission) | `missions.html` (new) | `UOpenWillowQuest::MissionLogJson` (VM mission state) | new this round: list, summary, rewards, level panel, hints, header tabs | `mission_log_test.js` (synthetic), `test_mission_log.ps1` (in engine) |
| StatusMenu: Map | marginal (no fast travel in the slice) | none | - | not started: `StatusMenuMap` loads a per-level map movie through `extStatusMenuMapPostMapMovieLoad`; the path given to `setMapMovie` was not observed | - |
| StatusMenu: Badass Rank / Challenges | no | none (tab disabled) | - | not started | - |
| Inspect (inventory) | yes | inside `inventory.html` | host preview actor | stand-in backdrop with a ring emblem; menu turn on the card and hint (round 23); weapon paint white | action suite (inspect steps) |
| Pause menu | yes (any session) | none | - | not started; the host has no pause screen | - |
| Mission accept / turn-in screens (QuestAccept movie) | yes | none | - | not started: the host accepts and turns in with the use key near the giver (`UOpenWillowQuest::TryUse`), no briefing screen | quest suite (host flow only) |
| Respawn (death) screen | yes | gameplay HUD's | HUD worker | outside this lane | - |
| Vendor screen | no (no shop in the slice) | none | - | not started | - |

Header tabs: Missions, Inventory and Skills switch pages (`/__ow_tab_missions|inventory|skills`, intercepted by the UE
browser). Map and Badass Rank stay disabled.

## Mission log (Missions tab)

The page runs the installed StatusMenu movie in Ruffle on its `missionlog` frame and fills it the way
`StatusMenuExGFxMovie` does (read). Presentation data comes from the installed game through
`tools/prepare_mission_log.py` into ignored `local/ui/run/missionlog.json`:

- `UI_StatusMenu.StatusMenu_Definition` (Startup): the per-status presentation table (the list row's icon frame label and text
  colour for not started, active, active plot-critical, ready to turn in, complete, failed and ineligible).
- `UI_StatusMenu.GFxTextList_Missions`: the list container (its clip name and row count, the normal and highlighted text
  colours, the category-row prefix/postfix markup and icon label, the scroll-arrow clip names).
- `GD_Globals.General.Globals`: the four `LevelDifference_*` thresholds that choose the difficulty bar frame.
- The markup constants the script functions build their text from (the fonts around an optional objective and the
  description, the markup around the header's sort name), read from the installed bytecode.
- Strings from `WillowGame.int` / `WillowMenu.int` (header, level, XP, story/optional labels, dividers, hints, the description
  pieces); key tokens resolved to PC key names from `DefaultGame.ini` `MenuInputMapArray`.

Rules read from script (implemented in `tools/hud_overlay/mission_log.js`, own words):

- **Screen setup** (`extSetupMissionLogScreen`): no missions -> the missions clip's `missionlog_none` frame. The script then
  writes `missions.title_questname.text`, a field this movie does not contain, so the frame's own static text stays (the host
  does the same). Otherwise the list header is the upper-cased "my missions" string, a space, and the sort name inside the
  script's markup constants, for a single player (primary host, default sort).
- **List rows** (`UpdateListOfMissionTitles`): the active and required-objectives-done statuses use the active presentation, the plot one when
  `bPlotCritical`; the row is added to the native `GFxTextListContainer`, which calls the movie's
  `SetTextEntry(container, index, text, type)`. The container (row order, category rows, row markup, scrolling) and
  `UpdateMissionTextList` are native and were not read: the host groups turn-in, in-progress, completed and failed missions
  under the localized divider strings, wraps the name in the status colour (the highlight colour when selected) and skips
  category rows when moving. **UNVERIFIED.**
- **Summary** (`UpdateMissionDetails`): header frame `new` (not started), `inprogress` / `inprogressMission` (active or
  required objectives done), `turnin` (ready to turn in), `complete`, `none` (failed); mission name in `header.description`; `OptionalRequired` frame and label from
  `bPlotCritical`; credits through `SetRewardsTotalCredits` (an amount of 0 hides the panel); XP
  through `SetRewardsXP` with the localized XP pattern; the localized level pattern and the difficulty frame from mission level
  minus player level against the thresholds (`>=`, in order impossible, hard, tough, normal, else `trivial`); the item reward
  clip hidden when there are no reward presentations. The host evaluates neither the credit reward
  (`MissionDefinition.GetCurrencyReward`) nor the reward presentations, so it sends no credit or item reward and the page
  hides both panels while they are unknown (**UNVERIFIED** for any mission).
- **Description** (`WillowPlayerController.GetMissionDescriptionForUI`, UnrealScript): the summary and a blank line; unless
  failed, complete or ready to turn in, one line per current objective (an indent, its ProgressMessage, then the localized colon
  and "p/n" when the count is above 1 - spaces around the slash for Japanese, Korean and Chinese - or the localized "complete"
  word for a done single-count objective); optional objectives inside the script's optional-objective font; the localized
  turn-in divider when ready to turn in; a blank line and the description inside the script's description font; for complete
  and ready-to-turn-in missions a blank line and the localized turn-in label with the turn-in location. The page escapes every
  piece of game and localized text and adds only those markup constants. Which strings the native
  `MissionTracker.GetMissionDescription` returns as summary and description was not read: the host sends MissionSummary and
  MissionDescription (TurnInDescription when ready to turn in). **UNVERIFIED.**
- **Level panel** (`UpdateGeneralDetails`): the localized level string with the level; XP text "xp / next" below the maximum
  level; bar frame `(xp - start) * 100 / (next - start) + 1` in integer arithmetic, 100 at the maximum level. The movie's
  `SetPlayerTotalCredits` writes `mission.credits` (not `missions.`), so no credits show on this frame; the host calls it the
  same way.
- **Hints** (`SetTooltipsText`): each hint is prepended, five spaces apart, so the bar reads scroll, track, sort, close. Track
  while the highlighted mission can be tracked (`CanTrackHighlightedMission` not read; host: active or ready, not already
  tracked); scroll while the description scrolls. Ignore/Unignore (`CanToggleHighlightedMissionFilter`) is left out.
- **Input**: the movie's own list events (`extLogListRollover`, `extLogListTrack`, `extLogUp`, `extLogDown`) select and
  track; keys Up/Down, Enter (track: `OWMISSION {"track":id}`, nothing to change with one mission), Page Up/Down scroll the
  description, Escape / Tab close, L stays on the host (seen once live, with Better UI Controls loaded; stock behaviour **UNVERIFIED**), I and K switch tabs. The PC key for the log itself and the Sort action were not observed.
- The root frame script turns and scales the missions clip; the page applies the turn with `menu3d.js`, as the other two pages
  do.

### Checks

- `node tests/mission_log_test.js`: synthetic rules (statuses, header frames, difficulty, XP bar, rows, selection window,
  description, escaping, hints, routes), invented definition values.
- `tools/test_mission_log.ps1` (in engine, `-owquest`): open the page, nothing listed before Accept, the mission under the
  in-progress category with the `inprogress` header after Accept, Down stays on the only mission, header tabs to Inventory and
  back, Escape closes. 2026-11-03 on pc-hummer: **7 PASS / 0 FAIL** (host behaviour only). The first run with the reset fix
  missing showed the no-missions frame after Accept (see "Movie quirks" in DECISIONS 2026-11-03); the check now also requires
  frame 1.

### Captures (host only, `local/menus/missionlog/`)

`OWMission_1_NotStarted.png` (the movie's no-missions panel) and `OWMission_2_Active.png` (the slice mission in the list and
the summary: name, summary, its one objective, the description, level and difficulty, the optional-mission label, the XP
reward, the level bar). The list row's embedded font subset dropped some punctuation from the mission name in the first
capture; rows now use the movie's `$WillowBody` face like the other fields. No real-game frame exists to score against, so no
critic round was run for this page.

**All four Fire mission states (2026-11-04, inventory lane, pc-hummer).** The slice quest resumed from local saves in each state
(`-owquestresume`; saves under ignored `local/inv24/`), the page opened with `-owstatusshots=missions,<state>` and its own
`OWMISSIONPAGE` report read (captures under ignored `local/inv24/missionshots/`). Host only, no real frame:

| State (saved status) | Category row | Header | Summary panel |
|---|---|---|---|
| Accepted (Active, GoToRange set) | IN PROGRESS | `inprogress` | the go-to-the-range objective |
| At the range (Active, final set) | IN PROGRESS | `inprogress` | "Burn vandal" (the active set's objective) |
| Objective done (ReadyToTurnIn) | TURN IN | `turnin` (green "?") | "TURN IN!", the turn-in text, "Turn In: Marcus" |
| Turned in (Complete) | COMPLETED MISSIONS | `complete` (grey tick) | the description and "Turn In: Marcus" |

The Fire mission is selected in each. No page change was needed. UNVERIFIED against the game: whether the log drops the
finished go-to-the-range line at the range, and whether a completed mission still shows "Turn In: Marcus".

**Quest text over the status pages fixed.** Under `-owquest` the quest's tracker and talk hints (`UOpenWillowQuest::
UpdateHints`, an on-screen debug line posted every frame) drew over every status page. They are no longer posted while a
status page or a vending shop is open (the shop since PR #101); the four captures above show none.

Known artefact: in a headless replay, a column of list scroll ticks shows after a no-missions -> missions transition (not
seen in the engine capture).

## Open

1. Real-game frames and live strings: the htmlText of the description per state, the list order and category rows, the
   PC keys of the mission log, the Map movie path. Needs bl-sdk on this machine (a maintainer decision) or the other PC.
2. Accept / turn-in briefing screens; pause menu. The Map tab has a first page since 2026-11-04 (`MAP_SCREEN.md`;
   centring, arrow, pan/zoom steps and the custom waypoint UNVERIFIED).
3. Inventory round-23 open list (see `INVENTORY_MOVIE_PROTOTYPE.md`, "Round 23"): Inspect weapon paint and emblem, glass-sheet
   curve, thumbnail renders, D3 card border, zoom lines (`weapon_gen.cpp`) and one flavour wrap, BRANDS order (native, UNVERIFIED),
   number keys (round 24: the script switches tabs 1-5, the page follows it; not yet seen in game).
