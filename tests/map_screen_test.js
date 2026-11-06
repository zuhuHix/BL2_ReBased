// Synthetic checks for the Map screen's rules (tools/hud_overlay/map.js). No game data, movie or browser: the page part
// of map.js does not run without Ruffle, so only the exported rules are exercised.
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const context = vm.createContext({console, Math, JSON, Number, Object});
context.window = context;
vm.runInContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/map.js'), 'utf8'), context);
const M = context.owMapScreen;
const close = (a, b) => Math.abs(a - b) < 1e-9;
const plain = value => JSON.parse(JSON.stringify(value)); // values from the vm context compared as plain data

let passed = 0;
function check(name, test) { test(); passed++; console.log('PASS ' + name); }

check('World to map movie: x follows world +Y, y world -X, 128 uu per unit, origin at the volume centre', () => {
  const [x, y] = M.moviePoint(1000 + 256, 2000 + 128, [1000, 2000]);
  assert.ok(close(x, 1) && close(y, -2));
  assert.deepStrictEqual(plain(M.moviePoint(1000, 2000, [1000, 2000])), [0, 0]);
});
check('Scale stays between MinScale 135 and MaxScale 395', () => {
  assert.strictEqual(M.clampScale(100), 135); assert.strictEqual(M.clampScale(500), 395); assert.strictEqual(M.clampScale(200), 200);
});
check('Pan step: PanIncrement times lerp(1, scale / MinScale, 0.25)', () => {
  assert.ok(close(M.panFraction(135), 0.05));
  assert.ok(close(M.panFraction(270), 0.05 * 1.25));
});
const state = {minimap:{centre:[0, 0], x:1280, y:-640, yaw:30, icons:[
  {kind:'shop', x:0, y:0}, {kind:'shop', x:128, y:0}, {kind:'objective', x:-256, y:256}, {kind:'unknown', x:1, y:1}]}};
check('Layout centres the player under the crosshairs and scales icons with the map', () => {
  const out = M.layout(state, {scale:200, pan:[0, 0], centre:[300, 200]});
  const player = out.landmarks[0];
  assert.deepStrictEqual([player.key, player.template, player.frame, player.r], ['player', 'players', 'player', 30]);
  assert.ok(close(player.x, 300) && close(player.y, 200));
  // The map origin: the player's movie point (-5, -10) times 2 away from the crosshairs.
  assert.ok(close(out.mapX, 300 + 5 * 2) && close(out.mapY, 200 + 10 * 2));
  const shop1 = out.landmarks.find(l => l.key === 'shop1');
  assert.ok(close(shop1.x, out.mapX) && close(shop1.y, out.mapY - 2)); // 128 uu north = one unit up, times 2
  assert.strictEqual(out.landmarks.find(l => l.key === 'objective0').frame, 'objective');
  assert.ok(!out.landmarks.some(l => l.key.startsWith('unknown')));
  assert.deepStrictEqual(plain(out.landmarks.map(l => l.key)), ['player', 'shop0', 'shop1', 'objective0']);
});
check('Pan moves the map and every icon together, not the crosshairs', () => {
  const a = M.layout(state, {scale:135, pan:[0, 0], centre:[300, 200]}), b = M.layout(state, {scale:135, pan:[40, -20], centre:[300, 200]});
  for (let i = 0; i < a.landmarks.length; i++)
    assert.ok(close(b.landmarks[i].x - a.landmarks[i].x, 40) && close(b.landmarks[i].y - a.landmarks[i].y, -20));
  assert.ok(close(b.mapX - a.mapX, 40) && close(b.mapY - a.mapY, -20));
});
check('Pan is limited so the point under the crosshairs stays on the map, at any zoom', () => {
  const rect = {xMin:-10, xMax:20, yMin:-5, yMax:5}, player = [0, 0];
  // At 100 %: the crosshair point (player - pan) must stay within x -10..20, y -5..5.
  assert.deepStrictEqual(plain(M.clampPan([0, 0], player, rect, 100)), [0, 0]);
  assert.deepStrictEqual(plain(M.clampPan([1000, -1000], player, rect, 100)), [10, -5]);
  assert.deepStrictEqual(plain(M.clampPan([-1000, 1000], player, rect, 100)), [-20, 5]);
  // At 200 % the limits double (pan is in screen units).
  assert.deepStrictEqual(plain(M.clampPan([1000, 1000], player, rect, 200)), [20, 10]);
  // A player off-centre on the map shifts the limits.
  assert.deepStrictEqual(plain(M.clampPan([1000, 1000], [5, 2], rect, 100)), [15, 7]);
  // No rectangle yet (the map movie still loading): unchanged.
  assert.deepStrictEqual(plain(M.clampPan([3, 4], player, null, 100)), [3, 4]);
});
function fakePage(owReply) {
  // The page part of map.js with a fake Ruffle (`owReply` answers every harness call), document and clock.
  let now = 0;
  const intervals = [], timeouts = [], keys = [];
  const small = {textContent:''};
  const loading = {hidden:false, classList:{add(){}, remove(){}}, querySelector:() => small};
  const page = vm.createContext({console:{log(){}, warn(){}}, Math, JSON, Number, Object, String, Map, Set, Array, Promise,
    performance:{now:() => now},
    setInterval:(f) => { intervals.push(f); return intervals.length; }, clearInterval:(i) => { intervals[i - 1] = null; },
    setTimeout:(f, ms) => { timeouts.push({f, at:now + (ms || 0)}); return timeouts.length; }, clearTimeout:(i) => { if (timeouts[i - 1]) timeouts[i - 1].f = null; },
    fetch:() => Promise.reject(new Error('no map.json')),
    location:{href:''},
    addEventListener:(type, f) => { if (type === 'keydown') keys.push(f); },
    owMovieGuard:{load:() => new Promise(() => {})},
    RufflePlayer:{newest:() => ({createPlayer:() => ({style:{}, ow:owReply, ruffle:() => ({load(){}})})})},
    document:{body:{appendChild(){}}, getElementById:() => loading, createElement:() => ({style:{}, setAttribute(){}, addEventListener(){}})}});
  page.window = page;
  vm.runInContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/map.js'), 'utf8'), page);
  const advance = ms => {
    for (let t = 0; t < ms; t += 100) {
      now += 100;
      for (const f of intervals) if (f) f();
      for (const t2 of timeouts) if (t2.f && t2.at <= now) { const f = t2.f; t2.f = null; f(); }
    }
  };
  const press = key => keys.forEach(f => f({key, preventDefault(){}}));
  return {page, small, loading, advance, press, intervals};
}
check('Keys on the Map page: M does nothing, L goes to the Missions, Escape and Tab close', () => {
  const p = fakePage(() => undefined);
  const after = key => { p.page.location.href = ''; p.press(key); return p.page.location.href; };
  assert.strictEqual(after('m'), '');
  assert.strictEqual(after('l'), '/__ow_tab_missions');
  assert.strictEqual(after('Escape'), '/__ow_close_map');
  assert.strictEqual(after('Tab'), '/__ow_close_map');
});
check('A status menu that never loads stops the polling at 15 s and says so; Escape still closes', () => {
  const p = fakePage(() => undefined);
  p.advance(14000);
  assert.strictEqual(p.small.textContent, '');
  p.advance(2000);
  assert.ok(/status menu movie did not load.*Press Esc/.test(p.small.textContent), p.small.textContent);
  assert.ok(p.intervals.every(f => f === null), 'polling stopped');
  p.press('Escape');
  assert.strictEqual(p.page.location.href, '/__ow_close_map');
});
check('A ready page with no map state, or with a map movie that never loads, shows why after 10 s', () => {
  // A harness that is ready at once and answers every read with 1 (frames loaded, clips present).
  const p = fakePage((target, op) => (op === 'get' ? 1 : undefined));
  p.advance(200);
  p.advance(10000);
  assert.ok(/host sent no map state/.test(p.small.textContent), p.small.textContent);
  const q = fakePage((target, op) => (op === 'get' ? 1 : undefined));
  q.advance(200);
  q.page.owMap({name:'X', minimap:null});
  assert.ok(/No map for this area/.test(q.small.textContent), q.small.textContent);
  const r = fakePage((target, op) => (op === 'get' ? 1 : undefined));
  r.advance(200);
  r.page.owMap({name:'X', minimap:{movie:'/m', centre:[0, 0], x:0, y:0, yaw:0, icons:[]}});
  r.advance(10100);
  assert.ok(/map movie did not load \(\/m\)/.test(r.small.textContent), r.small.textContent);
});
check('Icon kinds map to the landmark frames (CompassIconFrames and the mission frames)', () => {
  assert.deepStrictEqual(plain(M.LANDMARK_FRAMES), {shop:'shop', health:'health', fasttravel:'rstation',
    customize:'customization', leveltravel:'transition', eligible:'missionNew', turnin:'missionComplete', objective:'objective'});
});
check('Tabs from the Map: 1 Missions, 2 stays, 3 Inventory, 4 Skills, 5 no page; Comma Missions (else none), Period Inventory', () => {
  assert.strictEqual(M.tabRoute('1'), '/__ow_tab_missions');
  assert.strictEqual(M.tabRoute('2'), '');
  assert.strictEqual(M.tabRoute('3'), '/__ow_tab_inventory');
  assert.strictEqual(M.tabRoute('4'), '/__ow_tab_skills');
  assert.strictEqual(M.tabRoute('5'), null);
  assert.strictEqual(M.tabRoute(','), '/__ow_tab_missions');
  assert.strictEqual(M.tabRoute(',', false), null);
  assert.strictEqual(M.tabRoute('.'), '/__ow_tab_inventory');
  assert.strictEqual(M.tabRoute('x'), undefined);
});

console.log(`${passed}/${passed} map screen checks passed (synthetic states; centring, arrow and pan/zoom steps UNVERIFIED)`);
