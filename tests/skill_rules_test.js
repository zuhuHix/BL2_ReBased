// Synthetic tests for tools/hud_overlay/skill_rules.js (no game data): the page's copy of the host spend rule.
// The tree is invented; it has the installed trees' shape (action skill worth 1 point, 5 points per tier).
// Run: node tests/skill_rules_test.js
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const rules = require('../tools/hud_overlay/skill_rules.js');

const tier = (points, ...skills) => ({ pointsToUnlockNext: points, skills: skills.map(([id, maxGrade]) => ({ id, maxGrade })) });
const tree = {
  actionSkill: { maxGrade: 1 },
  actionSkillPoints: 1,
  branches: [
    { tiers: [tier(5, ['a', 5], ['b', 5]), tier(5, ['c', 5]), tier(0, ['d', 1])] },
    { tiers: [tier(5, ['e', 5])] },
  ],
};
const skill = id => tree.branches.flatMap(b => b.tiers.flatMap(t => t.skills)).find(s => s.id === id);

let passed = 0;
function check(name, body) { body(); passed++; console.log('ok', name); }

// Replays spends through the rule, as the host would accept them, and returns the final state.
function spend(state, branch, tierIndex, id) {
  const why = rules.refusal(tree, state, branch, tierIndex, branch < 0 ? null : skill(id));
  if (why) return [state, why];
  const next = { ...state, points: state.points - 1, grades: { ...state.grades } };
  if (branch < 0) next.actionGrade++;
  else next.grades[id] = (next.grades[id] || 0) + 1;
  return [next, ''];
}

check('trees stay closed until the action skill has its point', () => {
  const s = { points: 3, actionGrade: 0, grades: {} };
  assert.equal(rules.treesOpen(tree, s), false);
  assert.equal(rules.refusal(tree, s, 0, 0, skill('a')), 'action skill not unlocked');
  assert.equal(rules.refusal(tree, s, -1, -1, null), '');
});

check('the action skill stops at its max grade', () => {
  assert.equal(rules.refusal(tree, { points: 3, actionGrade: 1, grades: {} }, -1, -1, null), 'action skill at max grade');
});

check('a tier opens at 5 points in its branch, summed over every lower tier', () => {
  let s = { points: 20, actionGrade: 0, grades: {} };
  [s] = spend(s, -1, -1);
  for (let i = 0; i < 4; i++) [s] = spend(s, 0, 0, i % 2 ? 'b' : 'a');
  assert.equal(rules.refusal(tree, s, 0, 1, skill('c')), 'tier locked (4 of 5 points in branch)');
  [s] = spend(s, 0, 0, 'a');
  assert.equal(rules.tierOpen(tree, s, 0, 1), true);
  assert.equal(rules.tierOpen(tree, s, 0, 2), false, 'tier 2 needs 10');
  assert.equal(rules.tierOpen(tree, s, 1, 0), true, 'another branch keeps its first tier open');
  for (let i = 0; i < 5; i++) [s] = spend(s, 0, 1, 'c');
  assert.equal(rules.tierRequirement(tree, 0, 2), 10);
  assert.equal(rules.refusal(tree, s, 0, 2, skill('d')), '');
});

check('max grade and no points refuse', () => {
  const s = { points: 0, actionGrade: 1, grades: { a: 5 } };
  assert.equal(rules.refusal(tree, s, 0, 0, skill('a')), 'skill at max grade');
  assert.equal(rules.refusal(tree, s, 0, 0, skill('b')), 'no skill points');
});

check('grades from the host are clamped to 0..max and non-numbers count as 0', () => {
  const s = { points: 0, actionGrade: 1, grades: { a: 9, b: 'x', c: -2 } };
  assert.equal(rules.rank(s, skill('a')), 5);
  assert.equal(rules.rank(s, skill('b')), 0);
  assert.equal(rules.invested(tree, s, 0), 5);
});

check('after a respec (no grades, action grade 0) the trees close again', () => {
  const s = { points: 12, actionGrade: 0, grades: {} };
  assert.equal(rules.tierOpen(tree, s, 0, 0), false);
  assert.equal(rules.invested(tree, s, 0), 0);
});

check('the Skills page loads skill_rules.js and serve.py routes it', () => {
  const dir = path.join(__dirname, '..', 'tools', 'hud_overlay');
  assert.match(fs.readFileSync(path.join(dir, 'skills.html'), 'utf8'), /<script src="skill_rules\.js"><\/script>[\s\S]*<script src="skills\.js">/);
  assert.match(fs.readFileSync(path.join(dir, 'serve.py'), 'utf8'), /'\/skill_rules\.js': 'skill_rules\.js'/);
});

console.log(`${passed}/7 skill rule checks passed (synthetic tree)`);
