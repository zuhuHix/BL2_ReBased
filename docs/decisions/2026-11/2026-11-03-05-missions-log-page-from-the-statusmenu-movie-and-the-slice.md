## 2026-11-03: Missions log page from the StatusMenu movie and the slice mission (menus lane)

AI-assisted (Claude). Record: `docs/verification/MENUS_PARITY.md` (new; coverage table of the slice's menus and the mission
log's rules). Script and AS2 listings and the presentation data stay under ignored `local/menus/` and `local/ui/run/`.

- **What.** A third status page, `tools/hud_overlay/missions.html` (`missions.js`, pure rules in `mission_log.js`), runs the
  installed StatusMenu movie's mission log and fills it the way `StatusMenuExGFxMovie` does: list rows through the movie's
  `SetTextEntry` / `MoveHighlightBar` / `SetMoreBars` / `setActiveMission`, the summary through the paths
  `UpdateMissionDetails` writes, the level panel through `SetPlayerLevel` / `SetPlayerXP`. Presentation values come from the
  installed game (`tools/prepare_mission_log.py`: `StatusMenu_Definition` icon frames and colours, `GFxTextList_Missions`,
  `GlobalsDefinition` difficulty thresholds, localized strings and PC key names) into ignored `local/ui/run/missionlog.json`.
  The description follows `WillowPlayerController.GetMissionDescriptionForUI`, which is UnrealScript (read, own words in the
  record). Host: `EOpenWillowStatusPage`, a preloaded Missions page, `/__ow_tab_missions` and `/__ow_close_missions` routes,
  the Missions header tab on the Inventory and Skills pages, and the summary / turn-in description fields in
  `UOpenWillowQuest::MissionLogJson` (`MissionSystem::summary()` / `turnInDescription()` added; no parsing change).
- **Read, not seen.** No rule here was compared with the running game: bl-sdk is not installed on this machine. Native parts
  stay host choices and UNVERIFIED: row order and category rows (`GFxTextListContainer`, `UpdateMissionTextList`), the row
  markup, which strings `MissionTracker.GetMissionDescription` returns, `CanTrackHighlightedMission`.
- **Movie quirks kept or worked around.** The script's no-missions text goes to `missions.title_questname`, which this movie
  does not have, so the frame's own static text shows (kept). `gotoRegularMissions` targets a label the missions clip
  does not have, so after the no-missions frame the page returns the clip to frame 1 itself (found by the in-engine capture;
  the game rebuilds the screen on each open). In a headless replay a column of list scroll ticks still shows after that
  transition (open; not seen in the engine capture). Several of the movie's text fields embed font subsets without `/ : - , !`;
  the page re-sets them in the movie's own `$WillowBody` face, as `skills.js` does.
- **Checks.** `node tests/mission_log_test.js` 9/9 (synthetic); `tools/test_mission_log.ps1` (new, in engine with
  `-owquest`) 7/7: open, nothing listed before Accept, the mission under the in-progress category with the `inprogress` header on frame 1
  after Accept, Down, tabs to Inventory and back, Escape. Inventory suite 49/0/0, navigation 40/40, `OpenWillow.Skills`
  passes. Other numbers are in the PR.
- **Review rounds.** Round 1: no game values in tracked files (the script's markup constants are read into the ignored
  JSON), escaped mission text, path guards in `menus_probe.py` and `serve.py`, the Missions browser removed in `EndPlay`.
  Round 2: `prepare_mission_log.py` refuses an output outside the repository's `local/`; the host sends no credit or item
  reward (neither is evaluated; the page hides both panels, UNVERIFIED) instead of a hard-coded 0; the page uses the named
  `status` field instead of numeric status IDs, and the no-host sample and fixtures use invented values.
- **One objective view.** After the HUD lane's tracker landed (PR #63), the mission log reads the active objectives from the
  same `MissionSystem::trackerLines()` as the HUD tracker; the separate `activeObjectives()` is gone. `TrackerLine` gained the
  objective's `count` (ObjectiveCount), which the log prints progress against; the tracker does not use it. The launcher
  `tools/test_mission_log.ps1` takes the machine-wide UE run lock (`tools/ue_lock.ps1`, PR #66).
- **Inventory round-21 list.** VALUE thumbnails do not reproduce on round-21 code (they were a round-22 regression); the
  BRANDS order is the engine's unstable sort, not a missing tie-break. Details in `INVENTORY_MOVIE_PROTOTYPE.md`.

