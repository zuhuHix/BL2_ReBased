// Synthetic checks of tools/hud_overlay/hud_warp.js: the perspective warp of the HUD's turned clips. No movie or game
// data: clip positions below are made up; the model and its constants are hud3d.js's
// (docs/verification/ACTION_SKILL_HUD.md, "HUD layout").
const assert = require('node:assert/strict');
const path = require('node:path');
const warp = require(path.join(__dirname, '../tools/hud_overlay/hud_warp.js'));
const hud3d = require(path.join(__dirname, '../tools/hud_overlay/hud3d.js'));

const near = (a, b, eps, what) => assert.ok(Math.abs(a - b) <= eps, `${what}: ${a} vs ${b}`);
const deg = Math.PI / 180;

// Same constants as the stand-in.
assert.equal(warp.FOCAL, hud3d.FOCAL);
assert.equal(warp.SCALE, hud3d.SCALE);

// The written-out formula, independent of the module.
function formula(a, ox, x, y) {
  const F = 640 / Math.tan(35 * deg), D = F / 1.095, u = x - ox, w = D + u * Math.sin(a * deg);
  return [640 + (ox + u * Math.cos(a * deg) - 640) * F / w, 360 + (y - 360) * F / w];
}

// 1. The homography reproduces the projection at many points (< 0.01 px), for every turn the movie uses.
let worst = 0;
for (const a of [25, 15, -20, -25, 40, -40])
  for (const ox of [-50, 96, 400, 640, 900, 1185, 1300]) {
    const H = warp.clipHomography(a, ox);
    for (let x = -100; x <= 1380; x += 37) for (let y = -40; y <= 760; y += 41) {
      const [X, Y] = warp.apply(H, x, y), [fx, fy] = formula(a, ox, x, y), [px, py] = warp.projectPoint(a, ox, x, y);
      worst = Math.max(worst, Math.hypot(X - fx, Y - fy), Math.hypot(px - fx, py - fy));
    }
  }
assert.ok(worst < 0.01, `homography vs formula: ${worst} px`);

// The stand-in meets the same projection at a clip's left and right bounds (hud3d.project's contract).
for (const [a, ox, u0, u1] of [[25, 100, -15, 180], [-25, 1180, -215, 40]]) {
  const m = hud3d.project(a, ox, 650, u0, u1);
  near(m.x0 + m.sx * u0, warp.projectPoint(a, ox, ox + u0, 650)[0], 1e-6, 'stand-in left edge');
  near(m.x0 + m.sx * u1, warp.projectPoint(a, ox, ox + u1, 650)[0], 1e-6, 'stand-in right edge');
}

// 2. Inverses round-trip; the canvas warp is the projection after undoing Ruffle's p1 scale, at any canvas size.
const I = warp.mul(warp.clipHomography(25, 100), warp.inv(warp.clipHomography(25, 100)));
I.forEach((v, i) => near(v / I[8], [1, 0, 0, 0, 1, 0, 0, 0, 1][i], 1e-9, `H * inv(H) [${i}]`));
for (const [w, h] of [[1280, 720], [1920, 1080], [1000, 700]]) {
  const r = warp.clipWarp({name: 'bar', a: 25, ox: 100, rect: [60, 620, 300, 670]}, w, h);
  const R = warp.ruffleMatrix();
  for (let x = 60; x <= 300; x += 20) for (let y = 620; y <= 670; y += 10) {
    const [rx, ry] = warp.apply(R, x, y);                         // where Ruffle drew the flat point (stage px)
    const [X, Y] = warp.apply(r.fwd, rx * w / 1280, ry * h / 720);  // the warp, in canvas px
    const [fx, fy] = formula(25, 100, x, y);
    near(X, fx * w / 1280, 1e-6, `warp x at ${w}x${h}`); near(Y, fy * h / 720, 1e-6, `warp y at ${w}x${h}`);
    const [bx, by] = warp.apply(r.back, X, Y);
    near(bx, rx * w / 1280, 1e-6, 'back x'); near(by, ry * h / 720, 1e-6, 'back y');
  }
}

// 3. Trapezoid. A +25 clip turned about its left end (a bar from its origin to 180 px right, 20 px tall): the
// near (left) edge is taller than the unturned plane's 1.095 x 20 and the far (right) tip shorter, as the real frame
// shows for the health and shield bars. A -25 clip extending left of its origin (ammo, grenades) is the mirror image:
// its right edge is taller than its left tip.
const height = (a, ox, x) => warp.projectPoint(a, ox, x, 670)[1] - warp.projectPoint(a, ox, x, 650)[1];
const left = height(25, 100, 95), right = height(25, 100, 280);
assert.ok(left > 1.095 * 20 && right < 1.095 * 20 && left > right * 1.05, `+25 bar: left ${left} right ${right}`);
const lTip = height(-25, 1180, 1000), rEdge = height(-25, 1180, 1185);
assert.ok(rEdge > 1.095 * 20 && lTip < 1.095 * 20, `-25 bar: left ${lTip} right ${rEdge}`);
// Against the unturned plane the near edge moves out towards the screen edge and the bar comes out longer (the game
// shows its health bar wider than the plain 1.095 scale would make it).
const X = x => warp.projectPoint(25, 100, x, 650)[0];
assert.ok(X(95) < 640 + (95 - 640) * 1.095);
assert.ok(X(280) - X(95) > 1.095 * 185, `bar length ${X(280) - X(95)}`);

// 4. Partition: every source pixel has one owner; a clip's bounds beat a neighbour's glow margin; pinned regions
// (the minimap's stand-in) keep their pixels in place; regions are drawn smallest first.
const clips = [
  {name: 'bar', a: 25, ox: 100, rect: [60, 620, 300, 670]},
  {name: 'icon', a: 25, ox: 106, rect: [80, 560, 145, 615]},   // its margin overlaps the bar's bounds
  {name: 'ammo', a: -25, ox: 1180, rect: [930, 620, 1225, 680]},
  {name: 'hidden', a: 25, ox: 0, rect: [10, 10, 10, 50]},     // empty: dropped
];
const pinned = [{name: 'map', rect: [1100, 0, 1280, 140]}];
const regions = warp.partition(clips, 1280, 720, {}, pinned);
assert.deepEqual(regions.map(r => r.name), ['map', 'icon', 'bar', 'ammo']);
assert.equal(regions.pinned, 1);
const iconK = 1, barK = 2;
// A pixel inside the bar's bounds and the icon's margin belongs to the bar.
const R = warp.ruffleMatrix();
const [bx, by] = warp.apply(R, 120, 622);
assert.equal(warp.owner(regions, bx, by), barK);
const [ix, iy] = warp.apply(R, 100, 590);
assert.equal(warp.owner(regions, ix, iy), iconK);
assert.equal(warp.owner(regions, 1150, 50), 0);     // pinned: drawn in place
assert.equal(warp.owner(regions, 640, 360), -1);    // nobody's: drawn in place
// Emulate the fragment shader's passes over the source grid: each source pixel is taken by exactly one pass, and a
// region's pixels land inside the quad it draws (its forward image), so none is lost.
const inQuad = (q, x, y) => q.every((p, i) => {
  const n = q[(i + 1) % q.length];
  return (n[0] - p[0]) * (y - p[1]) - (n[1] - p[1]) * (x - p[0]) >= -1e-6;
});
let counted = 0;
for (let x = 0.5; x < 1280; x += 3) for (let y = 0.5; y < 720; y += 3) {
  const o = warp.owner(regions, x, y);
  const passes = [o < regions.pinned].concat(regions.map((r, k) => !r.pinned && o === k)).filter(Boolean).length;
  assert.equal(passes, 1, `source pixel ${x},${y} drawn ${passes} times`);
  if (o >= regions.pinned) {
    const [X, Y] = warp.apply(regions[o].fwd, x, y);
    assert.ok(inQuad(regions[o].quad, X, Y), `${regions[o].name} pixel ${x},${y} outside its quad`);
    counted++;
  }
}
assert.ok(counted > 1000);
// At most MAX_REGIONS regions, pinned ones kept.
const many = Array.from({length: 30}, (_, i) => ({name: `c${i}`, a: 25, ox: 10 * i, rect: [10 * i, 0, 10 * i + 5 + i, 20]}));
assert.equal(warp.partition(many, 1280, 720, {}, pinned).length, warp.MAX_REGIONS);

// 5. Path choice: the warp only with a WebGL overlay and a readable Ruffle canvas, else the affine stand-in.
const choose = env => warp.choosePath(env).path;
assert.equal(choose({overlayGl: true, ruffleCanvas: {}, ruffleContext: 'webgl2', preserved: true}), 'warp');
assert.equal(choose({overlayGl: true, ruffleCanvas: {}, ruffleContext: '2d', preserved: true}), 'warp');
assert.equal(choose({overlayGl: false, ruffleCanvas: {}, ruffleContext: 'webgl2', preserved: true}), 'affine');
assert.equal(choose({overlayGl: true, ruffleCanvas: {}, ruffleContext: 'webgl2', preserved: false}), 'affine');
assert.equal(choose({overlayGl: true, ruffleCanvas: {}, ruffleContext: 'webgpu', preserved: false}), 'affine');
assert.equal(choose({overlayGl: true, ruffleCanvas: null, ruffleContext: null}), 'affine');
assert.equal(choose({disabled: true, overlayGl: true, ruffleCanvas: {}, ruffleContext: 'webgl2', preserved: true}), 'affine');
assert.equal(choose(undefined), 'affine');

// The context patch asks Ruffle's WebGL context for preserveDrawingBuffer and records what it got (a fake canvas).
const calls = [];
globalThis.HTMLCanvasElement = function () {};
HTMLCanvasElement.prototype.getContext = function (type, attrs) {
  calls.push([type, attrs]);
  return {getContextAttributes: () => Object.assign({}, attrs)};
};
warp.patchContexts();
const fake = new HTMLCanvasElement();
fake.getContext('webgl2', {antialias: false});
assert.deepEqual(calls.at(-1), ['webgl2', {antialias: false, preserveDrawingBuffer: true}]);
assert.equal(warp.seen.context, 'webgl2'); assert.equal(warp.seen.preserved, true); assert.equal(warp.seen.canvas, fake);
const overlay = new HTMLCanvasElement(); overlay.__owOverlay = true;
overlay.getContext('webgl', {alpha: true});
assert.deepEqual(calls.at(-1), ['webgl', {alpha: true}]);   // the overlay's own context is left alone
assert.equal(warp.seen.canvas, fake);

// 6. Visible bounds through a fake harness: hidden children and empty text fields do not count; a clip's own shapes
// reaching outside its children do.
const tree = {
  'p1.a': {_x: 100, _y: 600, _xscale: 90, _yscale: 100, _rotation: 0, _yrotation: 25, _visible: true, _alpha: 100,
    bounds: {xMin: -300, xMax: 300, yMin: -30, yMax: 30}, kids: ['bar', 'panel', 'label', 'empty']},
  'p1.a.bar': {_x: 0, _y: 0, _xscale: 100, _yscale: 100, _rotation: 0, _visible: true, _alpha: 100, bounds: {xMin: 0, xMax: 180, yMin: -10, yMax: 10}},
  'p1.a.panel': {_x: -300, _y: -30, _xscale: 100, _yscale: 100, _rotation: 0, _visible: false, _alpha: 100, bounds: {xMin: 0, xMax: 600, yMin: 0, yMax: 60}},
  'p1.a.label': {_x: 5, _y: -20, _width: 40, _height: 15, _visible: true, _alpha: 100, text: '181', bounds: null},
  'p1.a.empty': {_x: 50, _y: 20, _width: 300, _height: 15, _visible: true, _alpha: 100, text: '', bounds: null},
  'p1.b': {_x: 900, _y: 600, _xscale: 100, _yscale: 100, _rotation: 0, _yrotation: -25, _visible: true, _alpha: 100,
    bounds: {xMin: -50, xMax: 10, yMin: -5, yMax: 5}, kids: ['dot']},
  'p1.b.dot': {_x: 0, _y: 0, _xscale: 100, _yscale: 100, _rotation: 0, _visible: true, _alpha: 100, bounds: {xMin: 0, xMax: 4, yMin: 0, yMax: 4}},
  'p1.c': {_x: 0, _y: 0, _xscale: 100, _yscale: 100, _rotation: 0, _yrotation: 0, _visible: true, _alpha: 100, bounds: {xMin: 0, xMax: 1, yMin: 0, yMax: 1}},
};
const ow = (target, op, a) => {
  const node = tree[target];
  if (!node) return undefined;
  if (op === 'keys') return (node.kids || []).join(',');
  if (op === 'apply') return node.bounds;
  if (a === '_name') return target.split('.').pop();
  return node[a];
};
const read = warp.readClips(ow, 'p1', ['a', 'b', 'c'], {x: 10, y: 20});
assert.deepEqual(read.map(c => c.name), ['a', 'b']);
// a: bar (0..180, -10..10) and label (5..45, -20..-5), scaled 0.9 in x, at 110, 620; panel and empty text left out.
assert.deepEqual(read[0].rect.map(v => +v.toFixed(6)), [110, 600, 110 + 0.9 * 180, 630]);
assert.equal(read[0].ox, 110);
// b: its own bounds reach outside the dot, so they count whole.
assert.deepEqual(read[1].rect, [860, 615, 920, 625]);

console.log('hud_warp_test: all checks passed');
