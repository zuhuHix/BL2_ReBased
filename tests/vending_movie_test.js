// Synthetic checks for the pure part of the vending page's movie view (tools/hud_overlay/vending_movie.js): which list a tab
// shows, its section headers, the scrolling and what the item card gets. Invented items only; no game data.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const movie = require('../tools/hud_overlay/vending_movie.js');
// The card writer needs the colour table first (colours.js; made-up colours here, none of the game's).
globalThis.OW_GAME_COLOURS = {};
globalThis.owColours = require('../tools/hud_overlay/colours.js');
require('../tools/hud_overlay/item_card.js'); // sets globalThis.owItemCard (the card writer the page uses)

let passed = 0;
function check(name, test) { test(); passed++; console.log('PASS ' + name); }

const gun = (index, type, extra = {}) => ({id: `g${index}`, name: `Gun ${index}`, kind: 'weapon', rarity: 2, level: 4, price: 100 + index,
  priceValid: true, featured: false, card: {type, manufacturer: 'Test', damage: 10}, ...extra});

check('Every machine kind has a header frame and a panel background label (the panels take a label, not a number)', () => {
  for (const kind of ['weapons', 'items', 'health', 'blackmarket']) {
    assert.ok(movie.HEADER_FRAME[kind]);
    assert.ok(['weapons', 'ammo', 'medicine', 'eridium'].includes(movie.PANEL_BACKGROUND[kind]));
  }
});

check('The tabs cover the three lists, the stock in the middle', () => {
  assert.deepEqual([...movie.TAB_ORDER].sort(), ['backpack', 'buyback', 'shop']);
  assert.equal(movie.TAB_SIDE.shop, 'middle');
  assert.equal(movie.TAB_ORDER.indexOf('shop'), 1);
});

check('Sections: guns by card type, other rows by kind, unknown types in a general one', () => {
  assert.equal(movie.sectionLabel(gun(1, 'Shotgun')), 'Shotguns');
  assert.equal(movie.sectionLabel(gun(1, 'assault rifle')), 'Assault Rifles');
  assert.equal(movie.sectionLabel(gun(1, 'spork')), 'Weapons');
  assert.equal(movie.sectionLabel({kind: 'class_mod'}), 'Class Mods');
  assert.equal(movie.sectionLabel({kind: 'health'}), 'Health');
});

check('A list gets a header where the section changes, and keeps each row index', () => {
  const rows = [gun(0, 'pistol', {featured: true}), gun(1, 'smg'), gun(2, 'smg'), gun(3, 'shotgun'), gun(4, 'smg')];
  const shop = movie.listEntries(rows, 'shop');
  assert.deepEqual(shop.map(entry => entry.header || entry.index), ['SMGs', 1, 2, 'Shotguns', 3, 'SMGs', 4]);
  // Only the stock tab leaves the item of the day out (it has its own panel).
  assert.equal(movie.listEntries(rows, 'buyback')[1].index, 0);
});

check('Grouped lists: one run per section in first-seen order, stable inside, the item of the day first', () => {
  const rows = [gun(0, 'pistol', {featured: true}), gun(1, 'smg'), gun(2, 'shotgun'), gun(3, 'smg'), gun(4, 'shotgun'), gun(5, 'pistol')];
  const grouped = movie.groupLists({shop: rows, backpack: rows.slice(1), buyback: []});
  assert.deepEqual(grouped.shop.map(row => row.id), ['g0', 'g1', 'g3', 'g2', 'g4', 'g5']);
  assert.deepEqual(grouped.backpack.map(row => row.id), ['g1', 'g3', 'g2', 'g4', 'g5']);
  assert.deepEqual(grouped.buyback, []);
  assert.deepEqual(movie.listEntries(grouped.shop, 'shop').filter(entry => entry.header).map(entry => entry.header), ['SMGs', 'Shotguns', 'Pistols']);
});

check('Scrolling keeps the view when the row shows, else moves the least, and shows the row\'s header', () => {
  const entries = movie.listEntries([1, 2, 3, 4, 5, 6].map(i => gun(i, i < 4 ? 'smg' : 'shotgun')), 'buyback');
  // entries: SMGs,0,1,2,Shotguns,3,4,5 (8 entries)
  assert.equal(movie.scrollTop(entries, 1, 0, 4), 0);
  assert.equal(movie.scrollTop(entries, 5, 0, 4), 4);
  assert.equal(movie.scrollTop(entries, 3, 4, 4), 4);  // row 3 sits right under its header at entry 4
  assert.equal(movie.scrollTop(entries, 0, 4, 4), 0);  // back up to the first header
  assert.equal(movie.scrollTop(entries, -1, 99, 4), 4); // nothing selected: clamped only
  assert.equal(movie.scrollTop([], 0, 3, 4), 0);
});

check('The card shows the row\'s name, level and price; gear gets its item type; no card means no item', () => {
  const card = movie.cardItem({...gun(7, 'pistol'), rarity: 3});
  assert.equal(card.name, 'Gun 7');
  assert.equal(card.value, 107);
  assert.equal(card.type, 'pistol');
  assert.equal(card.level, 4);
  assert.equal(movie.cardItem({id: 'x', name: 'Shield', kind: 'shield', rarity: 1, level: 0, price: 5, priceValid: true}).itemType, 'shield');
  assert.equal(movie.cardItem({id: 'x', name: 'Ammo', kind: 'ammo', rarity: 1, price: 5, priceValid: false}).value, null);
  assert.equal(movie.cardItem(null), null);
});

check('The tooltip names what Enter does on each tab', () => {
  assert.ok(movie.tooltip('shop', false).includes('Purchase'));
  assert.ok(movie.tooltip('backpack', false).includes('Sell'));
  assert.ok(movie.tooltip('buyback', false).includes('Buy back'));
  assert.ok(movie.tooltip('backpack', true).includes('Purchase'));
});

check('Cell icons: the card type icon frame by gun type, ammo by its gun type, gear by kind; none where no frame fits', () => {
  assert.equal(movie.cellIcon(gun(1, 'Sniper Rifle')), 'Sniper');
  assert.equal(movie.cellIcon(gun(1, 'spork')), null);
  assert.equal(movie.cellIcon({kind: 'ammo', ammoType: 'ar'}), 'ar');
  assert.equal(movie.cellIcon({kind: 'ammo', ammoType: 'grenade'}), 'Grenade');
  assert.equal(movie.cellIcon({kind: 'shield'}), 'Shield');
  assert.equal(movie.cellIcon({kind: 'health'}), 'health');
  assert.equal(movie.cellIcon({kind: 'class_mod'}), null);
  assert.equal(movie.cellIcon(null), null);
  assert.equal(movie.cardItem({id: 'a', name: 'AR Ammo', kind: 'ammo', ammoType: 'ar', rarity: 1, price: 9, priceValid: true}).type, 'assault rifle');
});

check('Feedback: a new message is a refusal; a buy or sale counts once the money moved its way in the same opening', () => {
  const at = (serial, money, extra = {}) => ({serial, money, opening: 1, machine: {id: 'm'}, ...extra});
  assert.deepEqual(movie.feedbackFor(null, null, null, 'Full', ''), {frame: 'bad', label: 'FULL'});
  assert.equal(movie.feedbackFor(null, null, null, 'FULL', 'FULL'), null);
  assert.deepEqual(movie.feedbackFor({action: 'buy', serial: 3}, at(3, 500), at(4, 400), '', ''), {frame: 'purchased', label: 'PURCHASED'});
  assert.deepEqual(movie.feedbackFor({action: 'buyback', serial: 3}, at(3, 500), at(4, 400), '', ''), {frame: 'purchased', label: 'PURCHASED'});
  assert.deepEqual(movie.feedbackFor({action: 'sell', serial: 3}, at(3, 500), at(4, 600), '', ''), {frame: 'sold', label: 'SOLD'});
  assert.equal(movie.feedbackFor({action: 'buy', serial: 3}, at(3, 500), at(4, 500), '', ''), null);       // a restock
  assert.equal(movie.feedbackFor({action: 'buy', serial: 3}, at(3, 500), at(3, 400), '', ''), null);       // not answered yet
  assert.equal(movie.feedbackFor({action: 'buy', serial: 3}, at(3, 500), at(4, 400, {opening: 2}), '', ''), null);
});

check('An empty tab: SOLD OUT for the stock, one (EMPTY) cell for the backpack and buy-back, nothing when rows exist', () => {
  assert.equal(movie.emptyView('shop', []), 'soldout');
  assert.equal(movie.emptyView('backpack', []), 'cell');
  assert.equal(movie.emptyView('buyback', []), 'cell');
  // Only the item of the day left: the list itself is empty.
  assert.equal(movie.emptyView('shop', movie.listEntries([gun(0, 'pistol', {featured: true})], 'shop')), 'soldout');
  assert.equal(movie.emptyView('buyback', movie.listEntries([gun(1, 'smg')], 'buyback')), null);
});

// The browser half, run in a VM with a stub Ruffle and movie guard: a movie load that never settles.
// The browser half with a stub guard whose load never settles: the page's timeout cancels it and removes the player once.
async function hangingLoad() {
  const lines = [];
  let removed = 0, cancels = 0;
  const player = {remove: () => { removed++; }, ow: () => undefined};
  const never = new Promise(() => {});
  never.cancel = () => { cancels++; };
  const window = {
    RufflePlayer: {newest: () => ({createPlayer: () => player})},
    owMovieGuard: {load: () => never, FILES: {vending: []}},
    console: {log: line => lines.push(line)}, setTimeout, clearTimeout
  };
  window.window = window;
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/vending_movie.js'), 'utf8'), window);
  const running = await window.owVendingMovie.start({appendChild() {}}, null, null, {loadTimeoutMs: 30});
  return {running, removed, cancels, lines};
}

// The guard wins the race: the page's timeout timer is cleared, not left pending.
async function guardWins() {
  const timers = new Map();
  let next = 1;
  const player = {remove() {}, ow: (target, op, member) => (op === 'get' && member === '_name' ? 'x' : op === 'apply' && member === 'getBounds' ? {xMin: 0, xMax: 1, yMin: 0, yMax: 1} : 0)};
  const won = Promise.resolve(true);
  won.cancel = () => {};
  const window = {
    RufflePlayer: {newest: () => ({createPlayer: () => player})},
    owMovieGuard: {load: () => won, FILES: {vending: []}}, console: {log() {}},
    setTimeout: (f, ms) => { const id = next++; timers.set(id, ms); setTimeout(() => { if (timers.delete(id)) f(); }, Math.min(ms, 5)); return id; },
    clearTimeout: id => { timers.delete(id); }, owItemCard: globalThis.owItemCard
  };
  window.window = window;
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/vending_movie.js'), 'utf8'), window);
  const running = await window.owVendingMovie.start({appendChild() {}}, null, null, {loadTimeoutMs: 60000});
  return {running, pendingLoadTimers: [...timers.values()].filter(ms => ms === 60000).length};
}

// The real movie guard, cancelled while Ruffle is still loading: a late resolve or reject does nothing.
async function guardCancel(settle) {
  const logs = [], observers = [];
  let removed = 0, finish = null;
  const player = {remove() { removed++; }, shadowRoot: {querySelector: () => null},
    ruffle: () => ({load: () => new Promise((resolve, reject) => { finish = {resolve, reject}; })})};
  const context = vm.createContext({console: {log: line => logs.push(line)}, Promise, setTimeout, clearTimeout,
    AbortController: class { constructor() { this.signal = {}; } abort() {} },
    MutationObserver: class { constructor() { this.live = false; observers.push(this); } observe() { this.live = true; } disconnect() { this.live = false; } },
    fetch: () => Promise.resolve({status: 200})});
  context.window = context;
  vm.runInContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/movie_guard.js'), 'utf8'), context);
  const loading = context.owMovieGuard.load(player, {url: 'x.swf'}, 'Vending', []);
  for (let i = 0; i < 20 && !finish; i++) await new Promise(resolve => setImmediate(resolve));
  if (settle === 'success') {
    finish.resolve();
    const result = await loading;
    loading.cancel();                     // a later setup step failed
    return {result, removed, logs, observers};
  }
  loading.cancel();
  if (settle === 'resolve') finish.resolve(); else finish.reject(new Error('late'));
  const result = await loading;
  return {result, removed, logs, observers};
}

// The guard resolves true, then a movie call throws during setup: start() still resolves false and leaves nothing behind.
async function setupThrows() {
  const lines = [];
  let removed = 0, cancels = 0;
  // Reads answer (the movie looks loaded); the first write or method call throws, as a broken movie call would.
  const player = {remove: () => { removed++; },
    ow: (target, op) => { if (op === 'get') return 'x'; throw new Error('movie call failed'); }};
  const loaded = Promise.resolve(true);
  loaded.cancel = () => { cancels++; };
  const window = {
    RufflePlayer: {newest: () => ({createPlayer: () => player})},
    owMovieGuard: {load: () => loaded, FILES: {vending: []}},
    console: {log: line => lines.push(line)}, setTimeout, clearTimeout
  };
  window.window = window;
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/vending_movie.js'), 'utf8'), window);
  const running = await window.owVendingMovie.start({appendChild() {}}, null, null, {loadTimeoutMs: 60000});
  return {running, removed, cancels, lines, ready: window.owVendingMovie.isReady()};
}

(async () => {
  const thrown = await setupThrows();
  assert.equal(thrown.running, false);   // the HTML page runs
  assert.equal(thrown.cancels, 1);       // the guard's watch is cancelled
  assert.equal(thrown.removed, 1);       // and the player removed, once
  assert.equal(thrown.ready, false);
  assert.equal(thrown.lines.filter(line => line.startsWith('OpenWillow movie setup failed')).length, 1);
  passed++;
  console.log('PASS A movie call that throws during setup gives the movie up (guard cancelled, player removed) and resolves false');

  const late = await guardCancel('resolve');
  assert.equal(late.result, false);
  assert.equal(late.removed, 0);           // the caller removed it already; no second remove
  assert.equal(late.logs.length, 0);       // and no second log
  assert.equal(late.observers.length, 0);  // no watch on a removed player
  const rejected = await guardCancel('reject');
  assert.equal(rejected.result, false);
  assert.equal(rejected.removed, 0);
  assert.equal(rejected.logs.length, 0);
  const after = await guardCancel('success');
  assert.equal(after.result, true);
  assert.equal(after.observers.length, 1);
  assert.equal(after.observers[0].live, false); // cancel after success disconnects the watch
  passed++;
  console.log('PASS The guard: a load cancelled before it settles does nothing on a late resolve or reject; cancel disconnects its watch');

  const hung = await hangingLoad();
  assert.equal(hung.running, false);   // the page falls back to HTML
  assert.equal(hung.removed, 1);       // no empty player stays up, removed once
  assert.equal(hung.cancels, 1);       // and the guard's load is cancelled
  assert.equal(hung.lines.filter(line => line.startsWith('OpenWillow movie timed out')).length, 1);
  assert.ok(movie.LOAD_TIMEOUT_MS >= 1000 && movie.LOAD_TIMEOUT_MS <= 15000);
  passed++;
  console.log('PASS A movie load that never settles times out once, cancels the guard, removes the player and runs the HTML page');

  const wins = await guardWins();
  assert.equal(wins.running, true);
  assert.equal(wins.pendingLoadTimers, 0);
  passed++;
  console.log('PASS When the movie loads first, the timeout timer is cleared');
  console.log(`${passed}/${passed} vending movie checks passed (synthetic rows; the movie layout is UNVERIFIED)`);
})().catch(error => { console.error(error); process.exit(1); });
