// Pure mission-log presentation rules, shared by missions.js and the Node tests. Each rule names where it was read;
// none of it has been compared with the running game yet (UNVERIFIED until a real-game session confirms it).
// `def` is local/ui/run/missionlog.json (tools/prepare_mission_log.py): the installed StatusMenu_Definition,
// GFxTextList_Missions, GlobalsDefinition thresholds and localized strings.
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.owMissionLog = api;
})(globalThis, function () {
  // Mission statuses by the names the host sends in each mission's "status" field (UOpenWillowQuest::MissionLogJson):
  // 'requireddone' (required objectives done, optional ones left) and 'failed' are listed for completeness; the host's slice
  // mission does not produce them.
  const NOT_STARTED = 'notstarted', ACTIVE = 'active', REQUIRED_DONE = 'requireddone', TURN_IN = 'turnin',
    COMPLETE = 'complete', FAILED = 'failed';

  // The list row's presentation (StatusMenuExGFxMovie.UpdateListOfMissionTitles): 1 and 2 share the active presentation,
  // the plot-critical one when MissionDefinition.bPlotCritical is set.
  function presentationKey(status, plot) {
    switch (status) {
      case ACTIVE: case REQUIRED_DONE: return plot ? 'activeplot' : 'active';
      case TURN_IN: return 'turnin';
      case NOT_STARTED: return 'notstarted';
      case COMPLETE: return 'complete';
      case FAILED: return 'failed';
      default: return 'ineligible';
    }
  }

  // The summary header's frame (UpdateMissionDetails: missions.missionSummary.header.gotoAndStop).
  function headerFrame(status, plot) {
    switch (status) {
      case ACTIVE: case REQUIRED_DONE: return plot ? 'inprogressMission' : 'inprogress';
      case NOT_STARTED: return 'new';
      case TURN_IN: return 'turnin';
      case COMPLETE: return 'complete';
      default: return 'none';
    }
  }

  // UpdateMissionDetails: level difference = mission game stage - player level, compared (>=) with the
  // GlobalsDefinition LevelDifference_Impossible / Hard / Tough / Normal thresholds in that order; below Normal, 'trivial'.
  function difficultyFrame(missionLevel, playerLevel, thresholds) {
    const difference = missionLevel - playerLevel;
    for (const name of ['impossible', 'hard', 'tough', 'normal'])
      if (difference >= thresholds[name]) return name;
    return 'trivial';
  }

  // UpdateGeneralDetails: the XP text is "<xp>" and, below the maximum level, " / <xp needed for the next level>"; the bar frame
  // is (xp - start) * 100 / (next - start) + 1 in integer arithmetic (100 at the maximum level). `required(level)` is the XP
  // needed to reach `level` (GetExpPointsRequiredForLevel). A level behind the XP is advanced first, as the script does.
  function xpBar(level, xp, maxLevel, required) {
    if (level >= maxLevel) return {level, text: String(xp), percent: 100};
    let start = required(level), next = required(level + 1);
    while (xp >= next && level < maxLevel) { ++level; start = required(level); next = required(level + 1); }
    return {level, text: `${xp} / ${next}`, percent: Math.trunc((xp - start) * 100 / (next - start)) + 1};
  }

  // The list header (extSetupMissionLogScreen): Caps(MissionLogMyMissions), a space, then the sort name inside the script's own
  // markup constants (def.markup.headerSortOpen / headerSortClose, read from the installed bytecode by prepare_mission_log.py).
  // A single player is the primary host, so the header is the player's own missions one; the sort is the default (all).
  function listHeader(def) {
    return `${escapeHtml(def.strings.myMissions.toUpperCase())} ${def.markup.headerSortOpen}${escapeHtml(def.strings.sortAll)}${
      def.markup.headerSortClose}`;
  }

  const escapeHtml = text => String(text).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

  // List rows. The order, the category rows and the row markup come from the native GFxTextListContainer and
  // UpdateMissionTextList, which were not read: UNVERIFIED host choices. Categories use the list definition's
  // CategoryPrefix / CategoryPostfix around the missions panel's Divider_* strings and its CategoryIconLabel; a mission row is
  // its MissionName in the status presentation's TextColor (the highlighted row in TextHighlightColor) with the presentation's
  // IconFrame as the row type. Not-started missions are not listed (the host lists only missions the player has).
  const CATEGORIES = [
    {key: 'turnin', label: 'dividerTurnIn', statuses: [TURN_IN]},
    {key: 'inprogress', label: 'dividerInProgress', statuses: [ACTIVE, REQUIRED_DONE]},
    {key: 'completed', label: 'dividerCompleted', statuses: [COMPLETE]},
    {key: 'failed', label: 'dividerFailed', statuses: [FAILED]},
  ];
  function listRows(missions, def) {
    const rows = [];
    for (const category of CATEGORIES) {
      const members = missions.filter(mission => category.statuses.includes(mission.status));
      if (!members.length) continue;
      rows.push({kind: 'category', key: category.key,
        text: def.list.categoryPrefix + escapeHtml(def.strings[category.label]) + def.list.categoryPostfix, type: def.list.categoryIcon});
      for (const mission of members) {
        const presentation = def.presentation[presentationKey(mission.status, mission.plot)];
        rows.push({kind: 'mission', id: mission.id, name: mission.name, color: presentation.color, type: presentation.frame});
      }
    }
    return rows;
  }
  function rowText(row, highlighted, def) {
    if (row.kind !== 'mission') return row.text;
    return `<font color='${highlighted ? def.list.highlightColor : row.color}'>${escapeHtml(row.name)}</font>`;
  }

  // The window of `entries` rows shown, keeping `selected` visible (GFxTextListContainer's IndexOfTopEntry; native, UNVERIFIED).
  function windowTop(rowCount, selected, previousTop, entries) {
    let top = Math.max(0, Math.min(previousTop, Math.max(0, rowCount - entries)));
    if (selected < top) top = selected;
    else if (selected >= top + entries) top = selected - entries + 1;
    return top;
  }
  // The next selectable row from `from` in direction `step` (+1 / -1); category rows are skipped; stays put at an end.
  function moveSelection(rows, from, step) {
    for (let index = from + step; index >= 0 && index < rows.length; index += step)
      if (rows[index].kind === 'mission') return index;
    return from;
  }
  const firstMission = rows => rows.findIndex(row => row.kind === 'mission');

  // The summary's description (missions.missionSummary.description.htmlText): WillowPlayerController.GetMissionDescriptionForUI,
  // an UnrealScript function, called by UpdateMissionDetails with the turn-in description wanted and no teaser for failed
  // missions. In order:
  //  - the mission's summary and a blank line;
  //  - unless the mission is failed, complete or ready to turn in, one line per current objective: four spaces, its
  //    ProgressMessage, then the localized colon and "p/n" when ObjectiveCount > 1 (spaces around the slash for Japanese,
  //    Korean and Chinese), the localized "complete" word when a single-count objective is done, else nothing; an optional
  //    objective's line is inside the script's optional-objective font (def.markup.optionalObjectiveOpen);
  //  - when the mission has objective sets and is ready to turn in, the localized turn-in divider and "!";
  //  - a blank line and the description inside the script's description font (def.markup.descriptionOpen);
  //  - when complete or ready to turn in, a blank line and the localized turn-in label with the turn-in location.
  // Every piece of game or localized text is escaped (the page adds only the script's markup), so text containing '<' or '&'
  // shows as written; whether the game would render markup inside these strings is not known.
  // Which text MissionTracker.GetMissionDescription (native) returns as summary and description is not read: the host sends
  // MissionSummary and MissionDescription, and TurnInDescription when ready to turn in (UNVERIFIED). Branch ("OR") objectives
  // are not modelled by the host. A not-started mission shows its teaser text instead (the host lists none).
  function descriptionHtml(mission, def, {wideCounts = false} = {}) {
    const s = def.strings, m = def.markup, status = mission.status, NL = '\n', e = escapeHtml;
    if (status === NOT_STARTED) return e(mission.teaser || '');
    let text = e(mission.summary || '') + NL + NL;
    if (![FAILED, COMPLETE, TURN_IN].includes(status)) {
      for (const objective of mission.objectives || []) {
        if (objective.optional) text += m.optionalObjectiveOpen;
        text += '    ' + e(objective.text || '');
        const count = objective.count || 1, progress = Math.min(objective.progress || 0, count);
        if (count > 1) text += e(s.colonAndSpace) + progress + (wideCounts ? ' / ' : '/') + count + NL;
        else if (progress >= 1) text += e(s.colonAndSpace + s.objectiveComplete) + NL;
        else text += ' ' + NL;
        if (objective.optional) text += '</font>';
      }
    }
    if (mission.hasSets !== false && status === TURN_IN) text += '    ' + e(s.turnInHud) + '!' + NL;
    const description = status === TURN_IN && mission.turnInDescription ? mission.turnInDescription : mission.description || '';
    text += NL + m.descriptionOpen + e(description) + '</font>';
    if (status === COMPLETE || status === TURN_IN) text += NL + NL + e(s.briefingTurnIn + ': ' + (mission.turnIn || ''));
    return text;
  }

  // The footer hint line (SetTooltipsText on the mission log): each hint is prepended, so the bar reads
  // [Scroll] Track Sort Close, five spaces apart. Track only while the highlighted mission can be tracked (active / turn-in, not
  // already tracked; CanTrackHighlightedMission is not read, UNVERIFIED), Scroll only while the description scrolls. The
  // Ignore/Unignore hint (CanToggleHighlightedMissionFilter) is left out: not read.
  function tooltipBar(def, {canTrack, canScroll}) {
    let bar = def.strings.tooltipCancel;
    bar = `${def.strings.tooltipSort}     ${bar}`;
    if (canTrack) bar = `${def.strings.tooltipTrack}     ${bar}`;
    if (canScroll) bar = `${def.strings.tooltipScroll}     ${bar}`;
    return bar;
  }

  return {ACTIVE, REQUIRED_DONE, TURN_IN, COMPLETE, FAILED, NOT_STARTED, presentationKey, headerFrame, difficultyFrame, xpBar,
    listHeader, listRows, rowText, windowTop, moveSelection, firstMission, descriptionHtml, tooltipBar, escapeHtml};
});
