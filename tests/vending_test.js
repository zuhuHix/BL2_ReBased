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
// Checks that wait for the page's movie start; they run after the others.
const laterChecks = [];
function checkLater(name, test) { laterChecks.push([name, test]); }

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

checkLater('Without the shop movie the HTML page runs, and the page says once which one ran', async () => {
  const page = loadPage({movie: false});
  await page.started;
  assert.deepEqual(page.lines().filter(line => line.startsWith('OWVENDPAGEMODE')), ['OWVENDPAGEMODE html']);
  assert.equal(page.bodyMovie(), false);
});

checkLater('With the shop movie the page draws into it, Left goes to the backpack tab, and losing the movie falls back', async () => {
  const page = loadPage({movie: true});
  await page.started;
  assert.deepEqual(page.lines().filter(line => line.startsWith('OWVENDPAGEMODE')), ['OWVENDPAGEMODE movie']);
  assert.equal(page.bodyMovie(), true);
  page.push(state({opening: 1}));
  assert.ok(page.drawn() > 0);
  page.key('ArrowLeft');                       // the movie's tabs: backpack, stock, buy-back
  assert.deepEqual(page.selection(), {column: 'backpack', index: 0});
  page.lose();
  assert.equal(page.bodyMovie(), false);
  assert.equal(page.lines().filter(line => line.startsWith('OWVENDPAGEMODE')).pop(), 'OWVENDPAGEMODE html');
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

check('A new opening starts the selection over; a state within one opening keeps it', () => {
  const moved = {column: 'backpack', index: 3};
  const first = state({opening: 1});
  assert.deepEqual(model.selectionAfterState(moved, null, first), {column: 'shop', index: 0});
  assert.deepEqual(model.selectionAfterState(moved, first, state({opening: 1, serial: 5})), moved);
  assert.deepEqual(model.selectionAfterState(moved, first, state({opening: 2})), {column: 'shop', index: 0});   // the same machine again
  const other = state({opening: 2, machine: {id: 'machine_b', kind: 'items', title: 'Other'}});
  assert.deepEqual(model.selectionAfterState(moved, state({opening: 2}), other), {column: 'shop', index: 0});
  assert.equal(model.isNewOpening(first, state({opening: 1, serial: 5})), false);
});

check('The opening count is checked when present', () => {
  assert.equal(model.validateState(state({opening: 2})).ok, true);
  assert.equal(model.validateState(state()).ok, true);
  assert.equal(model.validateState(state({opening: -1})).ok, false);
  assert.equal(model.validateState(state({opening: '2'})).ok, false);
});

check('Refusals show the stock labels; any other message is left alone', () => {
  assert.equal(model.refusalLabel('Insufficient funds'), 'INSUFFICIENT FUNDS');
  for (const reason of ['Ammo full', 'Health full', 'Backpack full']) assert.equal(model.refusalLabel(reason), 'FULL');
  for (const other of ['', 'Try again', 'Equipped items cannot be sold', 'constructor']) assert.equal(model.refusalLabel(other), other);
});

// The real vending.js, with stub DOM elements: what a key press and a host push actually do to the page.
function loadPage(options = {movie: false}) {
  const elements = {}, lines = [];
  let bodyMovie = false, drawn = 0;
  const body = {classList: {toggle: (_name, on) => { bodyMovie = on; }}};
  const element = () => ({textContent: '', hidden: false, children: [], classList: {add() {}}, setAttribute() {}, addEventListener() {},
    append(...c) { this.children.push(...c); }, appendChild(c) { this.children.push(c); }, replaceChildren(...c) { this.children = c; }});
  let onKey = null;
  // A short answer timeout, so a test's leftover wait does not hold the run open (the page's own is 3 s).
  // Request numbers start at 0 here (options.requestBase) so the tests can name them; the page's own base is random.
  const window = {OwVendingModel: model, RufflePlayer: {}, owVendingPendingTimeoutMs: options.pendingMs || 200,
    owVendingRequestBase: options.requestBase === undefined ? 0 : options.requestBase};
  let started = null, startMovie = null;
  const played = [];
  const movieView = require('../tools/hud_overlay/vending_movie.js');
  // options.manual: the movie start settles only when the test calls page.startMovie(running).
  window.owVendingMovie = {TAB_ORDER: movieView.TAB_ORDER, feedbackFor: movieView.feedbackFor, groupLists: movieView.groupLists,
    render: () => { drawn++; }, feedback: shown => { if (shown) played.push(shown); },
    start: () => (started = options.manual ? new Promise(resolve => { startMovie = resolve; }) : Promise.resolve(options.movie))};
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/vending.js'), 'utf8'), {
    window, setTimeout, clearTimeout, console: {log: line => lines.push(line)}, fetch: () => Promise.resolve({ok: false}),
    document: {body, getElementById: id => (elements[id] = elements[id] || element()), createElement: element,
      addEventListener: (_name, handler) => { onKey = handler; }}});
  return {
    started: started.then(() => new Promise(resolve => setTimeout(resolve, 0))),
    startMovie: running => { startMovie(running); return started.then(() => new Promise(resolve => setTimeout(resolve, 0))); },
    lines: () => lines,
    bodyMovie: () => bodyMovie,
    drawn: () => drawn,
    played: () => played.map(shown => shown.frame + (shown.label ? ' ' + shown.label : '')),
    lose: () => window.owVendingMovieLost(),
    push: raw => window.owVending(raw),
    key: key => onKey({key, preventDefault() {}}),
    message: () => elements.message.textContent,
    selection: () => { window.owVendingReport(); return JSON.parse(lines[lines.length - 1].slice('OWVENDPAGE '.length)).selection; },
    requests: () => lines.filter(line => line.startsWith('OWVEND ')).map(line => JSON.parse(line.slice(7)))};
}
const TOP = {column: 'shop', index: 0};

check('Page: a push within one opening keeps the selection and a refusal the page made', () => {
  const page = loadPage();
  page.push(state({opening: 1}));
  page.key('ArrowDown');                       // s2, which cannot be afforded
  page.key('Enter');
  assert.equal(page.message(), 'INSUFFICIENT FUNDS');
  assert.deepEqual(page.requests(), []);       // the page refuses this one itself
  page.push(state({opening: 1, serial: 5}));   // the host's one-second push
  assert.equal(page.message(), 'INSUFFICIENT FUNDS');
  assert.deepEqual(page.selection(), {column: 'shop', index: 1});
});

check('Page: moving clears a refusal the page made', () => {
  const page = loadPage();
  page.push(state({opening: 1}));
  page.key('ArrowDown');
  page.key('Enter');
  page.key('ArrowUp');
  assert.equal(page.message(), '');
});

check('Page: a reopen resets the selection and the refusal, on the same machine or another', () => {
  for (const machine of [undefined, {id: 'machine_b', kind: 'items', title: 'Other'}]) {
    const page = loadPage();
    page.push(state({opening: 1}));
    page.key('ArrowRight');                    // into the backpack
    page.key('ArrowDown');                     // b2, which is worn
    page.key('Enter');
    assert.equal(page.message(), 'Equipped items cannot be sold');
    page.push(state({opening: 2, serial: 5, ...(machine ? {machine} : {})}));
    assert.deepEqual(page.selection(), TOP);
    assert.equal(page.message(), '');
  }
});

check('Page: a request carries the serial of the state it is pressed on', () => {
  const page = loadPage();
  page.push(state({opening: 1}));
  page.key('Enter');
  page.push(state({opening: 1, serial: 9}));
  page.key('Enter');
  assert.deepEqual(page.requests().map(request => [request.serial, request.action, request.id]), [[4, 'buy', 's1'], [9, 'buy', 's1']]);
});

check('Page: a host refusal shows in the stock label and a late, older state is ignored', () => {
  const page = loadPage();
  page.push(state({opening: 1, serial: 7, message: 'Ammo full'}));
  assert.equal(page.message(), 'FULL');
  page.push(state({opening: 1, serial: 6, message: ''}));
  assert.equal(page.message(), 'FULL');
  page.push(state({opening: 1, serial: 7, message: 'Try again'}));   // an equal serial is a current state: the timer's
  assert.equal(page.message(), 'Try again');
});

check('Page: a bad state is reported, and the next good one clears that', () => {
  const page = loadPage();
  page.push(state({opening: 1}));
  page.push({serial: 'x'});
  assert.match(page.message(), /^Bad state from host/);
  page.push(state({opening: 1, serial: 5}));
  assert.equal(page.message(), '');
});

check('Request line has the serial, action and id', () => {
  const line = model.buildRequest(4, 'buy', 'stock_3');
  assert.equal(line, 'OWVEND {"serial":4,"action":"buy","id":"stock_3"}');
  assert.equal(line.slice(0, 7), 'OWVEND ');
  assert.deepEqual(JSON.parse(line.slice(7)), {serial: 4, action: 'buy', id: 'stock_3'});
  assert.equal(model.buildRequest(9, 'close', undefined), 'OWVEND {"serial":9,"action":"close","id":null}');
  assert.equal(model.buildRequest(4, 'buy', 'stock_3', 12), 'OWVEND {"serial":4,"action":"buy","id":"stock_3","request":12}');
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

// Host order smg a, shotgun b, smg c; the movie groups it a, c, b. The selected row must stay the same item.
const mixed = () => state({opening: 1, shop: [item('a', {card: {type: 'smg'}}), item('b', {card: {type: 'shotgun'}}),
  item('c', {card: {type: 'smg'}})]});

checkLater('Page: when the movie starts after a push, the selection keeps its item (the movie groups the rows)', async () => {
  const page = loadPage({manual: true});
  page.push(mixed());
  page.key('ArrowDown');                       // b, the shotgun, at index 1 on the HTML page
  await page.startMovie(true);
  assert.deepEqual(page.selection(), {column: 'shop', index: 2}); // b is third once grouped
  page.key('Enter');
  assert.deepEqual(page.requests().map(r => r.id), ['b']);
});

checkLater('Page: when the movie is lost, the HTML page keeps the selected item too', async () => {
  const page = loadPage({manual: true});
  await page.startMovie(true);
  page.push(mixed());
  page.key('ArrowDown');                       // c, second once grouped
  page.lose();
  assert.deepEqual(page.selection(), {column: 'shop', index: 2}); // c is third in the host order
  page.key('Enter');
  assert.deepEqual(page.requests().map(r => r.id), ['c']);
});

check('The refusal field is optional and must be text', () => {
  assert.equal(model.validateState(state({refusal: ''})).ok, true);
  assert.equal(model.validateState(state({refusal: 'Insufficient funds'})).ok, true);
  assert.equal(model.validateState(state({refusal: 3})).ok, false);
  assert.equal(model.validateState(state({handled: 7})).ok, true);
  assert.equal(model.validateState(state({handled: -1})).ok, false);
});

check('remapSelection keeps the row by id and clamps when it is gone', () => {
  const before = {shop: [{id: 'a'}, {id: 'b'}, {id: 'c'}], backpack: [], buyback: []};
  assert.deepEqual(model.remapSelection({column: 'shop', index: 1}, before, {shop: [{id: 'a'}, {id: 'c'}, {id: 'b'}]}), {column: 'shop', index: 2});
  assert.deepEqual(model.remapSelection({column: 'shop', index: 2}, before, {shop: [{id: 'a'}]}), {column: 'shop', index: 0});
  assert.deepEqual(model.remapSelection({column: 'backpack', index: 0}, before, {backpack: []}), {column: 'backpack', index: 0});
});

// The host's answer to request n: the state names it as `handled` and carries the action's `refusal` (empty when it went through).
const answer = (n, extra = {}) => state({opening: 1, refusal: '', handled: n, ...extra});
const lastRequest = page => page.requests()[page.requests().length - 1].request;

checkLater('Movie: a refused press plays at once, each time; a push does not replay it; a buy answered plays purchased', async () => {
  const page = loadPage({movie: true});
  await page.started;
  page.push(answer(0));
  page.key('ArrowDown');                       // s2, which cannot be afforded: the page refuses it itself
  page.key('Enter');
  page.push(answer(0, {serial: 5}));           // the host's one-second push
  page.key('Enter');
  assert.deepEqual(page.played(), ['bad INSUFFICIENT FUNDS', 'bad INSUFFICIENT FUNDS']);
  page.key('ArrowUp');                         // s1, affordable
  page.key('Enter');
  assert.deepEqual(page.requests().map(r => r.action), ['buy']);
  assert.equal(lastRequest(page), 1);          // requests are numbered
  page.push(answer(1, {serial: 6, money: 900}));
  assert.equal(page.played().pop(), 'purchased PURCHASED');
  page.push(answer(1, {serial: 7, money: 900}));  // a later push plays nothing
  assert.equal(page.played().length, 3);
});

checkLater('Movie: a sale plays sold; a host refusal plays its label; a restock alone plays nothing', async () => {
  const page = loadPage({movie: true});
  await page.started;
  page.push(answer(0));
  page.key('ArrowLeft');                       // backpack tab
  page.key('Enter');                           // sell b1 (sorted first), not worn
  page.push(answer(1, {serial: 5, money: 1040}));
  assert.deepEqual(page.played(), ['sold SOLD']);
  page.key('Enter');                           // request 2
  page.push(answer(2, {serial: 5, money: 1040, refusal: 'Backpack full', message: 'Backpack full'}));
  assert.equal(page.played().pop(), 'bad FULL');
  page.key('Enter');                           // request 3; a restock moves the serial, but this state answers request 2
  page.push(answer(2, {serial: 6, money: 1040}));
  assert.equal(page.played().length, 2);
});

checkLater('Movie: a sell pressed before the buy is answered is ignored, so the buy gets its own result', async () => {
  const page = loadPage({movie: true});
  await page.started;
  page.push(answer(0));
  page.key('Enter');                           // buy s1
  page.key('ArrowLeft');                       // backpack tab
  page.key('Enter');                           // sell b1 before the buy's answer: ignored
  page.key('Enter');
  assert.deepEqual(page.requests().map(r => r.action), ['buy']);
  assert.equal(page.lines().filter(line => line.startsWith('OWVENDBUSY')).length, 1);
  page.push(answer(1, {serial: 5, money: 900}));
  assert.deepEqual(page.played(), ['purchased PURCHASED']);
  page.key('Enter');                           // now the sale goes
  assert.deepEqual(page.requests().map(r => r.action), ['buy', 'sell']);
});

checkLater('Movie: a late answer to a timed-out buy is not taken for the second buy (Luna r2 repro)', async () => {
  const page = loadPage({movie: true, pendingMs: 30});
  await page.started;
  page.push(answer(0));
  page.key('Enter');                           // buy, request 1
  await new Promise(resolve => setTimeout(resolve, 80));
  assert.equal(page.lines().filter(line => line.startsWith('OWVENDPENDING')).length, 1);
  page.key('Enter');                           // buy again with the same serial, request 2
  assert.deepEqual(page.requests().map(r => [r.action, r.serial, r.request]), [['buy', 4, 1], ['buy', 4, 2]]);
  page.push(answer(1, {serial: 5, money: 900})); // the first buy's late success: not request 2's
  assert.deepEqual(page.played(), []);
  assert.equal(page.lines().filter(line => line.startsWith('OWVENDLATE')).length, 1);
  page.push(answer(2, {serial: 5, money: 900, refusal: 'Try again', message: 'Try again'})); // the host refuses the stale second
  assert.deepEqual(page.played(), ['bad TRY AGAIN']);
});

checkLater('Movie: after a reload, the host still echoing the old page number does not answer the new request', async () => {
  const first = loadPage({movie: true});
  await first.started;
  first.push(answer(0));
  first.key('Enter');                          // request 1 from the first page load
  const reloaded = loadPage({movie: true, requestBase: 5000});
  await reloaded.started;
  reloaded.push(answer(1));                    // the host still names request 1 (no new opening yet)
  reloaded.key('Enter');                       // the reloaded page's request 5001
  assert.equal(lastRequest(reloaded), 5001);
  reloaded.push(answer(1, {serial: 5, money: 900}));  // a state that names the old number: not this request's answer
  assert.deepEqual(reloaded.played(), []);
  reloaded.key('ArrowLeft');
  reloaded.key('Enter');                       // still waiting, so this press is ignored
  assert.deepEqual(reloaded.requests().map(r => r.request), [5001]);
  reloaded.push(answer(5001, {serial: 6, money: 800}));
  assert.deepEqual(reloaded.played(), ['purchased PURCHASED']);
});

check('The page source starts its request numbers at a random base, not at a fixed number or a clock', () => {
  const source = fs.readFileSync(path.join(__dirname, '..', 'tools', 'hud_overlay', 'vending.js'), 'utf8');
  assert.ok(/Math\.random\(\)/.test(source));
  assert.ok(!source.includes('requestCount = 0'));
});

checkLater('Movie: a stock notice is never played as a refusal, and a host without the answer fields plays no host results', async () => {
  const page = loadPage({movie: true});
  await page.started;
  page.push(answer(0, {message: 'No weapons listed'}));
  page.key('Enter');
  page.push(answer(1, {serial: 5, money: 900, message: 'No weapons listed'}));
  assert.deepEqual(page.played(), ['purchased PURCHASED']);
  const old = loadPage({movie: true});
  await old.started;
  old.push(state({opening: 1, message: 'No weapons listed'})); // no refusal or handled field
  old.key('Enter');
  old.push(state({opening: 1, serial: 5, money: 900, message: 'Insufficient funds'}));
  assert.deepEqual(old.played(), []);
  assert.equal(old.lines().filter(line => line.startsWith('OWVENDNOANSWER')).length, 1);
});

checkLater('HTML page: presses are not held back (only the movie waits for answers)', async () => {
  const page = loadPage({movie: false});
  await page.started;
  page.push(state({opening: 1}));
  page.key('Enter');
  page.key('Enter');
  assert.deepEqual(page.requests().map(r => r.action), ['buy', 'buy']);
});

(async () => {
  for (const [name, test] of laterChecks) { await test(); passed++; console.log('PASS ' + name); }
  console.log(`${passed}/${passed} vending checks passed (synthetic items; movie layout and host numbers UNVERIFIED)`);
})().catch(error => { console.error(error); process.exit(1); });
