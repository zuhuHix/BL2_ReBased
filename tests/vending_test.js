// Synthetic checks for the vending page rules (tools/hud_overlay/vending_model.js) and the page glue's syntax.
// All items, names and prices here are invented; no game data is used.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const model = require('../tools/hud_overlay/vending_model.js');

const item = (id, extra = {}) => ({id, name: `Test ${id}`, kind: 'weapon', rarity: 1, level: 5, price: 100,
  affordable: true, featured: false, quantity: 1, ...extra});
const bagItem = (id, extra = {}) => ({id, name: `Bag ${id}`, kind: 'item', rarity: 1, level: 3, sell: 40,
  equipped: false, ...extra});
function state(extra = {}) {
  return {serial: 4, open: true, machine: {id: 'machine_a', kind: 'weapons', title: 'Test Machine'},
    money: 1000, eridium: 2, capacity: 12, used: 2, shop: [item('s1'), item('s2', {affordable: false, price: 900})],
    featured: null, backpack: [bagItem('b1'), bagItem('b2', {equipped: true})], buyback: [],
    resetSeconds: 873, message: '', ...extra};
}

let passed = 0;
function check(name, test) { test(); passed++; console.log('PASS ' + name); }

check('Price text groups thousands and never goes below zero', () => {
  assert.equal(model.formatPrice(0), '$0');
  assert.equal(model.formatPrice(999), '$999');
  assert.equal(model.formatPrice(1234), '$1,234');
  assert.equal(model.formatPrice(1234567), '$1,234,567');
  assert.equal(model.formatPrice(-5), '$0');
});

check('Restock countdown reads mm:ss and clamps at zero', () => {
  assert.equal(model.formatCountdown(873), '14:33');
  assert.equal(model.formatCountdown(1200), '20:00');
  assert.equal(model.formatCountdown(0), '00:00');
  assert.equal(model.formatCountdown(-5), '00:00');
});

check('A well-formed state validates and comes back unchanged', () => {
  const result = model.validateState(state());
  assert.equal(result.ok, true);
  assert.equal(result.state.serial, 4);
});

check('Validation rejects malformed state with a reason', () => {
  const cases = [
    [null, 'not an object'],
    [state({serial: -1}), 'serial'],
    [state({open: 'yes'}), 'open'],
    [state({machine: {id: 'm', kind: 'casino', title: 'x'}}), 'machine.kind'],
    [state({money: -1}), 'money'],
    [state({used: 13}), 'used is larger'],
    [state({shop: [item('s9', {kind: 'vehicle'})]}), 'shop[0].kind'],
    [state({shop: [item('s9', {rarity: 6})]}), 'shop[0].rarity'],
    [state({shop: [item('s9', {price: 1.5})]}), 'shop[0].price'],
    [state({shop: [{...item('s9'), affordable: undefined}]}), 'shop[0].affordable'],
    [state({backpack: [bagItem('b9', {equipped: 'no'})]}), 'backpack[0].equipped'],
    [state({buyback: [{...item('r1'), id: ''}]}), 'buyback[0].id'],
    [state({featured: {...item('f1'), name: ''}}), 'featured.name'],
    [state({message: 4}), 'message'],
  ];
  for (const [value, reason] of cases) {
    const result = model.validateState(value);
    assert.equal(result.ok, false, `accepted ${reason}`);
    assert.ok(result.error.includes(reason), `error "${result.error}" should mention ${reason}`);
  }
});

check('Featured item is listed first, and listed once', () => {
  const lists = model.buildLists(state({featured: item('f1', {price: 555, affordable: true}),
    shop: [item('s1'), item('f1', {price: 555, affordable: true})]}));
  assert.deepEqual(lists.shop.map(row => row.id), ['f1', 's1']);
  assert.equal(lists.shop[0].featured, true);
  assert.equal(lists.shop[0].price, 555);
});

check('Backpack sorts by rarity high to low, then name', () => {
  const lists = model.buildLists(state({backpack: [bagItem('c', {rarity: 2, sell: 5}), bagItem('a', {rarity: 2}),
    bagItem('z', {rarity: 5})], shop: []}));
  assert.deepEqual(lists.backpack.map(row => row.id), ['z', 'a', 'c']);
  assert.equal(lists.backpack[0].price, 40, 'backpack rows show the sell price');
});

check('Buy-back list is capped', () => {
  const buyback = Array.from({length: 12}, (_, i) => item('r' + i, {affordable: true}));
  assert.equal(model.buildLists(state({buyback})).buyback.length, model.BUYBACK_LIMIT);
});

check('An unaffordable buy is not sent', () => {
  const lists = model.buildLists(state());
  const result = model.actionFor({column: 'shop', index: 1}, lists, state());
  assert.deepEqual(result, {ok: false, message: 'Insufficient funds'});
});

check('A buy with a full backpack is not sent', () => {
  const full = state({used: 12, capacity: 12});
  const result = model.actionFor({column: 'shop', index: 0}, model.buildLists(full), full);
  assert.deepEqual(result, {ok: false, message: 'Backpack full'});
});

check('An affordable buy is sent as buy with the item id', () => {
  const current = state();
  assert.deepEqual(model.actionFor({column: 'shop', index: 0}, model.buildLists(current), current),
    {ok: true, action: 'buy', id: 's1'});
});

check('An equipped item is never sold', () => {
  const lists = model.buildLists(state());
  assert.deepEqual(model.actionFor({column: 'backpack', index: 1}, lists, state()),
    {ok: false, message: 'Equipped items cannot be sold'});
  assert.deepEqual(model.actionFor({column: 'backpack', index: 0}, lists, state()), {ok: true, action: 'sell', id: 'b1'});
});

check('An item the host marks unsellable (a lent mission weapon) is never sold', () => {
  const current = state({backpack: [bagItem('m1', {sellable: false}), bagItem('b1')]});
  const lists = model.buildLists(current);
  const index = lists.backpack.findIndex(row => row.id === 'm1');
  assert.deepEqual(model.actionFor({column: 'backpack', index}, lists, current),
    {ok: false, message: 'Mission items cannot be sold'});
  const other = lists.backpack.findIndex(row => row.id === 'b1');
  assert.deepEqual(model.actionFor({column: 'backpack', index: other}, lists, current), {ok: true, action: 'sell', id: 'b1'});
});

check('A missing or failed movie removes the Ruffle player instead of leaving its error panel', () => {
  const source = fs.readFileSync(path.join(__dirname, '..', 'tools', 'hud_overlay', 'vending.js'), 'utf8');
  assert.ok(source.includes("method: 'HEAD'"));          // checks the movie exists before creating a player
  assert.ok(source.includes("querySelector('#panic')"));  // and drops the player when Ruffle panics
  assert.ok(source.includes('window.owPlayer.remove()'));
});

check('The page tells the host when it can take input and keeps no local clock', () => {
  const source = fs.readFileSync(path.join(__dirname, '..', 'tools', 'hud_overlay', 'vending.js'), 'utf8');
  assert.ok(source.includes("console.log('OWVENDREADY')"));
  assert.ok(!source.includes('Date.now'));
});

check('A row the host gives no valid price is never bought, even when marked affordable', () => {
  const current = state({shop: [item('s9', {priceValid: false, affordable: true, price: 0})]});
  const lists = model.buildLists(current);
  const index = lists.shop.findIndex(row => row.id === 's9');
  assert.deepEqual(model.actionFor({column: 'shop', index}, lists, current), {ok: false, message: 'Not for sale'});
});

check('Buy-back needs enough money', () => {
  const current = state({buyback: [item('r1', {affordable: false})]});
  const lists = model.buildLists(current);
  assert.deepEqual(model.actionFor({column: 'buyback', index: 0}, lists, current),
    {ok: false, message: 'Insufficient funds'});
  current.buyback[0].affordable = true;
  assert.deepEqual(model.actionFor({column: 'buyback', index: 0}, model.buildLists(current), current),
    {ok: true, action: 'buyback', id: 'r1'});
});

check('Up and Down wrap inside a list', () => {
  const counts = {shop: 3, backpack: 2, buyback: 0};
  assert.deepEqual(model.moveSelection({column: 'shop', index: 0}, 'up', counts), {column: 'shop', index: 2});
  assert.deepEqual(model.moveSelection({column: 'shop', index: 2}, 'down', counts), {column: 'shop', index: 0});
  assert.deepEqual(model.moveSelection({column: 'buyback', index: 0}, 'down', counts), {column: 'buyback', index: 0});
});

check('Tab cycles the lists and Left and Right stop at the ends', () => {
  const counts = {shop: 3, backpack: 2, buyback: 1};
  let sel = {column: 'shop', index: 1};
  sel = model.moveSelection(sel, 'tab', counts); assert.equal(sel.column, 'backpack');
  sel = model.moveSelection(sel, 'tab', counts); assert.equal(sel.column, 'buyback');
  sel = model.moveSelection(sel, 'tab', counts); assert.equal(sel.column, 'shop');
  assert.deepEqual(model.moveSelection({column: 'shop', index: 0}, 'left', counts), {column: 'shop', index: 0});
  assert.deepEqual(model.moveSelection({column: 'buyback', index: 0}, 'right', counts), {column: 'buyback', index: 0});
});

check('Switching to a shorter list clamps the index', () => {
  const counts = {shop: 4, backpack: 2, buyback: 0};
  assert.deepEqual(model.moveSelection({column: 'shop', index: 3}, 'right', counts), {column: 'backpack', index: 1});
  assert.deepEqual(model.moveSelection({column: 'shop', index: 3}, 'tab', {shop: 4, backpack: 2, buyback: 0}),
    {column: 'backpack', index: 1});
  assert.equal(model.clampIndex(5, 0), 0);
});

check('Key names map to page commands; unknown keys do nothing', () => {
  assert.equal(model.keyCommand('ArrowUp'), 'up');
  assert.equal(model.keyCommand('S'), 'down');
  assert.equal(model.keyCommand('Tab'), 'tab');
  assert.equal(model.keyCommand('E'), 'act');
  assert.equal(model.keyCommand('Escape'), 'close');
  assert.equal(model.keyCommand('x'), null);
  assert.equal(model.keyCommand('constructor'), null);
});

check('Request line has the serial, action and id', () => {
  const line = model.buildRequest(4, 'buy', 'stock_3');
  assert.equal(line, 'OWVEND {"serial":4,"action":"buy","id":"stock_3"}');
  assert.equal(line.slice(0, 7), 'OWVEND ');
  assert.deepEqual(JSON.parse(line.slice(7)), {serial: 4, action: 'buy', id: 'stock_3'});
  assert.equal(model.buildRequest(9, 'close', undefined), 'OWVEND {"serial":9,"action":"close","id":null}');
  assert.throws(() => model.buildRequest(1, 'steal', 'x'), RangeError);
  assert.throws(() => model.buildRequest(1, 'sell', ''), RangeError);
});

check('Every row kind the host sends is accepted, and an unknown kind is not', () => {
  for (const kind of ['weapon', 'item', 'ammo', 'health', 'shield', 'grenade_mod', 'class_mod']) {
    assert.equal(model.validateState(state({shop: [item('k1', {kind})]})).ok, true, `kind ${kind} should validate`);
  }
  assert.equal(model.validateState(state({shop: [item('k2', {kind: 'relic'})]})).ok, false);
});

check('An ammo or health buy goes through a full backpack; a shield or grenade mod buy does not', () => {
  const full = state({used: 12, capacity: 12, shop: [item('a1', {kind: 'ammo', price: 10}), item('h1', {kind: 'health', price: 10}),
    item('g1', {kind: 'shield', price: 10}), item('n1', {kind: 'grenade_mod', price: 10})]});
  const lists = model.buildLists(full);
  assert.deepEqual(model.actionFor({column: 'shop', index: 0}, lists, full), {ok: true, action: 'buy', id: 'a1'});
  assert.deepEqual(model.actionFor({column: 'shop', index: 1}, lists, full), {ok: true, action: 'buy', id: 'h1'});
  assert.deepEqual(model.actionFor({column: 'shop', index: 2}, lists, full), {ok: false, message: 'Backpack full'});
  assert.deepEqual(model.actionFor({column: 'shop', index: 3}, lists, full), {ok: false, message: 'Backpack full'});
});

check('A shield in the backpack is sold unless it is worn', () => {
  const current = state({backpack: [bagItem('sh1', {kind: 'shield', sell: 47, sellable: true}),
    bagItem('sh2', {kind: 'shield', sell: 47, equipped: true})]});
  const lists = model.buildLists(current);
  assert.deepEqual(model.actionFor({column: 'backpack', index: 0}, lists, current), {ok: true, action: 'sell', id: 'sh1'});
  assert.deepEqual(model.actionFor({column: 'backpack', index: 1}, lists, current),
    {ok: false, message: 'Equipped items cannot be sold'});
});

check('Ammo and health state objects are checked when present, and a health float is allowed', () => {
  const good = state({ammo: {pistol: {current: 40, max: 100}}, health: {current: 136.97, max: 400}});
  assert.equal(model.validateState(good).ok, true);
  assert.equal(model.validateState(state({ammo: {pistol: {current: 1.5, max: 100}}})).ok, false);
  assert.equal(model.validateState(state({ammo: 'many'})).ok, false);
  assert.equal(model.validateState(state({health: {current: -1, max: 400}})).ok, false);
  assert.equal(model.validateState(state({health: {current: 'high', max: 400}})).ok, false);
});

check('The pool note shows the ammo pool for an ammo row and the health for a health row only', () => {
  const current = state({ammo: {pistol: {current: 40, max: 100}}, health: {current: 136.97, max: 400}});
  assert.equal(model.poolNote(current, {kind: 'ammo', ammoType: 'pistol'}), '40/100 rounds in the pool');
  assert.equal(model.poolNote(current, {kind: 'health'}), 'Health 136/400');
  assert.equal(model.poolNote(current, {kind: 'weapon'}), '');
  assert.equal(model.poolNote(current, {kind: 'ammo', ammoType: 'grenade'}), '');
});

check('Page script parses and stays small', () => {
  const dir = path.join(__dirname, '../tools/hud_overlay');
  const source = fs.readFileSync(path.join(dir, 'vending.js'), 'utf8');
  assert.ok(source.split('\n').length < 600);
  new vm.Script(source, {filename: 'vending.js'});
});

console.log(`${passed}/${passed} vending checks passed (synthetic items; movie layout and host numbers UNVERIFIED)`);
