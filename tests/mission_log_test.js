// Synthetic checks of tools/hud_overlay/mission_log.js (invented definition values, not game data).
const assert = require('node:assert/strict');
const log = require('../tools/hud_overlay/mission_log.js');

const def = {
  presentation: {notstarted: {frame: 'n', color: '#111111'}, active: {frame: 'a', color: '#222222'},
    activeplot: {frame: 'ap', color: '#333333'}, turnin: {frame: 't', color: '#444444'}, complete: {frame: 'c', color: '#555555'},
    failed: {frame: 'f', color: '#666666'}, ineligible: {frame: 'i', color: '#777777'}},
  list: {entries: 3, highlightColor: '#FFFFFF', categoryPrefix: '<b>', categoryPostfix: '</b>', categoryIcon: 'div'},
  difficulty: {impossible: 7, hard: 4, tough: 2, normal: -2},
  markup: {optionalObjectiveOpen: '<i>', descriptionOpen: '<u>', headerSortOpen: '<s>[', headerSortClose: ']</s>'},
  strings: {myMissions: 'Mine', sortAll: 'Every', dividerTurnIn: 'TI', dividerInProgress: 'IP', dividerCompleted: 'DONE',
    dividerFailed: 'FAIL', tooltipCancel: 'X', tooltipSort: 'S', tooltipTrack: 'T', tooltipScroll: 'R'},
};
function check(name, fn) { fn(); console.log(`PASS ${name}`); }

check('status 1 and 2 share the active presentation; plot picks the plot one', () => {
  assert.equal(log.presentationKey(1, false), 'active');
  assert.equal(log.presentationKey(2, false), 'active');
  assert.equal(log.presentationKey(2, true), 'activeplot');
  assert.equal(log.presentationKey(3, true), 'turnin');
  assert.equal(log.presentationKey(0, false), 'notstarted');
  assert.equal(log.presentationKey(9, false), 'ineligible');
});

check('summary header frames per status', () => {
  assert.deepEqual([0, 1, 2, 3, 4, 5].map(s => log.headerFrame(s, false)), ['new', 'inprogress', 'inprogress', 'turnin', 'complete', 'none']);
  assert.equal(log.headerFrame(1, true), 'inprogressMission');
});

check('difficulty: thresholds compared with >= in order, trivial below normal', () => {
  const at = diff => log.difficultyFrame(10 + diff, 10, def.difficulty);
  assert.deepEqual([8, 7, 6, 4, 3, 2, 0, -2, -3].map(at),
    ['impossible', 'impossible', 'hard', 'hard', 'tough', 'tough', 'normal', 'normal', 'trivial']);
});

check('XP bar: text and integer percent plus one', () => {
  const required = level => [0, 0, 100, 300, 600][level];
  assert.deepEqual(log.xpBar(2, 100, 4, required), {level: 2, text: '100 / 300', percent: 1});
  assert.deepEqual(log.xpBar(2, 299, 4, required), {level: 2, text: '299 / 300', percent: 100});
  assert.deepEqual(log.xpBar(2, 150, 4, required), {level: 2, text: '150 / 300', percent: 26});
  assert.deepEqual(log.xpBar(2, 300, 4, required), {level: 3, text: '300 / 600', percent: 1});
  assert.deepEqual(log.xpBar(4, 900, 4, required), {level: 4, text: '900', percent: 100});
});

check('list header and rows: categories in order, not-started missions not listed by the caller', () => {
  assert.equal(log.listHeader(def), 'MINE <s>[Every]</s>');
  assert.equal(log.listHeader({...def, strings: {...def.strings, myMissions: 'a<b', sortAll: 'x&y'}}), 'A&lt;B <s>[x&amp;y]</s>');
  const rows = log.listRows([{id: 'b', name: 'B', statusNumber: 4}, {id: 'a', name: 'A & <x>', statusNumber: 1},
    {id: 'c', name: 'C', statusNumber: 3, plot: true}], def);
  assert.deepEqual(rows.map(r => r.kind === 'mission' ? r.id : `#${r.key}`), ['#turnin', 'c', '#inprogress', 'a', '#completed', 'b']);
  assert.equal(rows[0].text, '<b>TI</b>');
  assert.equal(rows[0].type, 'div');
  assert.equal(rows[3].type, 'a');
  assert.equal(log.rowText(rows[3], false, def), "<font color='#222222'>A &amp; &lt;x&gt;</font>");
  assert.equal(log.rowText(rows[3], true, def), "<font color='#FFFFFF'>A &amp; &lt;x&gt;</font>");
});

check('selection skips category rows and stops at the ends; window keeps the selection visible', () => {
  const rows = [{kind: 'category'}, {kind: 'mission'}, {kind: 'category'}, {kind: 'mission'}, {kind: 'mission'}];
  assert.equal(log.firstMission(rows), 1);
  assert.equal(log.moveSelection(rows, 1, +1), 3);
  assert.equal(log.moveSelection(rows, 3, -1), 1);
  assert.equal(log.moveSelection(rows, 1, -1), 1);
  assert.equal(log.moveSelection(rows, 4, +1), 4);
  assert.equal(log.windowTop(5, 4, 0, 3), 2);
  assert.equal(log.windowTop(5, 1, 2, 3), 1);
  assert.equal(log.windowTop(2, 1, 5, 3), 0);
});

check('description: summary, objective lines by count, optional colour, grey description, turn-in lines', () => {
  const d = {...def, strings: {...def.strings, colonAndSpace: ': ', objectiveComplete: 'Done', turnInHud: 'TI', briefingTurnIn: 'Give'}};
  const mission = {statusNumber: 1, summary: 'S', description: 'D', turnInDescription: 'TD', turnIn: 'Bob', objectives: [
    {text: 'One', progress: 1, count: 1}, {text: 'Two', progress: 7, count: 5, optional: true}, {text: 'Three', progress: 0, count: 1}]};
  assert.equal(log.descriptionHtml(mission, d),
    'S\n\n    One: Done\n<i>    Two: 5/5\n</font>    Three \n\n<u>D</font>');
  assert.equal(log.descriptionHtml({...mission, objectives: [{text: 'Two', progress: 2, count: 5}]}, d, {wideCounts: true}),
    'S\n\n    Two: 2 / 5\n\n<u>D</font>');
  assert.equal(log.descriptionHtml({...mission, statusNumber: 3}, d), 'S\n\n    TI!\n\n<u>TD</font>\n\nGive: Bob');
  assert.equal(log.descriptionHtml({...mission, statusNumber: 4}, d), 'S\n\n\n<u>D</font>\n\nGive: Bob');
  assert.equal(log.descriptionHtml({...mission, statusNumber: 0, teaser: 'T'}, d), 'T');
});

check('description: game and localized text is escaped, only the script markup is added', () => {
  const d = {...def, strings: {...def.strings, colonAndSpace: ' <&> ', objectiveComplete: '<b>ok</b>', turnInHud: 'T&I',
    briefingTurnIn: '<font color="#123456">Give</font>'}};
  const mission = {statusNumber: 1, summary: 'a < b & c', description: '<font color="#ABCDEF">D</font>', turnIn: 'Bo<b>',
    objectives: [{text: '<font size="40">Big</font>', progress: 1, count: 1, optional: true}, {text: 'x&y', progress: 1, count: 3}]};
  assert.equal(log.descriptionHtml(mission, d),
    'a &lt; b &amp; c\n\n<i>    &lt;font size="40"&gt;Big&lt;/font&gt; &lt;&amp;&gt; &lt;b&gt;ok&lt;/b&gt;\n</font>' +
    '    x&amp;y &lt;&amp;&gt; 1/3\n\n<u>&lt;font color="#ABCDEF"&gt;D&lt;/font&gt;</font>');
  assert.equal(log.descriptionHtml({...mission, statusNumber: 3}, d),
    'a &lt; b &amp; c\n\n    T&amp;I!\n\n<u>&lt;font color="#ABCDEF"&gt;D&lt;/font&gt;</font>\n\n' +
    '&lt;font color="#123456"&gt;Give&lt;/font&gt;: Bo&lt;b&gt;');
  assert.equal(log.descriptionHtml({statusNumber: 0, teaser: '<b>t</b>'}, d), '&lt;b&gt;t&lt;/b&gt;');
});

check('hint bar', () => {
  assert.equal(log.tooltipBar(def, {canTrack: false, canScroll: false}), 'S     X');
  assert.equal(log.tooltipBar(def, {canTrack: true, canScroll: true}), 'R     T     S     X');
});

check('the overlay server routes every script the Missions page loads', () => {
  const fs = require('node:fs');
  const html = fs.readFileSync('tools/hud_overlay/missions.html', 'utf8');
  const served = fs.readFileSync('tools/hud_overlay/serve.py', 'utf8');
  const scripts = [...html.matchAll(/<script src="([^"]+)"/g)].map(m => m[1]).filter(src => !src.includes('/'));
  assert.ok(scripts.includes('missions.js') && scripts.includes('mission_log.js'));
  for (const src of scripts) assert.ok(served.includes(`'/${src}': '${src}'`), `serve.py does not route ${src}`);
});
