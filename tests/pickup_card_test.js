// Synthetic checks for the world pickup card (tools/hud_overlay/pickup_card.js) and the card code it shares with the
// inventory page (tools/hud_overlay/item_card.js). No game data, movie or renderer: a fake harness API records calls.
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

function load(locationPath, extra = {}) {
  const context = vm.createContext({console:{log(){}}, setTimeout, Math, JSON, Number, String, Map, Set, Array, Object,
    location:{pathname:locationPath}, ...extra});
  context.window = context;
  for (const file of ['item_card.js', 'pickup_card.js'])
    vm.runInContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay', file), 'utf8'), context);
  return context;
}

let passed = 0;
function check(name, test) { test(); passed++; console.log('PASS ' + name); }

const hud = load('/');
const {placement, plan} = hud.owPickupCard;

check('Placement centres the card over the item, its bottom a gap above it', () => {
  const box = placement(0.5, 0.8, 1280, 720, 328 / 259);
  assert.ok(Math.abs(box.left + box.width / 2 - 640) < 1e-9);
  assert.ok(Math.abs(box.top + box.height - (0.8 * 720 - 0.03 * 720)) < 1e-9);
  assert.ok(Math.abs(box.height - 0.3 * 720) < 1e-9);
  assert.ok(Math.abs(box.width / box.height - 328 / 259) < 1e-9);
});
check('Placement keeps the card inside the viewport at the edges', () => {
  for (const [x, y] of [[0, 0], [1, 1], [0.02, 0.1], [0.99, 0.95]]) {
    const box = placement(x, y, 1280, 720, 1.3);
    assert.ok(box.left >= 0 && box.top >= 0 && box.left + box.width <= 1280 + 1e-9 && box.top + box.height <= 720 + 1e-9, `${x},${y}`);
  }
});
check('Plan: no item hides; a new item, other data or another level refills; a move only shows', () => {
  const card = {id:'a', x:0.5, y:0.5, level:36, item:{id:'a', damage:10}, compare:null};
  assert.strictEqual(plan(null, null), 'hide');
  assert.strictEqual(plan(card, null), 'hide');
  assert.strictEqual(plan(card, {id:'a'}), 'hide'); // no item object
  assert.strictEqual(plan(null, card), 'fill');
  assert.strictEqual(plan(card, {...card, x:0.6, y:0.4}), 'show');
  assert.strictEqual(plan(card, {...card, id:'b', item:{id:'b', damage:10}}), 'fill');
  assert.strictEqual(plan(card, {...card, item:{id:'a', damage:11}}), 'fill');
  assert.strictEqual(plan(card, {...card, compare:{id:'w0', damage:9}}), 'fill');
  assert.strictEqual(plan(card, {...card, level:37}), 'fill');
});
check('In the HUD page apply() forwards to the frame; the frame page drives the movie', () => {
  const made = [];
  const doc = {createElement:tag => { const el = {tag, style:{}, attrs:{}, listeners:{}, setAttribute(k, v) { this.attrs[k] = v; },
    addEventListener(k, f) { this.listeners[k] = f; }}; made.push(el); return el; }, body:{appendChild(){}}};
  const page = load('/', {document:doc});
  const card = {id:'a', x:0.5, y:0.5, level:36, item:{id:'a'}, compare:null};
  page.owPickupCard.apply(card);
  assert.strictEqual(made.length, 1); assert.strictEqual(made[0].tag, 'iframe'); assert.strictEqual(made[0].src, 'pickup_card.html');
  const received = [];
  made[0].contentWindow = {owPickupCard:{apply:c => received.push(c)}};
  made[0].listeners.load();
  assert.deepStrictEqual(received.map(c => c && c.id), ['a']);
  page.owPickupCard.apply(null);
  assert.deepStrictEqual(received.map(c => c && c.id), ['a', null]);
  assert.strictEqual(made.length, 1); // one frame
  // The frame page creates no frame of its own (it waits for Ruffle, absent here).
  const framed = load('/pickup_card.html', {document:doc});
  framed.owPickupCard.apply(card);
  assert.strictEqual(made.length, 1);
});
check('The shared card writer fills BL2 card rows with arrows against the compared item', () => {
  const calls = [];
  const writer = hud.owItemCard.createCardWriter({
    call:(p, m, ...a) => { calls.push([p, m, ...a]); return undefined; },
    get:(p, k) => (k === 'bottomScroll' ? 1 : 0), set(){}});
  writer.configureCard('c', {id:'a', name:'Gun', manufacturer:'Maliwan', type:'smg', element:'fire', damage:100.2, fireRate:9,
    reloadTime:3.2, magazine:31, accuracy:93.8, statusDps:700, statusChance:18.8, level:34, value:100},
    {id:'b', damage:120, fireRate:8, reloadTime:3.5, magazine:30, accuracy:90}, 'default', false, 36);
  const rows = calls.filter(c => c[1] === 'SetTopStat').map(c => [c[3], c[4], c[5]]);
  assert.deepStrictEqual(rows.slice(0, 5).map(r => r.join('|')),
    ['Damage|101|down', 'Accuracy|93.8|up', 'Fire Rate|9.0|up', 'Reload Speed|3.2|up', 'Magazine Size|31|up']);
  assert.strictEqual(rows.length, 7); // the fire weapon's two status rows
  const level = calls.find(c => c[1] === 'SetLevelRequirement');
  assert.deepStrictEqual(level.slice(2), [true, true, false, 'LEVEL REQUIREMENT: 34']);
  assert.strictEqual(calls.find(c => c[1] === 'SetBackgroundStyle')[2], 'default');
});

console.log(`${passed}/${passed} pickup card checks passed (synthetic items; card size, placement and timing UNVERIFIED)`);
