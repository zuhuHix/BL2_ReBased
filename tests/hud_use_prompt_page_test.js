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

// The page, booted: `answer(target, op, a, b)` decides what the player's ow() returns or throws.
function bootPage(answer) {
  const logs = [], calls = [], intervals = [], clock = {now: 0};
  const player = {style: {}, ruffle: () => ({load: () => Promise.resolve()}), remove() {},
    ow(target, op, a, b) {
      calls.push([target, op, a, b]);
      if (target === '_level1' && op === 'get') return 1;   // loaded
      if (target === '_level1.p1' && op === 'get' && a === '_name') return 'p1';
      return answer(target, op, a, b);
    }};
  const context = vm.createContext({
    console: {log: line => logs.push(String(line)), warn() {}, error() {}},
    document: {body: {appendChild() {}}, getElementById: () => null},
    location: {search: ''},
    performance: {now: () => clock.now},
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
  return {context, logs, calls, clock};
}

const TOOLS = '_level1.p1.tools';
const FRAMES = {none: 1, shop: 13, pickup: 55, talk: 9};
const wrap = text => `<TEXTFORMAT LEADING="-6"><P ALIGN="LEFT"><FONT FACE="WillowBody" SIZE="17">${text}</FONT></P></TEXTFORMAT>`;
// The clip as the Ruffle bench showed it: a label moves the frame, the bound variable holds the text in format tags, and a dot path to desc
// reads nothing. Faults: `goto` off ignores jumps, `write` off ignores the text.
function movie(faults = {}) {
  const clip = {frame: 1, desc: wrap('')};
  return {clip, answer(target, op, a, b) {
    if (target !== TOOLS) return target.startsWith(TOOLS) ? undefined : fine(target, op, a);
    if (op === 'gotoAndStop') { if (!faults.goto) clip.frame = FRAMES[a]; return undefined; }
    if (op === 'set' && a === 'desc') { if (!faults.write) clip.desc = wrap(b); return true; }
    if (op === 'get' && a === '_currentframe') return clip.frame;
    if (op === 'get' && a === 'desc') return clip.desc;
    return undefined;
  }};
}
const fine = (target, op, a) => (op === 'get' ? 1 : undefined);
const called = (page, target, op, a) => page.calls.some(c => c[0] === target && c[1] === op && (a === undefined || c[2] === a));
const toolCalls = page => page.calls.filter(c => c[0].startsWith(TOOLS)).length;
const shopPrompt = {frame: 'shop', text: '[E] SHOP'};
let passed = 0;
const pass = name => { ++passed; console.log(`PASS ${name}`); };

{
  const m = movie();
  const page = bootPage(m.answer);
  page.context.owHud({usePrompt: shopPrompt});
  assert.strictEqual(m.clip.frame, 13);
  assert.ok(m.clip.desc.includes('[E] SHOP'));
  assert.ok(page.logs.some(line => /OWUSEPROMPTPAGE asked=.*shop.*frame=13 text=\[E\] SHOP$/.test(line)), page.logs.join(' | '));
  pass('A prompt moves the clip to its frame and fills its text, read back without the format tags');

  const before = toolCalls(page);
  page.context.owHud({usePrompt: shopPrompt});
  assert.strictEqual(toolCalls(page), before);
  pass('The same prompt again makes no calls on the clip');

  page.context.owHud({usePrompt: {frame: 'pickup', text: '[E] PICK UP'}});
  assert.strictEqual(m.clip.frame, 55);
  assert.ok(m.clip.desc.includes('[E] PICK UP'), 'a later prompt changes the words too');
  page.context.owHud({usePrompt: null});
  assert.strictEqual(m.clip.frame, 1);
  assert.ok(!page.logs.some(line => /failed/.test(line)), page.logs.join(' | '));
  pass('A second prompt has its own words, and no prompt is the none frame');
}
{
  const m = movie({write: true});
  const page = bootPage(m.answer);
  page.context.owHud({usePrompt: shopPrompt});
  assert.ok(page.logs.some(line => /OWUSEPROMPTPAGE failed.*text reads/.test(line)), page.logs.join(' | '));
  pass('Text that did not change is not taken as applied');
}
{
  const page = bootPage(movie({goto: true}).answer);
  page.context.owHud({usePrompt: shopPrompt});
  assert.ok(page.logs.some(line => /OWUSEPROMPTPAGE failed.*frame 1, not shop/.test(line)), page.logs.join(' | '));
  pass('A jump that did nothing is not taken as applied');
}
{
  const page = bootPage((target, op, a) => {
    if (target.startsWith(TOOLS)) throw new Error('no such clip');
    return fine(target, op, a);
  });
  page.context.owHud({usePrompt: shopPrompt, down: {phase: 'injured', bleed: .5}});
  assert.ok(called(page, '_level1.p1.dying1', 'gotoAndStop', 'fight'), 'the down state after the prompt still applies');
  assert.ok(page.logs.some(line => /OWUSEPROMPTPAGE failed/.test(line)), page.logs.join(' | '));
  pass('A clip that throws stops nothing after it, and the log says so');
}
{
  let present = false;
  const m = movie();
  const page = bootPage((target, op, a, b) => (target.startsWith(TOOLS) && !present ? undefined : m.answer(target, op, a, b)));
  page.context.owHud({usePrompt: shopPrompt});
  const failures = () => page.logs.filter(line => /OWUSEPROMPTPAGE failed/.test(line)).length;
  assert.strictEqual(failures(), 1);
  const before = toolCalls(page);
  page.context.owHud({usePrompt: shopPrompt});
  assert.strictEqual(toolCalls(page), before, 'no bridge calls inside the backoff');
  page.clock.now += 1100;
  page.context.owHud({usePrompt: shopPrompt});
  assert.ok(toolCalls(page) > before, 'tried again after it');
  assert.strictEqual(failures(), 1, 'the single log line');
  present = true;
  page.clock.now += 1100;
  page.context.owHud({usePrompt: shopPrompt});
  assert.ok(page.logs.some(line => /OWUSEPROMPTPAGE asked=/.test(line)), page.logs.join(' | '));
  pass('A clip that does not answer backs off about a second, logs once, and recovers');
}
console.log(`hud_use_prompt_page_test: ${passed}/${passed} passed`);
