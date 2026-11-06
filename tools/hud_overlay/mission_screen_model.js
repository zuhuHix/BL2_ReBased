// The rules of Marcus's mission screen page, with no DOM or movie, so Node can test them (tests/mission_screen_test.js). The host owns the keys
// (OpenWillowMissionPage) and the page only draws, so what a key does is in OpenWillowQuest; this file says what is written to the movie and which
// prompt each page shows. Paths and movie functions are the ones the installed movie has, recorded in docs/verification/MISSION_SCREEN.md.
(function (root) {
  'use strict';
  const SETTLED_POLLS = 3;   // polls of the same frame (50 ms apart) before the movie counts as settled
  // Neutral prompts for a run without missionscreen.json (tools/prepare_mission_screen.py). Our own words; the keys are the host's.
  const FALLBACK = {accept: 'Enter: Accept', decline: 'Escape: Decline', scroll: 'Page Up / Page Down: Scroll', acceptTooltips: '%s1     %s2     %s3',
    complete: 'Enter: Turn in     Escape: Leave     Page Up / Page Down: Scroll', acceptReward: 'Enter: Accept',
    xpRewards: '%d XP', missionLevel: 'Level %d', storyMission: 'Story mission', optionalMission: 'Optional mission',
    missionComplete: 'Mission complete', rewardsTitle: 'Rewards'};

  function escapeHtml(text) {
    return String(text ?? '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // The prompt a page shows: it says what the keys do on that page. Script read, UNVERIFIED in game until the live read.
  function prompt(page, strings) {
    if (page === 'offer') return strings.acceptTooltips.replace('%s1', strings.accept).replace('%s2', strings.decline).replace('%s3', strings.scroll);
    if (page === 'turnin') return strings.complete;
    if (page === 'reward') return strings.acceptReward;
    return '';
  }

  // The one mission the slice's screen shows: the host's log snapshot has it under any status; the page never guesses another.
  function missionOf(state) {
    return state?.log?.missions?.[0] ?? null;
  }

  // Which frame of the movie's header says what the row is: the movie's labels for an offer and a turn-in.
  const HEADER_FRAME = {offer: 'new', turnin: 'turnin'};

  // What goes in the description field. A turn-in shows the mission's turn-in text when it has one.
  function descriptionOf(page, mission) {
    return escapeHtml(page === 'turnin' && mission.turnInDescription ? mission.turnInDescription : mission.description);
  }

  // Writes for the offer and turn-in pages, once the `missions` clip exists: [op, path, name, value]. Credits are shown only when the host knows them
  // (its log leaves the field out otherwise, as the mission log page does): UNVERIFIED for this mission beyond the golden trace's 0.
  function listPlan(page, mission, strings) {
    const M = '_level1.missions';
    const plan = [
      ['goto', `${M}.header`, 'gotoAndStop', HEADER_FRAME[page]],
      ['set', `${M}.header.description`, 'text', mission.name],
      ['goto', `${M}.OptionalRequired`, 'gotoAndStop', mission.plot ? 'required' : 'optional'],
      ['html', `${M}.OptionalRequired.textField`, 'htmlText', escapeHtml(mission.plot ? strings.storyMission : strings.optionalMission)],
      ['html', `${M}.description`, 'htmlText', descriptionOf(page, mission)],
      ['html', `${M}.tooltips`, 'htmlText', escapeHtml(prompt(page, strings))],
      ['call', '_level1', 'SetRewardsXP', [strings.xpRewards.replace('%d', String(mission.xp ?? 0))]],
      ['call', '_level1', 'SetRewardsTotalCredits', [Number.isFinite(mission.credits) ? mission.credits : 0]],
      // The slice's mission has one row, so the movie's row list is hidden. The real screen also moves the panel for one mission; the page does not yet (UNVERIFIED look).
      ['set', `${M}.selections`, '_visible', false],
      ['set', `${M}.reward_weap`, '_visible', false],
      ['set', `${M}.reward_other`, '_visible', false],
    ];
    return plan;
  }

  // Writes for the reward page, once the `reward` clip exists. Fields that are for items and credits this mission lacks are hidden.
  function rewardPlan(mission, strings) {
    const R = '_level1.reward';
    const credits = Number.isFinite(mission.credits) ? mission.credits : 0;
    return [
      ['call', R, 'SetNumItemChoices', [0]],
      ['set', `${R}.rewardsHeader`, 'text', strings.missionComplete],
      ['set', `${R}.missionTitle`, 'text', mission.name],
      ['set', `${R}.description`, 'text', mission.turnInDescription || mission.description || ''],
      ['set', `${R}.rewards`, 'text', strings.rewardsTitle],
      ['set', `${R}.xpReward`, 'text', strings.xpRewards.replace('%d', String(mission.xp ?? 0))],
      ['goto', `${R}.OptionalRequired`, 'gotoAndStop', mission.plot ? 'required' : 'optional'],
      ['html', `${R}.OptionalRequired.textField`, 'htmlText', escapeHtml(mission.plot ? strings.storyMission : strings.optionalMission)],
      ['set', `${R}.credits`, '_visible', credits > 0],
      ['set', `${R}.cashHeader`, '_visible', credits > 0],
      ['set', `${R}.reward_other`, '_visible', false],
      ['set', `${R}.norewardtext`, '_visible', false],
      ['html', `${R}.tooltips`, 'htmlText', escapeHtml(prompt('reward', strings))],
    ];
  }

  // The text of the plain panel a run without the movie shows: the player still sees what is offered and what the keys do.
  function panelText(page, mission, strings) {
    const lines = [page === 'reward' ? strings.missionComplete : page === 'turnin' ? strings.complete.split('  ')[0] : strings.accept, mission.name];
    lines.push(page === 'turnin' || page === 'reward' ? mission.turnInDescription || mission.description || '' : mission.description || '');
    lines.push(`${strings.rewardsTitle}: ${strings.xpRewards.replace('%d', String(mission.xp ?? 0))}`);
    lines.push(prompt(page, strings));
    return lines;
  }

  root.owMissionScreen = {SETTLED_POLLS, FALLBACK, escapeHtml, prompt, missionOf, descriptionOf, listPlan, rewardPlan, panelText};
})(typeof window !== 'undefined' ? window : globalThis);
if (typeof module !== 'undefined') module.exports = globalThis.owMissionScreen;
