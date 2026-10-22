// Synthetic tests for tools/hud_overlay/minimap.js (no game data): the clip values against the rules read from the
// live game (docs/verification/HUD_MINIMAP.md), restated here as plain arithmetic, and the call economy of plan().
'use strict';
const assert = require('assert');
const mm = require('../tools/hud_overlay/minimap.js');

let passed = 0;
function test(name, fn) {
  fn();
  passed++;
  console.log('ok', name);
}
const near = (a, b, tol, what) => assert.ok(Math.abs(a - b) <= tol, `${what}: ${a} vs ${b}`);

// A synthetic map: centre (1000, -2000); player 2500 uu along +X and 1250 along +Y from it.
const centre = [1000, -2000];
const player = {x: 3500, y: -750};
const state = (yaw, icons = []) => ({movie: '/ package/Map/Map', centre, x: player.x, y: player.y, yaw, icons});

test('constants: one minimap unit is WorldRadius / 48 uu, the map at 128 / 125', () => {
  assert.strictEqual(mm.UU_PER_UNIT, 125);
  near(mm.MAP_SCALE, 1.024, 1e-12, 'map scale');
});

test('map: yaw 0 puts the player map point at the origin, unturned', () => {
  const L = mm.layout(state(0));
  // map point: x = (Y - cy) / 128, y = -(X - cx) / 128, times 1.024; the clip is moved by minus that.
  near(L.map.x, -(1250 / 128) * 1.024, 0.05, 'x');
  near(L.map.y, (2500 / 128) * 1.024, 0.05, 'y');
  assert.strictEqual(L.map.rotation, 0);
  near(L.map.scale, 102.4, 1e-9, 'scale');
});

test('map: rotation is -yaw in [-180, 180), position turns with it', () => {
  for (const [yaw, rot] of [[90, -90], [180, -180], [270, 90], [-90, 90], [45, -45], [359, 1]]) {
    const L = mm.layout(state(yaw));
    near(L.map.rotation, rot, 1e-9, `rotation at ${yaw}`);
    const base = mm.layout(state(0)).map;
    const t = -yaw * Math.PI / 180;
    near(L.map.x, base.x * Math.cos(t) - base.y * Math.sin(t), 0.06, `x at ${yaw}`);
    near(L.map.y, base.x * Math.sin(t) + base.y * Math.cos(t), 0.06, `y at ${yaw}`);
  }
});

test('north marker: on the 58 x 48 box edge plus (-16, -18.8)', () => {
  const cases = [[0, -16, -66.8], [90, -74, -18.8], [180, -16, 29.2], [270, 42, -18.8], [45, -64, -66.8]];
  for (const [yaw, x, y] of cases) {
    const n = mm.layout(state(yaw)).north;
    near(n.x, x, 0.06, `N x at ${yaw}`);
    near(n.y, y, 0.06, `N y at ${yaw}`);
  }
});

test('icons: offset (dY, -dX) / 125 turned by -yaw; view direction is up', () => {
  const ahead = {kind: 'shop', x: player.x + 2500, y: player.y};   // 20 units ahead at yaw 0
  const right = {kind: 'health', x: player.x, y: player.y + 1250}; // 10 units right at yaw 0
  let L = mm.layout(state(0, [ahead, right]));
  near(L.icons[0].x, 0, 0.05, 'ahead x'); near(L.icons[0].y, -20, 0.05, 'ahead y');
  near(L.icons[1].x, 10, 0.05, 'right x'); near(L.icons[1].y, 0, 0.05, 'right y');
  L = mm.layout(state(90, [ahead]));   // looking along +Y: +X is to the left
  near(L.icons[0].x, -20, 0.05, 'ahead at 90 x'); near(L.icons[0].y, 0, 0.05, 'ahead at 90 y');
});

test('icons: names and symbols follow the game (shop0.., hstation0.., objective0)', () => {
  const L = mm.layout(state(0, [{kind: 'shop', x: 3500, y: -700}, {kind: 'shop', x: 3500, y: -600},
    {kind: 'health', x: 3500, y: -700}, {kind: 'objective', x: 3600, y: -750}, {kind: 'nonsense', x: 0, y: 0}]));
  assert.deepStrictEqual(L.icons.map(i => i.name), ['shop0', 'shop1', 'hstation0', 'objective0']);
  assert.deepStrictEqual(L.icons.map(i => i.symbol), ['icon_shop', 'icon_shop', 'icon_healthstation', 'objective clip']);
});

test('icons: stations hide outside the box, mission icons clamp onto its edge', () => {
  const far = {x: player.x + 125 * 60, y: player.y + 125 * 30};   // 60 ahead, 30 right: outside (|y| > 48)
  const L = mm.layout(state(0, [{kind: 'shop', ...far}, {kind: 'eligible', ...far}]));
  assert.strictEqual(L.icons[0].visible, false);
  assert.strictEqual(L.icons[1].visible, true);
  near(L.icons[1].x, 30 * 48 / 60, 0.05, 'clamped x');
  near(L.icons[1].y, -48, 0.05, 'clamped y');
  // Inside the box but outside the 58 x 48 ellipse: still shown (the game uses the box).
  const corner = mm.layout(state(0, [{kind: 'shop', x: player.x + 125 * 40, y: player.y - 125 * 50}]));
  assert.strictEqual(corner.icons[0].visible, true);
});

test('objective: scale 140 up to 6000 uu, then 140 * (1 - (d - 6000) / 18000)', () => {
  near(mm.objectiveScale(0), 140, 1e-9, 'near');
  near(mm.objectiveScale(6000), 140, 1e-9, 'at radius');
  near(mm.objectiveScale(6500), 136.11, 0.01, '6500');
  near(mm.objectiveScale(9000), 116.67, 0.01, '9000');
  const L = mm.layout(state(0, [{kind: 'objective', x: player.x + 9000, y: player.y}]));
  near(L.icons[0].scale, 116.67, 0.01, 'layout');
  near(L.icons[0].y, -48, 0.05, 'clamped');
});

test('plan: first state loads the map and attaches icons, hidden sub-clips, arrow and N on top', () => {
  const attached = new Set();
  const calls = mm.plan({}, state(0, [{kind: 'objective', x: 3600, y: -750}]), attached);
  assert.deepStrictEqual(calls[0], [mm.MM, 'setMapMovie', '/ package/Map/Map']);
  assert.ok(calls.some(c => c[1] === 'apply' && c[2] === 'attachMovie' && c[3][0] === 'objective clip' && c[3][1] === 'objective0'));
  for (const sub of ['above', 'below', 'edgeArrow', 'objective.optional'])
    assert.ok(calls.some(c => c[0] === `${mm.MM}.objective0.${sub}` && c[2] === '_visible' && c[3] === false), sub);
  assert.ok(calls.some(c => c[0] === `${mm.MM}.DirArrow` && c[2] === 'swapDepths'));
  assert.ok(attached.has('objective0'));
});

test('plan: an unchanged state sends nothing; a turn sends only what moved', () => {
  const attached = new Set();
  const a = state(0, [{kind: 'shop', x: 3700, y: -750}]);
  mm.plan({}, a, attached);
  assert.deepStrictEqual(mm.plan(a, {...a}, attached), []);
  const turned = mm.plan(a, {...a, yaw: 30}, attached);
  assert.ok(turned.length > 0);
  assert.ok(!turned.some(c => c[2] === 'attachMovie' || c[1] === 'setMapMovie'));
  assert.ok(!turned.some(c => c[2] === '_xscale'));
});

test('plan: a slot the host stops reporting is hidden once', () => {
  const attached = new Set();
  const a = state(0, [{kind: 'eligible', x: 3700, y: -750}]);
  mm.plan({}, a, attached);
  const b = {...a, icons: []};
  const calls = mm.plan(a, b, attached);
  assert.deepStrictEqual(calls, [[`${mm.MM}.eligible0`, 'set', '_visible', false]]);
  assert.deepStrictEqual(mm.plan(b, {...b}, attached), []);
});

test('plan: no state (no manifest on the host) sends nothing', () => {
  assert.deepStrictEqual(mm.plan({}, undefined, new Set()), []);
  assert.deepStrictEqual(mm.plan({}, {}, new Set()), []);
});

test('fog: one toggleFogOfWarBlob per blob with its uncovered flag', () => {
  assert.deepStrictEqual(mm.fogCalls({A_PWDA_0: false, A_PWDA_1: true}),
    [[mm.MM, 'toggleFogOfWarBlob', 'A_PWDA_0', false], [mm.MM, 'toggleFogOfWarBlob', 'A_PWDA_1', true]]);
});

test('splitAffine: R(phi) diag(s1, s2) R(theta) rebuilds the matrix', () => {
  const [a, b, c, d] = [1.1, -0.08, 0.03, 0.97];
  const k = mm.splitAffine(a, b, c, d);
  const rot = deg => { const t = deg * Math.PI / 180; return [Math.cos(t), Math.sin(t), -Math.sin(t), Math.cos(t)]; };
  const mul = (P, Q) => [P[0] * Q[0] + P[2] * Q[1], P[1] * Q[0] + P[3] * Q[1], P[0] * Q[2] + P[2] * Q[3], P[1] * Q[2] + P[3] * Q[3]];
  const M = mul(mul(rot(k.phi), [k.s1, 0, 0, k.s2]), rot(k.theta));
  [a, b, c, d].forEach((v, i) => near(M[i], v, 1e-12, `entry ${i}`));
});

test('placeEcho: the nested clips reproduce the fitted projection of the panel', () => {
  const hud3d = require('../tools/hud_overlay/hud3d.js');
  const S = hud3d.SCALE;
  const values = {
    '_level1.p1': {_x: 640 + (216 - 640) * S, _y: 360 + (30 - 360) * S},
    '_level1.p1.minimapEcho': {_yrotation: -20},
    [mm.MM]: {_x: -58, _y: 3.5}, [mm.MM + '.mapmask']: {_x: 0, _y: -0.25},
  };
  const sets = {};
  const call = (t, op, a, b) => {
    if (op === 'get') return (values[t] || {})[a] ?? 0;
    if (op === 'apply' && a === 'getBounds') return {xMin: -58, xMax: 58, yMin: -48, yMax: 48};
    if (op === 'set') (sets[t] = sets[t] || {})[a] = b;
  };
  mm.page.echo0 = {x: 980, y: 74, xs: 0.9, ys: 1};
  mm.page.log = () => {};
  const out = mm.placeEcho(call, '_level1.p1', hud3d);
  assert.ok(out.residual < 3, `residual ${out.residual}`);
  const E = sets['_level1.p1.minimapEcho'], m = sets[mm.MM];
  // Compose: stage = P1' + S * (Echo(R S) * (R(theta) p + tm) + Te) for a minimap-local point p.
  const R = deg => { const t = deg * Math.PI / 180; return (x, y) => [x * Math.cos(t) - y * Math.sin(t), x * Math.sin(t) + y * Math.cos(t)]; };
  const stage = (px, py) => {
    const [ax, ay] = R(m._rotation)(px, py);
    const [bx, by] = R(E._rotation)((ax - 58) * E._xscale / 100, (ay + 3.5) * E._yscale / 100);
    return [values['_level1.p1']._x + S * (bx + E._x), values['_level1.p1']._y + S * (by + E._y)];
  };
  // Against the projection itself at the mask's corners: within the fit's residual.
  const r = -20 * Math.PI / 180, D = hud3d.FOCAL / S, ox = 216 + 980, oy = 30 + 74;
  for (const [u, v] of [[-58, -48.25], [58, -48.25], [-58, 47.75], [58, 47.75]]) {
    const su = 0.9 * (u - 58), w = D + su * Math.sin(r);
    const X = 640 + (ox + su * Math.cos(r) - 640) * hud3d.FOCAL / w, Y = 360 + (oy + (v + 3.5) - 360) * hud3d.FOCAL / w;
    const [hx, hy] = stage(u, v);
    assert.ok(Math.hypot(hx - X, hy - Y) <= out.residual + 1e-6, `corner ${u},${v}: ${hx},${hy} vs ${X},${Y}`);
  }
  // The slant is there: the right edge is taller than the left, and the top edge rises to the right.
  const [, tl] = stage(-58, -48.25), [, tr] = stage(58, -48.25), [, bl] = stage(-58, 47.75), [, br] = stage(58, 47.75);
  assert.ok(tr < tl, 'top edge rises to the right');
});

test('warpClips: the warp gets the panel own rectangle for minimapEcho, others untouched', () => {
  const values = {
    '_level1.p1.minimapEcho': {_x: 980, _y: 74, _xscale: 90, _yscale: 100},
    [mm.MM]: {_x: -58, _y: 3.5}, [mm.MM + '.mapmask']: {_x: 0, _y: -0.25},
  };
  const call = (t, op, a) => {
    if (op === 'get') return (values[t] || {})[a] ?? 0;
    if (op === 'apply' && a === 'getBounds') return {xMin: -58, xMax: 58, yMin: -48, yMax: 48};
  };
  const origin = {x: 216, y: 30};
  // readClips' union of visible children reaches the parked library icons far right of and above the panel.
  const clips = [{name: 'health', a: 25, ox: 300, rect: [1, 2, 3, 4]},
    {name: 'minimapEcho', a: -20, ox: 1196, rect: [1100, -120, 1750, 180]}];
  const out = mm.warpClips(clips, call, '_level1.p1', origin);
  assert.deepStrictEqual(out[0], clips[0]);
  assert.strictEqual(out[1].a, -20); assert.strictEqual(out[1].ox, 1196);
  const [x0, y0, x1, y1] = out[1].rect;
  near(x0, 216 + 980 + 0.9 * (-58 - 58 - 14), 1e-9, 'left');
  near(x1, 216 + 980 + 0.9 * (-58 + 58 + 14), 1e-9, 'right');
  near(y0, 30 + 74 + (3.5 - 0.25 - 48 - 26), 1e-9, 'top (room for the N marker)');
  near(y1, 30 + 74 + (3.5 - 0.25 + 48 + 14), 1e-9, 'bottom');
  // No mask bounds (movie not ready): the reader's rectangle is kept.
  assert.deepStrictEqual(mm.warpClips(clips, () => null, '_level1.p1', origin)[1].rect, clips[1].rect);
});

console.log(`${passed} passed`);
