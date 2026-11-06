// Synthetic tests for the mission screen page's rules (invented text, no game data). Run: node tests/mission_screen_test.js
const assert = require('assert');
const path = require('path');
const fs = require('fs');
const model = require(path.join(__dirname, '../tools/hud_overlay/mission_screen_model.js'));

const strings = {...model.FALLBACK, accept: 'A', decline: 'D', scroll: 'S', complete: 'C then', acceptReward: 'R', xpRewards: '%d points'};
const mission = {id: 'm', name: 'Name <1>', description: 'Plain', turnInDescription: 'Done', plot: false, xp: 7};
const find = (plan, path, name) => plan.find(step => step[1] === path && step[2] === name);

// Each page's prompt says what the keys do there.
assert.strictEqual(model.prompt('offer', strings), 'A     D     S');
assert.strictEqual(model.prompt('turnin', strings), 'C then');
assert.strictEqual(model.prompt('reward', strings), 'R');
assert.strictEqual(model.prompt('closed', strings), '');

// The offer shows the description and the offer header, the turn-in its own text and header; both show the prompt and the XP, and text is escaped.
const offer = model.listPlan('offer', mission, strings), turnin = model.listPlan('turnin', mission, strings);
assert.strictEqual(find(offer, '_level1.missions.header', 'gotoAndStop')[3], 'new');
assert.strictEqual(find(turnin, '_level1.missions.header', 'gotoAndStop')[3], 'turnin');
assert.strictEqual(find(offer, '_level1.missions.description', 'htmlText')[3], 'Plain');
assert.strictEqual(find(turnin, '_level1.missions.description', 'htmlText')[3], 'Done');
assert.strictEqual(find(offer, '_level1.missions.tooltips', 'htmlText')[3], 'A     D     S');
assert.deepStrictEqual(find(offer, '_level1', 'SetRewardsXP')[3], ['7 points']);
assert.strictEqual(model.escapeHtml('a<b>&'), 'a&lt;b&gt;&amp;');
assert.strictEqual(find(offer, '_level1.missions.selections', '_visible')[3], false);

// Unknown credits hide the credits; known ones show them.
assert.deepStrictEqual(find(offer, '_level1', 'SetRewardsTotalCredits')[3], [0]);
assert.strictEqual(find(model.rewardPlan(mission, strings), '_level1.reward.credits', '_visible')[3], false);
assert.strictEqual(find(model.rewardPlan({...mission, credits: 5}, strings), '_level1.reward.credits', '_visible')[3], true);

// The reward page: no item choices, the XP, the mission's name, the prompt.
const reward = model.rewardPlan(mission, strings);
assert.deepStrictEqual(find(reward, '_level1.reward', 'SetNumItemChoices')[3], [0]);
assert.strictEqual(find(reward, '_level1.reward.xpReward', 'text')[3], '7 points');
assert.strictEqual(find(reward, '_level1.reward.missionTitle', 'text')[3], 'Name <1>');
assert.strictEqual(find(reward, '_level1.reward.tooltips', 'htmlText')[3], 'R');

// The plain panel (no movie) still names the mission, its reward and the keys.
for (const page of ['offer', 'turnin', 'reward']) {
  const lines = model.panelText(page, mission, strings);
  assert.ok(lines.includes('Name <1>'), page);
  assert.ok(lines.some(line => line.includes('7 points')), page);
  assert.ok(lines.includes(model.prompt(page, strings)), page);
}

// A state with no mission draws nothing rather than an empty page.
assert.strictEqual(model.missionOf({page: 'offer', log: {missions: []}}), null);
assert.strictEqual(model.missionOf(null), null);

// The page is served, and goes through the guard so a missing movie cannot cover the game.
const overlay = path.join(__dirname, '../tools/hud_overlay');
const serve = fs.readFileSync(path.join(overlay, 'serve.py'), 'utf8');
for (const file of ['mission_screen.html', 'mission_screen.js', 'mission_screen_model.js']) assert.ok(serve.includes(`'/${file}'`), file);
const glue = fs.readFileSync(path.join(overlay, 'mission_screen.js'), 'utf8');
assert.ok(glue.includes('owMovieGuard.load(player, MOVIE'));
assert.ok(!/ruffle\(\)\.load/.test(glue), 'the page must not load the movie past the guard');
console.log('ok');
