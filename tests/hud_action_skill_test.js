// Synthetic checks of tools/hud_overlay/action_skill.js: the host's action-skill
// state to the HUD movie's clip calls. No movie or game data is needed. The frame
// rules come from the movie sampled during real casts
// (docs/verification/ACTION_SKILL_HUD.md); the cast below uses that record's timings
// (lock 0.70-5.70 s, cooldown pool 13 s draining from 5.71 s, ready at 18.73 s).
const assert = require('node:assert/strict');
const path = require('node:path');
const skill = require(path.join(__dirname, '../tools/hud_overlay/action_skill.js'));

const AA = '_level1.p1.activeAbility', CD = '_level1.p1.character.cooldown';
const ICON = '/ package/UI_HUD_AAIcons/AA-Siren';

// Emblem bar: 1 full, 100 empty, floor(1 + 99 * used).
assert.equal(skill.activeBarFrame(1), 1);
assert.equal(skill.activeBarFrame(0), 100);
assert.equal(skill.activeBarFrame(0.5), 50);
assert.equal(skill.activeBarFrame(2), 1);
assert.equal(skill.activeBarFrame(-1), 100);
// Icon: 2 with the pool full (skill running), 100 ready, floor(2 + 98 * used).
assert.equal(skill.cooldownFrame(1), 2);
assert.equal(skill.cooldownFrame(0), 100);
assert.equal(skill.cooldownFrame(0.5), 51);
assert.equal(skill.cooldownFrame(0.001), 99);

// The host's state over a hit cast, as the HUD pushes it, against the frames the game showed.
const LOCK_AT = 0.70, END = 5.70, POOL = 13;
function hostState(t) {
  const active = t >= LOCK_AT && t < END ? 1 - (t - LOCK_AT) / (END - LOCK_AT) : null;
  const pool = t < 0 ? 0 : t < END ? POOL : Math.max(0, POOL - (t - END));
  return {character: 6, skillIcon: ICON, skillActive: active, skillCooldown: pool / POOL};
}
function gameFrames(t) {   // the fitted rules of the record, on the same clock
  const aa = t >= LOCK_AT && t < END ? 3 : 1;
  const bar = aa === 3 ? Math.floor(1 + 99 * (t - LOCK_AT) / (END - LOCK_AT)) : null;
  const pool = t < 0 ? 0 : t < END ? POOL : Math.max(0, POOL - (t - END));
  return {aa, bar, cooldown: Math.floor(2 + 98 * (1 - pool / POOL))};
}
// Replay the plan's calls onto a model of the three clips.
const clips = {aa: 1, bar: 1, cooldown: 1, icon: null};
let prev = {};
for (let i = -20; i <= 400; i++) {
  const t = i / 20;
  const state = hostState(t);
  for (const [target, op, a] of skill.plan(prev, state)) {
    if (target === AA) clips.aa = a === 'siren_action' ? 3 : 1;
    else if (target === AA + '.bar') clips.bar = a;
    else if (target === CD) clips.cooldown = a;
    else if (op === 'loadMovie') clips.icon = a;
    else assert.fail(`unexpected call ${target} ${op}`);
  }
  prev = Object.assign({}, prev, state);
  const want = gameFrames(t);
  assert.equal(clips.icon, ICON, `icon at ${t}`);
  assert.equal(clips.aa, want.aa, `emblem at ${t}`);
  if (want.bar !== null) assert.ok(Math.abs(clips.bar - want.bar) <= 1, `bar at ${t}: ${clips.bar} vs ${want.bar}`);
  assert.ok(Math.abs(clips.cooldown - want.cooldown) <= 1, `icon frame at ${t}: ${clips.cooldown} vs ${want.cooldown}`);
}
assert.equal(clips.cooldown, 100);   // ready again

// Unchanged frames send nothing; the icon loads once.
assert.deepEqual(skill.plan(prev, hostState(20)), []);

// hud3d.js: the 2D stand-in for the clusters' 3D turn.
const hud3d = require(path.join(__dirname, '../tools/hud_overlay/hud3d.js'));
const near = (a, b, eps, what) => assert.ok(Math.abs(a - b) <= eps, `${what}: ${a} vs ${b}`);
near(hud3d.FOCAL, 914.0, 0.1, 'focal length of a 70-degree view across 1280 px');
// Unturned: a plain scale by SCALE about the stage centre.
let m = hud3d.project(0, 100, 600, -20, 80);
near(m.sx, hud3d.SCALE, 1e-9, 'unturned x scale'); near(m.sy, hud3d.SCALE, 1e-9, 'unturned y scale');
near(m.x0, 640 + (100 - 640) * hud3d.SCALE, 1e-9, 'unturned x'); near(m.y0, 360 + (600 - 360) * hud3d.SCALE, 1e-9, 'unturned y');
// A synthetic clip turned +25 near the left edge: the stand-in meets the exact projection at both bounds edges, its
// near (left) side comes out larger than the plain scale and the far side smaller.
const exact = (a, ox, u) => {
  const r = a * Math.PI / 180, f = hud3d.FOCAL, d = f / hud3d.SCALE;
  return 640 + (ox + u * Math.cos(r) - 640) * f / (d + u * Math.sin(r));
};
m = hud3d.project(25, 100, 650, -10, 190);
near(m.x0 + m.sx * -10, exact(25, 100, -10), 1e-9, 'left edge'); near(m.x0 + m.sx * 190, exact(25, 100, 190), 1e-9, 'right edge');
assert.ok(m.x0 < 640 + (100 - 640) * hud3d.SCALE, 'turned clip origin moves outwards');
assert.ok(m.sx > hud3d.SCALE * Math.cos(25 * Math.PI / 180), 'near side magnified');

console.log('hud_action_skill_test: all checks passed');
