// Synthetic tests for tools/hud_overlay/skill_info.js (no game data): the
// info-box HTML layout observed in the real-game trace, on invented text.
// Run: node tests/skill_info_test.js
const assert = require('assert');
const path = require('path');
const { skillInfoHtml } = require(path.join(__dirname, '..', 'tools', 'hud_overlay', 'skill_info.js'));
const colours = require(path.join(__dirname, '..', 'tools', 'hud_overlay', 'colours.js'));
// Made-up colours standing in for the ignored local file (the real ones are game-derived and never in the repository).
const MADE_UP = { skill_current: '#102030', skill_maxed: '#405060', skill_locked: '#708090', skill_class_mod: '#0A0B0C' };
globalThis.OW_GAME_COLOURS = MADE_UP;

const strings = { nextLevel: 'Next:', classModBonus: '<font size="12">+%n %s from Mod</font>',
  classModUntrained: '<font size="12">(train first)</font>', pointSingular: 'Point', pointPlural: 'Points' };
const skill = { description: 'Desc.', maxGrade: 2,
  stats: [[], [['Stat: ', '+1', '']], [['Stat: ', '+2', '']], [['Stat: ', '+3', '']], [['Stat: ', '+4', '']]] };
const quote = { description: 'Q.', maxGrade: 1, stats: [[], [['', '', ' Words.']]] };

const cases = [
  [skill, { grade: 0 }, 'Desc.\n\n<font size="15" color="#102030"><i>Next:\nStat: +1</i></font>'],
  [skill, { grade: 1 }, "Desc.\n\nStat: <font color='#102030'>+1</font>\n\n<font size=\"15\" color=\"#102030\"><i>Next:\nStat: +2</i></font>"],
  [skill, { grade: 2 }, "Desc.\n\nStat: <font color='#405060'>+2</font>"],
  [skill, { grade: 0, locked: true }, '<font color="#708090">Desc.\n\n<font size="15" color="#708090"><i>Next:\nStat: +1</i></font></font>'],
  [skill, { grade: 0, bonus: 2 }, 'Desc.\n\n<font size="15" color="#102030"><i>Next:\nStat: +3</i></font>'
    + "\n\n<font color='#0a0b0c'><font size=\"12\">+2 Points from Mod</font>\n<font size=\"12\">(train first)</font></font>"],
  [skill, { grade: 0, bonus: 1, locked: true }, '<font color="#708090">Desc.\n\n<font size="15" color="#708090"><i>Next:\nStat: +1</i></font></font>'
    + "\n\n<font color='#0a0b0c'><font size=\"12\">+1 Point from Mod</font>\n<font size=\"12\">(train first)</font></font>"],
  [skill, { grade: 1, bonus: 2 }, "Desc.\n\nStat: <font color='#102030'>+3</font>\n\n<font size=\"15\" color=\"#102030\"><i>Next:\nStat: +4</i></font>"
    + "\n\n<font color='#0a0b0c'><font size=\"12\">+2 Points from Mod</font></font>"],
  [quote, { grade: 0 }, 'Q.\n\n<font size="15" color="#102030"><i>Next:\nWords.</i></font>'],
  [quote, { grade: 1 }, "Q.\n\n<font color='#405060'></font> Words."],
];
cases.forEach(([s, state, expected], i) => assert.strictEqual(skillInfoHtml(s, state, strings), expected, `case ${i}`));
// Without the local file every colour is colours.js's neutral grey, and the layout is the same.
globalThis.OW_GAME_COLOURS = null;
const neutral = skillInfoHtml(skill, { grade: 1, bonus: 2 }, strings);
assert.strictEqual(neutral, cases[6][2].split('#102030').join(colours.neutral('skill_current'))
  .split('#0a0b0c').join(colours.neutral('skill_class_mod')), 'neutral fallback');
for (const value of Object.values(MADE_UP)) assert.ok(!neutral.includes(value.toLowerCase()), 'no made-up colour without the file');
console.log(`${cases.length + 1} skill info cases passed`);
