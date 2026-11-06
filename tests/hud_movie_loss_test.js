// The real index.html script with a fake player, guard and clock: what the HUD page does when its movie is missing or lost.
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const html = fs.readFileSync(path.join(__dirname, '../tools/hud_overlay/index.html'), 'utf8');
const script = html.slice(html.lastIndexOf('<script>') + '<script>'.length, html.lastIndexOf('</script>'));

// The page's other helpers (action skill, minimap, warp...) do not matter here: anything called on this does nothing and iterates empty.
const anything = new Proxy(function () {}, {
  get:(target, key) => key === Symbol.iterator ? [][Symbol.iterator].bind([]) : key === Symbol.toPrimitive ? () => '' : anything,
  apply:() => anything,
});

// found: what the guard's load resolves to. Returns the page's handles.
function page(found) {
  const logs = [], intervals = new Map();
  let nextInterval = 1, onLost = null, resolveHead = () => {};
  const frames = {_totalframes:1, _framesloaded:1, _name:'p1'};
  const player = {style:{}, ow:(target, op, member) => (op === 'get' ? frames[member] : undefined)};
  const context = vm.createContext({
    console:{log:line => logs.push(String(line))}, Promise, Object, Math, Number, String, JSON, Array, Map, Set,
    performance:{now:() => 0}, location:{search:''},
    setTimeout:() => 0, clearTimeout() {},
    setInterval:callback => { intervals.set(nextInterval, callback); return nextInterval++; },
    clearInterval:id => intervals.delete(id),
    document:{body:{appendChild() {}}},
    // hud_fields.js's page fetch is held back, so a test can lose the movie between the ready frame and the ready line.
    fetch:url => (url === 'hud_fields.js'
      ? new Promise(resolve => { resolveHead = () => resolve({headers:{get:() => 'pages'}}); })
      : Promise.resolve({ok:false, json:() => ({})})),
    RufflePlayer:{newest:() => ({createPlayer:() => player})},
    owActionSkill:anything, owHud3d:anything, owHudWarp:anything, owMinimap:anything, owPickupCard:anything,
    owMovieGuard:{FILES:{hud:[]}, load:(p, options, label, requires, lost) => { onLost = lost; return Promise.resolve(found); }},
    owHudFields:new Proxy({readyLine:() => 'READY LINE', textCall:() => [], weaponCalls:() => []}, {get:(t, k) => t[k] || (() => '')}),
  });
  context.window = context;
  vm.runInContext(script, context);
  const tick = () => { for (const callback of [...intervals.values()]) callback(); };
  return {logs, intervals, tick, lose:() => onLost(), releaseHead:() => resolveHead()};
}
const settle = () => new Promise(resolve => setImmediate(resolve));
const LOST = 'OpenWillow HUD movie lost';

(async () => {
  let passed = 0;
  const check = (name, ok) => { assert.ok(ok, name); passed++; console.log('PASS ' + name); };
  {
    const p = page(false);
    await settle();
    check('A missing movie stops the boot timer and tells the host', p.intervals.size === 0 && p.logs.includes(LOST));
    check('...and the page never reports ready', !p.logs.includes('READY LINE'));
  }
  {
    const p = page(true);
    await settle();
    p.tick(); // the ready frame: the page has asked for hud_fields.js and has not yet logged ready
    await settle();
    assert.ok(!p.logs.includes('READY LINE'));
    p.lose(); // the panel, in the gap
    p.releaseHead();
    await settle();
    check('A movie lost between the ready frame and the ready line never reports ready', p.logs.includes(LOST) && !p.logs.includes('READY LINE'));
  }
  {
    const p = page(true);
    await settle();
    p.tick();
    p.releaseHead();
    await settle();
    check('A movie that stays reports ready', p.logs.includes('READY LINE') && !p.logs.includes(LOST));
    p.lose();
    await settle();
    check('A movie lost after ready tells the host once', p.logs.filter(line => line === LOST).length === 1);
    p.lose();
    check('...and a second report changes nothing', p.logs.filter(line => line === LOST).length === 1);
  }
  console.log(`${passed}/${passed} HUD movie loss checks passed (real index.html script, synthetic player; in-engine UNVERIFIED)`);
})().catch(error => { console.error(error); process.exit(1); });
