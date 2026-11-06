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
  page.owPickupCard.apply(null);
  assert.strictEqual(made.length, 0); // no frame before the first card
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
check('Level requirement as SetItemCardEx sets it: shown above level 1, not met above the player level', () => {
  const levelCall = (itemLevel, playerLevel, extra = {}) => {
    const calls = [];
    const writer = hud.owItemCard.createCardWriter({call:(p, m, ...a) => { calls.push([m, ...a]); }, get:() => 0, set(){}});
    writer.configureCard('c', {id:'a', name:'Gun', level:itemLevel, ...extra}, null, 'default', false, playerLevel);
    return calls.find(c => c[0] === 'SetLevelRequirement').slice(1);
  };
  assert.deepStrictEqual(levelCall(34, 36), [true, true, false, 'LEVEL REQUIREMENT: 34']);
  assert.deepStrictEqual(levelCall(36, 36), [true, true, false, 'LEVEL REQUIREMENT: 36']);
  assert.deepStrictEqual(levelCall(40, 36), [true, false, false, 'LEVEL REQUIREMENT: 40']);
  assert.deepStrictEqual(levelCall(1, 36), [false, true, false, '']);
  assert.deepStrictEqual(levelCall(30, 36, {levelKnown:false}), [false, true, false, '']);
  // Level 0, a missing, null or non-numeric level: no strip, never shown as unmet (PR #96 review).
  assert.deepStrictEqual(levelCall(0, 36), [false, true, false, '']);
  assert.deepStrictEqual(levelCall(undefined, 36), [false, true, false, '']);
  assert.deepStrictEqual(levelCall(null, 36), [false, true, false, '']);
  assert.deepStrictEqual(levelCall('abc', 36), [false, true, false, '']);
  assert.deepStrictEqual(levelCall('', 36), [false, true, false, '']);
});

check('A frame whose card clip never appears stops polling and logs why', () => {
  let now = 0, timers = 0;
  const logs = [];
  const fakePlayer = {style:{}, ow:() => null};
  const ctx = load('/pickup_card.html', {
    console:{log:line => logs.push(line)},
    Date:{now:() => now},
    setTimeout:(f) => { timers++; now += 1000; if (timers < 100) f(); },
    owMovieGuard:{FILES:{}, load:() => ({then:ok => ok(true)})},
    RufflePlayer:{newest:() => ({createPlayer:() => fakePlayer})},
    document:{body:{appendChild(){}}}});
  ctx.owPickupCard.apply({id:'a', x:0.5, y:0.5, level:1, item:{id:'a'}, compare:null});
  assert.ok(timers > 0 && timers < 100, `polled ${timers} times`);
  assert.ok(logs.some(line => /pickup card movie failed: no .*card1 after 15 s/.test(line)), logs.join(' | '));
});

// The guard itself, with a fake fetch, player and observer: what a missing or broken movie does.
function guardContext(answer, panicAfterLoad = false) {
  const logs = [], requests = [], observers = [];
  const panic = {shown:panicAfterLoad, classList:{contains:name => name === 'hidden' ? !panic.shown : false}};
  const player = {removed:0, loads:0, panic, remove() { this.removed++; }, shadowRoot:{querySelector:() => panic},
    ruffle() { return {load:() => { player.loads++; return Promise.resolve(); }}; }};
  // The abort timeout is held back: a test fires it by hand. Retry waits run at once.
  const aborts = [];
  const context = vm.createContext({console:{log:line => logs.push(line)}, Promise, clearTimeout() {},
    setTimeout:(f, ms) => { if (ms >= 3000) aborts.push(f); else setImmediate(f); return 0; },
    AbortController:class { constructor() { this.signal = {}; } abort() { if (this.signal.onabort) this.signal.onabort(); } },
    MutationObserver:class { constructor(callback) { this.callback = callback; this.live = false; observers.push(this); }
      observe() { this.live = true; } disconnect() { this.live = false; } },
    fetch:(url, options) => { requests.push([url, options.method]); return answer(url, requests.length, options.signal); }});
  context.window = context;
  vm.runInContext(fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/movie_guard.js'), 'utf8'), context);
  return {context, player, logs, requests, observers, aborts};
}
const OPTIONS = {url:'UI_Inworld/harness.swf', base:'UI_Inworld/'};
const reply = status => Promise.resolve({status});
const FILES = ['UI_Inworld/Weapon_Card.swf'];
(async () => {
  {
    const g = guardContext(() => reply(404));
    assert.strictEqual(await g.context.owMovieGuard.load(g.player, OPTIONS, 'pickup card', FILES), false);
    assert.strictEqual(g.player.loads, 0, 'Ruffle must not be asked to load a missing movie: that is what draws the panel');
    assert.strictEqual(g.player.removed, 1);
    assert.ok(g.logs.some(line => /OpenWillow movie missing .*harness\.swf.*pickup card/.test(line)), g.logs.join(' | '));
    assert.strictEqual(g.requests.length, 2, 'a 404 is an answer: not asked again');
    assert.deepStrictEqual(g.requests[0], ['UI_Inworld/harness.swf', 'HEAD']);
    passed++; console.log('PASS A missing movie (404) is not loaded: the player is removed and the log names the file');
  }
  {
    const g = guardContext(url => reply(url === FILES[0] ? 404 : 200));
    assert.strictEqual(await g.context.owMovieGuard.load(g.player, OPTIONS, 'pickup card', FILES), false);
    assert.strictEqual(g.player.loads, 0);
    passed++; console.log('PASS A harness whose real movie is missing is not loaded either');
  }
  {
    // A hiccup: the connection fails and the server errors, then it answers. The movie loads.
    const g = guardContext((url, count) => count === 1 ? Promise.reject(new Error('reset')) : reply(count === 2 ? 503 : 200));
    assert.strictEqual(await g.context.owMovieGuard.load(g.player, OPTIONS, 'pickup card'), true);
    assert.strictEqual(g.requests.length, 3);
    assert.strictEqual(g.player.loads, 1);
    assert.strictEqual(g.player.removed, 0);
    passed++; console.log('PASS A failed request or a server error is asked again, and the movie then loads');
  }
  {
    // Never an answer: bounded, and not called missing.
    const g = guardContext(() => Promise.reject(new Error('down')));
    assert.strictEqual(await g.context.owMovieGuard.load(g.player, OPTIONS, 'pickup card'), false);
    assert.strictEqual(g.requests.length, 4);
    assert.ok(g.logs.some(line => /OpenWillow movie unavailable/.test(line)), g.logs.join(' | '));
    passed++; console.log('PASS No answer at all gives up after a few tries and says unavailable, not missing');
  }
  {
    // A request that never answers: aborted by its timeout, counted as a try, and the load still ends.
    const hung = (url, count, signal) => new Promise((resolve, reject) => { signal.onabort = () => reject(new Error('aborted')); });
    const g = guardContext(hung);
    let done = null;
    g.context.owMovieGuard.load(g.player, OPTIONS, 'pickup card').then(found => { done = found; });
    for (let round = 0; round < 40 && done === null; round++) {
      for (const fire of g.aborts.splice(0)) fire();
      await new Promise(resolve => setImmediate(resolve));
    }
    assert.strictEqual(done, false);
    assert.strictEqual(g.requests.length, 4);
    assert.strictEqual(g.player.removed, 1);
    assert.ok(g.logs.some(line => /OpenWillow movie unavailable/.test(line)), g.logs.join(' | '));
    passed++; console.log('PASS A request that hangs is aborted and retried, and the load ends without a player');
  }
  {
    const g = guardContext(() => reply(200));
    let lost = 0;
    assert.strictEqual(await g.context.owMovieGuard.load(g.player, OPTIONS, 'pickup card', [], () => lost++), true);
    assert.strictEqual(g.player.removed, 0);
    assert.ok(g.observers[0].live, 'the watch has no time limit: it lasts as long as the player');
    g.observers[0].callback(); // a mutation with no panel: nothing happens
    assert.strictEqual(g.player.removed, 0);
    g.player.panic.shown = true; // however much later
    g.observers[0].callback();
    assert.strictEqual(g.player.removed, 1);
    assert.strictEqual(lost, 1);
    assert.ok(!g.observers[0].live);
    passed++; console.log('PASS A panel shown any time later removes the player and tells the page');
  }
  {
    const g = guardContext(() => reply(200), true);
    let lost = 0;
    assert.strictEqual(await g.context.owMovieGuard.load(g.player, OPTIONS, 'pickup card', [], () => lost++), false);
    assert.strictEqual(g.player.removed, 1);
    assert.strictEqual(lost, 0, 'the false is the report: onLost as well would stop the page twice');
    passed++; console.log('PASS A panel already up when the load resolves counts as not loaded, reported once');
  }
  {
    const g = guardContext(() => reply(200));
    assert.strictEqual(await g.context.owMovieGuard.load(g.player, OPTIONS, 'pickup card'), true);
    assert.strictEqual(g.player.loads, 1);
    assert.strictEqual(g.player.removed, 0);
    passed++; console.log('PASS A present movie loads and the player stays');
  }
  {
    // The card page: no movie, no card; a later card asks again, so a file that arrives (or a server back) restores it.
    let created = 0, now = 1000;
    const logs = [];
    const ctx = load('/pickup_card.html', {console:{log:line => logs.push(line)}, Promise, Date:{now:() => now},
      owMovieGuard:{FILES:{pickupCard:[]}, load:() => Promise.resolve(false)},
      RufflePlayer:{newest:() => ({createPlayer:() => { created++; return {style:{}, ow:() => null}; }})},
      document:{body:{appendChild(){}}}});
    const card = {id:'a', x:0.5, y:0.5, level:1, item:{id:'a'}, compare:null};
    ctx.owPickupCard.apply(card);
    await Promise.resolve(); await Promise.resolve();
    ctx.owPickupCard.apply(card);
    ctx.owPickupCard.apply(null);
    assert.strictEqual(created, 1, 'not asked again at once');
    assert.ok(logs.some(line => line === 'OpenWillow pickup card off: no movie'), logs.join(' | '));
    now += 31000;
    ctx.owPickupCard.apply(card);
    assert.strictEqual(created, 2, 'asked again later');
    passed++; console.log('PASS Without its movie the card page shows nothing, and a later card asks again');
  }
  {
    // A page script served without a route is a 404 that takes the page down (round 21 hit this with skill_nav.js).
    const serve = fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/serve.py'), 'utf8');
    assert.ok(serve.includes("'/movie_guard.js': 'movie_guard.js'"));
    for (const page of ['index', 'inventory', 'map', 'missions', 'skills', 'pickup_card'])
      assert.ok(fs.readFileSync(path.join(__dirname, `../tools/hud_overlay/${page}.html`), 'utf8').includes('<script src="movie_guard.js"></script>'), page);
    const hud = fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/index.html'), 'utf8');
    assert.ok(hud.includes("console.log('OpenWillow HUD movie lost')"), 'the host goes back to the canvas HUD on this line (hud_movie_loss_test.js runs that page)');
    const host = fs.readFileSync(path.join(__dirname, '../host/ue5/OpenWillow/Source/OpenWillow/OpenWillowMayaHUD.cpp'), 'utf8');
    assert.ok(host.includes('TEXT("OpenWillow HUD movie lost")'));
    passed++; console.log('PASS Every page that loads a movie has the guard, serve.py routes it, and the HUD and host agree on the lost line');
  }
  console.log(`${passed}/${passed} pickup card checks passed (synthetic items; card size, placement and timing UNVERIFIED)`);
})().catch(error => { console.error(error); process.exit(1); });
