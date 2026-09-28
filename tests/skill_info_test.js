// Synthetic tests for tools/hud_overlay/skill_info.js (no game data): the
// info-box HTML layout observed in the real-game trace, on invented text.
// Run: node tests/skill_info_test.js
const assert = require('assert');
const path = require('path');
const { skillInfoHtml } = require(path.join(__dirname, '..', 'tools', 'hud_overlay', 'skill_info.js'));

const strings = { nextLevel: 'Next:', classModBonus: '<font size="12">+%n %s from Mod</font>',
  classModUntrained: '<font size="12">(train first)</font>', pointSingular: 'Point', pointPlural: 'Points' };
const skill = { description: 'Desc.', maxGrade: 2,
  stats: [[], [['Stat: ', '+1', '']], [['Stat: ', '+2', '']], [['Stat: ', '+3', '']], [['Stat: ', '+4', '']]] };
const quote = { description: 'Q.', maxGrade: 1, stats: [[], [['', '', ' Words.']]] };

const cases = [
  [skill, { grade: 0 }, 'Desc.\n\n<font size="15" color="#cc6600"><i>Next:\nStat: +1</i></font>'],
  [skill, { grade: 1 }, "Desc.\n\nStat: <font color='#cc6600'>+1</font>\n\n<font size=\"15\" color=\"#cc6600\"><i>Next:\nStat: +2</i></font>"],
  [skill, { grade: 2 }, "Desc.\n\nStat: <font color='#00cc00'>+2</font>"],
  [skill, { grade: 0, locked: true }, '<font color="#a3a3b0">Desc.\n\n<font size="15" color="#a3a3b0"><i>Next:\nStat: +1</i></font></font>'],
  [skill, { grade: 0, bonus: 2 }, 'Desc.\n\n<font size="15" color="#cc6600"><i>Next:\nStat: +3</i></font>'
    + "\n\n<font color='#33FFFF'><font size=\"12\">+2 Points from Mod</font>\n<font size=\"12\">(train first)</font></font>"],
  [skill, { grade: 0, bonus: 1, locked: true }, '<font color="#a3a3b0">Desc.\n\n<font size="15" color="#a3a3b0"><i>Next:\nStat: +1</i></font></font>'
    + "\n\n<font color='#33FFFF'><font size=\"12\">+1 Point from Mod</font>\n<font size=\"12\">(train first)</font></font>"],
  [skill, { grade: 1, bonus: 2 }, "Desc.\n\nStat: <font color='#cc6600'>+3</font>\n\n<font size=\"15\" color=\"#cc6600\"><i>Next:\nStat: +4</i></font>"
    + "\n\n<font color='#33FFFF'><font size=\"12\">+2 Points from Mod</font></font>"],
  [quote, { grade: 0 }, 'Q.\n\n<font size="15" color="#cc6600"><i>Next:\nWords.</i></font>'],
  [quote, { grade: 1 }, "Q.\n\n<font color='#00cc00'></font> Words."],
];
cases.forEach(([s, state, expected], i) => assert.strictEqual(skillInfoHtml(s, state, strings), expected, `case ${i}`));
console.log(`${cases.length} skill info cases passed`);
