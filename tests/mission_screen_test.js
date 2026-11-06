// Synthetic tests for the mission screen page (invented text, no game data, no browser): the real mission_screen.js and its model run against a stub movie, DOM and guard.
// Run: node tests/mission_screen_test.js
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const OVERLAY = path.join(__dirname, '../tools/hud_overlay');
const mission = {id: 'm', name: 'Name <1>', description: 'Plain', turnInDescription: 'Done', plot: false, xp: 7};
const state = page => ({page, log: {missions: [mission]}});

// Runs the page with the guard answering `guard`. Timers run only when flush() says so, so a test can look between polls.
async function page({guard = true, strings = null, search = ''} = {}) {
  const calls = [], lines = [], elements = {}, timers = [];
  const movie = {frame: 1, clips: new Set(['_level1']), lost: null};
  const element = id => elements[id] ??= {id, hidden: id === 'panel', children: [], remove() {}, appendChild(child) { this.children.push(child); },
    replaceChildren(...children) { this.children = children; }};
  const player = {
    remove() { player.removed = true; },
    ow(target, op, a, b) {
      calls.push([target, op, a, b]);
      if (op === 'get' && a === '_name') return movie.clips.has(target) ? 'clip' : undefined;
      if (op === 'get' && a === '_currentframe') return movie.frame;
      if (op === 'get' && a === 'scroll') return 3;
      if (op === 'gotoAndPlay') movie.goto = a;
      return undefined;
    },
  };
  const classes = new Set();
  const context = vm.createContext({
    console: {log: line => lines.push(line)}, JSON, Number, String, Object, Array, Set, Promise, URLSearchParams,
    location: {search},
    setTimeout: fn => timers.push(fn) && timers.length, clearTimeout: () => { timers.length = 0; },
    document: {getElementById: element, createElement: tag => ({tag}), body: {classList: {toggle: (name, on) => (on ? classes.add(name) : classes.delete(name))}}},
    fetch: () => Promise.resolve(strings ? {ok: true, json: () => Promise.resolve({strings})} : {ok: false}),
    RufflePlayer: {newest: () => ({createPlayer: () => player})},
    owMovieGuard: {FILES: {missionScreen: ['UI_Mission/MissionInterface.swf']}, load: (p, options, label, requires, onLost) => { movie.lost = onLost; return Promise.resolve(guard); }},
  });
  context.window = context;
  for (const file of ['mission_screen_model.js', 'mission_screen.js']) vm.runInContext(fs.readFileSync(path.join(OVERLAY, file), 'utf8'), context);
  const settle = () => new Promise(resolve => setImmediate(resolve));
  const flush = async (count = 1) => { for (let i = 0; i < count; i++) { const due = timers.splice(0); due.forEach(fn => fn()); await settle(); } };
  await settle(); await settle(); await settle();
  return {context, calls, lines, elements, movie, player, classes, flush, written: (target, name) => calls.filter(c => c[0] === target && c[1] === 'set' && c[2] === name).map(c => c[3])};
}
const ready = p => p.lines.includes('OWMISSIONREADY');
const report = p => p.lines.filter(l => l.startsWith('OWMISSIONPAGE ')).map(l => JSON.parse(l.slice(14))).pop();

(async () => {
  // The offer: the page reports ready only once the movie has settled; nothing is written until the frame stops moving, because the fields are created again on the way.
  {
    const p = await page();
    assert.ok(!ready(p));
    await p.flush(5);
    assert.ok(ready(p));
    p.context.owMissionScreenState(state('offer'));
    assert.strictEqual(p.movie.goto, 'intro');
    assert.ok(p.classes.has('open'));
    p.movie.clips.add('_level1.missions.header');
    p.movie.frame = 3;
    await p.flush(1);
    assert.deepStrictEqual(p.written('_level1.missions.description', 'htmlText'), [], 'no write while the frame moves');
    p.movie.frame = 6;
    await p.flush(5);
    assert.deepStrictEqual(p.written('_level1.missions.description', 'htmlText'), ['Plain']);
    assert.deepStrictEqual(p.written('_level1.missions.header.description', 'text'), ['Name <1>']);
    assert.ok(p.written('_level1.missions.tooltips', 'htmlText')[0].includes('Enter: Accept'), 'the prompt says what closes it');
    assert.deepStrictEqual(p.written('_level1.missions.selections', '_visible'), [false]);
    const rep = report(p);
    assert.ok(rep.drawn && rep.movie && rep.page === 'offer');
    assert.strictEqual(p.elements.panel.hidden, true);
    // The same state again does not start the movie over, and scroll keys reach the field.
    p.movie.goto = null;
    p.context.owMissionScreenState(state('offer'));
    assert.strictEqual(p.movie.goto, null);
    p.context.owMissionScreenKey('PageDown');
    assert.deepStrictEqual(p.written('_level1.missions.description', 'scroll'), [4]);
    // The next page, then closed, and opening again draws again.
    p.movie.clips.add('_level1.reward');
    p.context.owMissionScreenState(state('reward'));
    assert.strictEqual(p.movie.goto, 'reward');
    p.movie.frame = 14;
    await p.flush(5);
    assert.ok(report(p).page === 'reward' && report(p).drawn);
    assert.deepStrictEqual(p.written('_level1.reward.xpReward', 'text'), ['7 XP']);
    assert.deepStrictEqual(p.written('_level1.reward.missionTitle', 'text'), ['Name <1>']);
    p.context.owMissionScreenState({page: 'closed'});
    assert.ok(!p.classes.has('open'));
    p.movie.goto = null;
    p.context.owMissionScreenState(state('turnin'));
    assert.strictEqual(p.movie.goto, 'intro');
  }
  // A reward clip that never appears gives the panel, not a blank page and not a hang.
  {
    const p = await page();
    await p.flush(5);
    p.context.owMissionScreenState(state('reward'));
    await p.flush(305);
    assert.deepStrictEqual([report(p).drawn, report(p).why], [false, 'the clip never appeared']);
    assert.strictEqual(p.elements.panel.hidden, false);
  }
  // The guard says the movie is missing: no movie call at all, ready anyway, and every page draws the panel with the mission and the keys.
  {
    const p = await page({guard: false});
    assert.ok(ready(p));
    const before = p.calls.length;
    for (const name of ['offer', 'turnin', 'reward']) {
      p.context.owMissionScreenState(state(name));
      assert.deepStrictEqual([report(p).page, report(p).movie, report(p).drawn], [name, false, true]);
      const text = p.elements.panel.children.map(c => c.textContent);
      assert.ok(p.elements.panel.hidden === false && text.includes('Name <1>') && text.some(t => t.includes('7 XP')) && text.some(t => t.includes('Enter')), name);
    }
    assert.strictEqual(p.calls.length, before);
    assert.ok(p.classes.has('open'));
    p.context.owMissionScreenKey('PageDown');   // no movie: nothing to scroll, and no throw
    p.context.owMissionScreenState({page: 'closed'});
    assert.strictEqual(p.elements.panel.hidden, true);
    assert.ok(!p.classes.has('open'));
  }
  // The movie is lost after it started: the open page falls back to the panel.
  {
    const p = await page();
    await p.flush(5);
    p.context.owMissionScreenState(state('offer'));
    p.movie.lost();
    assert.strictEqual(report(p).movie, false);
    assert.strictEqual(p.elements.panel.hidden, false);
  }
  // A state with no mission draws nothing rather than an empty page, and the prepared strings replace the neutral ones.
  {
    const p = await page({guard: false, strings: {accept: 'PREPARED'}});
    p.context.owMissionScreenState({page: 'offer', log: {missions: []}});
    assert.ok(!p.classes.has('open') && p.context.document.getElementById('panel').hidden);
    p.context.owMissionScreenState(state('offer'));
    assert.ok(p.elements.panel.children.some(c => c.textContent.includes('PREPARED')));
  }
  // The page is served, and only the guard loads the movie.
  const serve = fs.readFileSync(path.join(OVERLAY, 'serve.py'), 'utf8');
  for (const file of ['mission_screen.html', 'mission_screen.js', 'mission_screen_model.js']) assert.ok(serve.includes(`'/${file}'`), file);
  assert.ok(!/ruffle\(\)\.load/.test(fs.readFileSync(path.join(OVERLAY, 'mission_screen.js'), 'utf8')));
  console.log('ok');
})().catch(error => { console.error(error); process.exit(1); });
