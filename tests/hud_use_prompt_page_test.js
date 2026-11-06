// tools/hud_overlay/index.html's real script with a stub player: the use prompt reaches the movie's clip, and a clip that throws or
// does not answer stops nothing else. Invented data only. Run: node tests/hud_use_prompt_page_test.js
'use strict';
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const OVERLAY = path.join(__dirname, '..', 'tools', 'hud_overlay');

// The page's inline script: the last <script> without a src.
function pageScript() {
  const html = fs.readFileSync(path.join(OVERLAY, 'index.html'), 'utf8');
  const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)].map(match => match[1]);
  assert.ok(scripts.length >= 1, 'index.html has an inline script');
  return scripts[scripts.length - 1];
}

// The page, booted: `answer(target, op, a)` decides what the player's ow() returns or throws.
function bootPage(answer) {
  const logs = [], calls = [], intervals = [];
  const player = {style: {}, ruffle: () => ({load: () => Promise.resolve()}), remove() {},
    ow(target, op, a, b) {
      calls.push([target, op, a, b]);
      if (target === '_level1' && op === 'get') return 1;   // loaded
      if (target === '_level1.p1' && op === 'get' && a === '_name') return 'p1';
      return answer(target, op, a);
    }};
  const context = vm.createContext({
    console: {log: line => logs.push(String(line)), warn() {}, error() {}},
    document: {body: {appendChild() {}}, getElementById: () => null},
    location: {search: ''},
    performance: {now: () => 0},
    setInterval: f => { intervals.push(f); return intervals.length; },
    clearInterval() {},
    setTimeout() { return 0; },
    fetch: () => Promise.resolve({ok: false, headers: {get: () => null}, json: () => Promise.resolve({})}),
    JSON, Object, Math, Number, String, Promise, Array,
  });
  context.window = context;
  context.globalThis = context;
  context.RufflePlayer = {newest: () => ({createPlayer: () => player})};
  // The page's other helpers, inert: this test is about the prompt and what follows it.
  context.owActionSkill = {plan: () => []};
  context.owMinimap = {apply() {}, initFog() {}, placeEcho() {}, warpClips: () => []};
  context.owHud3d = {apply: () => null};
  context.owMovieGuard = {FILES: {hud: []}, load: () => Promise.resolve(true)};
  vm.runInContext(fs.readFileSync(path.join(OVERLAY, 'hud_fields.js'), 'utf8'), context);
  vm.runInContext(pageScript(), context);
  assert.ok(intervals.length >= 1, 'the page polls for the movie');
  intervals[0]();   // the boot step: the movie is loaded, so the page is ready and applies state from here on
  return {context, logs, calls};
}

const TOOLS = '_level1.p1.tools';
const fine = (target, op, a) => (op !== 'get' ? undefined : a === '_currentframe' ? 13 : 'text');
const called = (page, target, op, a) => page.calls.some(c => c[0] === target && c[1] === op && (a === undefined || c[2] === a));
let passed = 0;
const pass = name => { ++passed; console.log(`PASS ${name}`); };

{
  const page = bootPage(fine);
  page.context.owHud({usePrompt: {frame: 'shop', text: '[E] SHOP'}});
  assert.ok(called(page, TOOLS, 'gotoAndStop', 'shop'));
  assert.ok(called(page, `${TOOLS}.desc`, 'set', 'text'));
  assert.ok(page.logs.some(line => /OWUSEPROMPTPAGE asked=.*shop.*frame=13/.test(line)), page.logs.join(' | '));
  pass('A prompt moves the clip to its frame and fills its text');

  const before = page.calls.filter(c => c[0].startsWith(TOOLS)).length;
  page.context.owHud({usePrompt: {frame: 'shop', text: '[E] SHOP'}});
  assert.strictEqual(page.calls.filter(c => c[0].startsWith(TOOLS)).length, before);
  pass('The same prompt again makes no calls on the clip');

  page.context.owHud({usePrompt: null});
  assert.ok(called(page, TOOLS, 'gotoAndStop', 'none'));
  pass('No prompt puts the clip on its none frame');
}
{
  const page = bootPage((target, op, a) => {
    if (target.startsWith(TOOLS)) throw new Error('no such clip');
    return fine(target, op, a);
  });
  page.context.owHud({usePrompt: {frame: 'shop', text: 'x'}, down: {phase: 'injured', bleed: .5}});
  assert.ok(called(page, '_level1.p1.dying1', 'gotoAndStop', 'fight'), 'the down state after the prompt still applies');
  assert.ok(page.logs.some(line => /OWUSEPROMPTPAGE failed/.test(line)), page.logs.join(' | '));
  pass('A clip that throws stops nothing after it, and the log says so');
}
{
  let present = false;
  const page = bootPage((target, op, a) => (target.startsWith(TOOLS) && !present ? undefined : fine(target, op, a)));
  page.context.owHud({usePrompt: {frame: 'shop', text: 'x'}});
  assert.ok(page.logs.some(line => /OWUSEPROMPTPAGE failed/.test(line)), page.logs.join(' | '));
  present = true;
  page.context.owHud({usePrompt: {frame: 'shop', text: 'x'}});
  assert.ok(page.logs.some(line => /OWUSEPROMPTPAGE asked=/.test(line)), page.logs.join(' | '));
  pass('A clip that does not answer is not taken as applied: the next state tries again');
}
console.log(`hud_use_prompt_page_test: ${passed}/${passed} passed`);
