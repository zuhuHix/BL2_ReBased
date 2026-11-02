const assert = require('node:assert/strict');
const fs = require('node:fs');
const { move } = require('../tools/hud_overlay/skill_nav.js');

const skill = (id, cell) => ({ id, cell });
const branch = tiers => ({ tiers: tiers.map(cells => ({
  skills: cells.map(([id, cell]) => skill(id, cell))
})) });
const filler = branch([[['x0', 0]]]);
const branches = [
  filler,
  branch([
    [['a0', 0], ['a2', 2]],
    [['b0', 0], ['b2', 2]],
    [['c0', 0], ['c1', 1], ['c2', 2]],
    [['d1', 1]],
  ]),
  filler,
];
const action = { branch: -1, tier: -1, skill: skill('action', 0) };
const selectedAt = (tier, cell, selectedBranch = 1) => ({
  branch: selectedBranch, tier, skill: skill(`t${tier}c${cell}`, cell)
});

function check(name, fn) {
  fn();
  console.log(`PASS ${name}`);
}

check('four Down moves from the action skill reach tier 4, column 1', () => {
  let current = action;
  let position;
  for (let i = 0; i < 4; i++) {
    position = move(branches, 1, current, 'down');
    current = { ...position, skill: branches[position.branch].tiers[position.tier].skills.find(s => s.cell === position.cell) };
  }
  assert.deepEqual(position, { branch: 1, tier: 3, cell: 1 });
  assert.equal(current.skill.id, 'd1');
});

check('the overlay server routes every script the Skills page loads', () => {
  // serve.py only serves listed files; an unlisted script 404s and the page runs without it.
  const html = fs.readFileSync('tools/hud_overlay/skills.html', 'utf8');
  const served = fs.readFileSync('tools/hud_overlay/serve.py', 'utf8');
  const scripts = [...html.matchAll(/<script src="([^"]+)"/g)].map(m => m[1]).filter(src => !src.includes('/'));
  assert.ok(scripts.includes('skill_nav.js'));
  for (const src of scripts) assert.ok(served.includes(`'/${src}': '${src}'`), `serve.py does not route ${src}`);
});

check('an action-skill tie picks the lower-numbered first-tier cell (UNVERIFIED)', () => {
  assert.deepEqual(move(branches, 1, action, 'down'), { branch: 1, tier: 0, cell: 0 });
});

check('vertical movement preserves the column when a two-cell tier becomes three cells', () => {
  assert.deepEqual(move(branches, 1, selectedAt(1, 2), 'down'), { branch: 1, tier: 2, cell: 2 });
});

check('a middle-column tie into a two-cell tier picks its lower-numbered cell (UNVERIFIED)', () => {
  assert.deepEqual(move(branches, 1, selectedAt(2, 1), 'up'), { branch: 1, tier: 1, cell: 0 });
});

check('Up from tier 1 returns to the action skill; top and bottom edges do not wrap', () => {
  assert.deepEqual(move(branches, 1, selectedAt(0, 0), 'up'), { branch: -1, tier: -1, cell: -1 });
  assert.equal(move(branches, 1, action, 'up'), null);
  assert.equal(move(branches, 1, selectedAt(3, 1), 'down'), null);
});

console.log('6/6 skill navigation checks passed (synthetic trees; tie and edge policies UNVERIFIED)');
